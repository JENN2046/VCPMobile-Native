//! Query the original pending receipt over authenticated HTTPS after restart.
//! No POST, model/tool execution, implicit retry, or remote task control.
use crate::history::{HistoryError, NativeHistoryPage};
use crate::https_transport::{PortalHttpClient, PortalHttpError};
use crate::pending::{PendingStore, RequestKey, StoreError, Submission};
use crate::submission_flow::{inspect_authenticated_history, FlowError, Reconciliation};
use crate::transport_policy::PortalCookie;
use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum ReceiptQueryError {
    #[error("pending storage failed: {0}")]
    Store(#[from] StoreError),
    #[error("authenticated receipt query failed: {0}")]
    Http(#[from] PortalHttpError),
    #[error("history page rejected: {0}")]
    History(#[from] HistoryError),
    #[error("original receipt inspection failed: {0}")]
    Flow(#[from] FlowError),
    #[error("original request has no durable first-attempt claim")]
    NotClaimed,
}

/// Produced only by a fresh authenticated session + validated history GET.
/// Querying does not mutate pending. The host must check its credential/view
/// epoch and credential expiry before committing; discard a late result.
/// This type is intentionally non-Clone, with no caller-supplied JSON constructor.
#[must_use = "check the host epoch before committing or discard this query result"]
pub struct PendingReceiptCheck {
    original: Submission,
    outcome: Reconciliation,
}
impl fmt::Debug for PendingReceiptCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingReceiptCheck")
            .field("original_and_outcome", &"[REDACTED]")
            .finish()
    }
}
impl PendingReceiptCheck {
    pub(crate) fn inspect_snapshot(
        session: &crate::portal_contract::PortalSession, store: &PendingStore,
        key: &RequestKey, snapshot: &crate::https_transport::HistorySnapshot,
    ) -> Result<Self, ReceiptQueryError> {
        let record = store.get(key)?.ok_or(StoreError::NotFound)?;
        if record.state != crate::pending::PendingState::AwaitingConfirmation {
            return Err(ReceiptQueryError::NotClaimed);
        }
        let original = record.submission;
        if original.key.endpoint_id != session.entry_id || original.topic_id != snapshot.topic_key {
            return Err(FlowError::EntryChanged.into());
        }
        NativeHistoryPage::parse(snapshot, session)?;
        let outcome = inspect_authenticated_history(session, &original, &snapshot.document["executions"])?;
        Ok(Self { original, outcome })
    }
    pub fn outcome(&self) -> &Reconciliation {
        &self.outcome
    }

    /// Only an exact accepted receipt permits local cleanup. SQLite checks
    /// the full original again, so a changed/replaced row cannot be removed.
    /// Accepted identifies the request; a running AA task remains running.
    pub fn commit(self, store: &PendingStore) -> Result<Reconciliation, ReceiptQueryError> {
        if matches!(self.outcome, Reconciliation::Accepted { .. })
            && !store.confirm_verified(&self.original)?
        {
            return Err(StoreError::NotFound.into());
        }
        Ok(self.outcome)
    }
}

impl PortalHttpClient {
    /// Caller supplies an unexpired Cookie from the protected platform vault.
    /// Always re-read current server authorization, then query the ORIGINAL
    /// Topic (never the selected Topic). Executions are unpaginated: one valid
    /// page at after=0 suffices, even if message history has more pages.
    /// All failures retain the journal; no retry or cleanup occurs here.
    pub async fn query_pending_receipt(
        &self,
        cookie: &PortalCookie,
        store: &PendingStore,
        original_key: &RequestKey,
    ) -> Result<PendingReceiptCheck, ReceiptQueryError> {
        self.query_pending_receipt_context(cookie, store, original_key, None)
            .await
    }

    pub(crate) async fn query_pending_receipt_expected(
        &self,
        cookie: &PortalCookie,
        store: &PendingStore,
        original_key: &RequestKey,
        expected: &crate::portal_contract::PortalSession,
    ) -> Result<PendingReceiptCheck, ReceiptQueryError> {
        self.query_pending_receipt_context(cookie, store, original_key, Some(expected))
            .await
    }

    async fn query_pending_receipt_context(
        &self,
        cookie: &PortalCookie,
        store: &PendingStore,
        original_key: &RequestKey,
        expected: Option<&crate::portal_contract::PortalSession>,
    ) -> Result<PendingReceiptCheck, ReceiptQueryError> {
        let record = store.get(original_key)?.ok_or(StoreError::NotFound)?;
        if record.state != crate::pending::PendingState::AwaitingConfirmation {
            return Err(ReceiptQueryError::NotClaimed);
        }
        let original = record.submission;
        let session = self.fetch_session(cookie).await?;
        if expected.is_some_and(|prior| {
            prior.entry_id != session.entry_id || prior.session_marker != session.session_marker
        }) {
            return Err(
                FlowError::Portal(crate::portal_contract::PortalError::ContextChanged).into(),
            );
        }
        if session.entry_id != original.key.endpoint_id {
            return Err(FlowError::EntryChanged.into());
        }
        if !session.allows(&original.topic_id) {
            return Err(FlowError::TopicNotAllowed.into());
        }
        let snapshot = self
            .fetch_history(cookie, &session, &original.topic_id, 0)
            .await?;
        // Preserve the shared gate's second authenticated authority read.
        let latest = self.fetch_session(cookie).await?;
        if latest.entry_id != session.entry_id || latest.session_marker != session.session_marker
            || !latest.allows(&original.topic_id) {
            return Err(FlowError::Portal(crate::portal_contract::PortalError::ContextChanged).into());
        }
        PendingReceiptCheck::inspect_snapshot(&latest, store, original_key, &snapshot)

    }
}

#[cfg(test)]
#[path = "receipt_query_tests.rs"]
mod tests;
