//! App-private per-origin pending journal for native recovery and deliberate first send.
//! Listing never dispatches. The initial-message API atomically claims once,
//! then calls the guarded HTTPS POST without retry or pending cleanup.
//! The host must supply its trusted app-private, no-backup directory.
use crate::pending::{PendingState, PendingStore, StoreError, SubmissionKind};
use crate::recovery::SubmissionKind as WireKind;
use crate::submission_flow::{first_attempt, FlowError, OutboundSubmission};
use serde_json::json;
use crate::session_credential::{credential_storage_key, NativeSessionInfo, SessionCredential};
use crate::submission_flow::HttpsOrigin;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum PendingJournalError {
    #[error("invalid HTTPS origin or app-private storage directory")]
    InvalidScope,
    #[error("pending database unavailable or unrecognized; retain existing files")]
    StoreUnavailable,
    #[error("credential does not belong to this pending journal")]
    OriginMismatch,
    #[error("authorized session could not be verified")]
    SessionUnavailable,
    #[error("authenticated entry or session changed")]
    ContextChanged,
    #[error("Topic not authorized")]
    TopicUnauthorized,
}

/// Non-secret recovery identifiers only; original message bytes are never returned.
/// An HTTP response is never proof that AA completed or that a receipt exists.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NativeFirstSendResult {
    pub request_id: String,
    pub disposition: String,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum NativeSendError {
    #[error("credential does not belong to this HTTPS origin")]
    CredentialMismatch,
    #[error("authenticated session cannot be verified")]
    SessionUnavailable,
    #[error("original entry/session changed; no message sent")]
    ContextChanged,
    #[error("Topic not authorized; no message sent")]
    TopicUnauthorized,
    #[error("invalid original message or request identity")]
    InvalidMessage,
    #[error("another original request is pending; reconcile first")]
    OutstandingPending,
    #[error("app-private pending journal unavailable")]
    StoreUnavailable,
    #[error("local send cancelled; inspect pending before another send")]
    Cancelled,
    #[error("send capability already consumed")]
    AlreadyConsumed,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NativePendingPreview {
    pub request_id: String,
    pub topic_key: String,
    pub kind: String,
    pub state: String,
    pub recovery_mode: String,
}

#[derive(Debug, uniffi::Object)]
pub struct NativePendingJournal {
    pub(crate) origin: String,
    storage_key: String,
    pub(crate) store: PendingStore,
}

#[uniffi::export]
impl NativePendingJournal {
    /// The mobile host supplies a *trusted* private directory, never a user
    /// selected path. Each canonical HTTPS origin has a distinct SQLite file.
    #[uniffi::constructor]
    pub fn new(origin: String, private_directory: String)
        -> Result<Arc<Self>, PendingJournalError>
    {
        let key = credential_storage_key(origin.clone()).map_err(|_| PendingJournalError::InvalidScope)?;
        Self::open_private_file(origin, PathBuf::from(private_directory).join(format!("vcp-pending-v1-{key}.sqlite")))
    }
}

impl NativePendingJournal {
    /// Both compatible constructors feed this one validated store owner.
    pub(crate) fn open_private_file(origin: String, location: PathBuf)
        -> Result<Arc<Self>, PendingJournalError>
    {
        let canonical = HttpsOrigin::parse(&origin).map_err(|_| PendingJournalError::InvalidScope)?;
        let storage_key = credential_storage_key(canonical.as_str().to_owned())
            .map_err(|_| PendingJournalError::InvalidScope)?;
        if !location.is_absolute() { return Err(PendingJournalError::InvalidScope); }
        let root = location.parent().ok_or(PendingJournalError::InvalidScope)?;
        let metadata = fs::symlink_metadata(root).map_err(|_| PendingJournalError::StoreUnavailable)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(PendingJournalError::InvalidScope);
        }
        match fs::symlink_metadata(&location) {
            Ok(existing) if !existing.file_type().is_file() ||
                existing.file_type().is_symlink() => {
                return Err(PendingJournalError::StoreUnavailable);
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(PendingJournalError::StoreUnavailable);
            }
            _ => {}
        }
        let store = PendingStore::open(&location)
            .map_err(|_| PendingJournalError::StoreUnavailable)?;
        Ok(Arc::new(Self { origin: canonical.as_str().to_owned(), storage_key, store }))
    }

}

#[uniffi::export(async_runtime = "tokio")]
impl NativePendingJournal {
    /// Explicit one-shot authenticated GET, followed by a bounded local view.
    /// The host must additionally compare its Keychain/Keystore credential epoch
    /// and selected Topic/view generation before displaying these summaries.
    /// No receipt inspection or local mutation occurs in this method.
    pub async fn list_for_topic(
        &self,
        credential: Arc<SessionCredential>,
        expected: NativeSessionInfo,
        topic_key: String,
    ) -> Result<Vec<NativePendingPreview>, PendingJournalError> {
        if credential.storage_key().map_err(|_| PendingJournalError::OriginMismatch)?
            != self.storage_key {
            return Err(PendingJournalError::OriginMismatch);
        }
        let current = credential.fetch_session_info().await
            .map_err(|_| PendingJournalError::SessionUnavailable)?;
        if current.entry_id != expected.entry_id ||
           current.session_marker != expected.session_marker {
            return Err(PendingJournalError::ContextChanged);
        }
        self.previews_for_topic(&current, &topic_key)
    }

    /// Explicit user-only initial send. No automatic retry, ever. A valid
    /// preflight is necessary but never treated as authority after a change:
    /// server guards still validate the original entry/session during POST.
    /// The host must fence credential epoch and selected Topic before calling.
    /// Once locally claimed, *all* transport outcomes retain pending until an
    /// authenticated, uniquely matching original receipt is inspected.
    pub async fn send_original_message_once(
        &self,
        credential: Arc<SessionCredential>,
        expected: NativeSessionInfo,
        topic_key: String,
        request_id: String,
        text: String,
    ) -> Result<NativeFirstSendResult, NativeSendError> {
        if credential.storage_key().map_err(|_| NativeSendError::CredentialMismatch)?
            != self.storage_key {
            return Err(NativeSendError::CredentialMismatch);
        }
        let expected_session = crate::session_credential::history_preflight(
            &self.origin, &expected, &topic_key, 0,
        ).map_err(|_| NativeSendError::TopicUnauthorized)?;
        let current = credential.fetch_session_info().await
            .map_err(|_| NativeSendError::SessionUnavailable)?;
        if current.entry_id != expected_session.entry_id ||
            current.session_marker != expected_session.session_marker {
            return Err(NativeSendError::ContextChanged);
        }
        let verified = crate::session_credential::history_preflight(
            &self.origin, &current, &topic_key, 0,
        ).map_err(|_| NativeSendError::TopicUnauthorized)?;
        // This is the sole durable dispatch claim. Failed claim means NO POST.
        let plan = self.claim_original_message(&verified, &topic_key, &request_id, &text)?;
        let outcome = credential.dispatch_claimed_once(plan).await;
        let disposition = crate::native_send::disposition(outcome);
        Ok(NativeFirstSendResult { request_id, disposition: disposition.to_owned() })
    }
}

impl NativePendingJournal {
    /// All payload bytes are persisted locally before claiming any send. A
    /// competing unknown for this entry/Topic prevents creating a new request.
    pub(crate) fn claim_original_message(
        &self, session: &crate::portal_contract::PortalSession,
        topic: &str, request_id: &str, text: &str,
    ) -> Result<OutboundSubmission, NativeSendError> {
        if !session.allows(topic) { return Err(NativeSendError::TopicUnauthorized); }
        let original = json!({"requestId": request_id, "text": text, "attachments": []});
        first_attempt(&self.origin, session, &self.store, topic,
            WireKind::Message, &original.to_string()).map_err(|error| match error {
                FlowError::ReconcileFirst |
                FlowError::Store(StoreError::TopicPending) |
                FlowError::Portal(crate::portal_contract::PortalError::Store(StoreError::TopicPending)) =>
                    NativeSendError::OutstandingPending,
                FlowError::InvalidSubmission => NativeSendError::InvalidMessage,
                FlowError::Portal(crate::portal_contract::PortalError::TopicNotAllowed) |
                FlowError::TopicNotAllowed => NativeSendError::TopicUnauthorized,
                FlowError::Portal(crate::portal_contract::PortalError::ContextChanged) |
                FlowError::EntryChanged => NativeSendError::ContextChanged,
                _ => NativeSendError::StoreUnavailable,
            })
    }

    fn previews_for_topic(
        &self, session: &NativeSessionInfo, topic_key: &str,
    ) -> Result<Vec<NativePendingPreview>, PendingJournalError> {
        let verified = super::session_credential::history_preflight(
            &self.origin, session, topic_key, 0,
        ).map_err(|_| PendingJournalError::TopicUnauthorized)?;
        let entries = self.store.list().map_err(|_| PendingJournalError::StoreUnavailable)?;
        Ok(entries.into_iter()
            .filter(|row| row.submission.key.endpoint_id == verified.entry_id &&
                row.submission.topic_id == topic_key)
            .map(|row| NativePendingPreview {
                request_id: row.submission.key.request_id,
                topic_key: row.submission.topic_id,
                kind: match row.submission.kind {
                    SubmissionKind::Message => "MESSAGE",
                    SubmissionKind::Invite => "INVITATION",
                }.to_owned(),
                state: match row.state {
                    PendingState::Prepared => "PREPARED_NOT_SENT",
                    PendingState::AwaitingConfirmation => "AWAITING_RECEIPT",
                }.to_owned(),
                recovery_mode: if row.submission.key.session_marker == verified.session_marker {
                    "QUERY_BEFORE_EXPLICIT_RETRY"
                } else {
                    "QUERY_ONLY_CROSS_SESSION"
                }.to_owned(),
            }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pending::{RequestKey, Submission};
    use serde_json::json;

    const ORIGIN: &str = "https://journal.example.test";

    fn session(marker: char) -> NativeSessionInfo {
        NativeSessionInfo {
            entry_id: "a".repeat(64),
            session_marker: marker.to_string().repeat(64),
            topic_keys: vec!["topic_a".into(), "topic_b".into()],
            selected_topic: Some("topic_a".into()),
        }
    }
    fn journal(root: &std::path::Path) -> Arc<NativePendingJournal> {
        NativePendingJournal::new(ORIGIN.into(), root.to_str().unwrap().into()).unwrap()
    }
    fn seed(journal: &NativePendingJournal, info: &NativeSessionInfo,
            topic: &str, id: &str, attempted: bool) {
        let row = Submission {
            key: RequestKey { endpoint_id: info.entry_id.clone(),
                session_marker: info.session_marker.clone(), request_id: id.into() },
            topic_id: topic.into(), kind: SubmissionKind::Message,
            payload_json: json!({"requestId":id, "text":"never show private message"}).to_string(),
        };
        journal.store.prepare_topic_exclusive(&row).unwrap();
        if attempted { assert!(journal.store.claim_first_attempt(&row.key).unwrap()); }
    }

    #[test]
    fn only_authorized_entry_and_topic_are_visible_without_payloads() {
        let temp = tempfile::tempdir().unwrap();
        let reader = journal(temp.path());
        let old = session('b');
        seed(&reader, &old, "topic_a", "request_123", true);
        seed(&reader, &old, "topic_b", "request_456", false);
        let results = reader.previews_for_topic(&old, "topic_a").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].state, "AWAITING_RECEIPT");
        assert_eq!(results[0].kind, "MESSAGE");
        assert_eq!(results[0].recovery_mode, "QUERY_BEFORE_EXPLICIT_RETRY");
        assert!(!format!("{results:?}").contains("never show private message"));
        let next = reader.previews_for_topic(&session('c'), "topic_a").unwrap();
        assert_eq!(next[0].recovery_mode, "QUERY_ONLY_CROSS_SESSION");
        assert_eq!(reader.previews_for_topic(&old, "topic_b").unwrap()[0].state,
            "PREPARED_NOT_SENT");
        let mut wrong_entry = old.clone();
        wrong_entry.entry_id = "d".repeat(64);
        assert!(reader.previews_for_topic(&wrong_entry, "topic_a").unwrap().is_empty());
        let mut unauthorized = old.clone();
        unauthorized.topic_keys = vec!["topic_b".into()];
        assert!(matches!(reader.previews_for_topic(&unauthorized, "topic_a"),
            Err(PendingJournalError::TopicUnauthorized)));
        assert_eq!(reader.store.list().unwrap().len(), 2);
    }

    #[test]
    fn canonical_origin_isolation_survives_reopen_without_resend() {
        let temp = tempfile::tempdir().unwrap();
        let original = journal(temp.path());
        seed(&original, &session('b'), "topic_a", "request_123", true);
        drop(original);
        let same = NativePendingJournal::new(
            "https://JOURNAL.EXAMPLE.TEST:443".into(),
            temp.path().to_str().unwrap().into()).unwrap();
        assert_eq!(same.previews_for_topic(&session('b'), "topic_a").unwrap().len(), 1);
        let other = NativePendingJournal::new(
            "https://other.example.test".into(),
            temp.path().to_str().unwrap().into()).unwrap();
        assert!(other.previews_for_topic(&session('b'), "topic_a").unwrap().is_empty());
        assert_eq!(same.store.list().unwrap().len(), 1);
    }

    #[test]
    fn invalid_directory_and_existing_unrelated_db_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        assert!(matches!(NativePendingJournal::new(
            "http://insecure.example.test".into(),
            temp.path().to_str().unwrap().into()),
            Err(PendingJournalError::InvalidScope)));
        assert!(matches!(NativePendingJournal::new(
            ORIGIN.into(), "relative/unsafe".into()),
            Err(PendingJournalError::InvalidScope)));
        let key = credential_storage_key(ORIGIN.into()).unwrap();
        let file = temp.path().join(format!("vcp-pending-v1-{key}.sqlite"));
        std::fs::write(&file, b"unrelated database bytes").unwrap();
        let previous = std::fs::read(&file).unwrap();
        assert!(matches!(NativePendingJournal::new(
            ORIGIN.into(), temp.path().to_str().unwrap().into()),
            Err(PendingJournalError::StoreUnavailable)));
        assert_eq!(std::fs::read(&file).unwrap(), previous);
    }

    #[test]
    fn first_send_claim_is_durable_once_only_and_private() {
        let temp = tempfile::tempdir().unwrap();
        let local = journal(temp.path());
        let current = session('b');
        let verified = crate::session_credential::history_preflight(
            ORIGIN, &current, "topic_a", 0).unwrap();
        let plan = local.claim_original_message(
            &verified, "topic_a", "request_123", "synthetic payload private 🧭").unwrap();
        assert_eq!(plan.request_id(), "request_123");
        assert!(plan.original_body().contains("synthetic payload private"));
        assert!(!format!("{plan:?}").contains("synthetic payload private"));
        let row = local.store.list().unwrap().pop().unwrap();
        assert_eq!(row.state, PendingState::AwaitingConfirmation);
        assert_eq!(row.submission.key.session_marker, verified.session_marker);
        assert_eq!(local.previews_for_topic(&current, "topic_a").unwrap().len(), 1);
        assert!(matches!(local.claim_original_message(
            &verified, "topic_a", "request_123", "synthetic payload private 🧭"),
            Err(NativeSendError::OutstandingPending)));
        assert!(matches!(local.claim_original_message(
            &verified, "topic_a", "request_456", "different"),
            Err(NativeSendError::OutstandingPending)));
        drop(local);
        assert_eq!(journal(temp.path()).store.list().unwrap().len(), 1);
    }

    #[test]
    fn invalid_message_and_changed_session_cannot_create_an_attempt() {
        let temp = tempfile::tempdir().unwrap();
        let journal = journal(temp.path());
        let verified = crate::session_credential::history_preflight(
            ORIGIN, &session('b'), "topic_a", 0).unwrap();
        for (id, text) in [("too", "hello"), ("request_123", "  "),
            ("request_123", "") ] {
            assert!(matches!(journal.claim_original_message(&verified,
                "topic_a", id, text), Err(NativeSendError::InvalidMessage)));
        }
        assert!(matches!(journal.claim_original_message(&verified,
            "topic_unauthorized", "request_123", "hello"),
            Err(NativeSendError::TopicUnauthorized)));
        assert!(journal.store.list().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_storage_root_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(temp.path(), &link).unwrap();
        assert!(matches!(NativePendingJournal::new(
            ORIGIN.into(), link.to_str().unwrap().into()),
            Err(PendingJournalError::InvalidScope)));
    }
}
