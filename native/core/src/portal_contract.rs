//! Source-derived portal preflight; no network calls, cookies, or task control.
use crate::pending::{PendingRecord, PendingStore, StoreError, Submission};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("invalid session JSON")] Malformed,
    #[error("session not authenticated")] Unauthenticated,
    #[error("invalid recovery identity")] Recovery,
    #[error("invalid authorized Topic catalog")] Catalog,
    #[error("Topic not authorized by session")] TopicNotAllowed,
    #[error("entry/session context changed; do not dispatch")] ContextChanged,
    #[error("pending storage denied: {0}")] Store(#[from] StoreError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalSession {
    pub entry_id: String,
    pub session_marker: String,
    pub topic_keys: Vec<String>,
    pub selected_topic: Option<String>,
}
fn fingerprint(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn topic_key(v: &str) -> bool {
    !v.is_empty() && v.len() <= 64
        && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
impl PortalSession {
    /// Parse the HTTPS, cookie-authorized /mobile/api/session response.
    /// Recovery hashes are non-credential reject-only guards, not authority.
    pub fn parse(body: &str) -> Result<Self, PortalError> {
        let data: Value = serde_json::from_str(body).map_err(|_| PortalError::Malformed)?;
        let obj = data.as_object().ok_or(PortalError::Malformed)?;
        if obj.get("authenticated") != Some(&Value::Bool(true)) {
            return Err(PortalError::Unauthenticated);
        }
        let r = obj.get("recovery").and_then(Value::as_object).ok_or(PortalError::Recovery)?;
        if r.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(PortalError::Recovery);
        }
        let entry = r.get("entryId").and_then(Value::as_str).filter(|x| fingerprint(x)).ok_or(PortalError::Recovery)?;
        let marker = r.get("sessionMarker").and_then(Value::as_str).filter(|x| fingerprint(x)).ok_or(PortalError::Recovery)?;
        let catalog = obj.get("topics").and_then(Value::as_array).ok_or(PortalError::Catalog)?;
        if catalog.is_empty() || catalog.len() > 8 { return Err(PortalError::Catalog); }
        let mut seen = HashSet::new();
        let mut topics = Vec::new();
        for t in catalog {
            let key = t.get("key").and_then(Value::as_str).filter(|x| topic_key(x)).ok_or(PortalError::Catalog)?;
            if !seen.insert(key) { return Err(PortalError::Catalog); }
            topics.push(key.to_owned());
        }
        let selected = match obj.get("selectedTopic") {
            None | Some(Value::Null) => None,
            Some(Value::String(k)) if seen.contains(k.as_str()) => Some(k.clone()),
            _ => return Err(PortalError::Catalog),
        };
        Ok(Self {entry_id: entry.to_owned(), session_marker: marker.to_owned(), topic_keys: topics, selected_topic: selected})
    }
    pub fn allows(&self, key: &str) -> bool {
        topic_key(key) && self.topic_keys.iter().any(|k| k == key)
    }
    pub fn submission_path(&self, key: &str, invitation: bool) -> Result<String, PortalError> {
        if !self.allows(key) { return Err(PortalError::TopicNotAllowed); }
        let end = if invitation { "invitations" } else { "messages" };
        Ok(format!("/mobile/api/conversations/{key}/{end}"))
    }
    pub fn guard_headers(&self) -> [(&'static str, &str);2] {
        [("X-NativeSync-Entry-Id", &self.entry_id), ("X-NativeSync-Session-Marker", &self.session_marker)]
    }
    /// Only local admission. Caller must persist attempted state before HTTP;
    /// no POST, approval or new request identity occurs here.
    pub fn admit(&self, store: &PendingStore, submission: &Submission) -> Result<PendingRecord, PortalError> {
        self.admit_at_stamp(store, submission, None)
    }
    pub(crate) fn admit_at_stamp(&self, store: &PendingStore, submission: &Submission,
        stamp: Option<crate::pending::StoreStamp>) -> Result<PendingRecord, PortalError> {
        if submission.key.endpoint_id != self.entry_id || submission.key.session_marker != self.session_marker {
            return Err(PortalError::ContextChanged);
        }
        if !self.allows(&submission.topic_id) { return Err(PortalError::TopicNotAllowed); }
        Ok(store.prepare_topic_exclusive_at_stamp(submission, stamp)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pending::{RequestKey, SubmissionKind};
    use serde_json::json;
    fn doc() -> Value {
        json!({"authenticated":true,"recovery":{"version":1,"entryId":"a".repeat(64),"sessionMarker":"b".repeat(64)},
            "topics":[{"key":"topicA"},{"key":"group_1"}],"selectedTopic":"topicA"})
    }
    fn session() -> PortalSession { PortalSession::parse(&doc().to_string()).unwrap() }
    fn submission() -> Submission {
        Submission{key: RequestKey{endpoint_id:"a".repeat(64),session_marker:"b".repeat(64),request_id:"req-1".into()},
            topic_id:"topicA".into(),kind:SubmissionKind::Message,payload_json:r#"{"requestId":"req-1","text":"synthetic"}"#.into()}
    }
    #[test] fn accepted_session_and_exact_paths() {
        let s=session();
        assert_eq!(s.selected_topic.as_deref(),Some("topicA"));
        assert_eq!(s.submission_path("topicA",false).unwrap(),"/mobile/api/conversations/topicA/messages");
        assert_eq!(s.submission_path("group_1",true).unwrap(),"/mobile/api/conversations/group_1/invitations");
        assert_eq!(s.guard_headers()[0].1,"a".repeat(64));
        assert!(matches!(s.submission_path("other",false),Err(PortalError::TopicNotAllowed)));
        assert!(matches!(s.submission_path("../p",false),Err(PortalError::TopicNotAllowed)));
    }
    #[test] fn reject_unauthed_and_absent_recovery() {
        let mut v=doc(); v["authenticated"]=json!(false);
        assert!(matches!(PortalSession::parse(&v.to_string()),Err(PortalError::Unauthenticated)));
        v=doc();v["recovery"]=Value::Null;
        assert!(matches!(PortalSession::parse(&v.to_string()),Err(PortalError::Recovery)));
    }
    #[test] fn reject_unknown_version_and_changed_fingerprint() {
        let mut v=doc();v["recovery"]["version"]=json!(2);
        assert!(matches!(PortalSession::parse(&v.to_string()),Err(PortalError::Recovery)));
        v=doc();v["recovery"]["entryId"]=json!("A".repeat(64));
        assert!(matches!(PortalSession::parse(&v.to_string()),Err(PortalError::Recovery)));
    }
    #[test] fn reject_duplicate_or_unauthorized_topic() {
        let mut v=doc();v["topics"]=json!([{"key":"dup"},{"key":"dup"}]);
        assert!(matches!(PortalSession::parse(&v.to_string()),Err(PortalError::Catalog)));
        v=doc();v["selectedTopic"]=json!("not_in_catalog");
        assert!(matches!(PortalSession::parse(&v.to_string()),Err(PortalError::Catalog)));
    }
    #[test] fn matching_session_is_idempotent_and_exclusive() {
        let dir=tempfile::tempdir().unwrap();
        let store=PendingStore::open(&dir.path().join("isolated.db")).unwrap();
        let s=session();let first=submission();
        let a=s.admit(&store,&first).unwrap();
        assert_eq!(s.admit(&store,&first).unwrap(),a);
        let mut second=first.clone();second.key.request_id="other".into();
        assert!(matches!(s.admit(&store,&second),Err(PortalError::Store(StoreError::TopicPending))));
        assert_eq!(store.list().unwrap().len(),1);
    }
    #[test] fn changed_session_or_topic_cannot_relabel_old_submission() {
        let dir=tempfile::tempdir().unwrap();
        let store=PendingStore::open(&dir.path().join("isolated.db")).unwrap();
        let s=session();let first=submission();s.admit(&store,&first).unwrap();
        let mut second=first.clone();second.key.session_marker="c".repeat(64);
        assert!(matches!(s.admit(&store,&second),Err(PortalError::ContextChanged)));
        second=first.clone();second.topic_id="unlisted".into();
        assert!(matches!(s.admit(&store,&second),Err(PortalError::TopicNotAllowed)));
        assert_eq!(store.list().unwrap().len(),1);
    }
}
