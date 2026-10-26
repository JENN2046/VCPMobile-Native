//! Minimal real read-only HTTPS transport, not a connected mobile app.
//! Platform must supply server-issued Cookie from Keychain/Keystore separately.
//! Does not pair, POST, grant task authority or contact any server until called.
use crate::portal_contract::PortalSession;
use crate::submission_flow::HttpsOrigin;
use crate::transport_policy::{parse_session_response, PortalCookie, TransportPolicyError};
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE, COOKIE, SET_COOKIE};
use reqwest::{Client, Method, Request};
use std::fmt;
use std::time::Duration;

const MAX_SESSION_BYTES: usize = 512 * 1024;

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
    #[error("session rejected: {0}")]
    SessionRejected(#[from] TransportPolicyError),
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
        let value = HeaderValue::from_str(&value).map_err(|_| PortalHttpError::CookieDenied)?;
        self.client.request(Method::GET, format!("{}{path}", self.origin.as_str()))
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .header(COOKIE, value)
            .build()
            .map_err(|_| PortalHttpError::Initialization)
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
        assert!(header.starts_with("__Secure-vcp_mobile="));
        assert_eq!(header.len(), "__Secure-vcp_mobile=".len() + 43);
        assert!(!request.headers().contains_key(reqwest::header::AUTHORIZATION));
        assert!(!format!("{client:?}").contains("zzzzzzzz"));
    }
    #[test]
    fn session_parser_is_not_post_acceptance() {
        assert!(matches!(parse_session_response(202, Some("application/json"), b"{}"),
            Err(TransportPolicyError::InvalidResponse)));
        assert!(matches!(parse_session_response(302, None, b""),
            Err(TransportPolicyError::RedirectDenied)));
    }
}
