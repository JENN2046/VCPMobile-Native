use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vcp_mobile_native::*;

fn request() -> ProbeRequest {
    ProbeRequest {
        label: "跨端 probe 🧭".into(),
        steps: 3,
        interval_ms: 0,
    }
}

type Events = Arc<Mutex<Vec<ProbeProgress>>>;

struct Recorder(Events);
impl ProbeListener for Recorder {
    fn on_progress(&self, progress: ProbeProgress) -> Result<(), ProbeError> {
        self.0.lock().unwrap().push(progress);
        Ok(())
    }
}
fn recorder() -> (Box<dyn ProbeListener>, Events) {
    let events = Arc::new(Mutex::new(Vec::new()));
    (Box::new(Recorder(events.clone())), events)
}

#[tokio::test]
async fn success_preserves_unicode_and_order() {
    let (listener, events) = recorder();
    let result = BridgeProbe::new().run(request(), listener).await.unwrap();
    assert_eq!(result.label, "跨端 probe 🧭");
    assert_eq!(result.completed, 3);
    assert_eq!(
        *events.lock().unwrap(),
        (1..=3)
            .map(|completed| ProbeProgress { completed, total: 3 })
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn invalid_inputs_emit_no_events() {
    for (label, steps, interval_ms) in [
        ("".to_owned(), 3, 0),
        ("  ".to_owned(), 3, 0),
        ("x".repeat(257), 3, 0),
        ("界".repeat(86), 3, 0),
        ("ok".to_owned(), 0, 0),
        ("ok".to_owned(), 129, 0),
        ("ok".to_owned(), 3, 1_001),
    ] {
        let (listener, events) = recorder();
        let error = BridgeProbe::new()
            .run(ProbeRequest { label, steps, interval_ms }, listener)
            .await
            .unwrap_err();
        assert!(matches!(error, ProbeError::InvalidInput { .. }));
        assert!(events.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn validation_failure_does_not_consume_instance() {
    let probe = BridgeProbe::new();
    let mut bad = request();
    bad.steps = 0;
    assert!(probe.run(bad, recorder().0).await.is_err());
    assert!(probe.run(request(), recorder().0).await.is_ok());
}

#[tokio::test]
async fn completed_probe_cannot_restart() {
    let probe = BridgeProbe::new();
    probe.run(request(), recorder().0).await.unwrap();
    assert_eq!(
        probe.run(request(), recorder().0).await.unwrap_err(),
        ProbeError::AlreadyStarted
    );
}

#[tokio::test]
async fn cancel_before_start_has_no_events() {
    let probe = BridgeProbe::new();
    probe.cancel();
    probe.cancel();
    let (listener, events) = recorder();
    assert_eq!(probe.run(request(), listener).await.unwrap_err(), ProbeError::Cancelled);
    assert!(events.lock().unwrap().is_empty());
}

struct CancelOnProgress {
    probe: Arc<BridgeProbe>,
    events: Arc<Mutex<Vec<ProbeProgress>>>,
}
impl ProbeListener for CancelOnProgress {
    fn on_progress(&self, progress: ProbeProgress) -> Result<(), ProbeError> {
        self.events.lock().unwrap().push(progress);
        self.probe.cancel();
        Ok(())
    }
}

#[tokio::test]
async fn cancel_from_callback_is_reentrant_and_final() {
    let probe = BridgeProbe::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let listener = Box::new(CancelOnProgress { probe: probe.clone(), events: events.clone() });
    assert_eq!(probe.run(request(), listener).await.unwrap_err(), ProbeError::Cancelled);
    assert_eq!(events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn cancellation_on_last_event_does_not_report_success() {
    let probe = BridgeProbe::new();
    let mut input = request();
    input.steps = 1;
    let listener = Box::new(CancelOnProgress {
        probe: probe.clone(),
        events: Arc::new(Mutex::new(Vec::new())),
    });
    assert_eq!(probe.run(input, listener).await.unwrap_err(), ProbeError::Cancelled);
}

struct FailingListener;
impl ProbeListener for FailingListener {
    fn on_progress(&self, _: ProbeProgress) -> Result<(), ProbeError> {
        Err(ProbeError::ListenerFailed)
    }
}
#[tokio::test]
async fn listener_failure_is_visible() {
    assert_eq!(
        BridgeProbe::new().run(request(), Box::new(FailingListener)).await.unwrap_err(),
        ProbeError::ListenerFailed
    );
}

#[tokio::test]
async fn independent_instances_do_not_share_cancellation() {
    let first = BridgeProbe::new();
    first.cancel();
    assert!(BridgeProbe::new().run(request(), recorder().0).await.is_ok());
}

#[tokio::test]
async fn concurrent_second_run_is_rejected() {
    let probe = BridgeProbe::new();
    let mut slow = request();
    slow.interval_ms = 1_000;
    let running = probe.run(slow, recorder().0);
    tokio::pin!(running);
    // Poll run exactly once into its first suspension point; no timing sleep.
    assert!(std::future::poll_fn(|cx| {
        assert!(running.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(true)
    }).await);
    assert_eq!(probe.run(request(), recorder().0).await.unwrap_err(), ProbeError::AlreadyStarted);
    probe.cancel();
    assert_eq!(running.await.unwrap_err(), ProbeError::Cancelled);
}

#[tokio::test]
async fn cancel_wakes_a_sleeping_probe() {
    let probe = BridgeProbe::new();
    let mut slow = request();
    slow.interval_ms = 1_000;
    let running = probe.run(slow, recorder().0);
    tokio::pin!(running);
    std::future::poll_fn(|cx| {
        assert!(running.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    }).await;
    probe.cancel();
    let result = tokio::time::timeout(Duration::from_millis(250), running).await.unwrap();
    assert_eq!(result.unwrap_err(), ProbeError::Cancelled);
}

#[tokio::test]
async fn boundary_values_are_supported() {
    let mut input = request();
    input.label = "x".repeat(256);
    input.steps = 128;
    let (listener, events) = recorder();
    assert_eq!(BridgeProbe::new().run(input, listener).await.unwrap().completed, 128);
    assert_eq!(events.lock().unwrap().len(), 128);
}
