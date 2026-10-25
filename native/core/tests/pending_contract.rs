use vcp_mobile_native::pending::*;
use tempfile::TempDir;
use std::sync::{Arc, Barrier};

fn sample(id: &str) -> Submission {
    Submission { key: RequestKey { endpoint_id: "test-entry".into(), session_marker: "origin-a".into(), request_id: id.into() },
        topic_id: "topic-a".into(), kind: SubmissionKind::Message, payload_json: "{\"content\":\"合成消息 🧭\"}".into() }
}
fn setup() -> (TempDir, PendingStore) {
    let dir = TempDir::new().unwrap();
    let store = PendingStore::open(&dir.path().join("pending.sqlite")).unwrap();
    (dir, store)
}
#[test]
fn prepare_is_idempotent_and_preserves_original_bytes() {
    let (_dir, store) = setup(); let mut s = sample("r"); s.payload_json = "{ \"z\": 1, \"a\": \"文\" }".into();
    assert_eq!(store.prepare(&s).unwrap(), store.prepare(&s).unwrap());
    assert_eq!(store.list().unwrap().len(), 1);
    assert_eq!(store.get(&s.key).unwrap().unwrap().submission.payload_json, s.payload_json);
}
#[test]
fn changed_payload_topic_or_kind_cannot_reuse_identity() {
    let (_dir, store) = setup(); let original=sample("r"); store.prepare(&original).unwrap();
    let mut changed=original.clone(); changed.topic_id="topic-b".into();
    assert!(matches!(store.prepare(&changed), Err(StoreError::IdentityConflict)));
    changed=original.clone(); changed.payload_json="{}".into();
    assert!(matches!(store.prepare(&changed), Err(StoreError::IdentityConflict)));
    changed=original.clone(); changed.kind=SubmissionKind::Invite;
    assert!(matches!(store.prepare(&changed), Err(StoreError::IdentityConflict)));
    assert_eq!(store.get(&original.key).unwrap().unwrap().submission, original);
}
#[test]
fn scope_separates_session_and_endpoint_without_global_request_ids() {
    let (_dir, store) = setup(); let a=sample("same-id");
    let mut b=a.clone(); b.key.session_marker="origin-b".into();
    let mut c=a.clone(); c.key.endpoint_id="other-entry".into();
    for value in [&a,&b,&c] { store.prepare(value).unwrap(); }
    assert_eq!(store.list().unwrap().len(),3);
}
#[test]
fn unknown_survives_reopen_without_new_request_or_expiry() {
    let (dir, store)=setup(); let s=sample("old-id"); store.prepare(&s).unwrap(); store.mark_attempted(&s.key).unwrap(); drop(store);
    let reopened=PendingStore::open(&dir.path().join("pending.sqlite")).unwrap();
    assert_eq!(reopened.get(&s.key).unwrap().unwrap(),PendingRecord{submission:s,state:PendingState::AwaitingConfirmation});
}
#[test]
fn cross_session_recovery_is_query_only_and_wrong_entry_rejected() {
    let (_dir,store)=setup(); let s=sample("r");store.prepare(&s).unwrap();
    assert_eq!(store.recovery_action(&s.key,"test-entry","origin-a").unwrap(),RecoveryAction::QueryOriginalBeforeExplicitRetry);
    assert_eq!(store.recovery_action(&s.key,"test-entry","origin-b").unwrap(),RecoveryAction::QueryOnlyAcrossSession);
    assert!(matches!(store.recovery_action(&s.key,"wrong-entry","origin-a"),Err(StoreError::IdentityConflict)));
    assert_eq!(store.list().unwrap().len(),1);
}
#[test]
fn confirmation_requires_exact_original_and_only_removes_that_record() {
    let (_dir,store)=setup();let a=sample("a");let b=sample("b");store.prepare(&a).unwrap();store.prepare(&b).unwrap();
    let mut wrong=a.clone();wrong.payload_json="{}".into();
    assert!(matches!(store.confirm_verified(&wrong),Err(StoreError::IdentityConflict)));
    assert_eq!(store.list().unwrap().len(),2);
    assert!(store.confirm_verified(&a).unwrap());assert!(!store.confirm_verified(&a).unwrap());
    assert_eq!(store.list().unwrap()[0].submission,b);
}
#[test]
fn count_capacity_never_discards_unknown_submissions() {
    let (_dir,store)=setup();for i in 0..MAX_RECORDS{store.prepare(&sample(&i.to_string())).unwrap();}
    assert!(matches!(store.prepare(&sample("overflow")),Err(StoreError::Capacity)));
    assert_eq!(store.list().unwrap().len(),MAX_RECORDS);
}
#[test]
fn byte_capacity_is_utf8_based_and_transactional() {
    let (_dir,store)=setup();let mut a=sample("a");a.payload_json=format!("{{\"x\":\"{}\"}}","文".repeat(60000));store.prepare(&a).unwrap();
    let mut b=sample("b");b.payload_json=a.payload_json.clone();
    assert!(matches!(store.prepare(&b),Err(StoreError::Capacity)));
    assert_eq!(store.list().unwrap().len(),1);
}
#[test]
fn malformed_input_and_missing_record_are_visible() {
    let (_dir,store)=setup();let mut a=sample("a");a.payload_json="{".into();
    assert!(matches!(store.prepare(&a),Err(StoreError::InvalidInput)));
    a.payload_json="[]".into();assert!(matches!(store.prepare(&a),Err(StoreError::InvalidInput)));
    assert!(matches!(store.mark_attempted(&a.key),Err(StoreError::NotFound)));
    assert!(store.list().unwrap().is_empty());
}
#[test]
fn unrelated_and_future_databases_are_not_reinitialized() {
    let dir=TempDir::new().unwrap();let path=dir.path().join("other.sqlite");
    let conn=rusqlite::Connection::open(&path).unwrap();conn.execute_batch("CREATE TABLE real_history(value TEXT);INSERT INTO real_history VALUES ('keep');").unwrap();drop(conn);
    assert!(matches!(PendingStore::open(&path),Err(StoreError::UnsupportedSchema)));
    let conn=rusqlite::Connection::open(&path).unwrap();assert_eq!(conn.query_row("SELECT value FROM real_history",[],|r|r.get::<_,String>(0)).unwrap(),"keep");
    let path2=dir.path().join("future.sqlite");let store=PendingStore::open(&path2).unwrap();drop(store);
    let conn=rusqlite::Connection::open(&path2).unwrap();conn.pragma_update(None,"user_version",99).unwrap();drop(conn);
    assert!(matches!(PendingStore::open(&path2),Err(StoreError::UnsupportedSchema)));
}
#[test]
fn two_connections_do_not_lose_concurrent_prepares_or_confirmation() {
    let (dir,store)=setup();let first=sample("first");store.prepare(&first).unwrap();
    let other=PendingStore::open(&dir.path().join("pending.sqlite")).unwrap();
    let barrier=Arc::new(Barrier::new(2));let b=barrier.clone();
    let thread=std::thread::spawn(move||{b.wait();other.prepare(&sample("second")).unwrap();});
    barrier.wait();store.confirm_verified(&first).unwrap();thread.join().unwrap();
    assert_eq!(store.list().unwrap()[0].submission.key.request_id,"second");
}
#[test]
fn damaged_record_is_reported_and_not_deleted() {
    let (dir,store)=setup();let s=sample("r");store.prepare(&s).unwrap();
    let conn=rusqlite::Connection::open(dir.path().join("pending.sqlite")).unwrap();
    conn.execute("UPDATE pending_submission SET payload_json='broken'",[]).unwrap();
    assert!(matches!(store.get(&s.key),Err(StoreError::Corrupt)));
    assert!(matches!(store.list(),Err(StoreError::Corrupt)));
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM pending_submission",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}
#[test]
fn missing_parent_does_not_create_a_replacement_store() {
    let dir=TempDir::new().unwrap();assert!(PendingStore::open(&dir.path().join("missing/child.sqlite")).is_err());
    assert!(matches!(PendingStore::open(std::path::Path::new("relative.sqlite")),Err(StoreError::InvalidInput)));
}
#[test]
fn committed_submission_survives_process_exit_without_connection_drop() {
    const CHILD_FLAG: &str = "VCP_PENDING_CONTRACT_CHILD";
    if std::env::var_os(CHILD_FLAG).is_some() {
        let path=std::path::PathBuf::from(std::env::var_os("VCP_PENDING_CONTRACT_PATH").unwrap());
        let store=PendingStore::open(&path).unwrap();
        let s=sample("crash-persisted");
        store.prepare(&s).unwrap();
        store.mark_attempted(&s.key).unwrap();
        // Exit after commit without Rust destructors; not physical power loss.
        std::process::exit(73);
    }
    let dir=TempDir::new().unwrap();
    let path=dir.path().join("pending.sqlite");
    let output=std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "committed_submission_survives_process_exit_without_connection_drop", "--nocapture"])
        .env(CHILD_FLAG,"1").env("VCP_PENDING_CONTRACT_PATH",&path)
        .output().unwrap();
    assert_eq!(output.status.code(),Some(73),"{}",String::from_utf8_lossy(&output.stderr));
    let store=PendingStore::open(&path).unwrap();
    assert_eq!(store.list().unwrap(),vec![PendingRecord{submission:sample("crash-persisted"),state:PendingState::AwaitingConfirmation}]);
}
