//! Source-grounded HTTP boundary; no network or credential persistence.
//! Native hosts must pin exact HTTPS origin, verify system TLS, disable
//! redirects and store credentials in Keychain/Keystore, never PendingStore.
use crate::portal_contract::PortalSession;
use crate::submission_flow::HttpsOrigin;
use serde_json::Value;
use std::fmt;

const MAX_SESSION_BYTES: usize = 512 * 1024;
const MAX_ACK_BYTES: usize = 128 * 1024;
const MAX_SESSION_SECONDS: u32 = 720 * 3600;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TransportPolicyError {
    #[error("untrusted response origin")] OriginMismatch,
    #[error("invalid or insecure session cookie")] InvalidCookie,
    #[error("cookie scope or target path denied")] CookieScopeDenied,
    #[error("HTTP redirect forbidden")] RedirectDenied,
    #[error("authentication required")] AuthenticationRequired,
    #[error("request forbidden")] Forbidden,
    #[error("invalid or oversized HTTP response")] InvalidResponse,
    #[error("session response lacks authorization")] InvalidSession,
}

/// Credentials remain volatile in Rust. Platform security storage is required
/// before actual use; Debug never exposes token or generated headers.
pub struct PortalCookie {
    origin: HttpsOrigin,
    token: String,
    max_age_seconds: u32,
}
impl fmt::Debug for PortalCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PortalCookie")
            .field("origin", &self.origin)
            .field("token", &"[REDACTED]")
            .field("max_age_seconds", &self.max_age_seconds).finish()
    }
}
#[derive(Debug)]
pub enum CookieTransition { Issued(PortalCookie), Revoked }

fn token_valid(token: &str) -> bool {
    token.len() == 43 && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Verified TLS response URL is supplied independently from the server body.
pub fn parse_set_cookie(
    pinned_origin: &HttpsOrigin, response_origin: &HttpsOrigin, header: &str,
) -> Result<CookieTransition, TransportPolicyError> {
    if response_origin != pinned_origin { return Err(TransportPolicyError::OriginMismatch); }
    if header.is_empty() || header.len() > 512 ||
        header.bytes().any(|b| b.is_ascii_control() || b >= 127)
    { return Err(TransportPolicyError::InvalidCookie); }
    let mut parts = header.split(';');
    let first = parts.next().ok_or(TransportPolicyError::InvalidCookie)?;
    let (name, raw_token) = first.trim().split_once('=').ok_or(TransportPolicyError::InvalidCookie)?;
    if name != "__Secure-vcp_mobile" { return Err(TransportPolicyError::InvalidCookie); }
    let token = raw_token.trim();
    let mut path = false;
    let mut http_only = false;
    let mut secure = false;
    let mut strict = false;
    let mut max_age = None;
    for part in parts {
        let part = part.trim();
        let (key, value) = match part.split_once('=') {
            Some((k, v)) => (k.trim(), Some(v.trim())),
            None => (part, None),
        };
        match key.to_ascii_lowercase().as_str() {
            "path" if !path && value == Some("/mobile") => path = true,
            "httponly" if !http_only && value.is_none() => http_only = true,
            "secure" if !secure && value.is_none() => secure = true,
            "samesite" if !strict && value.is_some_and(|v| v.eq_ignore_ascii_case("Strict")) => strict = true,
            "max-age" if max_age.is_none() => {
                let raw = value.ok_or(TransportPolicyError::InvalidCookie)?;
                if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(TransportPolicyError::InvalidCookie);
                }
                let seconds = raw.parse::<u32>().map_err(|_| TransportPolicyError::InvalidCookie)?;
                if seconds > MAX_SESSION_SECONDS { return Err(TransportPolicyError::InvalidCookie); }
                max_age = Some(seconds);
            }
            _ => return Err(TransportPolicyError::InvalidCookie),
        }
    }
    if !(path && http_only && secure && strict) {
        return Err(TransportPolicyError::InvalidCookie);
    }
    let seconds = max_age.ok_or(TransportPolicyError::InvalidCookie)?;
    if seconds == 0 && token.is_empty() { return Ok(CookieTransition::Revoked); }
    if seconds == 0 || !token_valid(token) { return Err(TransportPolicyError::InvalidCookie); }
    Ok(CookieTransition::Issued(PortalCookie {
        origin: pinned_origin.clone(), token: token.to_owned(), max_age_seconds: seconds,
    }))
}

impl PortalCookie {
    pub fn origin(&self) -> &HttpsOrigin { &self.origin }
    pub fn max_age_seconds(&self) -> u32 { self.max_age_seconds }
    /// Only for a platform secure vault. Never log or persist in local chat DB.
    pub fn secret_for_secure_store(&self) -> &str { &self.token }
    /// Generates a single ephemeral request header; sends nothing.
    pub fn header_for(
        &self, target_origin: &HttpsOrigin, path_and_query: &str,
    ) -> Result<String, TransportPolicyError> {
        if target_origin != &self.origin { return Err(TransportPolicyError::OriginMismatch); }
        if path_and_query.len() > 1024 ||
            !path_and_query.starts_with("/mobile/api/") ||
            path_and_query.bytes().any(|b| !(b.is_ascii_alphanumeric() ||
                matches!(b, b'/' | b'_' | b'-' | b'?' | b'=' | b'&' | b'.'))) ||
            path_and_query.contains("//") ||
            path_and_query.split('?').next().unwrap_or("").split('/').any(|seg| seg == "." || seg == "..")
        { return Err(TransportPolicyError::CookieScopeDenied); }
        Ok(format!("__Secure-vcp_mobile={}", self.token))
    }
}

fn is_json(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|v|
        v.split(';').next().is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json")))
}
fn guard_status(status: u16) -> Result<(), TransportPolicyError> {
    match status {
        300..=399 => Err(TransportPolicyError::RedirectDenied),
        401 => Err(TransportPolicyError::AuthenticationRequired),
        403 => Err(TransportPolicyError::Forbidden),
        _ => Ok(()),
    }
}
/// Caller attests real TLS origin and the GET /mobile/api/session route.
pub fn parse_session_response(
    status: u16, content_type: Option<&str>, body: &[u8],
) -> Result<PortalSession, TransportPolicyError> {
    guard_status(status)?;
    if status != 200 || body.len() > MAX_SESSION_BYTES || !is_json(content_type) {
        return Err(TransportPolicyError::InvalidResponse);
    }
    let text = std::str::from_utf8(body).map_err(|_| TransportPolicyError::InvalidResponse)?;
    PortalSession::parse(text).map_err(|_| TransportPolicyError::InvalidSession)
}

/// 202 is not a durable NativeSync execution receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionHttpOutcome {
    AcceptedButUnverified,
    SubmissionContextChanged,
    AuthenticationRequired,
    Forbidden,
    RedirectDenied,
    UnknownKeepPending,
}
pub fn classify_submission_response(
    status: u16, content_type: Option<&str>, body: &[u8],
) -> SubmissionHttpOutcome {
    match status {
        300..=399 => return SubmissionHttpOutcome::RedirectDenied,
        401 => return SubmissionHttpOutcome::AuthenticationRequired,
        403 => return SubmissionHttpOutcome::Forbidden,
        _ => {}
    }
    if body.len() > MAX_ACK_BYTES || !is_json(content_type) {
        return SubmissionHttpOutcome::UnknownKeepPending;
    }
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return SubmissionHttpOutcome::UnknownKeepPending;
    };
    if status == 409 &&
        value.pointer("/error/code").and_then(Value::as_str) == Some("SUBMISSION_CONTEXT_CHANGED")
    { return SubmissionHttpOutcome::SubmissionContextChanged; }
    if status == 202 && value.is_object() {
        return SubmissionHttpOutcome::AcceptedButUnverified;
    }
    SubmissionHttpOutcome::UnknownKeepPending
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn origin() -> HttpsOrigin { HttpsOrigin::parse("https://native.example.org").unwrap() }
    fn token() -> String { "A".repeat(43) }
    fn issued() -> String {
        format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=28800",token())
    }
    #[test]
    fn issued_cookie_is_scoped_and_debug_redacted() {
        let CookieTransition::Issued(c) = parse_set_cookie(&origin(),&origin(),&issued()).unwrap() else { panic!(); };
        assert_eq!(c.max_age_seconds(),28800);
        assert_eq!(c.secret_for_secure_store(),token());
        assert_eq!(c.header_for(&origin(),"/mobile/api/session").unwrap(),format!("__Secure-vcp_mobile={}",token()));
        assert!(!format!("{c:?}").contains(&token()));
        assert!(c.header_for(&origin(),"/mobile/api/conversations/topic_1?after=100").is_ok());
    }
    #[test]
    fn rejects_cross_origin_and_unsafe_cookie_paths() {
        let CookieTransition::Issued(c) = parse_set_cookie(&origin(),&origin(),&issued()).unwrap() else { panic!(); };
        let other=HttpsOrigin::parse("https://other.example.org").unwrap();
        assert!(matches!(c.header_for(&other,"/mobile/api/session"),Err(TransportPolicyError::OriginMismatch)));
        for p in ["/api/session","/mobile/api2/session","/mobile/private","//mobile/api/session",
            "/mobile/api/../admin","/mobile/api//session","/mobile/api/session#frag",
            "/mobile/api/%2e%2e/admin","/mobile/api/session\r\nInjection: 1"] {
            assert!(matches!(c.header_for(&origin(),p),Err(TransportPolicyError::CookieScopeDenied)),"{p}");
        }
    }
    #[test]
    fn refuses_weak_or_malformed_cookie_attributes() {
        let good=issued();
        for bad in [
            good.replace("HttpOnly; ",""),
            good.replace("Secure; ",""),
            good.replace("SameSite=Strict","SameSite=Lax"),
            good.replace("Path=/mobile","Path=/"),
            good.replace("Max-Age=28800","Max-Age=2592001"),
            good.replace("Max-Age=28800","Max-Age=-1"),
            good.replace("Max-Age=28800","Max-Age=0"),
            good.replace("__Secure-vcp_mobile","other"),
            good.replace("Max-Age=28800","Domain=example.org; Max-Age=28800"),
            good.replace("Secure; ","Secure; Secure; "),
            good.replace("Max-Age=28800","Max-Age=abc"),
            good.replace(&token(),"token; Injected=1"),
            format!("{good}\r\nX-Injection: yes"),
        ] {
            assert!(matches!(parse_set_cookie(&origin(),&origin(),&bad),Err(TransportPolicyError::InvalidCookie)));
        }
        let other=HttpsOrigin::parse("https://other.example.org").unwrap();
        assert!(matches!(parse_set_cookie(&origin(),&other,&good),Err(TransportPolicyError::OriginMismatch)));
    }
    #[test]
    fn revoked_cookie_cannot_be_mistaken_for_issued() {
        let revoke="__Secure-vcp_mobile=; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=0";
        assert!(matches!(parse_set_cookie(&origin(),&origin(),revoke),Ok(CookieTransition::Revoked)));
        assert!(parse_set_cookie(&origin(),&origin(),&revoke.replace("Max-Age=0","Max-Age=1")).is_err());
    }
    #[test]
    fn only_authenticated_json_session_is_valid() {
        let body=json!({"authenticated":true,"recovery":{"version":1,
            "entryId":"a".repeat(64),"sessionMarker":"b".repeat(64)},
            "topics":[{"key":"topic_a"}],"selectedTopic":"topic_a"}).to_string();
        let s=parse_session_response(200,Some("application/json; charset=utf-8"),body.as_bytes()).unwrap();
        assert!(s.allows("topic_a"));
        assert!(matches!(parse_session_response(302,Some("application/json"),body.as_bytes()),Err(TransportPolicyError::RedirectDenied)));
        assert!(matches!(parse_session_response(401,Some("application/json"),body.as_bytes()),Err(TransportPolicyError::AuthenticationRequired)));
        assert!(matches!(parse_session_response(403,Some("application/json"),body.as_bytes()),Err(TransportPolicyError::Forbidden)));
        assert!(matches!(parse_session_response(200,Some("text/html"),body.as_bytes()),Err(TransportPolicyError::InvalidResponse)));
        assert!(matches!(parse_session_response(200,Some("application/json"),b"{}"),Err(TransportPolicyError::InvalidSession)));
        assert!(matches!(parse_session_response(200,Some("application/json"),&vec![0;MAX_SESSION_BYTES+1]),Err(TransportPolicyError::InvalidResponse)));
    }
    #[test]
    fn ack_status_never_equates_to_verified_receipt() {
        use SubmissionHttpOutcome::*;
        assert_eq!(classify_submission_response(202,Some("application/json"),br#"{"id":"execution_1"}"#),AcceptedButUnverified);
        assert_eq!(classify_submission_response(409,Some("application/json"),br#"{"error":{"code":"SUBMISSION_CONTEXT_CHANGED"}}"#),SubmissionContextChanged);
        assert_eq!(classify_submission_response(409,Some("application/json"),br#"{"error":{"code":"CONFLICT"}}"#),UnknownKeepPending);
        assert_eq!(classify_submission_response(202,Some("text/html"),b"<html>ok</html>"),UnknownKeepPending);
        assert_eq!(classify_submission_response(202,Some("application/json"),b"{}"),AcceptedButUnverified);
        assert_eq!(classify_submission_response(500,Some("application/json"),br#"{"error":{"code":"FAIL"}}"#),UnknownKeepPending);
        assert_eq!(classify_submission_response(302,None,b""),RedirectDenied);
        assert_eq!(classify_submission_response(401,None,b""),AuthenticationRequired);
        assert_eq!(classify_submission_response(403,None,b""),Forbidden);
    }
}