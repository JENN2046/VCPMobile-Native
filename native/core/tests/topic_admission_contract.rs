use std::sync::{Arc, Barrier};
use tempfile::TempDir;
use vcp_mobile_native::pending::*;

fn submission(entry: &str, topic: &str, session: &str, request: &str) -> Submission {
    Submission {
        key: RequestKey { endpoint_id: entry.into(), session_marker: session.into(), request_id: request.into() },
        topic_id: topic.into(),
        kind: SubmissionKind::Message,
        payload_json: format!(r#"{{"requestId":"{request}","text":"synthetic"}}"#),
    }
}
fn open() -> (TempDir, PendingStore) {
    let dir = TempDir::new().unwrap();
    let store = PendingStore::open(&dir.path().join("pending.sqlite")).unwrap();
    (dir, store)
}
#[test]
fn entry_topic_allows_only_one_unknown_across_sessions_and_kinds() {
    let (_dir, store) = open();
    let first = submission("entry", "topic", "old", "r1");
    let mut second = submission("entry", "topic", "new", "r2");
    store.prepare_topic_exclusive(&first).unwrap();
    assert!(matches!(store.prepare_topic_exclusive(&second), Err(StoreError::TopicPending)));
    second.kind = SubmissionKind::Invite;
    second.payload_json = r#"{"requestId":"r2","agentId":"agent"}"#.into();
    assert!(matches!(store.prepare_topic_exclusive(&second), Err(StoreError::TopicPending)));
    assert_eq!(store.list().unwrap().len(), 1);
}
#[test]
fn exact_repeat_is_idempotent_and_never_resets_unknown() {
    let (_dir, store) = open();
    let first = submission("entry", "topic", "old", "r1");
    store.prepare_topic_exclusive(&first).unwrap();
    store.mark_attempted(&first.key).unwrap();
    let result = store.prepare_topic_exclusive(&first).unwrap();
    assert_eq!(result.state, PendingState::AwaitingConfirmation);
    let mut changed = first.clone();
    changed.payload_json = r#"{"requestId":"r1","text":"changed"}"#.into();
    assert!(matches!(store.prepare_topic_exclusive(&changed), Err(StoreError::IdentityConflict)));
}
#[test]
fn unrelated_topic_or_entry_has_independent_admission() {
    let (_dir, store) = open();
    for input in [
        submission("entry-1", "topic-a", "one", "r1"),
        submission("entry-1", "topic-b", "two", "r2"),
        submission("entry-2", "topic-a", "one", "r3"),
    ] { store.prepare_topic_exclusive(&input).unwrap(); }
    assert_eq!(store.list().unwrap().len(), 3);
}
#[test]
fn restart_blocks_old_unknown_until_exact_confirmation() {
    let (dir, store) = open();
    let first = submission("entry", "topic", "old", "r1");
    let fresh = submission("entry", "topic", "new", "r2");
    store.prepare_topic_exclusive(&first).unwrap();
    store.mark_attempted(&first.key).unwrap();
    drop(store);
    let reopened = PendingStore::open(&dir.path().join("pending.sqlite")).unwrap();
    assert_eq!(reopened.recovery_action(&first.key, "entry", "new").unwrap(), RecoveryAction::QueryOnlyAcrossSession);
    assert!(matches!(reopened.prepare_topic_exclusive(&fresh), Err(StoreError::TopicPending)));
    let mut wrong = first.clone(); wrong.topic_id = "other".into();
    assert!(matches!(reopened.confirm_verified(&wrong), Err(StoreError::IdentityConflict)));
    assert!(reopened.confirm_verified(&first).unwrap());
    reopened.prepare_topic_exclusive(&fresh).unwrap();
    assert_eq!(reopened.list().unwrap().len(), 1);
}
#[test]
fn existing_multiple_unknowns_fail_closed_without_deleting_history() {
    let (_dir, store) = open();
    let first = submission("entry", "topic", "old", "r1");
    let second = submission("entry", "topic", "new", "r2");
    store.prepare(&first).unwrap(); store.prepare(&second).unwrap();
    assert!(matches!(store.prepare_topic_exclusive(&first), Err(StoreError::TopicPending)));
    assert_eq!(store.list().unwrap().len(), 2);
}
#[test]
fn independent_connections_serialize_simultaneous_topic_admission() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("pending.sqlite");
    PendingStore::open(&path).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for session in ["one", "two"] {
        let path = path.clone();
        let gate = barrier.clone();
        threads.push(std::thread::spawn(move || {
            let store = PendingStore::open(&path).unwrap();
            let value = submission("entry", "topic", session, session);
            gate.wait();
            store.prepare_topic_exclusive(&value)
        }));
    }
    barrier.wait();
    let outcomes: Vec<_> = threads.into_iter().map(|j| j.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|v| v.is_ok()).count(), 1);
    assert_eq!(outcomes.iter().filter(|v| matches!(v, Err(StoreError::TopicPending))).count(), 1);
    assert_eq!(PendingStore::open(&path).unwrap().list().unwrap().len(), 1);
}
#[test]
fn malformed_input_cannot_reserve_a_topic() {
    let (_dir, store) = open();
    let mut bad = submission("entry", "topic", "old", "r1");
    bad.payload_json = "{".into();
    assert!(matches!(store.prepare_topic_exclusive(&bad), Err(StoreError::InvalidInput)));
    store.prepare_topic_exclusive(&submission("entry", "topic", "new", "r2")).unwrap();
}
