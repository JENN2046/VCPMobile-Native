//! Explicit, one-use NativeSync /mobile/api/pair exchange.
//! Never retries, follows redirects, uses an existing Cookie, or sends a model request.
use crate::session_credential::{now_ms, SessionCredential};
use crate::submission_flow::HttpsOrigin;
use reqwest::header::{HeaderMap, ACCEPT, CONTENT_TYPE, ORIGIN, SET_COOKIE};
use reqwest::{Client, Request};
use serde_json::{json, Value};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_PAIR_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum PairingError {
    #[error("invalid HTTPS origin")] InvalidOrigin,
    #[error("pairing code must be exactly 32 URL-safe ASCII characters")] InvalidCode,
    #[error("pairing attempt already consumed; do not replay")] AlreadyAttempted,
    #[error("cannot initialize HTTPS pairing")] Initialization,
    #[error("pairing rejected or expired; request a new code")] Rejected,
    #[error("pairing forbidden by server")] Forbidden,
    #[error("pairing rate limited; no automatic retry")] RateLimited,
    #[error("redirect refused")] RedirectDenied,
    #[error("pairing outcome unknown; code may have been consumed")] OutcomeUnknown,
    #[error("unverified pairing response")] InvalidResponse,
    #[error("invalid or expired issued credential")] InvalidCredential,
}

/// One object represents one explicit user action, not an automatic retry queue.
#[derive(uniffi::Object)]
pub struct PairingAttempt {
    origin: HttpsOrigin,
    code: Mutex<Option<String>>,
}
impl fmt::Debug for PairingAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairingAttempt").field("code", &"[REDACTED]").finish()
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl PairingAttempt {
    #[uniffi::constructor]
    pub fn new(origin: String, code: String) -> Result<Arc<Self>, PairingError> {
        let origin = HttpsOrigin::parse(&origin).map_err(|_| PairingError::InvalidOrigin)?;
        if code.len() != 32 || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
            return Err(PairingError::InvalidCode);
        }
        Ok(Arc::new(Self { origin, code: Mutex::new(Some(code)) }))
    }

    pub fn origin(&self) -> String { self.origin.as_str().to_owned() }

    /// Returns an opaque credential for immediate PLATFORM secure storage.
    /// Save before fetching session metadata: a failed later GET must not lose it
    /// or cause another POST with the already-used pairing code.
    pub async fn pair(&self) -> Result<Arc<SessionCredential>, PairingError> {
        let client = pairing_client().build().map_err(|_| PairingError::Initialization)?;
        self.exchange(&client).await
    }
}

fn pairing_client() -> reqwest::ClientBuilder {
    Client::builder().https_only(true).no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(8)).timeout(Duration::from_secs(15))
}

impl PairingAttempt {
    fn request(&self, client: &Client) -> Result<Request, PairingError> {
        let code = self.code.lock().map_err(|_| PairingError::AlreadyAttempted)?
            .take().ok_or(PairingError::AlreadyAttempted)?;
        client.post(format!("{}/mobile/api/pair", self.origin.as_str()))
            .header(ORIGIN, self.origin.as_str())
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .body(json!({"code":code}).to_string())
            .build().map_err(|_| PairingError::Initialization)
    }

    async fn exchange(&self, client: &Client) -> Result<Arc<SessionCredential>, PairingError> {
        // Start time is deliberately conservative: network time never extends TTL.
        let started = now_ms().map_err(|_| PairingError::InvalidCredential)?;
        let request = self.request(client)?;
        let expected_url = request.url().clone();
        let mut response = client.execute(request).await.map_err(|_| PairingError::OutcomeUnknown)?;
        if response.url() != &expected_url { return Err(PairingError::InvalidResponse); }
        check_status(response.status().as_u16())?;
        if response.content_length().is_some_and(|n| n > MAX_PAIR_BYTES as u64) {
            return Err(PairingError::InvalidResponse);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| PairingError::OutcomeUnknown)? {
            if chunk.len() > MAX_PAIR_BYTES.saturating_sub(body.len()) {
                return Err(PairingError::InvalidResponse);
            }
            body.extend_from_slice(&chunk);
        }
        parse_response(self.origin.as_str(), response.status().as_u16(), response.headers(), &body, started)
    }
}

fn check_status(status: u16) -> Result<(), PairingError> {
    match status {
        200 => Ok(()),
        300..=399 => Err(PairingError::RedirectDenied),
        401 => Err(PairingError::Rejected),
        403 => Err(PairingError::Forbidden),
        429 => Err(PairingError::RateLimited),
        _ => Err(PairingError::OutcomeUnknown),
    }
}

fn parse_response(origin: &str, status: u16, headers: &HeaderMap, body: &[u8], started: u64)
    -> Result<Arc<SessionCredential>, PairingError>
{
    check_status(status)?;
    let json_mime = headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next()).is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json_mime || body.len() > MAX_PAIR_BYTES { return Err(PairingError::InvalidResponse); }
    let value: Value = serde_json::from_slice(body).map_err(|_| PairingError::InvalidResponse)?;
    if value.get("authenticated").and_then(Value::as_bool) != Some(true) {
        return Err(PairingError::InvalidResponse);
    }
    // The exact source issues one Cookie. Ambiguous/coalesced headers are refused.
    let mut values = headers.get_all(SET_COOKIE).iter();
    let header = values.next().ok_or(PairingError::InvalidCredential)?;
    if values.next().is_some() { return Err(PairingError::InvalidCredential); }
    let header = header.to_str().map_err(|_| PairingError::InvalidCredential)?;
    SessionCredential::from_pairing_response(origin, header, started)
        .map_err(|_| PairingError::InvalidCredential)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderValue, AUTHORIZATION, COOKIE};
    const ORIGIN_URL: &str = "https://pairing.example.test";
    fn attempt() -> Arc<PairingAttempt> { PairingAttempt::new(ORIGIN_URL.into(), "a".repeat(32)).unwrap() }
    fn headers() -> HeaderMap {
        let mut h = HeaderMap::new(); h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json; charset=utf-8"));
        h.insert(SET_COOKIE, HeaderValue::from_str(&format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600", "B".repeat(43))).unwrap()); h
    }
    #[test] fn exact_code_and_https_validation_before_network() {
        for code in ["".to_owned(), "a".repeat(31), "a".repeat(33), "文".repeat(32), " ".repeat(32), "a".repeat(31)+"\n"] {
            assert!(matches!(PairingAttempt::new(ORIGIN_URL.into(),code),Err(PairingError::InvalidCode)));
        }
        for origin in ["http://pairing.example.test", "https://user@pairing.example.test", "https://pairing.example.test/mobile", "https://pairing.example.test?code=x"] {
            assert!(matches!(PairingAttempt::new(origin.into(),"a".repeat(32)),Err(PairingError::InvalidOrigin)));
        }
        assert!(PairingAttempt::new(ORIGIN_URL.into(),"_-".repeat(16)).is_ok());
    }
    #[test] fn exact_request_has_no_old_cookie_or_authorization() {
        let a=attempt(); let client=pairing_client().build().unwrap(); let req=a.request(&client).unwrap();
        assert_eq!(req.method(),reqwest::Method::POST);
        assert_eq!(req.url().as_str(),format!("{ORIGIN_URL}/mobile/api/pair"));
        assert_eq!(req.headers()[ORIGIN],ORIGIN_URL);
        assert_eq!(req.headers()[CONTENT_TYPE],"application/json");
        assert!(req.headers().get(COOKIE).is_none()); assert!(req.headers().get(AUTHORIZATION).is_none());
        let body:Value=serde_json::from_slice(req.body().unwrap().as_bytes().unwrap()).unwrap();
        assert_eq!(body,json!({"code":"a".repeat(32)}));
        assert!(matches!(a.request(&client),Err(PairingError::AlreadyAttempted)));
        assert!(!format!("{a:?}").contains(&"a".repeat(32)));
    }
    #[test] fn concurrent_callers_cannot_build_two_posts() {
        let a=attempt();let client=pairing_client().build().unwrap();
        let threads:Vec<_>=(0..8).map(|_|{let a=a.clone();let c=client.clone();std::thread::spawn(move||a.request(&c).is_ok())}).collect();
        let count=threads.into_iter().map(|t|usize::from(t.join().unwrap())).sum::<usize>();assert_eq!(count,1);
    }
    #[test] fn valid_response_keeps_request_start_expiry_and_origin() {
        let started=now_ms().unwrap()-2000;
        let c=parse_response(ORIGIN_URL,200,&headers(),br#"{"authenticated":true}"#,started).unwrap();
        assert_eq!(c.origin(),ORIGIN_URL);assert_eq!(c.expires_at_ms(),started+600000);
        assert!(!format!("{c:?}").contains(&"B".repeat(43)));
    }
    #[test] fn no_success_from_redirect_denial_or_server_errors() {
        for (status,error) in [(302,PairingError::RedirectDenied),(401,PairingError::Rejected),(403,PairingError::Forbidden),(429,PairingError::RateLimited),(500,PairingError::OutcomeUnknown),(202,PairingError::OutcomeUnknown)] {
            assert_eq!(parse_response(ORIGIN_URL,status,&headers(),br#"{"authenticated":true}"#,now_ms().unwrap()).unwrap_err(),error);
        }
    }
    #[test] fn malformed_oversized_or_false_response_is_rejected() {
        for b in [b"{}".to_vec(),b"{\"authenticated\":false}".to_vec(),b"{\"authenticated\":\"true\"}".to_vec(),vec![b' ';MAX_PAIR_BYTES+1]] {
            assert!(parse_response(ORIGIN_URL,200,&headers(),&b,now_ms().unwrap()).is_err());
        }
        let mut h=headers();h.insert(CONTENT_TYPE,HeaderValue::from_static("text/html"));
        assert!(parse_response(ORIGIN_URL,200,&h,br#"{"authenticated":true}"#,now_ms().unwrap()).is_err());
    }
    #[test] fn ambiguous_insecure_and_revoked_cookie_not_saved() {
        let good=headers();let mut missing=good.clone();missing.remove(SET_COOKIE);
        let mut duplicate=good.clone();duplicate.append(SET_COOKIE,good[SET_COOKIE].clone());
        let mut insecure=good.clone();insecure.insert(SET_COOKIE,HeaderValue::from_str(&good[SET_COOKIE].to_str().unwrap().replace("Secure; ","")).unwrap());
        let mut revoked=good.clone();revoked.insert(SET_COOKIE,HeaderValue::from_static("__Secure-vcp_mobile=; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=0"));
        for h in [missing,duplicate,insecure,revoked] {
            assert!(matches!(parse_response(ORIGIN_URL,200,&h,br#"{"authenticated":true}"#,now_ms().unwrap()),Err(PairingError::InvalidCredential)));
        }
    }
}


#[cfg(test)]
mod tls_contract {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::process::{Child, Command, Stdio};

    // This uses real TLS + HTTP, but a synthetic protocol fixture, NOT the VCP service.
    struct Server(Child);
    impl Drop for Server { fn drop(&mut self) { let _=self.0.kill(); let _=self.0.wait(); } }
    fn openssl(dir: &Path, args: &[&str]) {
        let output=Command::new("openssl").current_dir(dir).args(args).output().expect("openssl required for synthetic HTTPS contract");
        assert!(output.status.success(),"synthetic certificate generation failed");
    }
    #[tokio::test]
    async fn real_tls_cookie_exchange_rejects_replay_redirect_and_ambiguous_outcomes() {
        let dir=tempfile::tempdir().unwrap();let root=dir.path();
        fs::write(root.join("cert.cnf"), "[req]\ndistinguished_name=dn\nprompt=no\n[dn]\nCN=pairing.example.test\n[ca_ext]\nbasicConstraints=critical,CA:true\nkeyUsage=critical,keyCertSign,cRLSign\n[leaf_ext]\nbasicConstraints=critical,CA:false\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:pairing.example.test\n").unwrap();
        openssl(root,&["req","-x509","-newkey","rsa:2048","-nodes","-days","2","-keyout","ca.key","-out","ca.pem","-subj","/CN=Synthetic VCP test CA","-config","cert.cnf","-extensions","ca_ext"]);
        openssl(root,&["req","-new","-newkey","rsa:2048","-nodes","-keyout","leaf.key","-out","leaf.csr","-config","cert.cnf"]);
        openssl(root,&["x509","-req","-in","leaf.csr","-CA","ca.pem","-CAkey","ca.key","-CAcreateserial","-days","2","-out","leaf.pem","-extfile","cert.cnf","-extensions","leaf_ext"]);
        fs::write(root.join("server.py"),include_str!("../fixtures/pairing-https.py")).unwrap();
        let ca=reqwest::Certificate::from_pem(&fs::read(root.join("ca.pem")).unwrap()).unwrap();
        for mode in ["success","rejected","redirect","dropped","duplicate"] {
            let ready=root.join(format!("{mode}.ready"));let count=root.join(format!("{mode}.count"));
            let child=Command::new("python3").current_dir(root).arg("server.py").arg(mode).arg(&ready).arg(&count)
                .stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("python3 required for synthetic HTTPS contract");
            let mut server=Server(child);
            for _ in 0..500 {
                if ready.exists() { break; }
                assert!(server.0.try_wait().unwrap().is_none(),"synthetic HTTPS fixture stopped");
                std::thread::sleep(Duration::from_millis(10));
            }
            let port:u16=fs::read_to_string(&ready).expect("bounded fixture startup").parse().unwrap();
            let origin=format!("https://pairing.example.test:{port}");
            let client=pairing_client().add_root_certificate(ca.clone())
                .resolve("pairing.example.test",std::net::SocketAddr::from(([127,0,0,1],port)))
                .build().unwrap();
            let operation=PairingAttempt::new(origin.clone(),"a".repeat(32)).unwrap();
            let result=operation.exchange(&client).await;
            match mode {
                "success" => { let c=result.unwrap();assert_eq!(c.origin(),origin);assert!(c.expires_at_ms()>now_ms().unwrap()); },
                "rejected" => assert_eq!(result.unwrap_err(),PairingError::Rejected),
                "redirect" => assert_eq!(result.unwrap_err(),PairingError::RedirectDenied),
                "dropped" => assert_eq!(result.unwrap_err(),PairingError::OutcomeUnknown),
                "duplicate" => assert_eq!(result.unwrap_err(),PairingError::InvalidCredential),
                _ => unreachable!(),
            }
            assert_eq!(operation.exchange(&client).await.unwrap_err(),PairingError::AlreadyAttempted);
            assert_eq!(fs::read_to_string(count).unwrap(),"1","pairing POST must not repeat");
        }
    }
}
