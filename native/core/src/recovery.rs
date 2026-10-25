//! NativeSync R2.1 submission fingerprint and receipt inspection.
//! The caller must use authenticated, authorized-topic responses. Confirmed
//! means accepted submission identity, never completed AA work.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecoveryError {
    #[error("invalid recovery submission")]
    InvalidSubmission,
    #[error("unsupported recovery context")]
    MissingMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionKind { Message, Invitation }
impl SubmissionKind {
    pub fn as_str(self) -> &'static str {
        match self { Self::Message => "message", Self::Invitation => "invitation" }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecovery {
    pub entry_id: String,
    pub session_marker: String,
}
impl SessionRecovery {
    pub fn parse(value: &Value) -> Result<Self, RecoveryError> {
        if !value.is_object() || value.get("version").and_then(Value::as_f64) != Some(1.0) {
            return Err(RecoveryError::MissingMetadata);
        }
        let entry_id = hex_field(value, "entryId").ok_or(RecoveryError::MissingMetadata)?;
        let session_marker = hex_field(value, "sessionMarker").ok_or(RecoveryError::MissingMetadata)?;
        Ok(Self { entry_id: entry_id.into(), session_marker: session_marker.into() })
    }
    pub fn is_valid(&self) -> bool { is_hex(&self.entry_id) && is_hex(&self.session_marker) }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoverySubmission {
    topic_key: String,
    kind: SubmissionKind,
    payload: Value,
    payload_hash: String,
}
impl RecoverySubmission {
    pub fn new(topic_key: String, kind: SubmissionKind, payload: Value) -> Result<Self, RecoveryError> {
        let invalid = RecoveryError::InvalidSubmission;
        if !bounded(&topic_key, 2048) || !payload.is_object() { return Err(invalid); }
        let request_id = payload.get("requestId").and_then(Value::as_str).ok_or(invalid.clone())?;
        if !bounded(request_id, 80) { return Err(invalid); }
        let fields = payload.as_object().ok_or(invalid.clone())?;
        let canonical_payload = match kind {
            SubmissionKind::Message => {
                if fields.keys().any(|key| !["requestId", "text", "attachments"].contains(&key.as_str())) {
                    return Err(invalid);
                }
                let text = payload.get("text").and_then(Value::as_str).ok_or(invalid.clone())?;
                if text.encode_utf16().count() > 12_000 { return Err(invalid); }
                let attachments = match payload.get("attachments") {
                    None => Vec::new(),
                    Some(Value::Array(items)) if items.len() <= 8 => {
                        let mut result = Vec::with_capacity(items.len());
                        for item in items {
                            let id = item.as_str().ok_or(invalid.clone())?;
                            if !bounded(id, 256) { return Err(invalid); }
                            result.push(id.to_owned());
                        }
                        result
                    }
                    _ => return Err(invalid),
                };
                if text.trim_matches(js_whitespace).is_empty() && attachments.is_empty() { return Err(invalid); }
                json!({"requestId": request_id, "text": text, "attachments": attachments})
            }
            SubmissionKind::Invitation => {
                if fields.keys().any(|key| !["requestId", "agentId"].contains(&key.as_str())) { return Err(invalid); }
                let agent = payload.get("agentId").and_then(Value::as_str).ok_or(invalid.clone())?;
                if !bounded(agent, 256) { return Err(invalid); }
                json!({"requestId": request_id, "agentId": agent})
            }
        };
        let normalized = json!({"topicKey": topic_key, "kind": kind.as_str(), "payload": canonical_payload});
        let encoded = stable(&normalized)?;
        let payload_hash = format!("{:x}", Sha256::digest(encoded.as_bytes()));
        Ok(Self { topic_key, kind, payload, payload_hash })
    }
    pub fn payload_hash(&self) -> &str { &self.payload_hash }
    pub fn topic_key(&self) -> &str { &self.topic_key }
    pub fn kind(&self) -> SubmissionKind { self.kind }
    pub fn payload(&self) -> &Value { &self.payload }
    pub fn request_id(&self) -> &str { self.payload["requestId"].as_str().unwrap_or("") }

    /// Additional real server identity syntax; required before new dispatch.
    pub fn validate_wire_identity(&self) -> Result<(), RecoveryError> {
        if !ascii_id(&self.topic_key, 1, 64) || !ascii_id(self.request_id(), 8, 80) {
            return Err(RecoveryError::InvalidSubmission);
        }
        Ok(())
    }

    pub fn inspect(&self, original_context: &SessionRecovery, current_context: &Value, executions: &Value) -> ReceiptInspection {
        let Ok(current) = SessionRecovery::parse(current_context) else { return ReceiptInspection::Review(ReviewReason::Metadata); };
        let Some(jobs) = executions.as_array() else { return ReceiptInspection::Review(ReviewReason::Metadata); };
        if !original_context.is_valid() || current.entry_id != original_context.entry_id {
            return ReceiptInspection::Review(ReviewReason::Metadata);
        }
        if jobs.iter().any(|job| !job.is_object() || job.get("topicKey").and_then(Value::as_str) != Some(self.topic_key.as_str())) {
            return ReceiptInspection::Review(ReviewReason::Scope);
        }
        let candidates: Vec<&Value> = jobs.iter().filter(|job| job.get("requestId").and_then(Value::as_str) == Some(self.request_id())).collect();
        if candidates.is_empty() {
            return if current.session_marker == original_context.session_marker { ReceiptInspection::AbsentSameSession }
                else { ReceiptInspection::Review(ReviewReason::Session) };
        }
        if candidates.len() != 1 { return ReceiptInspection::Review(ReviewReason::Ambiguous); }
        let job = candidates[0];
        let Some(id) = job.get("id").and_then(Value::as_str).filter(|s| bounded(s, 256)) else { return ReceiptInspection::Review(ReviewReason::Metadata); };
        let Some(state) = job.get("state").and_then(Value::as_str).filter(|s| bounded(s, 80)) else { return ReceiptInspection::Review(ReviewReason::Metadata); };
        let Some(meta) = job.get("recovery").filter(|v| v.is_object()) else { return ReceiptInspection::Review(ReviewReason::Metadata); };
        let marker = hex_field(meta, "sessionMarker");
        let hash = hex_field(meta, "payloadHash");
        let kind = meta.get("kind").and_then(Value::as_str);
        if meta.get("version").and_then(Value::as_f64) != Some(1.0) || marker.is_none() || hash.is_none()
            || !matches!(kind, Some("message" | "invitation")) {
            return ReceiptInspection::Review(ReviewReason::Metadata);
        }
        if marker != Some(original_context.session_marker.as_str()) || hash != Some(self.payload_hash.as_str()) || kind != Some(self.kind.as_str()) {
            return ReceiptInspection::Review(ReviewReason::Conflict);
        }
        ReceiptInspection::Confirmed { execution_id: id.into(), execution_state: state.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewReason { Metadata, Scope, Ambiguous, Conflict, Session }
impl ReviewReason {
    pub fn as_str(self) -> &'static str {
        match self { Self::Metadata => "metadata", Self::Scope => "scope", Self::Ambiguous => "ambiguous", Self::Conflict => "conflict", Self::Session => "session" }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptInspection {
    Confirmed { execution_id: String, execution_state: String },
    /// Not permission to resend: explicit retry is still required.
    AbsentSameSession,
    Review(ReviewReason),
}
fn bounded(value: &str, max: usize) -> bool { !value.is_empty() && value.encode_utf16().count() <= max }
fn ascii_id(value: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn is_hex(value: &str) -> bool { value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }
fn hex_field<'a>(value: &'a Value, name: &str) -> Option<&'a str> { value.get(name).and_then(Value::as_str).filter(|s| is_hex(s)) }
fn js_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
fn stable(value: &Value) -> Result<String, RecoveryError> {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            // Protocol keys are ASCII; not a general arbitrary-JS-key canonicalizer.
            keys.sort();
            let fields: Result<Vec<String>, RecoveryError> = keys.into_iter().map(|key| {
                let name = serde_json::to_string(key).map_err(|_| RecoveryError::InvalidSubmission)?;
                Ok(format!("{}:{}", name, stable(&map[key])?))
            }).collect();
            Ok(format!("{{{}}}", fields?.join(",")))
        }
        Value::Array(values) => {
            let entries: Result<Vec<String>, RecoveryError> = values.iter().map(stable).collect();
            Ok(format!("[{}]", entries?.join(",")))
        }
        _ => serde_json::to_string(value).map_err(|_| RecoveryError::InvalidSubmission),
    }
}
