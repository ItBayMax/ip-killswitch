//! IP detection result cache with TTL + subscription.
//!
//! Several call sites care about "what was the last IP verdict" but should
//! NOT each trigger a fresh HTTP probe — that's expensive (hundreds of ms,
//! sometimes seconds) and would race the scheduler's own probes. Instead the
//! scheduler is the *only* writer; everyone else reads from this cache.
//!
//! Two concepts on the read side:
//!
//! - **`current()`** — the latest verdict regardless of age. Use for UI
//!   that already conveys staleness in its presentation, or for fail-open
//!   audit logging.
//! - **`current_fresh(max_age)`** — `Some(verdict)` only if the cache is
//!   newer than `max_age`. Use for decision-critical paths
//!   (process-launch interception, firewall toggles) so a stale verdict
//!   doesn't drive a wrong action.
//!
//! Subscribers are notified after every `update()`. The notify happens
//! outside the lock so listener callbacks may re-enter the cache APIs.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::config::Schedule;
use crate::detector::DetectionReport;

type Listener = Arc<dyn Fn(&DetectionReport) + Send + Sync>;

#[derive(Clone, Default)]
pub struct VerdictCache {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    /// (report, when this entry was written)
    last: Option<(DetectionReport, Instant)>,
    listeners: Vec<Listener>,
}

impl VerdictCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the most recent verdict regardless of age. `None` if no
    /// detection has ever completed.
    pub fn current(&self) -> Option<DetectionReport> {
        self.inner.lock().last.as_ref().map(|(r, _)| r.clone())
    }

    /// Return the most recent verdict iff it's younger than `max_age`.
    /// `None` if the cache is empty OR if the entry is stale.
    // Used by the process-watcher and firewall modules added in subsequent
    // commits — annotated so the dead_code lint doesn't fire during the
    // landing of this commit alone.
    #[allow(dead_code)]
    pub fn current_fresh(&self, max_age: Duration) -> Option<DetectionReport> {
        let guard = self.inner.lock();
        let (report, at) = guard.last.as_ref()?;
        if at.elapsed() <= max_age {
            Some(report.clone())
        } else {
            None
        }
    }

    /// Replace the cached verdict with `report` and notify all subscribers.
    /// Callbacks fire outside the inner lock to keep re-entry safe.
    pub fn update(&self, report: DetectionReport) {
        let listeners = {
            let mut guard = self.inner.lock();
            guard.last = Some((report.clone(), Instant::now()));
            guard.listeners.clone()
        };
        for cb in listeners {
            cb(&report);
        }
    }

    /// Register a long-lived listener. Listeners live for the lifetime of
    /// the cache; there's intentionally no unsubscribe handle yet because
    /// our usage is "register once during app setup".
    #[allow(dead_code)] // wired up by the firewall commit
    pub fn subscribe<F>(&self, cb: F)
    where
        F: Fn(&DetectionReport) + Send + Sync + 'static,
    {
        self.inner.lock().listeners.push(Arc::new(cb));
    }
}

/// TTL derived from the user's configured schedule. The cache is naturally
/// refreshed on every scheduler tick, so anything younger than the tick
/// interval is fresh by definition; older entries deserve scrutiny.
///
/// - `Interval(N)`     → TTL = N seconds. Matches the user-controlled
///   detection cadence exactly.
/// - `Cron(_)`         → TTL = 5 minutes. Cron intervals vary; we don't
///   parse the expression. Users running cron mode usually expect coarser
///   updates anyway.
/// - `Disabled`        → TTL = effectively infinite. The cache only fills
///   when the user manually triggers a detection; treat whatever it holds
///   as authoritative until they re-check.
#[allow(dead_code)] // wired up by the process-watcher / firewall commits
pub fn ttl_for_schedule(schedule: &Schedule) -> Duration {
    match schedule {
        Schedule::Interval { seconds } => Duration::from_secs(*seconds),
        Schedule::Cron { .. } => Duration::from_secs(300),
        Schedule::Disabled => Duration::from_secs(u64::MAX / 2),
    }
}
