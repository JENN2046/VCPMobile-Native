//! Source-derived page envelope for the authorized NativeSync history GET.
//! Pages and execution receipts are separate. Parsing never writes history,
//! removes pending submissions, dispatches a model, or grants task control.
use crate::https_transport::HistorySnapshot;
use crate::portal_contract::PortalSession;
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

pub const MAX_SAFE_CURSOR: u64 = 9_007_199_254_740_991;
const MAX_PAGE_ROWS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HistoryError {
    #[error("history response belongs to another context")]
    ContextChanged,
    #[error("history Topic is not authorized")]
    TopicDenied,
    #[error("invalid history page envelope")]
    InvalidPage,
    #[error("invalid history cursor or sequence")]
    InvalidCursor,
    #[error("duplicate message identity in history page")]
    DuplicateMessage,
}

#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct NativeHistoryRow {
    pub sequence: u64,
    pub message_id: String,
    pub deleted: bool,
    /// Opaque native message object, never HTML to execute or diagnostic text.
    /// Tombstones intentionally carry no content.
    pub message_json: Option<String>,
}
impl fmt::Debug for NativeHistoryRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeHistoryRow")
            .field("sequence", &self.sequence)
            .field("deleted", &self.deleted)
            .field("identity_and_content", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct NativeHistoryPage {
    pub entry_id: String,
    pub session_marker: String,
    pub topic_key: String,
    pub after: u64,
    pub next_cursor: u64,
    pub has_more: bool,
    pub rows: Vec<NativeHistoryRow>,
    /// Unpaginated execution projection. Not a successful-completion claim;
    /// pending cleanup still requires exact original receipt reconciliation.
    pub executions_json: String,
    pub topic_state_json: String,
}
impl fmt::Debug for NativeHistoryPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeHistoryPage")
            .field("after", &self.after)
            .field("next_cursor", &self.next_cursor)
            .field("has_more", &self.has_more)
            .field("row_count", &self.rows.len())
            .field("context_and_content", &"[REDACTED]")
            .finish()
    }
}

impl NativeHistoryPage {
    /// Only the authenticated HTTP adapter supplies a snapshot in production.
    /// Snapshot tags bind the request context, not a server-issued signature.
    pub fn parse(snapshot: &HistorySnapshot, session: &PortalSession) -> Result<Self, HistoryError> {
        if snapshot.entry_id != session.entry_id || snapshot.session_marker != session.session_marker {
            return Err(HistoryError::ContextChanged);
        }
        if !session.allows(&snapshot.topic_key) { return Err(HistoryError::TopicDenied); }
        if snapshot.after > MAX_SAFE_CURSOR { return Err(HistoryError::InvalidCursor); }
        let object = snapshot.document.as_object().ok_or(HistoryError::InvalidPage)?;
        let messages = object.get("messages").and_then(Value::as_array)
            .filter(|rows| rows.len() <= MAX_PAGE_ROWS).ok_or(HistoryError::InvalidPage)?;
        let next_cursor = object.get("next").and_then(Value::as_u64)
            .filter(|n| *n <= MAX_SAFE_CURSOR).ok_or(HistoryError::InvalidCursor)?;
        let has_more = object.get("hasMore").and_then(Value::as_bool).ok_or(HistoryError::InvalidPage)?;
        let executions = object.get("executions").filter(|v| v.is_array()).ok_or(HistoryError::InvalidPage)?;
        let state = object.get("topicState").filter(|v| v.is_object()).ok_or(HistoryError::InvalidPage)?;
        if state.get("historyReady").and_then(Value::as_bool).is_none()
            || state.get("executable").and_then(Value::as_bool).is_none()
        { return Err(HistoryError::InvalidPage); }

        let mut seen = HashSet::new();
        let mut previous = snapshot.after;
        let mut rows = Vec::with_capacity(messages.len());
        for row in messages {
            let sequence = row.get("seq").and_then(Value::as_u64)
                .filter(|n| *n > previous && *n <= MAX_SAFE_CURSOR).ok_or(HistoryError::InvalidCursor)?;
            // Protocol message identities are preserved verbatim; not restricted
            // to the Topic-key alphabet or normalized before application.
            let id = row.get("id").and_then(Value::as_str)
                .filter(|s| !s.is_empty()).ok_or(HistoryError::InvalidPage)?;
            if !seen.insert(id) { return Err(HistoryError::DuplicateMessage); }
            let deleted = row.get("deleted").and_then(Value::as_bool).ok_or(HistoryError::InvalidPage)?;
            let message = row.get("message").ok_or(HistoryError::InvalidPage)?;
            if !(message.is_object() || deleted && message.is_null()) {
                return Err(HistoryError::InvalidPage);
            }
            rows.push(NativeHistoryRow { sequence, message_id: id.to_owned(), deleted,
                message_json: if deleted { None } else { Some(message.to_string()) } });
            previous = sequence;
        }
        // Server SQL pages use LIMIT 201 and emit the first 200 rows, including
        // tombstones. Empty pages keep the requested cursor and cannot loop.
        if next_cursor != previous || (has_more && messages.len() != MAX_PAGE_ROWS) {
            return Err(HistoryError::InvalidCursor);
        }
        Ok(Self { entry_id: snapshot.entry_id.clone(), session_marker: snapshot.session_marker.clone(),
            topic_key: snapshot.topic_key.clone(), after: snapshot.after, next_cursor, has_more, rows,
            executions_json: executions.to_string(), topic_state_json: state.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn session() -> PortalSession {
        PortalSession::parse(&json!({"authenticated":true,"topics":[{"key":"topic_a"}],
            "recovery":{"version":1,"entryId":"a".repeat(64),"sessionMarker":"b".repeat(64)}}).to_string()).unwrap()
    }
    fn row(seq: u64, id: &str) -> Value {
        json!({"seq":seq,"id":id,"deleted":false,"message":{"role":"assistant","content":"合成内容 🧭","timestamp":1}})
    }
    fn snapshot() -> HistorySnapshot {
        let s = session();
        HistorySnapshot { entry_id: s.entry_id, session_marker: s.session_marker, topic_key:"topic_a".into(),after:0,
            document:json!({"messages":[row(2,"msg_1")],"next":2,"hasMore":false,
                "executions":[{"id":"job","state":"running","extension":{"nativeTasks":[]}}],
                "topicState":{"historyReady":true,"executable":false,"unavailableReason":"CONFIG_REQUIRED"}}) }
    }
    #[test] fn preserves_native_content_and_separate_execution_projection() {
        let page = NativeHistoryPage::parse(&snapshot(), &session()).unwrap();
        assert_eq!(page.rows.len(),1);
        let msg:Value = serde_json::from_str(page.rows[0].message_json.as_ref().unwrap()).unwrap();
        assert_eq!(msg["content"],"合成内容 🧭");
        assert_eq!(serde_json::from_str::<Value>(&page.executions_json).unwrap()[0]["state"],"running");
        assert_eq!(page.next_cursor,2);
    }
    #[test] fn empty_page_retains_cursor_but_keeps_execution_updates() {
        let mut snap=snapshot();snap.after=9;snap.document["messages"]=json!([]);snap.document["next"]=json!(9);
        let page=NativeHistoryPage::parse(&snap,&session()).unwrap();
        assert!(page.rows.is_empty());assert_eq!(page.next_cursor,9);
        assert_ne!(page.executions_json,"[]");
    }
    #[test] fn tombstones_advance_cursor_without_exposing_removed_content() {
        let mut snap=snapshot();snap.document["messages"]=json!([row(2,"msg_1"),{"seq":8,"id":"removed","deleted":true,"message":null}]);snap.document["next"]=json!(8);
        let page=NativeHistoryPage::parse(&snap,&session()).unwrap();
        assert!(page.rows[1].deleted);assert_eq!(page.rows[1].message_json,None);assert_eq!(page.next_cursor,8);
    }
    #[test] fn full_page_and_additive_envelope_fields_are_supported() {
        let mut snap=snapshot();snap.document["messages"]=Value::Array((1..=200).map(|n|row(n,&format!("m{n}"))).collect());
        snap.document["next"]=json!(200);snap.document["hasMore"]=json!(true);snap.document["future_field"]=json!({"v":2});
        let p=NativeHistoryPage::parse(&snap,&session()).unwrap();assert_eq!(p.rows.len(),200);assert!(p.has_more);
        snap.document["messages"].as_array_mut().unwrap().push(row(201,"overflow"));
        assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
    }
    #[test] fn wrong_context_or_topic_cannot_apply_page() {
        for field in ["entry","session","topic"] {
            let mut snap=snapshot();match field {"entry"=>snap.entry_id="c".repeat(64),"session"=>snap.session_marker="d".repeat(64),_=>snap.topic_key="other".into()};
            assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
        }
    }
    #[test] fn rejects_backwards_duplicate_and_unsafe_sequences() {
        for seq in [0,MAX_SAFE_CURSOR+1] { let mut snap=snapshot();snap.document["messages"][0]["seq"]=json!(seq);assert!(NativeHistoryPage::parse(&snap,&session()).is_err()); }
        let mut snap=snapshot();snap.document["messages"]=json!([row(2,"m1"),row(2,"m2")]);assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
        snap.document["messages"]=json!([row(3,"m1"),row(2,"m2")]);assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
        snap.document["messages"]=json!([row(1,"same"),row(2,"same")]);
        assert_eq!(NativeHistoryPage::parse(&snap,&session()).unwrap_err(),HistoryError::DuplicateMessage);
    }
    #[test] fn next_cursor_cannot_skip_rows_or_loop() {
        for next in [0,1,3,MAX_SAFE_CURSOR+1] { let mut snap=snapshot();snap.document["next"]=json!(next);assert!(NativeHistoryPage::parse(&snap,&session()).is_err()); }
        let mut snap=snapshot();snap.document["hasMore"]=json!(true);assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
        snap.document["messages"]=json!([]);snap.document["next"]=json!(0);assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
        snap.after=MAX_SAFE_CURSOR+1;assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
    }
    #[test] fn malformed_arrays_flags_and_state_fail_without_partial_result() {
        for (key,value) in [("messages",json!({})),("executions",json!(null)),("hasMore",json!("false")),("topicState",json!({"historyReady":true}))] {
            let mut snap=snapshot();snap.document[key]=value;assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
        }
        for (key,value) in [("seq",json!(-1)),("deleted",json!(0)),("id",json!("")),("message",json!([])),("message",json!(null))] {
            let mut snap=snapshot();snap.document["messages"][0][key]=value;assert!(NativeHistoryPage::parse(&snap,&session()).is_err());
        }
    }
    #[test] fn diagnostics_redact_ids_context_content_and_task_bodies() {
        let snap=snapshot();let page=NativeHistoryPage::parse(&snap,&session()).unwrap();let debug=format!("{page:?} {:?}",page.rows[0]);
        for secret in ["msg_1","topic_a","合成内容","running","job",&snap.entry_id,&snap.session_marker] { assert!(!debug.contains(secret)); }
    }
}
