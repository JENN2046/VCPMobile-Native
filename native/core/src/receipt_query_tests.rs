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

// Low-level receipt/transport fixtures intentionally use a fixed synthetic file.
// This checked crate-private opener is not a production native bypass.
fn test_journal(origin: String, path: String) -> Result<std::sync::Arc<crate::pending_journal::NativePendingJournal>, crate::pending_journal::PendingJournalError> {
    crate::pending_journal::NativePendingJournal::open_private_file(origin, PathBuf::from(path))
}

#[tokio::test]
async fn fenced_native_send_ack_loss_reopen_queries_original_and_never_posts_twice() {
    use crate::native_recovery::{NativeRecoveryFence, NativeRecoveryOutcome};
    use crate::pending_journal::NativeSendError;
    use crate::session_credential::{NativeSessionInfo, SessionCredential};
    let root = tempfile::tempdir().unwrap();
    let ca = certificate(root.path());
    for mode in ["fenced_exact", "fenced_authority_drift"] {
        let server = Server::start(root.path(), mode).await;
        let client = PortalHttpClient::loopback_for_test(&server.origin, ca.clone(), server.port);
        let expected = NativeSessionInfo { entry_id: "a".repeat(64), session_marker: "b".repeat(64),
            topic_keys: vec!["topic_a".into()], selected_topic: None };
        let credential = SessionCredential::from_server_cookie(server.origin.clone(), server.origin.clone(),
            format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600", "B".repeat(43))).unwrap();
        let path = root.path().join(format!("{mode}.sqlite")).to_str().unwrap().to_owned();
        let journal = test_journal(server.origin.clone(), path.clone()).unwrap();
        let fence = NativeRecoveryFence::new(expected.clone(), "topic_a".into(), 4).unwrap();
        let check = credential.message_check_with_client(journal.clone(), expected.clone(), "topic_a".into(),
            "lost_ack_123".into(), "synthetic ACK loss 🧭".into(), fence.clone(), &client).await.unwrap();
        assert!(journal.store.list().unwrap().is_empty(), "preflight cannot create dispatch authority");
        let dispatch = credential.claim_message_send(journal.clone(), check, expected.clone(), "topic_a".into(), 4).unwrap();
        let result = credential.message_dispatch_with_client(dispatch.clone(), &client).await.unwrap();
        assert_eq!(result.disposition, "UNKNOWN_KEEP_PENDING");
        assert!(matches!(credential.message_dispatch_with_client(dispatch, &client).await, Err(NativeSendError::AlreadyConsumed)));
        drop(journal);
        let reopened = test_journal(server.origin.clone(), path).unwrap();
        let original = reopened.pending_for_topic("topic_a".into()).unwrap().remove(0);
        assert_eq!(reopened.store.list().unwrap()[0].state, PendingState::AwaitingConfirmation);
        let query = credential.recovery_query_with_client(reopened.clone(), original, fence, &client).await;
        if mode == "fenced_exact" {
            let ticket = query.unwrap();
            assert_eq!(reopened.store.list().unwrap().len(), 1, "query is read-only until vault consume");
            assert!(matches!(credential.confirm_pending_recovery(reopened.clone(), ticket, expected, "topic_a".into(), 4).unwrap(),
                NativeRecoveryOutcome::Accepted { execution_state, .. } if execution_state == "running"));
            assert!(reopened.store.list().unwrap().is_empty());
        } else {
            assert!(query.is_err(), "second session GET detects drift");
            assert_eq!(reopened.store.list().unwrap().len(), 1);
        }
        assert_eq!(server.counts(), json!({"post":1,"session":3,"history":1,"other":0}));
    }
}

// Public constructors + real SQLite + real isolated TLS, not mocked claims.
#[tokio::test]
async fn directed_two_instances_claim_once_and_reopen_only_consumes_exact_receipt() {
    use crate::native_recovery::{NativeRecoveryFence, NativeRecoveryOutcome};
    use crate::pending_journal::{NativePendingJournal, NativeSendError};
    use crate::session_credential::{NativeSessionInfo, SessionCredential};
    use std::sync::{Arc, Barrier};
    let root = tempfile::tempdir().unwrap();
    let ca = certificate(root.path());
    for mode in ["fenced_race_same_id", "fenced_race_new_ids", "fenced_202", "fenced_dual_before_consume"] {
        let server = Server::start(root.path(), mode).await;
        let client = PortalHttpClient::loopback_for_test(&server.origin, ca.clone(), server.port);
        let scope = root.path().join(mode); fs::create_dir(&scope).unwrap();
        let expected = NativeSessionInfo { entry_id:"a".repeat(64), session_marker:"b".repeat(64),
            topic_keys:vec!["topic_a".into()],selected_topic:None };
        let credential = SessionCredential::from_server_cookie(server.origin.clone(),server.origin.clone(),
            format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600","B".repeat(43))).unwrap();
        let first = NativePendingJournal::open_scoped(server.origin.clone(),scope.to_str().unwrap().into()).unwrap();
        let second = NativePendingJournal::new(server.origin.clone(),scope.to_str().unwrap().into()).unwrap();
        assert_eq!(first.private_path(),second.private_path());
        let mut late_checks=vec![];
        for journal in [first.clone(),second.clone()] {
            let late_fence=NativeRecoveryFence::new(expected.clone(),"topic_a".into(),8).unwrap();
            let late=credential.message_check_with_client(journal.clone(),expected.clone(),"topic_a".into(),
                "lost_ack_123".into(),"synthetic ACK loss 🧭".into(),late_fence,&client).await.unwrap();
            late_checks.push((journal,late));
        }
        let barrier=Arc::new(Barrier::new(2)); let mut threads=vec![];
        for (index,journal) in [first.clone(),second.clone()].into_iter().enumerate() {
            let fence=NativeRecoveryFence::new(expected.clone(),"topic_a".into(),8).unwrap();
            let id=if mode=="fenced_race_new_ids" && index==1 {"other_ack_123"} else {"lost_ack_123"};
            let check=credential.message_check_with_client(journal.clone(),expected.clone(),"topic_a".into(),
                id.into(),"synthetic ACK loss 🧭".into(),fence,&client).await.unwrap();
            let credential=credential.clone();let expected=expected.clone();let barrier=barrier.clone();
            threads.push(std::thread::spawn(move || { barrier.wait();
                credential.claim_message_send(journal,check,expected,"topic_a".into(),8) }));
        }
        let mut winners=vec![];let mut refused=0;
        for thread in threads { match thread.join().unwrap() {
            Ok(dispatch)=>winners.push(dispatch),Err(NativeSendError::OutstandingPending | NativeSendError::ContextChanged)=>refused+=1,
            _=>panic!("unexpected admission outcome"),
        }}
        assert_eq!(refused,1);assert_eq!(winners.len(),1);
        let dispatch=winners.remove(0);
        let result=credential.message_dispatch_with_client(dispatch.clone(),&client).await.unwrap();
        assert_eq!(result.disposition,if mode=="fenced_202" {"HTTP_202_UNVERIFIED"} else {"UNKNOWN_KEEP_PENDING"});
        assert!(matches!(credential.message_dispatch_with_client(dispatch,&client).await,Err(NativeSendError::AlreadyConsumed)));
        drop(first);
        let reopened=NativePendingJournal::new(server.origin.clone(),scope.to_str().unwrap().into()).unwrap();
        assert_eq!(reopened.store.list().unwrap().len(),1);
        assert_eq!(reopened.store.list().unwrap()[0].state,PendingState::AwaitingConfirmation);
        let original=reopened.pending_for_topic("topic_a".into()).unwrap().remove(0);
        let fence=NativeRecoveryFence::new(expected.clone(),"topic_a".into(),9).unwrap();
        let ticket=credential.recovery_query_with_client(reopened.clone(),original,fence,&client).await.unwrap();
        assert_eq!(reopened.store.list().unwrap().len(),1,"GET cannot clear pending");
        if mode=="fenced_dual_before_consume" {
            let legacy=scope.join(crate::session_credential::credential_storage_key(server.origin.clone()).unwrap());
            fs::create_dir(&legacy).unwrap();fs::write(legacy.join("pending.sqlite"),[]).unwrap();
            assert!(matches!(credential.confirm_pending_recovery(reopened.clone(),ticket,expected,"topic_a".into(),9),
                Err(crate::native_recovery::NativeRecoveryError::StorageRejected)));
            assert_eq!(reopened.store.list().unwrap().len(),1,"held receipt cannot bypass dual storage");
        } else {
            assert!(matches!(credential.confirm_pending_recovery(reopened.clone(),ticket,expected.clone(),"topic_a".into(),9).unwrap(),
                NativeRecoveryOutcome::Accepted{execution_state,..} if execution_state=="running"));
            assert!(reopened.store.list().unwrap().is_empty());
            for (journal,late) in late_checks {
                assert!(matches!(credential.claim_message_send(journal,late,expected.clone(),"topic_a".into(),8),
                    Err(NativeSendError::ContextChanged)),"a delayed preflight must not recreate a confirmed original");
            }
            assert!(reopened.store.list().unwrap().is_empty());
        }
        assert_eq!(server.counts(),json!({"post":1,"session":6,"history":1,"other":0}));
    }
}

#[tokio::test]
async fn directed_cancel_after_http_acceptance_is_unknown_and_survives_reopen() {
    use crate::native_recovery::NativeRecoveryFence;
    use crate::pending_journal::NativePendingJournal;
    use crate::session_credential::{NativeSessionInfo,SessionCredential};
    let root=tempfile::tempdir().unwrap();let ca=certificate(root.path());
    let server=Server::start(root.path(),"fenced_slow_ack").await;
    let client=PortalHttpClient::loopback_for_test(&server.origin,ca,server.port);
    let expected=NativeSessionInfo{entry_id:"a".repeat(64),session_marker:"b".repeat(64),topic_keys:vec!["topic_a".into()],selected_topic:None};
    let scope=root.path().join("journal");fs::create_dir(&scope).unwrap();
    let journal=NativePendingJournal::open_scoped(server.origin.clone(),scope.to_str().unwrap().into()).unwrap();
    let credential=SessionCredential::from_server_cookie(server.origin.clone(),server.origin.clone(),
        format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600","B".repeat(43))).unwrap();
    let fence=NativeRecoveryFence::new(expected.clone(),"topic_a".into(),10).unwrap();
    let check=credential.message_check_with_client(journal.clone(),expected.clone(),"topic_a".into(),"lost_ack_123".into(),
        "synthetic ACK loss 🧭".into(),fence.clone(),&client).await.unwrap();
    let dispatch=credential.claim_message_send(journal.clone(),check,expected,"topic_a".into(),10).unwrap();
    let send=credential.message_dispatch_with_client(dispatch,&client);
    let cancel=async {
        let marker=root.path().join("fenced_slow_ack.accepted");
        for _ in 0..400 { if marker.exists() {break} tokio::time::sleep(Duration::from_millis(5)).await; }
        assert!(marker.exists(),"must cancel after fixture accepted POST, not before send");fence.cancel();
    };
    let (result,())=tokio::join!(send,cancel);
    assert_eq!(result.unwrap().disposition,"UNKNOWN_KEEP_PENDING");drop(journal);
    let reopened=NativePendingJournal::new(server.origin.clone(),scope.to_str().unwrap().into()).unwrap();
    assert_eq!(reopened.store.list().unwrap()[0].state,PendingState::AwaitingConfirmation);
    assert_eq!(server.counts(),json!({"post":1,"session":1,"history":0,"other":0}));
}

#[tokio::test]
async fn directed_preflight_in_flight_login_topic_and_cancel_changes_prevent_claim() {
    use crate::native_recovery::NativeRecoveryFence;
    use crate::pending_journal::{NativePendingJournal,NativeSendError};
    use crate::session_credential::{NativeSessionInfo,SessionCredential};
    let root=tempfile::tempdir().unwrap();let ca=certificate(root.path());
    for change in ["login","topic","cancel"] {
        // Separate service files because each fixture must expose its first GET.
        let fixture=root.path().join(change);fs::create_dir(&fixture).unwrap();
        for name in ["leaf.pem","leaf.key"] {fs::copy(root.path().join(name),fixture.join(name)).unwrap();}
        let server=Server::start(&fixture,"fenced_slow_preflight").await;
        let client=PortalHttpClient::loopback_for_test(&server.origin,ca.clone(),server.port);
        let expected=NativeSessionInfo{entry_id:"a".repeat(64),session_marker:"b".repeat(64),topic_keys:vec!["topic_a".into()],selected_topic:None};
        let credential=SessionCredential::from_server_cookie(server.origin.clone(),server.origin.clone(),
            format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600","B".repeat(43))).unwrap();
        let scope=fixture.join("journal");fs::create_dir(&scope).unwrap();
        let journal=NativePendingJournal::open_scoped(server.origin.clone(),scope.to_str().unwrap().into()).unwrap();
        let fence=NativeRecoveryFence::new(expected.clone(),"topic_a".into(),11).unwrap();
        let prepare=credential.message_check_with_client(journal.clone(),expected.clone(),"topic_a".into(),"lost_ack_123".into(),
            "synthetic ACK loss 🧭".into(),fence.clone(),&client);
        let replace=async {
            for _ in 0..400 {if server.counts()["session"]==1 {break} tokio::time::sleep(Duration::from_millis(5)).await;}
            assert_eq!(server.counts()["session"],1,"change while session GET is in flight");
            if change!="login" {fence.cancel();}
            SessionCredential::from_server_cookie(server.origin.clone(),server.origin.clone(),
                format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600","C".repeat(43))).unwrap()
        };
        let (prepared,replacement)=tokio::join!(prepare,replace);
        if change=="login" {
            assert!(matches!(replacement.claim_message_send(journal.clone(),prepared.unwrap(),expected,"topic_a".into(),11),Err(NativeSendError::CredentialMismatch)));
        } else { assert!(matches!(prepared,Err(NativeSendError::Cancelled))); }
        assert!(journal.store.list().unwrap().is_empty());
        assert_eq!(server.counts(),json!({"post":0,"session":1,"history":0,"other":0}));
    }
}

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
            json!({"post":1,"session":reads * if skip_history || matches!(mode,
                "history_unauthenticated" | "history_forbidden" | "history_redirect" | "bad_content_type" | "drop_history") {1} else {2},
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

fn native_info(marker: char) -> crate::session_credential::NativeSessionInfo {
    crate::session_credential::NativeSessionInfo {
        entry_id: "a".repeat(64),
        session_marker: marker.to_string().repeat(64),
        topic_keys: vec!["topic_a".into(), "topic_b".into()],
        selected_topic: Some("topic_b".into()),
    }
}
fn bridge_credential(
    origin: &str,
    age: u32,
    token: char,
) -> std::sync::Arc<crate::session_credential::SessionCredential> {
    crate::session_credential::SessionCredential::from_server_cookie(origin.into(), origin.into(),
        format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age={age}", token.to_string().repeat(43))).unwrap()
}

#[tokio::test]
async fn native_bridge_expiry_context_cancel_and_single_consumption_keep_originals_safe() {
    use crate::native_recovery::*;
    use crate::session_credential::SessionCredential;
    let root = tempfile::tempdir().unwrap();
    let ca = certificate(root.path());
    for mode in [
        "exact",
        "cross_exact",
        "absent",
        "cross_absent",
        "forbidden",
        "topic_revoked",
        "cross_context_changed",
        "cancel_before",
        "cancel_after",
        "stale_generation",
        "stale_topic",
        "stale_session",
        "changed_credential",
        "wrong_journal",
        "expired_before",
        "expired_after",
        "cancel_race",
    ] {
        let server = Server::start(root.path(), mode).await;
        let path = root.path().join(format!("bridge-{mode}.sqlite"));
        let journal =
            test_journal(server.origin.clone(), path.to_str().unwrap().into())
                .unwrap();
        let pending = journal
            .prepare_original(
                native_info('b'),
                "topic_a".into(),
                NativePendingKind::Message,
                BODY.into(),
            )
            .unwrap();
        let client = PortalHttpClient::loopback_for_test(&server.origin, ca.clone(), server.port);
        let plan = first_attempt(
            &server.origin,
            &original_session(),
            &journal.store,
            "topic_a",
            SubmissionKind::Message,
            BODY,
        )
        .unwrap();
        assert!(matches!(
            client
                .submit_claimed_once(&synthetic_cookie(&server.origin), plan)
                .await,
            Err(SubmissionTransportError::Network)
        ));
        let credential = bridge_credential(
            &server.origin,
            if mode.starts_with("expired_") { 2 } else { 600 },
            'B',
        );
        let secret = credential.secret_bytes_for_vault().unwrap();
        drop(credential);
        drop(journal);
        let credential = SessionCredential::restore(server.origin.clone(), secret).unwrap();
        let journal =
            test_journal(server.origin.clone(), path.to_str().unwrap().into())
                .unwrap();
        let expected = native_info(if mode == "cross_exact" || mode == "cross_absent" {
            'c'
        } else {
            'b'
        });
        let fence = NativeRecoveryFence::new(expected.clone(), "topic_a".into(), 7).unwrap();
        if mode == "cancel_before" {
            fence.cancel();
        }
        if mode == "expired_before" {
            tokio::time::sleep(Duration::from_millis(2100)).await;
        }
        let query = credential
            .recovery_query_with_client(journal.clone(), pending, fence.clone(), &client)
            .await;
        assert_eq!(
            journal.pending_for_topic("topic_a".into()).unwrap().len(),
            1,
            "query must not consume: {mode}"
        );
        let mut accepted = false;
        if matches!(
            mode,
            "forbidden"
                | "topic_revoked"
                | "cross_context_changed"
                | "cancel_before"
                | "expired_before"
        ) {
            let error = query.err().expect("bridge query must reject");
            assert_eq!(
                error,
                match mode {
                    "forbidden" | "topic_revoked" => NativeRecoveryError::Forbidden,
                    "cross_context_changed" => NativeRecoveryError::ContextChanged,
                    "cancel_before" => NativeRecoveryError::Cancelled,
                    _ => NativeRecoveryError::Expired,
                },
                "{mode}"
            );
        } else {
            let check = query.unwrap_or_else(|e| panic!("{mode}: {e}"));
            if mode == "cancel_race" {
                let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
                let ready = barrier.clone();
                let c = credential.clone();
                let j = journal.clone();
                let token = check.clone();
                let context = expected.clone();
                let commit = std::thread::spawn(move || {
                    ready.wait();
                    c.confirm_pending_recovery(j, token, context, "topic_a".into(), 7)
                });
                barrier.wait();
                fence.cancel();
                match commit.join().unwrap() {
                    Ok(NativeRecoveryOutcome::Accepted { .. }) => accepted = true,
                    Err(NativeRecoveryError::Cancelled) => {}
                    _ => panic!("cancellation must linearize before or after exact cleanup"),
                }
                let remaining = journal.pending_for_topic("topic_a".into()).unwrap().len();
                assert_eq!(remaining, usize::from(!accepted));
                assert_eq!(
                    credential
                        .confirm_pending_recovery(
                            journal.clone(),
                            check,
                            expected,
                            "topic_a".into(),
                            7
                        )
                        .unwrap_err(),
                    NativeRecoveryError::Cancelled
                );
                assert_eq!(
                    journal.pending_for_topic("topic_a".into()).unwrap().len(),
                    remaining,
                    "no cleanup after cancel returns"
                );
                assert_eq!(
                    server.counts(),
                    json!({"post":1,"session":2,"history":1,"other":0})
                );
                continue;
            }
            if mode == "cancel_after" {
                fence.cancel();
            }
            if mode == "expired_after" {
                tokio::time::sleep(Duration::from_millis(2100)).await;
            }
            let mut current = expected.clone();
            if mode == "stale_session" {
                current.session_marker = "e".repeat(64);
            }
            let topic = if mode == "stale_topic" {
                "topic_b"
            } else {
                "topic_a"
            };
            let generation = if mode == "stale_generation" { 8 } else { 7 };
            let replacement = if mode == "changed_credential" {
                bridge_credential(&server.origin, 600, 'D')
            } else {
                credential.clone()
            };
            let target_journal = if mode == "wrong_journal" {
                test_journal(
                    server.origin.clone(),
                    root.path().join("other.sqlite").to_str().unwrap().into(),
                )
                .unwrap()
            } else {
                journal.clone()
            };
            let result = replacement.confirm_pending_recovery(
                target_journal,
                check.clone(),
                current,
                topic.into(),
                generation,
            );
            match mode {
                "exact" | "cross_exact" => {
                    assert_eq!(
                        result.unwrap(),
                        NativeRecoveryOutcome::Accepted {
                            execution_id: "exec_synthetic".into(),
                            execution_state: "running".into()
                        }
                    );
                    accepted = true;
                    assert_eq!(
                        credential
                            .confirm_pending_recovery(
                                journal.clone(),
                                check,
                                expected,
                                "topic_a".into(),
                                7
                            )
                            .unwrap_err(),
                        NativeRecoveryError::AlreadyConsumed
                    );
                }
                "absent" => {
                    assert_eq!(result.unwrap(), NativeRecoveryOutcome::SameSessionNoReceipt)
                }
                "cross_absent" => assert_eq!(
                    result.unwrap(),
                    NativeRecoveryOutcome::Review {
                        reason: "session".into()
                    }
                ),
                _ => assert_eq!(
                    result.unwrap_err(),
                    match mode {
                        "cancel_after" => NativeRecoveryError::Cancelled,
                        "expired_after" => NativeRecoveryError::Expired,
                        "changed_credential" | "wrong_journal" =>
                            NativeRecoveryError::CredentialRejected,
                        _ => NativeRecoveryError::ContextChanged,
                    },
                    "{mode}"
                ),
            }
        }
        assert_eq!(
            journal.pending_for_topic("topic_a".into()).unwrap().len(),
            usize::from(!accepted),
            "{mode}"
        );
        let no_network = matches!(mode, "cancel_before" | "expired_before");
        let no_history = no_network
            || matches!(
                mode,
                "forbidden" | "topic_revoked" | "cross_context_changed"
            );
        assert_eq!(
            server.counts(),
            json!({"post":1,"session":usize::from(!no_network) + usize::from(!no_history),"history":usize::from(!no_history),"other":0}),
            "{mode}"
        );
        println!("native recovery bridge scenario passed: {mode}");
    }
}

/// The subprocess exits without destructors AFTER the server accepts the POST
/// and drops its ACK. The parent reopens the same real SQLite/WAL files and
/// uses the exported native recovery types; POST count remains exactly one.
#[test]
fn native_bridge_abrupt_process_reopen_after_lost_ack_does_not_reexecute() {
    use crate::native_recovery::*;
    if let Ok(root) = std::env::var("VCP_RECOVERY_CHILD_ROOT") {
        let port: u16 = std::env::var("VCP_RECOVERY_CHILD_PORT")
            .unwrap()
            .parse()
            .unwrap();
        let origin = format!("https://recovery.example.test:{port}");
        let ca =
            reqwest::Certificate::from_pem(&fs::read(Path::new(&root).join("ca.pem")).unwrap())
                .unwrap();
        let journal = test_journal(
            origin.clone(),
            Path::new(&root)
                .join("process.sqlite")
                .to_str()
                .unwrap()
                .into(),
        )
        .unwrap();
        journal
            .prepare_original(
                native_info('b'),
                "topic_a".into(),
                NativePendingKind::Message,
                BODY.into(),
            )
            .unwrap();
        let client = PortalHttpClient::loopback_for_test(&origin, ca, port);
        let plan = first_attempt(
            &origin,
            &original_session(),
            &journal.store,
            "topic_a",
            SubmissionKind::Message,
            BODY,
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert!(matches!(
            runtime.block_on(client.submit_claimed_once(&synthetic_cookie(&origin), plan)),
            Err(SubmissionTransportError::Network)
        ));
        std::process::exit(0);
    }
    let root = tempfile::tempdir().unwrap();
    let ca = certificate(root.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(Server::start(root.path(), "exact"));
    let output = Command::new(std::env::current_exe().unwrap()).args(["--exact","receipt_query::tests::native_bridge_abrupt_process_reopen_after_lost_ack_does_not_reexecute","--nocapture"])
        .env("VCP_RECOVERY_CHILD_ROOT",root.path()).env("VCP_RECOVERY_CHILD_PORT",server.port.to_string()).output().unwrap();
    assert!(output.status.success(), "synthetic child dispatch failed");
    let journal = test_journal(
        server.origin.clone(),
        root.path().join("process.sqlite").to_str().unwrap().into(),
    )
    .unwrap();
    let pending = journal
        .pending_for_topic("topic_a".into())
        .unwrap()
        .remove(0);
    assert_eq!(
        journal.store.get(&pending.key()).unwrap().unwrap().state,
        PendingState::AwaitingConfirmation
    );
    let credential = bridge_credential(&server.origin, 600, 'B');
    let client = PortalHttpClient::loopback_for_test(&server.origin, ca, server.port);
    let fence = NativeRecoveryFence::new(native_info('b'), "topic_a".into(), 9).unwrap();
    let check = runtime
        .block_on(credential.recovery_query_with_client(journal.clone(), pending, fence, &client))
        .unwrap();
    assert!(matches!(
        credential
            .confirm_pending_recovery(
                journal.clone(),
                check,
                native_info('b'),
                "topic_a".into(),
                9
            )
            .unwrap(),
        NativeRecoveryOutcome::Accepted { .. }
    ));
    assert!(journal
        .pending_for_topic("topic_a".into())
        .unwrap()
        .is_empty());
    assert_eq!(
        server.counts(),
        json!({"post":1,"session":2,"history":1,"other":0})
    );
}
