use super::*;
use crate::https_transport::SubmissionTransportError;
use crate::pending::PendingState;
use crate::portal_contract::PortalSession;
use crate::recovery::{ReviewReason, SubmissionKind};
use crate::submission_flow::{first_attempt, HttpsOrigin};
use crate::transport_policy::{parse_set_cookie, CookieTransition, TransportPolicyError};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const BODY: &str = r#"{ "requestId":"lost_ack_123", "text":"synthetic ACK loss 🧭" }"#;

struct Server {
    child: Child,
    counts: PathBuf,
    origin: String,
    port: u16,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Server {
    async fn start(root: &Path, mode: &str) -> Self {
        let ready = root.join(format!("{mode}.ready"));
        let counts = root.join(format!("{mode}.count"));
        let child = Command::new("python3")
            .current_dir(root)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/recovery-https.py"))
            .arg(mode)
            .arg(&ready)
            .arg(&counts)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("python3 required for synthetic TLS recovery contract");
        let mut server = Self {
            child,
            counts,
            origin: String::new(),
            port: 0,
        };
        for _ in 0..500 {
            if ready.exists() {
                break;
            }
            assert!(
                server.child.try_wait().unwrap().is_none(),
                "fixture stopped: {mode}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        server.port = fs::read_to_string(ready)
            .expect("bounded TLS fixture startup")
            .parse()
            .unwrap();
        server.origin = format!("https://recovery.example.test:{}", server.port);
        server
    }
    fn counts(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(&self.counts).unwrap()).unwrap()
    }
}
fn certificate(root: &Path) -> reqwest::Certificate {
    fs::write(root.join("cert.cnf"), "[req]\ndistinguished_name=dn\nprompt=no\n[dn]\nCN=recovery.example.test\n[ca_ext]\nbasicConstraints=critical,CA:true\nkeyUsage=critical,keyCertSign,cRLSign\n[leaf_ext]\nbasicConstraints=critical,CA:false\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:recovery.example.test\n").unwrap();
    for args in [
        vec![
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "2",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
            "-subj",
            "/CN=Synthetic VCP recovery CA",
            "-config",
            "cert.cnf",
            "-extensions",
            "ca_ext",
        ],
        vec![
            "req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", "leaf.key", "-out",
            "leaf.csr", "-config", "cert.cnf",
        ],
        vec![
            "x509",
            "-req",
            "-in",
            "leaf.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-days",
            "2",
            "-out",
            "leaf.pem",
            "-extfile",
            "cert.cnf",
            "-extensions",
            "leaf_ext",
        ],
    ] {
        assert!(Command::new("openssl")
            .current_dir(root)
            .args(args)
            .output()
            .expect("openssl required for synthetic TLS recovery contract")
            .status
            .success());
    }
    reqwest::Certificate::from_pem(&fs::read(root.join("ca.pem")).unwrap()).unwrap()
}
fn original_session() -> PortalSession {
    PortalSession::parse(
        &json!({"authenticated":true,
        "recovery":{"version":1,"entryId":"a".repeat(64),"sessionMarker":"b".repeat(64)},
        "topics":[{"key":"topic_a"}],"selectedTopic":"topic_a"})
        .to_string(),
    )
    .unwrap()
}
fn synthetic_cookie(origin: &str) -> PortalCookie {
    let origin = HttpsOrigin::parse(origin).unwrap();
    let header = format!(
        "__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600",
        "B".repeat(43)
    );
    let CookieTransition::Issued(cookie) = parse_set_cookie(&origin, &origin, &header).unwrap()
    else {
        panic!("synthetic Cookie not issued")
    };
    cookie
}

// Real TLS, synthetic service/Cookie, no model or actual NativeSync backend.
#[tokio::test]
async fn ack_loss_reopen_and_authenticated_receipt_query_never_repeat_post() {
    let dir = tempfile::tempdir().unwrap();
    let ca = certificate(dir.path());
    for mode in [
        "exact",
        "cross_exact",
        "absent",
        "cross_absent",
        "conflict",
        "duplicate",
        "metadata",
        "wrong_topic",
        "malformed_page",
        "entry_changed",
        "topic_revoked",
        "unauthenticated",
        "forbidden",
        "redirect",
        "cookie_rotation",
        "history_unauthenticated",
        "history_forbidden",
        "history_redirect",
        "drop_history",
        "bad_content_type",
        "paged",
        "repeated",
        "discarded",
        "stale_original",
    ] {
        let server = Server::start(dir.path(), mode).await;
        let client = PortalHttpClient::loopback_for_test(&server.origin, ca.clone(), server.port);
        let cookie = synthetic_cookie(&server.origin);
        let path = dir.path().join(format!("{mode}.sqlite"));
        let store = PendingStore::open(&path).unwrap();
        let plan = first_attempt(
            &server.origin,
            &original_session(),
            &store,
            "topic_a",
            SubmissionKind::Message,
            BODY,
        )
        .unwrap();
        let original = store.list().unwrap().remove(0).submission;
        assert!(
            matches!(
                client.submit_claimed_once(&cookie, plan).await,
                Err(SubmissionTransportError::Network)
            ),
            "ACK must be lost: {mode}"
        );
        drop(store);

        // Reopen the actual SQLite file, not an in-memory stand-in.
        let reopened = PendingStore::open(&path).unwrap();
        let row = reopened.get(&original.key).unwrap().unwrap();
        assert_eq!(row.state, PendingState::AwaitingConfirmation, "{mode}");
        assert_eq!(row.submission.payload_json, BODY, "{mode}");
        assert!(
            matches!(
                first_attempt(
                    &server.origin,
                    &original_session(),
                    &reopened,
                    "topic_a",
                    SubmissionKind::Message,
                    BODY
                ),
                Err(FlowError::ReconcileFirst)
            ),
            "{mode}"
        );

        let result = client
            .query_pending_receipt(&cookie, &reopened, &original.key)
            .await;
        // Even a positive query cannot clear before the host's epoch check.
        assert_eq!(
            reopened.list().unwrap().len(),
            1,
            "query mutated journal: {mode}"
        );
        let expected = match mode {
            "exact" | "cross_exact" | "paged" | "repeated" | "discarded" | "stale_original" => {
                Some(Reconciliation::Accepted {
                    execution_id: "exec_synthetic".into(),
                    execution_state: "running".into(),
                })
            }
            "absent" => Some(Reconciliation::SameSessionNoReceipt),
            "cross_absent" => Some(Reconciliation::Review(ReviewReason::Session)),
            "conflict" => Some(Reconciliation::Review(ReviewReason::Conflict)),
            "duplicate" => Some(Reconciliation::Review(ReviewReason::Ambiguous)),
            "metadata" => Some(Reconciliation::Review(ReviewReason::Metadata)),
            "wrong_topic" => Some(Reconciliation::Review(ReviewReason::Scope)),
            _ => None,
        };
        let mut confirmed = false;
        if let Some(expected) = expected {
            let check = result.unwrap_or_else(|e| panic!("{mode}: {e}"));
            assert_eq!(check.outcome(), &expected, "{mode}");
            assert!(!format!("{check:?}").contains("exec_synthetic"));
            assert!(!format!("{check:?}").contains("lost_ack_123"));
            if mode == "repeated" {
                drop(check);
                let fresh = client
                    .query_pending_receipt(&cookie, &reopened, &original.key)
                    .await
                    .unwrap();
                assert_eq!(reopened.list().unwrap().len(), 1);
                assert_eq!(fresh.commit(&reopened).unwrap(), expected);
                confirmed = true;
            } else if mode == "discarded" {
                drop(check); // Credential removal / view change: discard late reply.
            } else if mode == "stale_original" {
                assert!(reopened.confirm_verified(&original).unwrap());
                let mut changed = original.clone();
                changed.payload_json = BODY.replace("ACK loss", "different payload");
                reopened.prepare(&changed).unwrap();
                assert!(matches!(
                    check.commit(&reopened),
                    Err(ReceiptQueryError::Store(StoreError::IdentityConflict))
                ));
                assert_eq!(
                    reopened.get(&changed.key).unwrap().unwrap().submission,
                    changed
                );
            } else {
                confirmed = matches!(expected, Reconciliation::Accepted { .. });
                assert_eq!(check.commit(&reopened).unwrap(), expected, "{mode}");
            }
        } else {
            let error = result.unwrap_err();
            let valid = match mode {
                "entry_changed" => {
                    matches!(error, ReceiptQueryError::Flow(FlowError::EntryChanged))
                }
                "topic_revoked" => {
                    matches!(error, ReceiptQueryError::Flow(FlowError::TopicNotAllowed))
                }
                "malformed_page" => matches!(
                    error,
                    ReceiptQueryError::History(HistoryError::InvalidCursor)
                ),
                "unauthenticated" | "history_unauthenticated" => matches!(
                    error,
                    ReceiptQueryError::Http(PortalHttpError::SessionRejected(
                        TransportPolicyError::AuthenticationRequired
                    ))
                ),
                "forbidden" | "history_forbidden" => matches!(
                    error,
                    ReceiptQueryError::Http(PortalHttpError::SessionRejected(
                        TransportPolicyError::Forbidden
                    ))
                ),
                "redirect" => matches!(
                    error,
                    ReceiptQueryError::Http(PortalHttpError::SessionRejected(
                        TransportPolicyError::RedirectDenied
                    ))
                ),
                "cookie_rotation" => matches!(
                    error,
                    ReceiptQueryError::Http(PortalHttpError::UnexpectedCookieChange)
                ),
                "history_redirect" | "bad_content_type" => matches!(
                    error,
                    ReceiptQueryError::Http(PortalHttpError::InvalidHistory)
                ),
                "drop_history" => {
                    matches!(error, ReceiptQueryError::Http(PortalHttpError::Network))
                }
                _ => false,
            };
            assert!(valid, "unexpected failure for {mode}: {error}");
        }
        assert_eq!(
            reopened.list().unwrap().len(),
            usize::from(!confirmed),
            "{mode}"
        );
        let skip_history = matches!(
            mode,
            "entry_changed"
                | "topic_revoked"
                | "unauthenticated"
                | "forbidden"
                | "redirect"
                | "cookie_rotation"
        );
        let reads = if mode == "repeated" { 2 } else { 1 };
        assert_eq!(
            server.counts(),
            json!({"post":1,"session":reads,
            "history":if skip_history { 0 } else { reads },"other":0}),
            "no resend, redirect, pagination, or task calls: {mode}"
        );
        println!("synthetic TLS ACK-loss/reopen scenario passed: {mode}");
    }
}

#[tokio::test]
async fn missing_original_fails_before_network() {
    let dir = tempfile::tempdir().unwrap();
    let client = PortalHttpClient::new("https://recovery.example.test").unwrap();
    let store = PendingStore::open(&dir.path().join("empty.sqlite")).unwrap();
    let key = RequestKey {
        endpoint_id: "a".repeat(64),
        session_marker: "b".repeat(64),
        request_id: "lost_ack_123".into(),
    };
    assert!(matches!(
        client
            .query_pending_receipt(&synthetic_cookie(client.origin().as_str()), &store, &key)
            .await,
        Err(ReceiptQueryError::Store(StoreError::NotFound))
    ));
}
