//! R2.1 portal request plans + durable first-attempt and exact receipt flow.
//! No HTTP or Cookie handler: platform MUST enforce exact HTTPS origin,
//! secure path-scoped Cookie, no redirects and authenticated GET provenance.
use crate::pending::{PendingState, PendingStore, RequestKey, StoreError, Submission};
use crate::pending::SubmissionKind as StoredKind;
use crate::portal_contract::{PortalError, PortalSession};
use crate::recovery::{ReceiptInspection, RecoverySubmission, ReviewReason, SessionRecovery, SubmissionKind as WireKind};
use serde_json::{json, Value};
use std::fmt;

const JS_MAX_SAFE_CURSOR: u64 = 9_007_199_254_740_991;

#[derive(Debug, thiserror::Error)]
pub enum FlowError {
    #[error("invalid HTTPS origin")]
    InvalidOrigin,
    #[error("invalid NativeSync submission")]
    InvalidSubmission,
    #[error("history cursor exceeds JavaScript safe integer")]
    InvalidCursor,
    #[error("attempt already claimed: reconcile original receipt first")]
    ReconcileFirst,
    #[error("entry changed: do not dispatch or clean up")]
    EntryChanged,
    #[error("pending Topic not authorized")]
    TopicNotAllowed,
    #[error("portal preflight failed: {0}")]
    Portal(#[from] PortalError),
    #[error("pending storage failed: {0}")]
    Store(#[from] StoreError),
}

/// Canonical DNS-only HTTPS origin. IP literals and IDNs fail closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpsOrigin(String);
impl HttpsOrigin {
    pub fn parse(raw: &str) -> Result<Self, FlowError> {
        let part = raw.strip_prefix("https://").ok_or(FlowError::InvalidOrigin)?;
        if part.is_empty() || part.len() > 259 ||
            part.bytes().any(|b| !b.is_ascii() || b <= 32 || b >= 127 ||
                matches!(b, b'/' | b'\\' | b'@' | b'?' | b'#' | b'%' | b'[' | b']'))
        { return Err(FlowError::InvalidOrigin); }
        let (host, port) = match part.rsplit_once(':') {
            Some((host, port)) => {
                let n = port.parse::<u16>().map_err(|_| FlowError::InvalidOrigin)?;
                if n == 0 { return Err(FlowError::InvalidOrigin); }
                (host, Some(n))
            }
            None => (part, None),
        };
        if host.is_empty() || host.len() > 253 || host.ends_with('.') ||
            host.split('.').any(|label| {
                label.is_empty() || label.len() > 63 ||
                !label.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric()) ||
                !label.bytes().last().is_some_and(|b| b.is_ascii_alphanumeric()) ||
                !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
        { return Err(FlowError::InvalidOrigin); }
        let suffix = match port { None | Some(443) => String::new(), Some(p) => format!(":{p}") };
        Ok(Self(format!("https://{}{}", host.to_ascii_lowercase(), suffix)))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyRequest {
    pub origin: HttpsOrigin,
    pub path_and_query: String,
}
pub fn session_request(origin: &str) -> Result<ReadOnlyRequest, FlowError> {
    Ok(ReadOnlyRequest { origin: HttpsOrigin::parse(origin)?,
        path_and_query: "/mobile/api/session".into() })
}
pub fn history_request(origin: &str, session: &PortalSession, key: &str, after: u64)
    -> Result<ReadOnlyRequest, FlowError>
{
    if after > JS_MAX_SAFE_CURSOR { return Err(FlowError::InvalidCursor); }
    if !session.allows(key) { return Err(FlowError::TopicNotAllowed); }
    Ok(ReadOnlyRequest { origin: HttpsOrigin::parse(origin)?,
        path_and_query: format!("/mobile/api/conversations/{key}?after={after}") })
}

/// Caller injects Origin, Content-Type, paired guards and the separately
/// protected __Secure-vcp_mobile Cookie. This data does NOT send itself.
#[derive(Clone, PartialEq, Eq)]
pub struct OutboundSubmission {
    pub origin: HttpsOrigin,
    pub path: String,
    pub entry_id: String,
    pub session_marker: String,
    pub request_id: String,
    original_json: String,
}
impl OutboundSubmission {
    pub fn original_body(&self) -> &str { &self.original_json }
    pub fn content_type(&self) -> &'static str { "application/json" }
    pub fn origin_header(&self) -> &str { self.origin.as_str() }
    pub fn guard_headers(&self) -> [(&'static str, &str); 2] {
        [("X-NativeSync-Entry-Id", &self.entry_id),
         ("X-NativeSync-Session-Marker", &self.session_marker)]
    }
}
impl fmt::Debug for OutboundSubmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutboundSubmission")
            .field("origin", &self.origin)
            .field("path", &self.path)
            .field("body_bytes_redacted", &self.original_json.len())
            .finish()
    }
}

/// An Ok intent is the ONE initial local dispatch claim, NOT server acceptance.
/// Persist exact bytes, reserve entry/Topic then atomically claim before POST.
/// Any failure means the platform must not send. A crash before HTTP is UNKNOWN.
pub fn first_attempt(
    origin: &str, session: &PortalSession, store: &PendingStore,
    topic_key: &str, kind: WireKind, original_json: &str,
) -> Result<OutboundSubmission, FlowError> {
    let origin = HttpsOrigin::parse(origin)?;
    let path = session.submission_path(topic_key, kind == WireKind::Invitation)?;
    let payload: Value = serde_json::from_str(original_json).map_err(|_| FlowError::InvalidSubmission)?;
    let wire = RecoverySubmission::new(topic_key.into(), kind, payload)
        .map_err(|_| FlowError::InvalidSubmission)?;
    wire.validate_wire_identity().map_err(|_| FlowError::InvalidSubmission)?;
    let submission = Submission {
        key: RequestKey { endpoint_id: session.entry_id.clone(),
            session_marker: session.session_marker.clone(),
            request_id: wire.request_id().into() },
        topic_id: topic_key.into(),
        kind: match kind { WireKind::Message => StoredKind::Message,
            WireKind::Invitation => StoredKind::Invite },
        payload_json: original_json.into(),
    };
    let saved = session.admit(store, &submission)?;
    if saved.state != PendingState::Prepared || !store.claim_first_attempt(&submission.key)? {
        return Err(FlowError::ReconcileFirst);
    }
    Ok(OutboundSubmission { origin, path,
        entry_id: submission.key.endpoint_id, session_marker: submission.key.session_marker,
        request_id: submission.key.request_id, original_json: submission.payload_json })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconciliation {
    /// Original accepted identity only, NOT completed AA work or task control.
    Accepted { execution_id: String, execution_state: String },
    /// No implicit retry even in the original session.
    SameSessionNoReceipt,
    Review(ReviewReason),
}

/// Executions must come from a real authenticated history GET for this Topic.
/// This pure function cannot itself attest network provenance.
pub fn reconcile_authenticated_history(
    session: &PortalSession, store: &PendingStore,
    original_key: &RequestKey, executions: &Value,
) -> Result<Reconciliation, FlowError> {
    let record = store.get(original_key)?.ok_or(StoreError::NotFound)?;
    if session.entry_id != record.submission.key.endpoint_id { return Err(FlowError::EntryChanged); }
    if !session.allows(&record.submission.topic_id) { return Err(FlowError::TopicNotAllowed); }
    let payload: Value = serde_json::from_str(&record.submission.payload_json)
        .map_err(|_| FlowError::InvalidSubmission)?;
    let kind = match record.submission.kind { StoredKind::Message => WireKind::Message,
        StoredKind::Invite => WireKind::Invitation };
    let wire = RecoverySubmission::new(record.submission.topic_id.clone(), kind, payload)
        .map_err(|_| FlowError::InvalidSubmission)?;
    if wire.request_id() != record.submission.key.request_id { return Err(FlowError::InvalidSubmission); }
    let original = SessionRecovery { entry_id: record.submission.key.endpoint_id.clone(),
        session_marker: record.submission.key.session_marker.clone() };
    let context = json!({"version":1,"entryId":session.entry_id,
        "sessionMarker":session.session_marker});
    match wire.inspect(&original, &context, executions) {
        ReceiptInspection::Confirmed { execution_id, execution_state } => {
            if !store.confirm_verified(&record.submission)? { return Err(FlowError::Store(StoreError::NotFound)); }
            Ok(Reconciliation::Accepted { execution_id, execution_state })
        }
        ReceiptInspection::AbsentSameSession => Ok(Reconciliation::SameSessionNoReceipt),
        ReceiptInspection::Review(reason) => Ok(Reconciliation::Review(reason)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc,Barrier};
    fn session(marker: char) -> PortalSession {
        PortalSession::parse(&json!({"authenticated":true,
            "recovery":{"version":1,"entryId":"a".repeat(64),
                "sessionMarker":marker.to_string().repeat(64)},
            "topics":[{"key":"topic_a"},{"key":"group_1"}],"selectedTopic":"topic_a"}).to_string()).unwrap()
    }
    fn body() -> &'static str { r#"{ "requestId":"test_123", "text":"合成发送 🧭" }"# }
    fn store() -> (tempfile::TempDir,PendingStore) {
        let dir=tempfile::tempdir().unwrap();
        let s=PendingStore::open(&dir.path().join("pending.sqlite")).unwrap();
        (dir,s)
    }
    #[test] fn origin_and_read_only_plan_strict() {
        assert_eq!(HttpsOrigin::parse("https://VCP.example.org:443").unwrap().as_str(),"https://vcp.example.org");
        assert_eq!(HttpsOrigin::parse("https://vcp.example.org:13848").unwrap().as_str(),"https://vcp.example.org:13848");
        for bad in ["http://vcp.example.org","https://u@vcp.example.org",
            "https://vcp.example.org/path","https://vcp.example.org/",
            "https://vcp.example.org\\evil","https://vcp.example.org#x",
            "https://vcp.example.org?x=1","https://vcp.example.org:0",
            "https://vcp..org","https://-abc.org","https://a%2fb.org","https://[::1]"] {
            assert!(HttpsOrigin::parse(bad).is_err(),"{bad}");
        }
        let s=session('b');
        assert_eq!(session_request("https://vcp.example.org").unwrap().path_and_query,"/mobile/api/session");
        assert_eq!(history_request("https://vcp.example.org",&s,"topic_a",10).unwrap().path_and_query,
            "/mobile/api/conversations/topic_a?after=10");
        assert!(matches!(history_request("https://vcp.example.org",&s,"topic_a",JS_MAX_SAFE_CURSOR+1),Err(FlowError::InvalidCursor)));
        assert!(matches!(history_request("https://vcp.example.org",&s,"unauthorized",0),Err(FlowError::TopicNotAllowed)));
    }
    #[test] fn once_only_preserves_original_payload_and_redacts_debug() {
        let (_d,store)=store();let s=session('b');
        let plan=first_attempt("https://vcp.example.org",&s,&store,"topic_a",WireKind::Message,body()).unwrap();
        assert_eq!(plan.original_body(),body());
        assert_eq!(plan.guard_headers(),s.guard_headers());
        assert_eq!(plan.content_type(),"application/json");
        assert!(!format!("{plan:?}").contains("合成发送"));
        let rows=store.list().unwrap();assert_eq!(rows.len(),1);
        assert_eq!(rows[0].state,PendingState::AwaitingConfirmation);
        assert_eq!(rows[0].submission.payload_json,body());
        assert!(matches!(first_attempt("https://vcp.example.org",&s,&store,"topic_a",WireKind::Message,body()),Err(FlowError::ReconcileFirst)));
    }
    #[test] fn reject_bad_payload_scope_and_origin_before_claim() {
        let (_d,store)=store();let s=session('b');
        assert!(matches!(first_attempt("https://vcp.example.org",&s,&store,"other",WireKind::Message,body()),Err(FlowError::Portal(_))));
        for input in [r#"{"requestId":"bad","text":"hi"}"#,r#"{"requestId":"test_123","text":""}"#,
            r#"{"requestId":"test_123","agentId":"x"}"#,
            r#"{"requestId":"test_123","text":"hi","extra":2}"#] {
            assert!(matches!(first_attempt("https://vcp.example.org",&s,&store,"topic_a",WireKind::Message,input),Err(FlowError::InvalidSubmission)));
        }
        assert!(matches!(first_attempt("http://bad",&s,&store,"topic_a",WireKind::Message,body()),Err(FlowError::InvalidOrigin)));
        assert!(store.list().unwrap().is_empty());
    }
    #[test] fn missing_receipt_never_resends_even_across_session() {
        let (_d,store)=store();let old=session('b');
        first_attempt("https://vcp.example.org",&old,&store,"topic_a",WireKind::Message,body()).unwrap();
        let key=store.list().unwrap()[0].submission.key.clone();
        assert_eq!(reconcile_authenticated_history(&old,&store,&key,&json!([])).unwrap(),Reconciliation::SameSessionNoReceipt);
        assert_eq!(reconcile_authenticated_history(&session('c'),&store,&key,&json!([])).unwrap(),
            Reconciliation::Review(ReviewReason::Session));
        assert_eq!(store.list().unwrap().len(),1);
    }
    #[test] fn exact_cross_session_receipt_closes_original_only() {
        let (_d,store)=store();let old=session('b');
        first_attempt("https://vcp.example.org",&old,&store,"topic_a",WireKind::Message,body()).unwrap();
        let key=store.list().unwrap()[0].submission.key.clone();
        let wire=RecoverySubmission::new("topic_a".into(),WireKind::Message,serde_json::from_str(body()).unwrap()).unwrap();
        let receipt=json!([{"id":"exec_1","topicKey":"topic_a","requestId":"test_123","state":"running",
            "recovery":{"version":1,"sessionMarker":old.session_marker,
                "payloadHash":wire.payload_hash(),"kind":"message"}}]);
        assert_eq!(reconcile_authenticated_history(&session('c'),&store,&key,&receipt).unwrap(),
            Reconciliation::Accepted{execution_id:"exec_1".into(),execution_state:"running".into()});
        assert!(store.list().unwrap().is_empty());
    }
    #[test] fn conflicting_and_ambiguous_receipts_preserve_unknown() {
        let (_d,store)=store();let old=session('b');
        first_attempt("https://vcp.example.org",&old,&store,"topic_a",WireKind::Message,body()).unwrap();
        let key=store.list().unwrap()[0].submission.key.clone();
        let bad=json!([{"id":"e1","topicKey":"topic_a","requestId":"test_123","state":"running",
            "recovery":{"version":1,"sessionMarker":old.session_marker,"payloadHash":"c".repeat(64),"kind":"message"}}]);
        assert_eq!(reconcile_authenticated_history(&old,&store,&key,&bad).unwrap(),Reconciliation::Review(ReviewReason::Conflict));
        assert_eq!(reconcile_authenticated_history(&old,&store,&key,&json!([bad[0].clone(),bad[0].clone()])).unwrap(),
            Reconciliation::Review(ReviewReason::Ambiguous));
        assert_eq!(store.list().unwrap().len(),1);
    }
    #[test] fn concurrent_instances_only_one_first_post_claim() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("race.sqlite");
        let a_store=PendingStore::open(&path).unwrap();let b_store=PendingStore::open(&path).unwrap();
        let s=session('b');let s2=s.clone();let gate=Arc::new(Barrier::new(2));let g=gate.clone();
        let handle=std::thread::spawn(move ||{
            g.wait();
            first_attempt("https://vcp.example.org",&s2,&b_store,"topic_a",WireKind::Message,body())
        });
        gate.wait();
        let one=first_attempt("https://vcp.example.org",&s,&a_store,"topic_a",WireKind::Message,body());
        let two=handle.join().unwrap();
        assert_eq!(usize::from(one.is_ok())+usize::from(two.is_ok()),1);
        assert!(matches!(one,Err(FlowError::ReconcileFirst)) || matches!(two,Err(FlowError::ReconcileFirst)));
        assert_eq!(a_store.list().unwrap().len(),1);
    }
    #[test] fn restart_cannot_implicitly_send_twice() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("restart.sqlite");
        let s=session('b');let first=PendingStore::open(&path).unwrap();
        first_attempt("https://vcp.example.org",&s,&first,"topic_a",WireKind::Message,body()).unwrap();
        drop(first);
        let next=PendingStore::open(&path).unwrap();
        assert!(matches!(first_attempt("https://vcp.example.org",&s,&next,"topic_a",WireKind::Message,body()),Err(FlowError::ReconcileFirst)));
    }
}
