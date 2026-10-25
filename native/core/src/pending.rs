//! Bounded local storage; no transport, credential access, or server authority.
//! Prepare and mark_attempted must both succeed before network dispatch.
//! A network failure never deletes a row or generates a new request identity.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

const APP_ID: i64 = 0x5643504E;
const SCHEMA: i64 = 1;
pub const MAX_RECORDS: usize = 16;
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage unavailable")]
    Storage(#[from] rusqlite::Error),
    #[error("invalid pending submission")]
    InvalidInput,
    #[error("unsupported or unrelated database")]
    UnsupportedSchema,
    #[error("pending identity or original payload does not match")]
    IdentityConflict,
    #[error("entry and Topic already have an unresolved submission")]
    TopicPending,
    #[error("pending storage capacity reached; no request may be sent")]
    Capacity,
    #[error("pending request not found")]
    NotFound,
    #[error("storage mutex poisoned")]
    Poisoned,
    #[error("stored submission is corrupt; original data retained")]
    Corrupt,
}

/// Namespace only. session_marker is non-credential continuity metadata,
/// never a Cookie, bearer token, pairing key, or global authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestKey {
    pub endpoint_id: String,
    pub session_marker: String,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionKind {
    Message,
    Invite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingState {
    Prepared,
    AwaitingConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub key: RequestKey,
    pub topic_id: String,
    pub kind: SubmissionKind,
    /// Original bytes: no JSON normalization or invented server request_hash.
    /// Keep credentials in platform secure storage, never in this payload.
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRecord {
    pub submission: Submission,
    pub state: PendingState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    QueryOriginalBeforeExplicitRetry,
    QueryOnlyAcrossSession,
}

/// SQLite handles cross-connection transactions. The mutex protects one
/// connection, not the whole application or a remote execution authority.
#[derive(Debug)]
pub struct PendingStore {
    connection: Mutex<Connection>,
}

impl PendingStore {
    /// Use a new file in the private application directory, off the UI thread.
    /// Unrelated and future-version databases are never reinitialized.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if !path.is_absolute() {
            return Err(StoreError::InvalidInput);
        }
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_millis(500))?;
        {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let app: i64 = tx.query_row("PRAGMA application_id", [], |r| r.get(0))?;
            let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            let tables: i64 = tx.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            if app == 0 && version == 0 && tables == 0 {
                tx.execute_batch("CREATE TABLE pending_submission (
                    endpoint_id TEXT NOT NULL,
                    session_marker TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    topic_id TEXT NOT NULL,
                    kind TEXT NOT NULL CHECK(kind IN ('message','invite')),
                    payload_json TEXT NOT NULL,
                    state TEXT NOT NULL CHECK(state IN ('prepared','awaiting_confirmation')),
                    PRIMARY KEY(endpoint_id, session_marker, request_id)
                ) WITHOUT ROWID;")?;
                tx.pragma_update(None, "application_id", APP_ID)?;
                tx.pragma_update(None, "user_version", SCHEMA)?;
            } else if app != APP_ID || version != SCHEMA || tables != 1 {
                return Err(StoreError::UnsupportedSchema);
            }
            tx.commit()?;
        }
        let mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
        if mode != "wal" {
            return Err(StoreError::UnsupportedSchema);
        }
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.prepare("SELECT endpoint_id,session_marker,request_id,topic_id,kind,payload_json,state FROM pending_submission LIMIT 0")?;
        Ok(Self { connection: Mutex::new(conn) })
    }

    pub fn prepare(&self, submission: &Submission) -> Result<PendingRecord, StoreError> {
        validate(submission)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = read_one(&tx, &submission.key)? {
            if existing.submission != *submission {
                return Err(StoreError::IdentityConflict);
            }
            tx.commit()?;
            return Ok(existing);
        }
        let (count, bytes): (i64, i64) = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(payload_json AS BLOB))),0) FROM pending_submission",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if count >= MAX_RECORDS as i64
            || bytes + submission.payload_json.len() as i64 > MAX_PAYLOAD_BYTES as i64
        {
            return Err(StoreError::Capacity);
        }
        tx.execute(
            "INSERT INTO pending_submission VALUES (?1,?2,?3,?4,?5,?6,'prepared')",
            params![submission.key.endpoint_id, submission.key.session_marker,
                submission.key.request_id, submission.topic_id,
                kind_name(&submission.kind), submission.payload_json],
        )?;
        let saved = read_one(&tx, &submission.key)?.ok_or(StoreError::Corrupt)?;
        if saved.submission != *submission {
            return Err(StoreError::Corrupt);
        }
        tx.commit()?;
        Ok(saved)
    }


    /// Application dispatch admission, distinct from low-level prepare().
    /// Reserve at most one unresolved request per (entry, Topic), including
    /// earlier sessions and invitation requests. Must happen before any POST.
    /// IMMEDIATE serializes admission across independent SQLite connections.
    pub fn prepare_topic_exclusive(&self, submission: &Submission) -> Result<PendingRecord, StoreError> {
        validate(submission)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (count, same): (i64, i64) = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN session_marker=?3 AND request_id=?4 THEN 1 ELSE 0 END), 0) FROM pending_submission WHERE endpoint_id=?1 AND topic_id=?2",
            params![submission.key.endpoint_id, submission.topic_id, submission.key.session_marker, submission.key.request_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // Existing ambiguous duplicates are retained and block new admission.
        if count > 1 || (count == 1 && same != 1) {
            return Err(StoreError::TopicPending);
        }
        if let Some(original) = read_one(&tx, &submission.key)? {
            if original.submission != *submission {
                return Err(StoreError::IdentityConflict);
            }
            tx.commit()?;
            return Ok(original);
        }
        let (rows, bytes): (i64, i64) = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(payload_json AS BLOB))),0) FROM pending_submission",
            [], |row| Ok((row.get(0)?, row.get(1)?))
        )?;
        if rows >= MAX_RECORDS as i64 || bytes + submission.payload_json.len() as i64 > MAX_PAYLOAD_BYTES as i64 {
            return Err(StoreError::Capacity);
        }
        tx.execute(
            "INSERT INTO pending_submission VALUES (?1,?2,?3,?4,?5,?6,'prepared')",
            params![submission.key.endpoint_id, submission.key.session_marker, submission.key.request_id, submission.topic_id, kind_name(&submission.kind), submission.payload_json],
        )?;
        let saved = read_one(&tx, &submission.key)?.ok_or(StoreError::Corrupt)?;
        if saved.submission != *submission { return Err(StoreError::Corrupt); }
        tx.commit()?;
        Ok(saved)
    }

    /// Persist before dispatch. This operation does not send or accept a request.
    pub fn mark_attempted(&self, key: &RequestKey) -> Result<(), StoreError> {
        validate_key(key)?;
        let conn = self.lock()?;
        let changed = conn.execute(
            "UPDATE pending_submission SET state='awaiting_confirmation' WHERE endpoint_id=?1 AND session_marker=?2 AND request_id=?3",
            params![key.endpoint_id, key.session_marker, key.request_id],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn get(&self, key: &RequestKey) -> Result<Option<PendingRecord>, StoreError> {
        validate_key(key)?;
        let conn = self.lock()?;
        read_one(&conn, key)
    }

    pub fn list(&self) -> Result<Vec<PendingRecord>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT endpoint_id,session_marker,request_id,topic_id,kind,payload_json,state FROM pending_submission ORDER BY endpoint_id,session_marker,request_id LIMIT 17")?;
        let rows = stmt.query_map([], decode_row)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(validate_stored(row?)?);
        }
        if result.len() > MAX_RECORDS
            || result.iter().map(|r| r.submission.payload_json.len()).sum::<usize>() > MAX_PAYLOAD_BYTES
        {
            return Err(StoreError::Corrupt);
        }
        Ok(result)
    }

    pub fn recovery_action(
        &self,
        key: &RequestKey,
        active_endpoint: &str,
        active_session_marker: &str,
    ) -> Result<RecoveryAction, StoreError> {
        let record = self.get(key)?.ok_or(StoreError::NotFound)?;
        if active_endpoint != record.submission.key.endpoint_id || !valid_id(active_session_marker) {
            return Err(StoreError::IdentityConflict);
        }
        Ok(if active_session_marker == record.submission.key.session_marker {
            RecoveryAction::QueryOriginalBeforeExplicitRetry
        } else {
            RecoveryAction::QueryOnlyAcrossSession
        })
    }

    /// Call only after an authenticated adapter verifies the exact original
    /// ownership, Topic, kind and payload. This is local cleanup, not AA success.
    pub fn confirm_verified(&self, original: &Submission) -> Result<bool, StoreError> {
        validate(original)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(existing) = read_one(&tx, &original.key)? else {
            return Ok(false);
        };
        if existing.submission != *original {
            return Err(StoreError::IdentityConflict);
        }
        tx.execute(
            "DELETE FROM pending_submission WHERE endpoint_id=?1 AND session_marker=?2 AND request_id=?3",
            params![original.key.endpoint_id, original.key.session_marker, original.key.request_id],
        )?;
        tx.commit()?;
        Ok(true)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.connection.lock().map_err(|_| StoreError::Poisoned)
    }
}

type StoredRow = (String, String, String, String, String, String, String);

fn decode_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))
}

fn read_one(conn: &Connection, key: &RequestKey) -> Result<Option<PendingRecord>, StoreError> {
    let row = conn.query_row(
        "SELECT endpoint_id,session_marker,request_id,topic_id,kind,payload_json,state FROM pending_submission WHERE endpoint_id=?1 AND session_marker=?2 AND request_id=?3",
        params![key.endpoint_id, key.session_marker, key.request_id],
        decode_row,
    ).optional()?;
    row.map(validate_stored).transpose()
}

fn validate_stored(row: StoredRow) -> Result<PendingRecord, StoreError> {
    let (endpoint_id, session_marker, request_id, topic_id, kind, payload_json, state) = row;
    let submission = Submission {
        key: RequestKey { endpoint_id, session_marker, request_id },
        topic_id,
        kind: match kind.as_str() {
            "message" => SubmissionKind::Message,
            "invite" => SubmissionKind::Invite,
            _ => return Err(StoreError::Corrupt),
        },
        payload_json,
    };
    validate(&submission).map_err(|_| StoreError::Corrupt)?;
    let state = match state.as_str() {
        "prepared" => PendingState::Prepared,
        "awaiting_confirmation" => PendingState::AwaitingConfirmation,
        _ => return Err(StoreError::Corrupt),
    };
    Ok(PendingRecord { submission, state })
}

fn kind_name(kind: &SubmissionKind) -> &'static str {
    match kind {
        SubmissionKind::Message => "message",
        SubmissionKind::Invite => "invite",
    }
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn validate_key(key: &RequestKey) -> Result<(), StoreError> {
    if !valid_id(&key.endpoint_id) || !valid_id(&key.session_marker) || !valid_id(&key.request_id) {
        return Err(StoreError::InvalidInput);
    }
    Ok(())
}

fn validate(submission: &Submission) -> Result<(), StoreError> {
    validate_key(&submission.key)?;
    if !valid_id(&submission.topic_id) || submission.payload_json.len() > MAX_PAYLOAD_BYTES {
        return Err(StoreError::InvalidInput);
    }
    let value: serde_json::Value = serde_json::from_str(&submission.payload_json)
        .map_err(|_| StoreError::InvalidInput)?;
    if !value.is_object() {
        return Err(StoreError::InvalidInput);
    }
    Ok(())
}
