//! App-private, per-origin read-only pending journal view for native recovery.
//! This module NEVER dispatches, retries, reconciles, or deletes a request.
//! The host must supply its trusted app-private, no-backup directory.
use crate::pending::{PendingState, PendingStore, SubmissionKind};
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
    origin: String,
    storage_key: String,
    store: PendingStore,
}

#[uniffi::export(async_runtime = "tokio")]
impl NativePendingJournal {
    /// The mobile host supplies a *trusted* private directory, never a user
    /// selected path. Each canonical HTTPS origin has a distinct SQLite file.
    #[uniffi::constructor]
    pub fn new(origin: String, private_directory: String)
        -> Result<Arc<Self>, PendingJournalError>
    {
        let canonical = HttpsOrigin::parse(&origin)
            .map_err(|_| PendingJournalError::InvalidScope)?;
        let storage_key = credential_storage_key(canonical.as_str().to_owned())
            .map_err(|_| PendingJournalError::InvalidScope)?;
        let root = PathBuf::from(private_directory);
        if !root.is_absolute() {
            return Err(PendingJournalError::InvalidScope);
        }
        let metadata = fs::symlink_metadata(&root)
            .map_err(|_| PendingJournalError::StoreUnavailable)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(PendingJournalError::InvalidScope);
        }
        let location = root.join(format!("vcp-pending-v1-{storage_key}.sqlite"));
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
}

impl NativePendingJournal {
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
