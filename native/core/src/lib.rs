//! Shared client building blocks. BridgeProbe is an isolated M1 diagnostic;
//! pending is bounded local storage; recovery checks submission receipts.
//! None dispatches network requests, calls models, or grants server authority.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::Notify;

uniffi::setup_scaffolding!();

pub mod pending;
pub mod recovery;
pub mod portal_contract;
pub mod submission_flow;
pub mod transport_policy;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum ProbeError {
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("a probe instance may only run once")]
    AlreadyStarted,
    #[error("probe was cancelled")]
    Cancelled,
    #[error("progress listener failed")]
    ListenerFailed,
}

impl From<uniffi::UnexpectedUniFFICallbackError> for ProbeError {
    fn from(_: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::ListenerFailed
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProbeRequest {
    pub label: String,
    pub steps: u32,
    pub interval_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProbeProgress {
    pub completed: u32,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProbeResult {
    pub label: String,
    pub completed: u32,
}

#[uniffi::export(callback_interface)]
pub trait ProbeListener: Send + Sync {
    fn on_progress(&self, progress: ProbeProgress) -> Result<(), ProbeError>;
}

#[derive(Debug, uniffi::Object)]
pub struct BridgeProbe {
    started: AtomicBool,
    cancelled: AtomicBool,
    wake: Notify,
}

#[uniffi::export]
pub fn core_info() -> String {
    "VCPMobile M1 bridge probe 0.1.0; NativeSync not connected".to_owned()
}

#[uniffi::export(async_runtime = "tokio")]
impl BridgeProbe {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            started: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            wake: Notify::new(),
        })
    }

    /// Cancels only this local diagnostic, never an AA task or remote request.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.wake.notify_one();
    }

    pub async fn run(
        &self,
        request: ProbeRequest,
        listener: Box<dyn ProbeListener>,
    ) -> Result<ProbeResult, ProbeError> {
        validate(&request)?;
        self.check_cancelled()?;
        self.started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ProbeError::AlreadyStarted)?;
        for completed in 1..=request.steps {
            self.check_cancelled()?;
            if request.interval_ms > 0 {
                tokio::select! {
                    biased;
                    _ = self.wake.notified() => self.check_cancelled()?,
                    _ = tokio::time::sleep(Duration::from_millis(u64::from(request.interval_ms))) => {}
                }
            } else {
                tokio::task::yield_now().await;
            }
            self.check_cancelled()?;
            // No mutex is held across foreign code. Reentrant cancel is supported.
            listener.on_progress(ProbeProgress {
                completed,
                total: request.steps,
            })?;
        }
        self.check_cancelled()?;
        Ok(ProbeResult {
            label: request.label,
            completed: request.steps,
        })
    }
}

impl BridgeProbe {
    fn check_cancelled(&self) -> Result<(), ProbeError> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(ProbeError::Cancelled)
        } else {
            Ok(())
        }
    }
}

fn validate(request: &ProbeRequest) -> Result<(), ProbeError> {
    if request.label.trim().is_empty() || request.label.len() > 256 {
        return Err(ProbeError::InvalidInput {
            reason: "label must contain text and be at most 256 UTF-8 bytes".to_owned(),
        });
    }
    if !(1..=128).contains(&request.steps) {
        return Err(ProbeError::InvalidInput {
            reason: "steps must be between 1 and 128".to_owned(),
        });
    }
    if request.interval_ms > 1_000 {
        return Err(ProbeError::InvalidInput {
            reason: "interval_ms must be at most 1000".to_owned(),
        });
    }
    Ok(())
}
