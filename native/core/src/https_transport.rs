//! Minimal real read-only HTTPS transport, not a connected mobile app.
//! Platform must supply server-issued Cookie from Keychain/Keystore separately.
//! Does not pair, POST, grant task authority or contact any server until called.
use crate::portal_contract::PortalSession;
use crate::submission_flow::HttpsOrigin;
use serde_json::Value;
use crate::transport_policy::{parse_session_response, PortalCookie, TransportPolicyError};
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE, COOKIE, SET_COOKIE};
use reqwest::{Client, Method, Request};
use std::fmt;
use std::time::Duration;

const MAX_SESSION_BYTES: usize = 512 * 1024;
// A single server page contains at most 200 history messages; fail closed
// on larger data rather than truncating it or advancing the cursor.
const MAX_HISTORY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PortalHttpError {
    #[error("invalid pinned HTTPS origin")]
    InvalidOrigin,
    #[error("cannot initialize HTTPS client")]
    Initialization,
    #[error("session cookie denied for this origin or route")]
    CookieDenied,
    #[error("HTTPS request timed out")]
    Timeout,
    #[error("TLS or HTTPS request failed")]
    Network,
    #[error("unexpected response URL")]
    ResponseOrigin,
    #[error("server attempted an unexpected session Cookie change")]
    UnexpectedCookieChange,
    #[error("response body exceeds bound")]
    ResponseTooLarge,
    #[error("history request denied or response unverified")]
    InvalidHistory,
    #[error("session rejected: {0}")]
    SessionRejected(#[from] TransportPolicyError),
}


/// Tagged, raw read-only history. It must not be treated as a persisted or
/// applied NativeSync fact until the separate history contract is validated.
#[derive(Clone)]
pub struct HistorySnapshot {
    pub entry_id: String,
    pub session_marker: String,
    pub topic_key: String,
    pub after: u64,
    pub document: Value,
}
impl fmt::Debug for HistorySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HistorySnapshot")
            .field("entry_id", &"[REDACTED]")
            .field("session_marker", &"[REDACTED]")
            .field("topic_key", &self.topic_key)
            .field("after", &self.after)
            .field("document", &"[REDACTED]")
            .finish()
    }
}

/// No automatic cookies or app secrets stored in this type.
pub struct PortalHttpClient {
    origin: HttpsOrigin,
    client: Client,
}
impl fmt::Debug for PortalHttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PortalHttpClient").field("origin", &self.origin).finish()
    }
}
impl PortalHttpClient {
    pub fn new(origin: &str) -> Result<Self, PortalHttpError> {
        let origin = HttpsOrigin::parse(origin).map_err(|_| PortalHttpError::InvalidOrigin)?;
        let client = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| PortalHttpError::Initialization)?;
        Ok(Self { origin, client })
    }
    pub fn origin(&self) -> &HttpsOrigin { &self.origin }

    /// Builds a request without network side effects.
    fn session_request(&self, cookie: &PortalCookie) -> Result<Request, PortalHttpError> {
        let path = "/mobile/api/session";
        let value = cookie.header_for(&self.origin, path)
            .map_err(|_| PortalHttpError::CookieDenied)?;
        let mut value = HeaderValue::from_str(&value).map_err(|_| PortalHttpError::CookieDenied)?;
        value.set_sensitive(true);
        self.client.request(Method::GET, format!("{}{path}", self.origin.as_str()))
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(COOKIE, value)
            .build()
            .map_err(|_| PortalHttpError::Initialization)
    }


    /// A synthetic-only request constructor; it rejects unknown Topics, unsafe
    /// cursors and cookies from a different origin before any network action.
    fn history_request(
        &self, cookie: &PortalCookie, session: &PortalSession,
        topic_key: &str, after: u64,
    ) -> Result<Request, PortalHttpError> {
        let plan = crate::submission_flow::history_request(
            self.origin.as_str(), session, topic_key, after,
        ).map_err(|_| PortalHttpError::InvalidHistory)?;
        let value = cookie.header_for(&self.origin, &plan.path_and_query)
            .map_err(|_| PortalHttpError::CookieDenied)?;
        let mut value = HeaderValue::from_str(&value)
            .map_err(|_| PortalHttpError::CookieDenied)?;
        value.set_sensitive(true);
        self.client.request(
            Method::GET, format!("{}{}", self.origin.as_str(), plan.path_and_query),
        )
        .header(ACCEPT, HeaderValue::from_static("application/json"))
        .header(COOKIE, value)
        .build()
        .map_err(|_| PortalHttpError::Initialization)
    }

    /// Fetches the original authorized Topic's read-only history page only.
    /// No new task, execution, model request, or client-side cursor update.
    /// The caller must retain provenance and never apply the opaque JSON
    /// automatically as a validated receipt or user-visible chat message.
    pub async fn fetch_history(
        &self, cookie: &PortalCookie, session: &PortalSession,
        topic_key: &str, after: u64,
    ) -> Result<HistorySnapshot, PortalHttpError> {
        let request = self.history_request(cookie, session, topic_key, after)?;
        let expected_url = request.url().clone();
        let mut response = self.client.execute(request).await.map_err(network_error)?;
        if response.url() != &expected_url {
            return Err(PortalHttpError::ResponseOrigin);
        }
        if response.headers().contains_key(SET_COOKIE) {
            return Err(PortalHttpError::UnexpectedCookieChange);
        }
        if response.status().as_u16() != 200 {
            return Err(PortalHttpError::InvalidHistory);
        }
        let mime_ok = response.headers().get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
        if !mime_ok || response.content_length()
            .is_some_and(|n| n > MAX_HISTORY_BYTES as u64) {
            return Err(PortalHttpError::InvalidHistory);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            if chunk.len() > MAX_HISTORY_BYTES.saturating_sub(body.len()) {
                return Err(PortalHttpError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        let document: Value = serde_json::from_slice(&body)
            .map_err(|_| PortalHttpError::InvalidHistory)?;
        if !document.is_object() {
            return Err(PortalHttpError::InvalidHistory);
        }
        Ok(HistorySnapshot {
            entry_id: session.entry_id.clone(),
            session_marker: session.session_marker.clone(),
            topic_key: topic_key.to_owned(), after, document,
        })
    }

    /// Actual request, but no execution or POST acceptance.
    pub async fn fetch_session(&self, cookie: &PortalCookie) -> Result<PortalSession, PortalHttpError> {
        let request = self.session_request(cookie)?;
        let expected_url = request.url().clone();
        let mut response = self.client.execute(request).await.map_err(network_error)?;
        if response.url() != &expected_url { return Err(PortalHttpError::ResponseOrigin); }
        // Explicit fail-closed until the native secure vault handles rotation.
        if response.headers().contains_key(SET_COOKIE) {
            return Err(PortalHttpError::UnexpectedCookieChange);
        }
        if response.content_length().is_some_and(|n| n > MAX_SESSION_BYTES as u64) {
            return Err(PortalHttpError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let content_type = response.headers().get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()).map(str::to_owned);
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            if chunk.len() > MAX_SESSION_BYTES.saturating_sub(body.len()) {
                return Err(PortalHttpError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        parse_session_response(status, content_type.as_deref(), &body)
            .map_err(PortalHttpError::SessionRejected)
    }
}
fn network_error(e: reqwest::Error) -> PortalHttpError {
    if e.is_timeout() { PortalHttpError::Timeout } else { PortalHttpError::Network }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport_policy::{parse_set_cookie, CookieTransition};
    fn origin() -> HttpsOrigin {
        HttpsOrigin::parse("https://mobile.example.org").unwrap()
    }
    fn cookie() -> PortalCookie {
        let header = format!(
            "__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=3600",
            "z".repeat(43));
        let CookieTransition::Issued(c) = parse_set_cookie(&origin(), &origin(), &header).unwrap()
            else { panic!("test setup did not issue Cookie") };
        c
    }
    #[test]
    fn refuses_downgrade_or_cookie_cross_origin() {
        for raw in ["http://mobile.example.org", "https://mobile.example.org/path",
            "https://user@mobile.example.org", "https://[::1]"] {
            assert!(matches!(PortalHttpClient::new(raw), Err(PortalHttpError::InvalidOrigin)));
        }
        let client = PortalHttpClient::new("https://other.example.org").unwrap();
        assert!(matches!(client.session_request(&cookie()), Err(PortalHttpError::CookieDenied)));
    }
    #[test]
    fn builds_exact_read_only_request_with_scoped_cookie() {
        let client = PortalHttpClient::new("https://MOBILE.example.org:443").unwrap();
        let request = client.session_request(&cookie()).unwrap();
        assert_eq!(request.method(), Method::GET);
        assert_eq!(request.url().as_str(), "https://mobile.example.org/mobile/api/session");
        assert_eq!(request.headers().get(ACCEPT).unwrap(), "application/json");
        let header = request.headers().get(COOKIE).unwrap().to_str().unwrap();
        assert!(request.headers().get(COOKIE).unwrap().is_sensitive());
        assert!(header.starts_with("__Secure-vcp_mobile="));
        assert_eq!(header.len(), "__Secure-vcp_mobile=".len() + 43);
        assert!(!request.headers().contains_key(reqwest::header::AUTHORIZATION));
        assert!(!format!("{client:?}").contains("zzzzzzzz"));
    }

    fn authorized_session() -> PortalSession {
        let doc = serde_json::json!({
            "authenticated":true,
            "recovery":{"version":1,"entryId":"a".repeat(64),"sessionMarker":"b".repeat(64)},
            "topics":[{"key":"topic_a"},{"key":"group_1"}],
            "selectedTopic":"topic_a"
        });
        PortalSession::parse(&doc.to_string()).unwrap()
    }
    #[test]
    fn authorized_history_request_stays_get_and_bound_to_topic_and_cookie() {
        let client = PortalHttpClient::new("https://mobile.example.org").unwrap();
        let s = authorized_session();
        let request = client.history_request(&cookie(), &s, "topic_a", 42).unwrap();
        assert_eq!(request.method(), Method::GET);
        assert_eq!(request.url().as_str(),
            "https://mobile.example.org/mobile/api/conversations/topic_a?after=42");
        assert!(request.headers().contains_key(COOKIE));
        assert!(request.headers().get(COOKIE).unwrap().is_sensitive());
        assert!(!request.headers().contains_key(reqwest::header::ORIGIN));
        assert!(request.body().is_none());
        assert!(matches!(client.history_request(&cookie(), &s, "other", 0),
            Err(PortalHttpError::InvalidHistory)));
        assert!(matches!(client.history_request(&cookie(), &s, "topic_a", 9_007_199_254_740_992),
            Err(PortalHttpError::InvalidHistory)));
        let foreign = PortalHttpClient::new("https://wrong.example.org").unwrap();
        assert!(matches!(foreign.history_request(&cookie(), &s, "topic_a", 0),
            Err(PortalHttpError::CookieDenied)));
    }
    #[test]
    fn history_debug_never_exposes_raw_messages_or_session_markers() {
        let s = authorized_session();
        let snapshot = HistorySnapshot {
            entry_id:s.entry_id.clone(),
            session_marker:s.session_marker.clone(),
            topic_key:"topic_a".into(), after:0,
            document:serde_json::json!({"messages":[{"text":"SYNTHETIC_SECRET_MARKER"}]}),
        };
        let output = format!("{snapshot:?}");
        assert!(!output.contains("SYNTHETIC_SECRET_MARKER"));
        assert!(!output.contains(&s.entry_id));
        assert!(!output.contains(&s.session_marker));
        assert!(output.contains("topic_a"));
    }

    #[test]
    fn session_parser_is_not_post_acceptance() {
        assert!(matches!(parse_session_response(202, Some("application/json"), b"{}"),
            Err(TransportPolicyError::InvalidResponse)));
        assert!(matches!(parse_session_response(302, None, b""),
            Err(TransportPolicyError::RedirectDenied)));
    }
}


/// An explicit single POST transport observation. A successful 202 is NOT a
/// durable receipt, an AA completion, or permission to remove pending state.
#[derive(Debug, thiserror::Error)]
pub enum SubmissionTransportError {
    #[error("claimed submission origin does not match pinned HTTPS entry")]
    PlanOrigin,
    #[error("session Cookie is not valid for this entry and route")]
    CookieScope,
    #[error("invalid guarded submission request")]
    Request,
    #[error("request timed out; preserve original pending state")]
    Timeout,
    #[error("transport failure; preserve original pending state")]
    Network,
    #[error("unexpected response URL or session Cookie mutation")]
    Response,
    #[error("bounded submission response exceeded size limit")]
    OversizedResponse,
}

impl PortalHttpClient {
    /// Pure builder. Do not expose this as a generic multi-use POST API.
    fn submission_request(
        &self,
        cookie: &PortalCookie,
        plan: &crate::submission_flow::OutboundSubmission,
    ) -> Result<Request, SubmissionTransportError> {
        if plan.origin() != &self.origin {
            return Err(SubmissionTransportError::PlanOrigin);
        }
        let cookie_header = cookie.header_for(&self.origin, plan.path())
            .map_err(|_| SubmissionTransportError::CookieScope)?;
        let mut cookie_header = HeaderValue::from_str(&cookie_header)
            .map_err(|_| SubmissionTransportError::CookieScope)?;
        cookie_header.set_sensitive(true);
        let origin_header = HeaderValue::from_str(plan.origin_header())
            .map_err(|_| SubmissionTransportError::Request)?;
        let mut builder = self.client.request(
            Method::POST,
            format!("{}{}", self.origin.as_str(), plan.path()),
        ).header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header(reqwest::header::ORIGIN, origin_header)
            .header(COOKIE, cookie_header);
        for (name, value) in plan.guard_headers() {
            builder = builder.header(name, HeaderValue::from_str(value)
                .map_err(|_| SubmissionTransportError::Request)?);
        }
        builder.body(plan.original_body().to_owned()).build()
            .map_err(|_| SubmissionTransportError::Request)
    }

    /// Send exactly once, consuming an atomically claimed, non-Clone intent.
    /// No retries, no pending cleanup, no new model calls, no task controls.
    /// The caller MUST have sourced the Cookie and session from authenticated
    /// platform custody; neither mobile app is wired to this method yet.
    pub async fn submit_claimed_once(
        &self,
        cookie: &PortalCookie,
        plan: crate::submission_flow::OutboundSubmission,
    ) -> Result<crate::transport_policy::SubmissionHttpOutcome, SubmissionTransportError> {
        let request = self.submission_request(cookie, &plan)?;
        let expected = request.url().clone();
        let mut response = self.client.execute(request).await.map_err(|e| {
            if e.is_timeout() { SubmissionTransportError::Timeout }
            else { SubmissionTransportError::Network }
        })?;
        if response.url() != &expected || response.headers().contains_key(SET_COOKIE) {
            return Err(SubmissionTransportError::Response);
        }
        const LIMIT: usize = 128 * 1024;
        if response.content_length().is_some_and(|n| n > LIMIT as u64) {
            return Err(SubmissionTransportError::OversizedResponse);
        }
        let status = response.status().as_u16();
        let content_type = response.headers().get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()).map(str::to_owned);
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| {
            if e.is_timeout() { SubmissionTransportError::Timeout }
            else { SubmissionTransportError::Network }
        })? {
            if chunk.len() > LIMIT.saturating_sub(body.len()) {
                return Err(SubmissionTransportError::OversizedResponse);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(crate::transport_policy::classify_submission_response(
            status, content_type.as_deref(), &body,
        ))
    }
}

#[cfg(test)]
mod submission_transport_tests {
    use super::*;
    use crate::pending::PendingStore;
    use crate::recovery::SubmissionKind;
    use crate::transport_policy::{parse_set_cookie, CookieTransition};
    fn session() -> PortalSession {
        PortalSession::parse(&serde_json::json!({
            "authenticated":true,
            "recovery":{"version":1,"entryId":"a".repeat(64),
                "sessionMarker":"b".repeat(64)},
            "topics":[{"key":"topic_a"}],"selectedTopic":"topic_a"
        }).to_string()).unwrap()
    }
    fn cookie() -> PortalCookie {
        let origin = HttpsOrigin::parse("https://mobile.example.org").unwrap();
        let raw = format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=3600",
            "z".repeat(43));
        let CookieTransition::Issued(value) = parse_set_cookie(&origin,&origin,&raw).unwrap()
            else { panic!("synthetic Cookie not issued") };
        value
    }
    #[test]
    fn post_intent_is_exact_guarded_and_persisted_before_build() {
        let client = PortalHttpClient::new("https://mobile.example.org").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = PendingStore::open(&dir.path().join("isolated.db")).unwrap();
        let s = session();
        let raw = r#"{ "requestId":"test_123", "text":"合成 🧭" }"#;
        let intent = crate::submission_flow::first_attempt(
            client.origin().as_str(), &s, &store, "topic_a",
            SubmissionKind::Message, raw,
        ).unwrap();
        let req = client.submission_request(&cookie(), &intent).unwrap();
        assert_eq!(req.method(),Method::POST);
        assert_eq!(req.url().as_str(),
            "https://mobile.example.org/mobile/api/conversations/topic_a/messages");
        assert_eq!(req.headers().get(reqwest::header::ORIGIN).unwrap(),
            "https://mobile.example.org");
        assert_eq!(req.headers().get("X-NativeSync-Entry-Id").unwrap(),
            s.entry_id.as_str());
        assert_eq!(req.headers().get("X-NativeSync-Session-Marker").unwrap(),
            s.session_marker.as_str());
        assert!(req.headers().contains_key(COOKIE));
        assert!(req.headers().get(COOKIE).unwrap().is_sensitive());
        assert!(!format!("{req:?}").contains(&"z".repeat(43)));
        assert_eq!(req.headers().get(CONTENT_TYPE).unwrap(),"application/json");
        assert_eq!(req.body().and_then(reqwest::Body::as_bytes),Some(raw.as_bytes()));
        assert!(!format!("{intent:?}").contains("合成"));
        assert!(matches!(
            crate::submission_flow::first_attempt(client.origin().as_str(),
                &s, &store, "topic_a", SubmissionKind::Message,raw),
            Err(crate::submission_flow::FlowError::ReconcileFirst)
        ));
        assert_eq!(store.list().unwrap().len(),1);
    }
    #[test]
    fn foreign_origin_cannot_build_or_delete_a_claimed_intent() {
        let client = PortalHttpClient::new("https://elsewhere.example.org").unwrap();
        let s = session();
        let dir = tempfile::tempdir().unwrap();
        let store = PendingStore::open(&dir.path().join("isolated.db")).unwrap();
        let plan = crate::submission_flow::first_attempt(
            "https://mobile.example.org",&s,&store,"topic_a",
            SubmissionKind::Message,r#"{"requestId":"test_123","text":"synthetic"}"#,
        ).unwrap();
        assert!(matches!(client.submission_request(&cookie(),&plan),
            Err(SubmissionTransportError::PlanOrigin)));
        assert_eq!(store.list().unwrap().len(),1);
    }
}
