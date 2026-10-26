//! Platform-fenced adaptation of the existing first-send seam. No second
//! journal, retry, model authority or receipt oracle is introduced here.
use crate::https_transport::PortalHttpClient;
use crate::native_recovery::{checked_session, NativeRecoveryFence};
use crate::pending_journal::{NativeFirstSendResult, NativePendingJournal, NativeSendError};
use crate::session_credential::{NativeSessionInfo, SessionCredential};
use crate::submission_flow::OutboundSubmission;
use crate::transport_policy::SubmissionHttpOutcome;
use std::sync::{Arc, Mutex};

/// New deliberate messages use the shared UI limit. Historic receipt/hash
/// validation retains the original 12000-unit NativeSync wire contract.
#[uniffi::export]
pub fn message_input_max_utf16() -> u32 { 4000 }
#[uniffi::export]
pub fn is_message_text_valid(text: String) -> bool {
    text.encode_utf16().count() <= message_input_max_utf16() as usize &&
        crate::recovery::RecoverySubmission::new("validation_topic".into(),
            crate::recovery::SubmissionKind::Message,
            serde_json::json!({"requestId":"validation_123","text":text,"attachments":[]})).is_ok()
}

// Original bytes stay opaque to native UI and debug output.
struct Message { request_id: String, text: String }
#[derive(uniffi::Object)]
pub struct NativeMessageCheck {
    journal: Arc<NativePendingJournal>,
    fence: Arc<NativeRecoveryFence>,
    fingerprint: String,
    message: Mutex<Option<Message>>,
    stamp: crate::pending::StoreStamp,
}
#[derive(uniffi::Object)]
pub struct NativeMessageDispatch {
    journal: Arc<NativePendingJournal>,
    fence: Arc<NativeRecoveryFence>,
    fingerprint: String,
    plan: Mutex<Option<OutboundSubmission>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl SessionCredential {
    /// Non-mutating server preflight. The host MUST reload the credential and
    /// call claim_message_send under its same replacement/removal vault gate.
    pub async fn prepare_message_send(
        &self, journal: Arc<NativePendingJournal>, expected: NativeSessionInfo,
        topic_key: String, request_id: String, text: String,
        fence: Arc<NativeRecoveryFence>,
    ) -> Result<Arc<NativeMessageCheck>, NativeSendError> {
        let client = PortalHttpClient::new(&self.origin()).map_err(|_| NativeSendError::SessionUnavailable)?;
        self.message_check_with_client(journal, expected, topic_key, request_id, text, fence, &client).await
    }

    /// Consumes a durably claimed intent at most once. Cancellation after the
    /// claim or HTTP initiation cannot prove it was unsent; pending survives.
    /// This never cancels AA and does not clear pending for ANY HTTP response.
    pub async fn dispatch_message_once(
        &self, dispatch: Arc<NativeMessageDispatch>,
    ) -> Result<NativeFirstSendResult, NativeSendError> {
        let client = PortalHttpClient::new(&self.origin()).map_err(|_| NativeSendError::SessionUnavailable)?;
        self.message_dispatch_with_client(dispatch, &client).await
    }
}

impl SessionCredential {
    // Same production path; only crate-internal tests supply an isolated CA/client.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn message_check_with_client(&self, journal: Arc<NativePendingJournal>,
        expected: NativeSessionInfo, topic_key: String, request_id: String, text: String,
        fence: Arc<NativeRecoveryFence>, client: &PortalHttpClient,
    ) -> Result<Arc<NativeMessageCheck>, NativeSendError> {
        self.recovery_valid().map_err(|_| NativeSendError::CredentialMismatch)?;
        if self.origin() != journal.origin { return Err(NativeSendError::CredentialMismatch); }
        validate_context(&expected, &topic_key, fence.view_generation, &fence)?;
        if !is_message_text_valid(text.clone()) { return Err(NativeSendError::InvalidMessage); }
        let wire = crate::recovery::RecoverySubmission::new(topic_key.clone(),
            crate::recovery::SubmissionKind::Message,
            serde_json::json!({"requestId":request_id,"text":text,"attachments":[]}))
            .map_err(|_| NativeSendError::InvalidMessage)?;
        wire.validate_wire_identity().map_err(|_| NativeSendError::InvalidMessage)?;
        let stamp = journal.store.change_stamp().map_err(|_| NativeSendError::StoreUnavailable)?;
        let current = self.session_info_with_client(client).await.map_err(|_| NativeSendError::SessionUnavailable)?;
        self.recovery_valid().map_err(|_| NativeSendError::CredentialMismatch)?;
        validate_context(&current, &topic_key, fence.view_generation, &fence)?;
        Ok(Arc::new(NativeMessageCheck {
            journal, fence, stamp, fingerprint: self.recovery_fingerprint(),
            message: Mutex::new(Some(Message { request_id, text })),
        }))
    }
    pub(crate) async fn message_dispatch_with_client(&self, dispatch: Arc<NativeMessageDispatch>,
        client: &PortalHttpClient) -> Result<NativeFirstSendResult, NativeSendError> {
        self.recovery_valid().map_err(|_| NativeSendError::CredentialMismatch)?;
        if dispatch.fingerprint != self.recovery_fingerprint() {
            return Err(NativeSendError::CredentialMismatch);
        }
        dispatch.journal.verify_location().map_err(|_| NativeSendError::StoreUnavailable)?;
        let plan = {
            let cancelled = dispatch.fence.cancelled.lock().map_err(|_| NativeSendError::Cancelled)?;
            if *cancelled { return Err(NativeSendError::Cancelled); }
            dispatch.plan.lock().map_err(|_| NativeSendError::AlreadyConsumed)?
                .take().ok_or(NativeSendError::AlreadyConsumed)?
        };
        let request_id = plan.request_id().to_owned();
        let result = self.dispatch_claimed_with_client(plan, client).await;
        Ok(NativeFirstSendResult { request_id, disposition: disposition(result).into() })
    }
}

#[uniffi::export]
impl SessionCredential {
    /// Synchronous privileged transition: reloaded credential, current view
    /// identity, cancellation fence and durable claim are checked together.
    /// Invoke continuously under the platform vault lock/actor; no await gap.
    pub fn claim_message_send(
        &self, journal: Arc<NativePendingJournal>, check: Arc<NativeMessageCheck>,
        expected: NativeSessionInfo, topic_key: String, view_generation: u64,
    ) -> Result<Arc<NativeMessageDispatch>, NativeSendError> {
        self.recovery_valid().map_err(|_| NativeSendError::CredentialMismatch)?;
        if !Arc::ptr_eq(&journal, &check.journal) || self.origin() != journal.origin
            || self.recovery_fingerprint() != check.fingerprint {
            return Err(NativeSendError::CredentialMismatch);
        }
        let cancelled = check.fence.cancelled.lock().map_err(|_| NativeSendError::Cancelled)?;
        if *cancelled { return Err(NativeSendError::Cancelled); }
        let session = checked_session(&expected, &topic_key).map_err(|_| NativeSendError::ContextChanged)?;
        if session.entry_id != check.fence.expected.entry_id
            || session.session_marker != check.fence.expected.session_marker
            || topic_key != check.fence.topic_key || view_generation != check.fence.view_generation {
            return Err(NativeSendError::ContextChanged);
        }
        let message = check.message.lock().map_err(|_| NativeSendError::AlreadyConsumed)?
            .take().ok_or(NativeSendError::AlreadyConsumed)?;
        // Existing admission and one-use dispatch claim: exactly one source.
        let plan = journal.claim_original_message_at_stamp(&session, &topic_key, &message.request_id, &message.text, Some(check.stamp))?;
        drop(cancelled);
        Ok(Arc::new(NativeMessageDispatch { journal: journal.clone(), fence: check.fence.clone(),
            fingerprint: check.fingerprint.clone(), plan: Mutex::new(Some(plan)) }))
    }
}

fn validate_context(expected: &NativeSessionInfo, topic: &str, generation: u64,
    fence: &NativeRecoveryFence) -> Result<(), NativeSendError> {
    fence.ensure_active().map_err(|_| NativeSendError::Cancelled)?;
    let session = checked_session(expected, topic).map_err(|_| NativeSendError::TopicUnauthorized)?;
    if session.entry_id != fence.expected.entry_id || session.session_marker != fence.expected.session_marker
        || topic != fence.topic_key || generation != fence.view_generation {
        return Err(NativeSendError::ContextChanged);
    }
    Ok(())
}
pub(crate) fn disposition(result: Result<SubmissionHttpOutcome, ()>) -> &'static str {
    match result {
        Ok(SubmissionHttpOutcome::AcceptedButUnverified) => "HTTP_202_UNVERIFIED",
        Ok(SubmissionHttpOutcome::SubmissionContextChanged) => "CONTEXT_CHANGED_KEEP_PENDING",
        Ok(SubmissionHttpOutcome::AuthenticationRequired) => "AUTH_REQUIRED_KEEP_PENDING",
        Ok(SubmissionHttpOutcome::Forbidden) => "FORBIDDEN_KEEP_PENDING",
        Ok(SubmissionHttpOutcome::RedirectDenied) => "REDIRECT_REJECTED_KEEP_PENDING",
        Ok(SubmissionHttpOutcome::UnknownKeepPending) | Err(()) => "UNKNOWN_KEEP_PENDING",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info() -> NativeSessionInfo { NativeSessionInfo { entry_id: "a".repeat(64),
        session_marker: "b".repeat(64), topic_keys: vec!["topic_a".into()], selected_topic: None } }
    fn credential() -> Arc<SessionCredential> {
        SessionCredential::from_server_cookie("https://send.example.test".into(), "https://send.example.test".into(),
            format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600", "B".repeat(43))).unwrap()
    }
    fn check(journal: &Arc<NativePendingJournal>, credential: &SessionCredential,
        fence: &Arc<NativeRecoveryFence>) -> Arc<NativeMessageCheck> {
        Arc::new(NativeMessageCheck { journal: journal.clone(), fence: fence.clone(), stamp: journal.store.change_stamp().unwrap(),
            fingerprint: credential.recovery_fingerprint(),
            message: Mutex::new(Some(Message { request_id: "request_123".into(), text: "synthetic".into() })) })
    }
    #[test]
    fn cancellation_stale_views_and_credential_changes_cannot_claim() {
        let root = tempfile::tempdir().unwrap();
        let journal = NativePendingJournal::new("https://send.example.test".into(),root.path().to_str().unwrap().into()).unwrap();
        let credential = credential();
        let fence = NativeRecoveryFence::new(info(),"topic_a".into(),7).unwrap();
        let ticket = check(&journal,&credential,&fence);
        assert!(matches!(credential.claim_message_send(journal.clone(),ticket.clone(),info(),"topic_a".into(),8), Err(NativeSendError::ContextChanged)));
        let other = SessionCredential::from_server_cookie("https://send.example.test".into(),"https://send.example.test".into(),
            format!("__Secure-vcp_mobile={}; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600","C".repeat(43))).unwrap();
        assert!(matches!(other.claim_message_send(journal.clone(),ticket.clone(),info(),"topic_a".into(),7),Err(NativeSendError::CredentialMismatch)));
        fence.cancel();
        assert!(matches!(credential.claim_message_send(journal.clone(),ticket,info(),"topic_a".into(),7),Err(NativeSendError::Cancelled)));
        assert!(journal.store.list().unwrap().is_empty());
    }
    #[tokio::test]
    async fn single_claim_survives_cancel_and_reopen_without_automatic_post() {
        let root = tempfile::tempdir().unwrap();
        let journal = NativePendingJournal::new("https://send.example.test".into(),root.path().to_str().unwrap().into()).unwrap();
        let credential = credential(); let fence = NativeRecoveryFence::new(info(),"topic_a".into(),7).unwrap();
        let ticket = check(&journal,&credential,&fence);
        let dispatch = credential.claim_message_send(journal.clone(),ticket.clone(),info(),"topic_a".into(),7).unwrap();
        assert!(matches!(credential.claim_message_send(journal.clone(),ticket,info(),"topic_a".into(),7),Err(NativeSendError::AlreadyConsumed)));
        fence.cancel();
        assert!(matches!(credential.dispatch_message_once(dispatch).await,Err(NativeSendError::Cancelled)));
        drop(journal);
        let reopened = NativePendingJournal::new("https://send.example.test".into(),root.path().to_str().unwrap().into()).unwrap();
        assert_eq!(reopened.store.list().unwrap()[0].state,crate::pending::PendingState::AwaitingConfirmation);
        assert!(matches!(reopened.claim_original_message(&checked_session(&info(),"topic_a").unwrap(),"topic_a","different_123","different"),Err(NativeSendError::OutstandingPending)));
    }
}
