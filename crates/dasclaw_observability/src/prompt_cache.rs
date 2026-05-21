//! Prompt cache hit/miss monitor.
//!
//! Tracks cache_read_tokens vs total_input_tokens across LLM requests,
//! emitting `PromptCache` events and periodic `PromptCacheHitRate` metrics.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tracing::{info, warn};

use super::traits::{Observer, ObserverEvent, ObserverMetric};

const STATS_LOG_INTERVAL: u64 = 10;

/// Thread-safe prompt cache monitor.
///
/// Accumulates per-request cache token stats and computes a running hit rate.
/// Emits structured events via the provided `Observer`.
pub struct PromptCacheMonitor {
    observer: Arc<dyn Observer>,
    total_requests: AtomicU64,
    total_input_tokens: AtomicU64,
    total_cache_read_tokens: AtomicU64,
    total_cache_creation_tokens: AtomicU64,
}

impl PromptCacheMonitor {
    pub fn new(observer: Arc<dyn Observer>) -> Self {
        Self {
            observer,
            total_requests: AtomicU64::new(0),
            total_input_tokens: AtomicU64::new(0),
            total_cache_read_tokens: AtomicU64::new(0),
            total_cache_creation_tokens: AtomicU64::new(0),
        }
    }

    /// Record token usage from a single LLM response.
    pub fn record(
        &self,
        input_tokens: u32,
        cache_read_tokens: u32,
        cache_creation_tokens: u32,
        static_layer_changed: bool,
    ) {
        let req_no = self.total_requests.fetch_add(1, Ordering::Relaxed) + 1;
        self.total_input_tokens
            .fetch_add(u64::from(input_tokens), Ordering::Relaxed);
        self.total_cache_read_tokens
            .fetch_add(u64::from(cache_read_tokens), Ordering::Relaxed);
        self.total_cache_creation_tokens
            .fetch_add(u64::from(cache_creation_tokens), Ordering::Relaxed);

        self.observer.record_event(&ObserverEvent::PromptCache {
            cache_read_tokens,
            cache_creation_tokens,
            total_input_tokens: input_tokens,
            static_layer_changed,
        });

        if static_layer_changed {
            warn!("Prompt static layer changed — cache will be invalidated for next request");
        }

        if req_no.is_multiple_of(STATS_LOG_INTERVAL) {
            self.log_stats(req_no);
        }
    }

    /// Compute the cumulative cache hit rate (0.0–1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_input_tokens.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let read = self.total_cache_read_tokens.load(Ordering::Relaxed);
        read as f64 / total as f64
    }

    /// Get a snapshot of totals.
    pub fn snapshot(&self) -> PromptCacheSnapshot {
        PromptCacheSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_input_tokens: self.total_input_tokens.load(Ordering::Relaxed),
            total_cache_read_tokens: self.total_cache_read_tokens.load(Ordering::Relaxed),
            total_cache_creation_tokens: self.total_cache_creation_tokens.load(Ordering::Relaxed),
            hit_rate: self.hit_rate(),
        }
    }

    fn log_stats(&self, req_no: u64) {
        let snap = self.snapshot();
        let hit_pct = snap.hit_rate * 100.0;

        self.observer
            .record_metric(&ObserverMetric::PromptCacheHitRate(snap.hit_rate));

        info!(
            total_requests = req_no,
            total_input_tokens = snap.total_input_tokens,
            cache_read_tokens = snap.total_cache_read_tokens,
            cache_creation_tokens = snap.total_cache_creation_tokens,
            hit_rate_pct = format!("{hit_pct:.1}"),
            "Prompt cache statistics"
        );
    }
}

#[derive(Debug, Clone)]
pub struct PromptCacheSnapshot {
    pub total_requests: u64,
    pub total_input_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopObserver;

    fn make_monitor() -> PromptCacheMonitor {
        PromptCacheMonitor::new(Arc::new(NoopObserver))
    }

    #[test]
    fn hit_rate_zero_when_no_requests() {
        let m = make_monitor();
        assert!((m.hit_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hit_rate_after_single_request() {
        let m = make_monitor();
        m.record(1000, 800, 200, false);
        let rate = m.hit_rate();
        assert!((rate - 0.8).abs() < 0.001);
    }

    #[test]
    fn hit_rate_accumulates_across_requests() {
        let m = make_monitor();
        m.record(1000, 900, 100, false); // 90%
        m.record(1000, 700, 100, false); // 70%
                                         // cumulative: 1600 read / 2000 total = 80%
        let rate = m.hit_rate();
        assert!((rate - 0.8).abs() < 0.001);
    }

    #[test]
    fn static_layer_change_recorded() {
        let m = make_monitor();
        m.record(500, 0, 500, true);
        let snap = m.snapshot();
        assert_eq!(snap.total_requests, 1);
        assert_eq!(snap.total_cache_creation_tokens, 500);
        assert_eq!(snap.total_cache_read_tokens, 0);
    }

    #[test]
    fn snapshot_returns_consistent_data() {
        let m = make_monitor();
        m.record(100, 80, 20, false);
        m.record(200, 150, 50, false);
        let snap = m.snapshot();
        assert_eq!(snap.total_requests, 2);
        assert_eq!(snap.total_input_tokens, 300);
        assert_eq!(snap.total_cache_read_tokens, 230);
        assert_eq!(snap.total_cache_creation_tokens, 70);
        assert!((snap.hit_rate - 230.0 / 300.0).abs() < 0.001);
    }
}
