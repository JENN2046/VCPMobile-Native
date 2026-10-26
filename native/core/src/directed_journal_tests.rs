use super::*;
use crate::portal_contract::PortalSession;
use crate::native_recovery::NativePendingKind;
use crate::recovery::{RecoverySubmission, SubmissionKind as WireKind};
use crate::submission_flow::first_attempt;
use std::path::Path;

const ORIGIN: &str = "https://directed.example.test";
fn info(marker: char) -> NativeSessionInfo { NativeSessionInfo { entry_id: "a".repeat(64),
    session_marker: marker.to_string().repeat(64), topic_keys: vec!["topic_a".into()], selected_topic: None } }
fn session(marker: char) -> PortalSession { crate::native_recovery::checked_session(&info(marker), "topic_a").unwrap() }
fn path(root: &Path, layout: usize) -> PathBuf {
    let key = credential_storage_key(ORIGIN.into()).unwrap();
    match layout {
        0 => root.join(format!("vcp-pending-v1-{key}.sqlite")),
        1 => root.join(key).join("pending.sqlite"),
        _ => root.join("d".repeat(64)).join(key).join("pending.sqlite"),
    }
}
fn seed(file: &Path, text: &str) {
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let store = PendingStore::open(file).unwrap();
    drop(first_attempt(ORIGIN, &session('b'), &store, "topic_a", WireKind::Message,
        &json!({"requestId":"original_123","text":text,"attachments":[]}).to_string()).unwrap());
}
fn credential() -> Arc<SessionCredential> { SessionCredential::from_server_cookie(ORIGIN.into(),ORIGIN.into(),
    format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600","B".repeat(43))).unwrap() }

#[tokio::test]
async fn every_prior_layout_is_seen_by_new_and_compat_entries_after_session_change_and_reopen() {
    for layout in 0..3 {
        let root = tempfile::tempdir().unwrap(); let file = path(root.path(),layout);
        seed(&file,"合成原消息🧭"); let original = fs::read(&file).unwrap();
        let first = NativePendingJournal::open_scoped(ORIGIN.into(),root.path().to_str().unwrap().into()).unwrap();
        assert_eq!(first.private_path(),file.to_str().unwrap());
        assert_eq!(first.previews_for_topic(&info('c'),"topic_a").unwrap()[0].recovery_mode,"QUERY_ONLY_CROSS_SESSION");
        for reader in [first.clone(),NativePendingJournal::new(ORIGIN.into(),root.path().to_str().unwrap().into()).unwrap()] {
            assert_eq!(reader.store.list().unwrap().len(),1);
            assert!(matches!(reader.claim_original_message(&session('c'),"topic_a","different_123","new"),Err(NativeSendError::OutstandingPending)));
            assert!(matches!(reader.send_original_message_once(credential(),info('c'),"topic_a".into(),"different_123".into(),"new".into()).await,
                Err(NativeSendError::GuardedSendRequired)));
        }
        assert!(crate::native_recovery::NativePendingJournal::open(ORIGIN.into(),root.path().join("escape.sqlite").to_str().unwrap().into()).is_err());
        assert!(!root.path().join("escape.sqlite").exists());
        drop(first);
        let reopened = NativePendingJournal::open_scoped(ORIGIN.into(),root.path().to_str().unwrap().into()).unwrap();
        assert_eq!(reopened.store.list().unwrap()[0].state,PendingState::AwaitingConfirmation);
        drop(reopened);
        assert_eq!(fs::read(&file).unwrap(),original,"no mutation or migration of old UNKNOWN");
    }
}

#[test]
fn an_empty_competing_layout_cannot_hide_unknown_even_from_an_already_open_instance() {
    let root = tempfile::tempdir().unwrap(); let first = path(root.path(),2); seed(&first,"synthetic");
    let held = NativePendingJournal::new(ORIGIN.into(),root.path().to_str().unwrap().into()).unwrap();
    let empty = path(root.path(),0); fs::write(&empty,[]).unwrap();
    let before = fs::read(&first).unwrap();
    assert!(matches!(NativePendingJournal::new(ORIGIN.into(),root.path().to_str().unwrap().into()),Err(PendingJournalError::MultipleStores)));
    assert!(matches!(NativePendingJournal::open_scoped(ORIGIN.into(),root.path().to_str().unwrap().into()),Err(PendingJournalError::MultipleStores)));
    assert!(matches!(held.claim_original_message(&session('b'),"topic_a","new_request","new"),Err(NativeSendError::StoreUnavailable)));
    assert!(held.prepare_original(info('b'),"topic_a".into(),NativePendingKind::Message,
        "{\"requestId\":\"new_request\",\"text\":\"new\"}".into()).is_err());
    assert_eq!(held.store.list().unwrap().len(),1);
    assert_eq!(fs::read(&first).unwrap(),before);
    assert_eq!(fs::metadata(&empty).unwrap().len(),0);
}

#[test]
fn orphan_sidecars_are_retained_and_never_replaced_with_an_empty_database() {
    for layout in 0..3 {
        let root = tempfile::tempdir().unwrap();let main=path(root.path(),layout);
        fs::create_dir_all(main.parent().unwrap()).unwrap();
        let wal=PathBuf::from(format!("{}-wal",main.to_str().unwrap()));fs::write(&wal,b"orphan UNKNOWN").unwrap();
        assert!(matches!(NativePendingJournal::new(ORIGIN.into(),root.path().to_str().unwrap().into()),Err(PendingJournalError::StoreUnavailable)));
        assert!(!main.exists());assert_eq!(fs::read(wal).unwrap(),b"orphan UNKNOWN");
    }
}

#[test]
fn utf16_new_message_limit_is_shared_without_breaking_historic_receipt_identity() {
    use crate::native_send::{is_message_text_valid,message_input_max_utf16};
    assert_eq!(message_input_max_utf16(),4000);
    for text in ["中".repeat(4000),"🧭".repeat(2000),format!("{}🧭","中".repeat(3998))] {
        assert!(is_message_text_valid(text));
    }
    for text in ["中".repeat(4001),"🧭".repeat(2001),format!("{}🧭","中".repeat(3999))," \u{feff}\u{3000}".into()] {
        assert!(!is_message_text_valid(text));
    }
    let root=tempfile::tempdir().unwrap();let file=path(root.path(),0);let text="中".repeat(4100);seed(&file,&text);
    let journal=NativePendingJournal::new(ORIGIN.into(),root.path().to_str().unwrap().into()).unwrap();
    let original=journal.store.list().unwrap().remove(0).submission;
    let wire=RecoverySubmission::new("topic_a".into(),WireKind::Message,serde_json::from_str(&original.payload_json).unwrap()).unwrap();
    let active=session('c');let snapshot=crate::https_transport::HistorySnapshot{entry_id:active.entry_id.clone(),
        session_marker:active.session_marker.clone(),topic_key:"topic_a".into(),after:0,
        document:json!({"messages":[],"next":0,"hasMore":false,"topicState":{"historyReady":true,"executable":true},
            "executions":[{"id":"execution_old","topicKey":"topic_a","requestId":"original_123","state":"running",
                "recovery":{"version":1,"sessionMarker":"b".repeat(64),"kind":"message","payloadHash":wire.payload_hash()}}]})};
    let check=crate::receipt_query::PendingReceiptCheck::inspect_snapshot(&active,&journal.store,&original.key,&snapshot).unwrap();
    assert_eq!(journal.store.list().unwrap().len(),1);
    assert!(matches!(check.commit(&journal.store).unwrap(),crate::submission_flow::Reconciliation::Accepted{execution_state,..} if execution_state=="running"));
    assert!(journal.store.list().unwrap().is_empty());
}
