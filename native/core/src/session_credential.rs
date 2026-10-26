//! Opaque, expiring credential bridge for the native platform vaults.
//! The exported secret bytes are ONLY for Keychain / Keystore-backed storage.
//! Construct from a verified same-origin TLS response, never from chat/model input.
use crate::https_transport::{PortalHttpClient, PortalHttpError};
use crate::history::NativeHistoryPage;
use crate::native_recovery::{NativePendingInfo, NativePendingJournal, NativeRecoveryCheck, NativeRecoveryError, NativeRecoveryFence, NativeRecoveryOutcome};
use crate::portal_contract::PortalSession;
use crate::submission_flow::HttpsOrigin;
use crate::transport_policy::{parse_set_cookie, CookieTransition, PortalCookie, TransportPolicyError};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_STORED_BYTES: usize = 2048;
const MAX_AGE_SECONDS: u64 = 720 * 3600;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum SessionCredentialError {
    #[error("invalid HTTPS origin")] InvalidOrigin,
    #[error("untrusted credential origin")] OriginMismatch,
    #[error("invalid or insecure Cookie")] InvalidCookie,
    #[error("server revoked this Cookie")] Revoked,
    #[error("session expired; authenticate again")] Expired,
    #[error("system clock precedes credential issuance")] ClockInvalid,
    #[error("invalid secure credential record")] InvalidStoredData,
    #[error("server requires authentication")] AuthenticationRequired,
    #[error("server denied this session")] Forbidden,
    #[error("HTTPS request timed out")] Timeout,
    #[error("HTTPS session request failed")] RequestFailed,
    #[error("session context changed; refresh before reading history")] ContextChanged,
    #[error("history page rejected; cursor unchanged")] HistoryRejected,
}

/// Non-credential session metadata; never a server authority or execution receipt.
#[derive(Clone, uniffi::Record)]
pub struct NativeSessionInfo {
    pub entry_id: String,
    pub session_marker: String,
    pub topic_keys: Vec<String>,
    pub selected_topic: Option<String>,
}

#[derive(uniffi::Object)]
pub struct SessionCredential {
    cookie: PortalCookie,
    issued_at_ms: u64,
    expires_at_ms: u64,
}
impl fmt::Debug for SessionCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionCredential")
            .field("secret", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms).finish()
    }
}

/// Canonical origin-scoped key, not server entryId or access credential.
#[uniffi::export]
pub fn credential_storage_key(origin: String) -> Result<String, SessionCredentialError> {
    let origin = parsed_origin(&origin)?;
    let mut hash = Sha256::new();
    hash.update(b"VCPMobile.SessionVault.v1\0");
    hash.update(origin.as_str().as_bytes());
    Ok(format!("{:x}", hash.finalize()))
}

#[uniffi::export(async_runtime = "tokio")]
impl SessionCredential {
    /// Fresh authenticated query only. Expiry and current server context are
    /// rechecked; the original request marker is used only for receipt matching.
    pub async fn query_pending_recovery(&self, journal: Arc<NativePendingJournal>,
        pending: NativePendingInfo, fence: Arc<NativeRecoveryFence>,
    ) -> Result<Arc<NativeRecoveryCheck>, NativeRecoveryError> {
        self.recovery_valid()?;
        fence.ensure_active()?;
        let client = PortalHttpClient::new(self.cookie.origin().as_str()).map_err(|_| NativeRecoveryError::QueryFailed)?;
        self.recovery_query_with_client(journal, pending, fence, &client).await
    }

    /// Host MUST reload the current credential and hold its vault lock/actor
    /// continuously through this call. The view fence is checked atomically
    /// with receipt consumption, independently of the host's scheduling.
    pub fn confirm_pending_recovery(&self, journal: Arc<NativePendingJournal>, check: Arc<NativeRecoveryCheck>,
        expected: NativeSessionInfo, topic_key: String, view_generation: u64,
    ) -> Result<NativeRecoveryOutcome, NativeRecoveryError> {
        self.recovery_valid()?;
        if self.cookie.origin() != &journal.origin { return Err(NativeRecoveryError::CredentialRejected); }
        check.consume(&journal, &self.recovery_fingerprint(), &expected, &topic_key, view_generation)
    }

    /// Only pass Set-Cookie from a caller-verified same-origin TLS response.
    /// Parsing a protocol shape does not authenticate or pair a device.
    #[uniffi::constructor]
    pub fn from_server_cookie(
        origin: String, response_origin: String, set_cookie: String,
    ) -> Result<Arc<Self>, SessionCredentialError> {
        Self::issue_at(&origin, &response_origin, &set_cookie, now_ms()?).map(Arc::new)
    }

    /// Only platform-protected bytes may enter here. No TTL extension.
    #[uniffi::constructor]
    pub fn restore(
        origin: String, secret_bytes: Vec<u8>,
    ) -> Result<Arc<Self>, SessionCredentialError> {
        Self::restore_at(&origin, &secret_bytes, now_ms()?).map(Arc::new)
    }

    pub fn storage_key(&self) -> Result<String, SessionCredentialError> {
        credential_storage_key(self.cookie.origin().as_str().to_owned())
    }
    pub fn origin(&self) -> String { self.cookie.origin().as_str().to_owned() }
    pub fn expires_at_ms(&self) -> u64 { self.expires_at_ms }

    /// Sensitive: hosts immediately encrypt/store these bytes and release copies.
    /// Never log, display, sync, or save them in a chat/history database.
    pub fn secret_bytes_for_vault(&self) -> Result<Vec<u8>, SessionCredentialError> {
        self.ensure_valid_at(now_ms()?)?;
        Ok(self.encode())
    }

    /// Calls the pinned read-only HTTPS client. No POST, model, or task.
    /// Native host controls invocation and owns the protected vault.
    pub async fn fetch_session_info(&self) -> Result<NativeSessionInfo, SessionCredentialError> {
        self.ensure_valid_at(now_ms()?)?;
        let client = PortalHttpClient::new(self.cookie.origin().as_str())
            .map_err(|_| SessionCredentialError::RequestFailed)?;
        let session = client.fetch_session(&self.cookie).await.map_err(|error| match error {
            PortalHttpError::Timeout => SessionCredentialError::Timeout,
            PortalHttpError::SessionRejected(TransportPolicyError::AuthenticationRequired) =>
                SessionCredentialError::AuthenticationRequired,
            PortalHttpError::SessionRejected(TransportPolicyError::Forbidden) =>
                SessionCredentialError::Forbidden,
            _ => SessionCredentialError::RequestFailed,
        })?;
        self.ensure_valid_at(now_ms()?)?;
        Ok(NativeSessionInfo {
            entry_id: session.entry_id, session_marker: session.session_marker,
            topic_keys: session.topic_keys, selected_topic: session.selected_topic,
        })
    }
    /// Read one page after rechecking the actual authenticated session. No
    /// POST or automatic pagination; caller applies a page only if its local
    /// view/credential epoch still matches. Pending cleanup is separate.
    pub async fn fetch_history_page(
        &self, expected: NativeSessionInfo, topic_key: String, after: u64,
    ) -> Result<NativeHistoryPage, SessionCredentialError> {
        self.ensure_valid_at(now_ms()?)?;
        let prior = history_preflight(self.cookie.origin().as_str(), &expected, &topic_key, after)?;
        let client = PortalHttpClient::new(self.cookie.origin().as_str())
            .map_err(|_| SessionCredentialError::RequestFailed)?;
        let current = client.fetch_session(&self.cookie).await.map_err(history_http_error)?;
        if current.entry_id != prior.entry_id || current.session_marker != prior.session_marker {
            return Err(SessionCredentialError::ContextChanged);
        }
        if !current.allows(&topic_key) { return Err(SessionCredentialError::Forbidden); }
        self.ensure_valid_at(now_ms()?)?;
        let snapshot = client.fetch_history(&self.cookie, &current, &topic_key, after)
            .await.map_err(history_http_error)?;
        self.ensure_valid_at(now_ms()?)?;
        NativeHistoryPage::parse(&snapshot, &current).map_err(|_| SessionCredentialError::HistoryRejected)
    }
}

fn history_preflight(origin: &str, expected: &NativeSessionInfo, topic_key: &str, after: u64)
    -> Result<PortalSession, SessionCredentialError>
{
    let document = json!({"authenticated":true,
        "recovery":{"version":1,"entryId":expected.entry_id,"sessionMarker":expected.session_marker},
        "topics":expected.topic_keys.iter().map(|key| json!({"key":key})).collect::<Vec<_>>(),
        "selectedTopic":expected.selected_topic});
    let session = PortalSession::parse(&document.to_string()).map_err(|_| SessionCredentialError::ContextChanged)?;
    crate::submission_flow::history_request(origin, &session, topic_key, after)
        .map_err(|_| SessionCredentialError::HistoryRejected)?;
    Ok(session)
}
fn history_http_error(error: PortalHttpError) -> SessionCredentialError {
    match error {
        PortalHttpError::Timeout => SessionCredentialError::Timeout,
        PortalHttpError::SessionRejected(TransportPolicyError::AuthenticationRequired) => SessionCredentialError::AuthenticationRequired,
        PortalHttpError::SessionRejected(TransportPolicyError::Forbidden) => SessionCredentialError::Forbidden,
        _ => SessionCredentialError::HistoryRejected,
    }
}

fn parsed_origin(raw: &str) -> Result<HttpsOrigin, SessionCredentialError> {
    HttpsOrigin::parse(raw).map_err(|_| SessionCredentialError::InvalidOrigin)
}
pub(crate) fn now_ms() -> Result<u64, SessionCredentialError> {
    let value = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|_| SessionCredentialError::ClockInvalid)?.as_millis();
    u64::try_from(value).map_err(|_| SessionCredentialError::ClockInvalid)
}
impl SessionCredential {
    fn recovery_fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"VCPMobile.RecoveryCredential.v1\0");
        hash.update(self.cookie.origin().as_str().as_bytes());
        hash.update([0]);
        hash.update(self.cookie.secret_for_secure_store().as_bytes());
        hash.update(self.issued_at_ms.to_le_bytes());
        hash.update(self.expires_at_ms.to_le_bytes());
        format!("{:x}", hash.finalize())
    }
    fn recovery_valid(&self) -> Result<(), NativeRecoveryError> {
        self.ensure_valid_at(now_ms().map_err(|_| NativeRecoveryError::Expired)?)
            .map_err(|e| match e { SessionCredentialError::Expired | SessionCredentialError::ClockInvalid => NativeRecoveryError::Expired, _ => NativeRecoveryError::CredentialRejected })
    }
    pub(crate) async fn recovery_query_with_client(&self, journal: Arc<NativePendingJournal>,
        pending: NativePendingInfo, fence: Arc<NativeRecoveryFence>, client: &PortalHttpClient,
    ) -> Result<Arc<NativeRecoveryCheck>, NativeRecoveryError> {
        self.recovery_valid()?;
        fence.ensure_active()?;
        if self.cookie.origin() != &journal.origin || client.origin() != &journal.origin { return Err(NativeRecoveryError::CredentialRejected); }
        if pending.topic_key != fence.topic_key || pending.entry_id != fence.expected.entry_id { return Err(NativeRecoveryError::ContextChanged); }
        let stored = journal.store.get(&pending.key()).map_err(|_| NativeRecoveryError::StorageRejected)?
            .ok_or(NativeRecoveryError::StorageRejected)?;
        if stored.submission.topic_id != pending.topic_key { return Err(NativeRecoveryError::ContextChanged); }
        let ticket = client.query_pending_receipt_expected(&self.cookie, &journal.store, &pending.key(), &fence.expected)
            .await.map_err(crate::native_recovery::query_error)?;
        self.recovery_valid()?;
        fence.ensure_active()?;
        Ok(NativeRecoveryCheck::new(journal, fence, self.recovery_fingerprint(), ticket))
    }

    /// Internal TLS adapter entry; conservative expiry begins before the POST.
    pub(crate) fn from_pairing_response(origin: &str, header: &str, started: u64)
        -> Result<Arc<Self>, SessionCredentialError>
    {
        let credential = Self::issue_at(origin, origin, header, started)?;
        credential.ensure_valid_at(now_ms()?)?;
        Ok(Arc::new(credential))
    }
    fn issue_at(origin: &str, response: &str, header: &str, now: u64)
        -> Result<Self, SessionCredentialError>
    {
        let origin = parsed_origin(origin)?;
        let response = parsed_origin(response)?;
        let transition = parse_set_cookie(&origin, &response, header).map_err(|error| match error {
            TransportPolicyError::OriginMismatch => SessionCredentialError::OriginMismatch,
            _ => SessionCredentialError::InvalidCookie,
        })?;
        let CookieTransition::Issued(cookie) = transition else {
            return Err(SessionCredentialError::Revoked);
        };
        let expires_at_ms = now.checked_add(u64::from(cookie.max_age_seconds()) * 1000)
            .ok_or(SessionCredentialError::ClockInvalid)?;
        Ok(Self { cookie, issued_at_ms: now, expires_at_ms })
    }
    fn ensure_valid_at(&self, now: u64) -> Result<(), SessionCredentialError> {
        if now < self.issued_at_ms { return Err(SessionCredentialError::ClockInvalid); }
        if now >= self.expires_at_ms { return Err(SessionCredentialError::Expired); }
        Ok(())
    }
    fn encode(&self) -> Vec<u8> {
        json!({"version":1, "origin":self.cookie.origin().as_str(),
            "token":self.cookie.secret_for_secure_store(),
            "maxAgeSeconds":self.cookie.max_age_seconds(),
            "issuedAtMs":self.issued_at_ms, "expiresAtMs":self.expires_at_ms})
            .to_string().into_bytes()
    }
    fn restore_at(origin: &str, data: &[u8], now: u64) -> Result<Self, SessionCredentialError> {
        let origin = parsed_origin(origin)?;
        if data.is_empty() || data.len() > MAX_STORED_BYTES {
            return Err(SessionCredentialError::InvalidStoredData);
        }
        let value: Value = serde_json::from_slice(data)
            .map_err(|_| SessionCredentialError::InvalidStoredData)?;
        let obj = value.as_object().filter(|o| o.len() == 6)
            .ok_or(SessionCredentialError::InvalidStoredData)?;
        if obj.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(SessionCredentialError::InvalidStoredData);
        }
        let stored_origin = obj.get("origin").and_then(Value::as_str)
            .ok_or(SessionCredentialError::InvalidStoredData)?;
        if stored_origin != origin.as_str() { return Err(SessionCredentialError::OriginMismatch); }
        let token = obj.get("token").and_then(Value::as_str)
            .filter(|v| v.len() == 43 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
            .ok_or(SessionCredentialError::InvalidStoredData)?;
        let age = obj.get("maxAgeSeconds").and_then(Value::as_u64)
            .filter(|v| (1..=MAX_AGE_SECONDS).contains(v)).ok_or(SessionCredentialError::InvalidStoredData)?;
        let issued = obj.get("issuedAtMs").and_then(Value::as_u64)
            .ok_or(SessionCredentialError::InvalidStoredData)?;
        let expires = obj.get("expiresAtMs").and_then(Value::as_u64)
            .ok_or(SessionCredentialError::InvalidStoredData)?;
        if issued.checked_add(age * 1000) != Some(expires) {
            return Err(SessionCredentialError::InvalidStoredData);
        }
        let header = format!("__Secure-vcp_mobile={token}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age={age}");
        let result = Self::issue_at(origin.as_str(), origin.as_str(), &header, issued)?;
        result.ensure_valid_at(now)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORIGIN: &str = "https://vault.example.test";
    fn header() -> String {
        format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=60", "A".repeat(43))
    }
    fn credential() -> SessionCredential { SessionCredential::issue_at(ORIGIN, ORIGIN, &header(), 10_000).unwrap() }
    #[test] fn roundtrip_does_not_extend_ttl() {
        let c = credential(); let bytes = c.encode();
        let restored = SessionCredential::restore_at(ORIGIN, &bytes, 60_000).unwrap();
        assert_eq!(restored.expires_at_ms, 70_000);
        assert_eq!(restored.encode(), bytes);
    }
    #[test] fn expiry_boundary_and_clock_rollback_are_rejected() {
        let c = credential();
        assert!(c.ensure_valid_at(69_999).is_ok());
        assert_eq!(c.ensure_valid_at(70_000), Err(SessionCredentialError::Expired));
        assert_eq!(c.ensure_valid_at(9_999), Err(SessionCredentialError::ClockInvalid));
        assert!(matches!(SessionCredential::restore_at(ORIGIN, &c.encode(), 70_000), Err(SessionCredentialError::Expired)));
    }
    #[test] fn canonical_scope_includes_nondefault_port() {
        assert_eq!(credential_storage_key("https://VAULT.example.test:443".into()).unwrap(), credential_storage_key(ORIGIN.into()).unwrap());
        assert_ne!(credential_storage_key("https://vault.example.test:8443".into()).unwrap(), credential_storage_key(ORIGIN.into()).unwrap());
        assert!(matches!(SessionCredential::restore_at("https://other.example.test", &credential().encode(), 20_000), Err(SessionCredentialError::OriginMismatch)));
        assert!(credential_storage_key("http://vault.example.test".into()).is_err());
    }
    #[test] fn secure_store_is_strictly_versioned_bounded_and_typed() {
        let good: Value = serde_json::from_slice(&credential().encode()).unwrap();
        for (field, bad) in [("version", json!(2)), ("maxAgeSeconds", json!(0)),
            ("maxAgeSeconds", json!(2_592_001)), ("issuedAtMs", json!(-1)),
            ("expiresAtMs", json!(70_001)), ("token", json!("bad\r\nheader")),
            ("extra", json!(true))] {
            let mut altered = good.clone(); altered[field] = bad;
            assert!(SessionCredential::restore_at(ORIGIN, &serde_json::to_vec(&altered).unwrap(), 20_000).is_err());
        }
        for data in [Vec::new(), b"broken".to_vec(), vec![b' '; MAX_STORED_BYTES + 1]] {
            assert!(SessionCredential::restore_at(ORIGIN, &data, 20_000).is_err());
        }
    }
    #[test] fn revoked_or_cross_origin_response_is_not_persistable() {
        let revoke="__Secure-vcp_mobile=; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=0";
        assert!(matches!(SessionCredential::issue_at(ORIGIN, ORIGIN, revoke, 0), Err(SessionCredentialError::Revoked)));
        assert!(matches!(SessionCredential::issue_at(ORIGIN, "https://other.example.test", &header(), 0), Err(SessionCredentialError::OriginMismatch)));
        assert!(SessionCredential::issue_at(ORIGIN, ORIGIN, &header().replace("HttpOnly; ", ""), 0).is_err());
    }
    #[test] fn errors_and_debug_never_include_secret() {
        let c=credential();
        assert!(!format!("{c:?}").contains(&"A".repeat(43)));
        let e=SessionCredential::restore_at(ORIGIN, b"secret that is not JSON", 0).unwrap_err();
        assert!(!format!("{e:?} {e}").contains("secret that is"));
    }
    #[tokio::test] async fn expired_credential_cannot_start_http() {
        let c=SessionCredential::issue_at(ORIGIN, ORIGIN, &header(), 0).unwrap();
        assert!(matches!(c.fetch_session_info().await, Err(SessionCredentialError::Expired)));
    }
}

#[cfg(test)]
mod history_bridge_tests {
    use super::*;
    fn info() -> NativeSessionInfo {
        NativeSessionInfo { entry_id:"a".repeat(64), session_marker:"b".repeat(64), topic_keys:vec!["topic_a".into()],selected_topic:None }
    }
    #[test] fn history_preflight_rejects_invalid_input_before_http() {
        let origin="https://history.example.test";let mut s=info();
        assert!(history_preflight(origin,&s,"topic_a",0).is_ok());
        assert!(history_preflight(origin,&s,"other",0).is_err());
        assert!(history_preflight(origin,&s,"topic_a",u64::MAX).is_err());
        s.session_marker="bad".into();
        assert!(matches!(history_preflight(origin,&s,"topic_a",0),Err(SessionCredentialError::ContextChanged)));
    }
    #[tokio::test] async fn expired_credential_cannot_read_history() {
        let origin="https://history.example.test";
        let header=format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=1","A".repeat(43));
        let c=SessionCredential::issue_at(origin,origin,&header,0).unwrap();
        assert!(matches!(c.fetch_history_page(info(),"topic_a".into(),0).await,Err(SessionCredentialError::Expired)));
    }
}
