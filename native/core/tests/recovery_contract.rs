use serde_json::{json, Value};
use vcp_mobile_native::recovery::*;

fn expected_json(result: ReceiptInspection) -> Value {
    match result {
        ReceiptInspection::Confirmed { execution_id, execution_state } => json!({"state":"confirmed","reason":null,"executionId":execution_id,"executionState":execution_state}),
        ReceiptInspection::AbsentSameSession => json!({"state":"absent","reason":null,"executionId":null,"executionState":null}),
        ReceiptInspection::Review(reason) => json!({"state":"review","reason":reason.as_str(),"executionId":null,"executionState":null}),
    }
}
#[test]
fn exact_accepted_javascript_oracle_fifty_cases() {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/recovery-golden.json")).unwrap();
    assert_eq!(fixture["source"]["git_blob"], "cd4729f206a2007951a468d088f0f085a61ba521");
    let entry = "a".repeat(64);
    let marker = "b".repeat(64);
    let original = SessionRecovery { entry_id: entry.clone(), session_marker: marker.clone() };
    let mut count = 0;
    for vector in fixture["vectors"].as_array().unwrap() {
        let input = &vector["submission"];
        let kind = match input["kind"].as_str().unwrap() { "message" => SubmissionKind::Message, "invitation" => SubmissionKind::Invitation, _ => panic!("unsupported fixture kind") };
        let submission = RecoverySubmission::new(input["topicKey"].as_str().unwrap().into(),kind,input["payload"].clone()).unwrap();
        submission.validate_wire_identity().unwrap();
        assert_eq!(submission.payload_hash(),vector["payloadHash"].as_str().unwrap());
        assert_eq!(submission.payload(),&input["payload"]);
        let context = json!({"version":1,"entryId":entry,"sessionMarker":marker});
        let execution = json!({"id":"execution-1","topicKey":submission.topic_key(),"requestId":submission.request_id(),"state":"completed", "recovery":{"version":1,"sessionMarker":marker,"payloadHash":submission.payload_hash(),"kind":kind.as_str()}});
        for (name, change) in [("same_session_missing",0),("different_session_missing",1),("exact_match",2),("different_session_exact_match",3),("duplicate_candidates",4),("wrong_topic",5),("missing_recovery",6),("different_hash",7),("wrong_entry",8),("unknown_is_still_accepted",9)] {
            let mut active = context.clone();
            let mut job = execution.clone();
            let jobs = match change {
                0 => json!([]),
                1 => { active["sessionMarker"]=json!("c".repeat(64));json!([]) }
                3 => { active["sessionMarker"]=json!("c".repeat(64));json!([job]) }
                4 => { let mut other=job.clone();other["id"]=json!("execution-2");json!([job,other]) }
                5 => { job["topicKey"]=json!("other-topic");json!([job]) }
                6 => { job.as_object_mut().unwrap().remove("recovery");json!([job]) }
                7 => { job["recovery"]["payloadHash"]=json!("d".repeat(64));json!([job]) }
                8 => { active["entryId"]=json!("f".repeat(64));json!([job]) }
                9 => { job["state"]=json!("review_required");json!([job]) }
                _ => json!([job]),
            };
            let id=format!("{}/{}",vector["name"].as_str().unwrap(),name);
            assert_eq!(expected_json(submission.inspect(&original,&active,&jobs)),fixture["scenarios"][name],"{id}");
            count+=1;
        }
    }
    assert_eq!(count,50);
}
#[test]
fn utf16_limits_and_ecmascript_whitespace_match_client_contract() {
    let make=|text: String| RecoverySubmission::new("topic_1".into(),SubmissionKind::Message,json!({"requestId":"request-1","text":text}));
    assert!(make("🧭".repeat(6000)).is_ok());
    assert!(make("🧭".repeat(6001)).is_err());
    assert!(make("\u{feff}\u{00a0}\u{3000}".into()).is_err());
    assert!(make("\u{0085}".into()).is_ok());
}
#[test]
fn unknown_fields_and_invalid_attachment_values_are_refused() {
    for payload in [json!({"requestId":"request-1","text":"hello","token":"must-not-accept"}),json!({"requestId":"request-1","text":"hello","attachments":null}),json!({"requestId":"request-1","text":"hello","attachments":[""]}),json!({"requestId":"request-1","text":"hello","attachments":[1]}),json!({"requestId":"request-1","text":"hello","attachments":vec!["id";9]})] {
        assert!(RecoverySubmission::new("topic_1".into(),SubmissionKind::Message,payload).is_err());
    }
}
#[test]
fn outgoing_identity_uses_real_server_syntax() {
    let parsed=RecoverySubmission::new("topic/unsafe".into(),SubmissionKind::Message,json!({"requestId":"short","text":"hello"})).unwrap();
    assert_eq!(parsed.validate_wire_identity(),Err(RecoveryError::InvalidSubmission));
    let good=RecoverySubmission::new("topic_1".into(),SubmissionKind::Message,json!({"requestId":"request-1","text":"hello"})).unwrap();
    assert!(good.validate_wire_identity().is_ok());
}
#[test]
fn missing_or_non_hex_context_cannot_confirm_or_permit_retry() {
    let parsed=RecoverySubmission::new("topic_1".into(),SubmissionKind::Message,json!({"requestId":"request-1","text":"hello"})).unwrap();
    let original=SessionRecovery{entry_id:"a".repeat(64),session_marker:"b".repeat(64)};
    for context in [Value::Null,json!({"version":2,"entryId":"a".repeat(64),"sessionMarker":"b".repeat(64)}),json!({"version":1,"entryId":"A".repeat(64),"sessionMarker":"b".repeat(64)})] {
        assert_eq!(parsed.inspect(&original,&context,&json!([])),ReceiptInspection::Review(ReviewReason::Metadata));
    }
}
#[test]
fn changing_attachment_order_changes_fingerprint_but_omission_equals_empty() {
    let make=|payload:Value|RecoverySubmission::new("topic_1".into(),SubmissionKind::Message,payload).unwrap();
    let a=make(json!({"requestId":"request-1","text":"hello"}));
    let b=make(json!({"requestId":"request-1","text":"hello","attachments":[]}));
    assert_eq!(a.payload_hash(),b.payload_hash());
    let c=make(json!({"requestId":"request-1","text":"hello","attachments":["a","b"]}));
    let d=make(json!({"requestId":"request-1","text":"hello","attachments":["b","a"]}));
    assert_ne!(c.payload_hash(),d.payload_hash());
}
