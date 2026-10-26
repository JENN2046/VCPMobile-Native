//! Native recovery bridge: durable originals, revocable view fences, and
//! single-consumption authenticated receipts. No POST or automatic retry API.
use crate::pending::{RequestKey, Submission, SubmissionKind as StoredKind};
use crate::portal_contract::PortalSession;
use crate::receipt_query::{PendingReceiptCheck, ReceiptQueryError};
use crate::recovery::{RecoverySubmission, SubmissionKind};
use crate::session_credential::NativeSessionInfo;
use crate::submission_flow::Reconciliation;
pub use crate::pending_journal::NativePendingJournal;
use serde_json::{json, Value};
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum NativeRecoveryError {
    #[error("private pending storage rejected; originals retained")]
    StorageRejected,
    #[error("recovery context changed; originals retained")]
    ContextChanged,
    #[error("credential rejected or changed; originals retained")]
    CredentialRejected,
    #[error("credential expired; originals retained")]
    Expired,
    #[error("authenticate before querying the original receipt")]
    AuthenticationRequired,
    #[error("original Topic is no longer authorized")]
    Forbidden,
    #[error("receipt query timed out; original remains unknown")]
    Timeout,
    #[error("receipt query failed; original remains unknown")]
    QueryFailed,
    #[error("recovery cancelled; originals retained")]
    Cancelled,
    #[error("receipt token already consumed")]
    AlreadyConsumed,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum NativePendingKind {
    Message,
    Invitation,
}

#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct NativePendingInfo {
    pub entry_id: String,
    pub original_session_marker: String,
    pub request_id: String,
    pub topic_key: String,
}
impl fmt::Debug for NativePendingInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativePendingInfo([REDACTED])")
    }
}
impl NativePendingInfo {
    pub(crate) fn key(&self) -> RequestKey {
        RequestKey {
            endpoint_id: self.entry_id.clone(),
            session_marker: self.original_session_marker.clone(),
            request_id: self.request_id.clone(),
        }
    }
    fn from_original(s: &Submission) -> Self {
        Self {
            entry_id: s.key.endpoint_id.clone(),
            original_session_marker: s.key.session_marker.clone(),
            request_id: s.key.request_id.clone(),
            topic_key: s.topic_id.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, uniffi::Enum)]
pub enum NativeRecoveryOutcome {
    Accepted {
        execution_id: String,
        execution_state: String,
    },
    SameSessionNoReceipt,
    Review {
        reason: String,
    },
}
impl fmt::Debug for NativeRecoveryOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeRecoveryOutcome([REDACTED])")
    }
}
fn outcome(value: Reconciliation) -> NativeRecoveryOutcome {
    match value {
        Reconciliation::Accepted {
            execution_id,
            execution_state,
        } => NativeRecoveryOutcome::Accepted {
            execution_id,
            execution_state,
        },
        Reconciliation::SameSessionNoReceipt => NativeRecoveryOutcome::SameSessionNoReceipt,
        Reconciliation::Review(reason) => NativeRecoveryOutcome::Review {
            reason: reason.as_str().into(),
        },
    }
}

#[uniffi::export]
impl NativePendingJournal {
    #[uniffi::constructor]
    pub fn open(origin: String, private_path: String) -> Result<Arc<Self>, NativeRecoveryError> {
        // Arbitrary-file opening cannot discover another layout's UNKNOWN.
        let _ = (origin, private_path);
        Err(NativeRecoveryError::StorageRejected)
    }
    /// Storage checkpoint only, before a future explicit dispatch. Preserves
    /// exact bytes and Topic exclusivity. This grants no transport authority.
    /// This checkpoint never grants a message-dispatch capability.
    pub fn prepare_original(
        &self,
        expected: NativeSessionInfo,
        topic_key: String,
        kind: NativePendingKind,
        original_json: String,
    ) -> Result<NativePendingInfo, NativeRecoveryError> {
        self.verify_location().map_err(|_| NativeRecoveryError::StorageRejected)?;
        let session = checked_session(&expected, &topic_key)?;
        let payload: Value = serde_json::from_str(&original_json)
            .map_err(|_| NativeRecoveryError::StorageRejected)?;
        let wire_kind = match kind {
            NativePendingKind::Message => SubmissionKind::Message,
            NativePendingKind::Invitation => SubmissionKind::Invitation,
        };
        let wire = RecoverySubmission::new(topic_key.clone(), wire_kind, payload)
            .map_err(|_| NativeRecoveryError::StorageRejected)?;
        wire.validate_wire_identity()
            .map_err(|_| NativeRecoveryError::StorageRejected)?;
        let original = Submission {
            key: RequestKey {
                endpoint_id: session.entry_id.clone(),
                session_marker: session.session_marker.clone(),
                request_id: wire.request_id().into(),
            },
            topic_id: topic_key,
            kind: match kind {
                NativePendingKind::Message => StoredKind::Message,
                NativePendingKind::Invitation => StoredKind::Invite,
            },
            payload_json: original_json,
        };
        session
            .admit(&self.store, &original)
            .map_err(|_| NativeRecoveryError::StorageRejected)?;
        Ok(NativePendingInfo::from_original(&original))
    }
    pub fn pending_for_topic(
        &self,
        topic_key: String,
    ) -> Result<Vec<NativePendingInfo>, NativeRecoveryError> {
        self.verify_location().map_err(|_| NativeRecoveryError::StorageRejected)?;
        Ok(self
            .store
            .list()
            .map_err(|_| NativeRecoveryError::StorageRejected)?
            .iter()
            .filter(|r| r.submission.topic_id == topic_key)
            .map(|r| NativePendingInfo::from_original(&r.submission))
            .collect())
    }
}

/// Shared with the view. Cancel synchronously BEFORE changing Topic, origin,
/// session or view generation. Cancel and commit share this same Rust lock,
/// so no late cleanup can linearize after cancellation.
#[derive(uniffi::Object)]
pub struct NativeRecoveryFence {
    pub(crate) expected: PortalSession,
    pub(crate) topic_key: String,
    pub(crate) view_generation: u64,
    pub(crate) cancelled: Mutex<bool>,
}
#[uniffi::export]
impl NativeRecoveryFence {
    #[uniffi::constructor]
    pub fn new(
        expected: NativeSessionInfo,
        topic_key: String,
        view_generation: u64,
    ) -> Result<Arc<Self>, NativeRecoveryError> {
        let session = checked_session(&expected, &topic_key)?;
        Ok(Arc::new(Self {
            expected: session,
            topic_key,
            view_generation,
            cancelled: Mutex::new(false),
        }))
    }
    pub fn cancel(&self) {
        if let Ok(mut cancelled) = self.cancelled.lock() {
            *cancelled = true;
        }
    }
}
impl NativeRecoveryFence {
    pub(crate) fn ensure_active(&self) -> Result<(), NativeRecoveryError> {
        if *self
            .cancelled
            .lock()
            .map_err(|_| NativeRecoveryError::Cancelled)?
        {
            return Err(NativeRecoveryError::Cancelled);
        }
        Ok(())
    }
}

/// No JSON constructor, no Clone and no public receipt/payload fields.
#[derive(uniffi::Object)]
pub struct NativeRecoveryCheck {
    journal: Arc<NativePendingJournal>,
    fence: Arc<NativeRecoveryFence>,
    fingerprint: String,
    ticket: Mutex<Option<PendingReceiptCheck>>,
}
impl NativeRecoveryCheck {
    pub(crate) fn new(
        journal: Arc<NativePendingJournal>,
        fence: Arc<NativeRecoveryFence>,
        fingerprint: String,
        ticket: PendingReceiptCheck,
    ) -> Arc<Self> {
        Arc::new(Self {
            journal,
            fence,
            fingerprint,
            ticket: Mutex::new(Some(ticket)),
        })
    }
    /// Invoked only while the host holds the SAME vault gate/actor that fences
    /// replacement/removal, after reloading the current protected credential.
    pub(crate) fn consume(
        &self,
        journal: &Arc<NativePendingJournal>,
        fingerprint: &str,
        expected: &NativeSessionInfo,
        topic_key: &str,
        view_generation: u64,
    ) -> Result<NativeRecoveryOutcome, NativeRecoveryError> {
        let cancelled = self
            .fence
            .cancelled
            .lock()
            .map_err(|_| NativeRecoveryError::Cancelled)?;
        if *cancelled {
            return Err(NativeRecoveryError::Cancelled);
        }
        let current = checked_session(expected, topic_key)?;
        if !Arc::ptr_eq(&self.journal, journal) || self.fingerprint != fingerprint {
            return Err(NativeRecoveryError::CredentialRejected);
        }
        if current.entry_id != self.fence.expected.entry_id
            || current.session_marker != self.fence.expected.session_marker
            || topic_key != self.fence.topic_key
            || view_generation != self.fence.view_generation
        {
            return Err(NativeRecoveryError::ContextChanged);
        }
        journal.verify_location().map_err(|_| NativeRecoveryError::StorageRejected)?;
        let ticket = self
            .ticket
            .lock()
            .map_err(|_| NativeRecoveryError::AlreadyConsumed)?
            .take()
            .ok_or(NativeRecoveryError::AlreadyConsumed)?;
        let result = ticket.commit(&journal.store).map_err(query_error)?;
        drop(cancelled);
        Ok(outcome(result))
    }
}
pub(crate) fn checked_session(
    info: &NativeSessionInfo,
    topic: &str,
) -> Result<PortalSession, NativeRecoveryError> {
    let doc = json!({"authenticated":true,"recovery":{"version":1,"entryId":info.entry_id,"sessionMarker":info.session_marker},
        "topics":info.topic_keys.iter().map(|k|json!({"key":k})).collect::<Vec<_>>(),"selectedTopic":info.selected_topic});
    let session =
        PortalSession::parse(&doc.to_string()).map_err(|_| NativeRecoveryError::ContextChanged)?;
    if !session.allows(topic) {
        return Err(NativeRecoveryError::Forbidden);
    }
    Ok(session)
}
pub(crate) fn query_error(error: ReceiptQueryError) -> NativeRecoveryError {
    use crate::https_transport::PortalHttpError;
    use crate::transport_policy::TransportPolicyError;
    match error {
        ReceiptQueryError::Store(_) => NativeRecoveryError::StorageRejected,
        ReceiptQueryError::Flow(crate::submission_flow::FlowError::TopicNotAllowed) => {
            NativeRecoveryError::Forbidden
        }
        ReceiptQueryError::Flow(_) => NativeRecoveryError::ContextChanged,
        ReceiptQueryError::Http(PortalHttpError::Timeout) => NativeRecoveryError::Timeout,
        ReceiptQueryError::Http(PortalHttpError::SessionRejected(
            TransportPolicyError::AuthenticationRequired,
        )) => NativeRecoveryError::AuthenticationRequired,
        ReceiptQueryError::Http(PortalHttpError::SessionRejected(
            TransportPolicyError::Forbidden,
        )) => NativeRecoveryError::Forbidden,
        _ => NativeRecoveryError::QueryFailed,
    }
}
