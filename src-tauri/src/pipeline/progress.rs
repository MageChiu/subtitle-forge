// ============================================================
// pipeline/progress.rs — Progress tracking utilities
// ============================================================

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Thread-safe progress tracker
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    inner: Arc<ProgressInner>,
}

#[derive(Debug)]
struct ProgressInner {
    total: AtomicU64,
    current: AtomicU64,
    cancelled: AtomicBool,
}

impl ProgressTracker {
    pub fn new(total: u64) -> Self {
        Self {
            inner: Arc::new(ProgressInner {
                total: AtomicU64::new(total),
                current: AtomicU64::new(0),
                cancelled: AtomicBool::new(false),
            }),
        }
    }

    pub fn set_current(&self, value: u64) {
        self.inner.current.store(value, Ordering::Relaxed);
    }

    pub fn increment(&self, delta: u64) {
        self.inner.current.fetch_add(delta, Ordering::Relaxed);
    }

    pub fn percent(&self) -> f32 {
        let total = self.inner.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let current = self.inner.current.load(Ordering::Relaxed);
        (current as f32 / total as f32) * 100.0
    }

    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Relaxed)
    }
}

/// Aggregate progress across multiple stages
#[derive(Debug, Clone, Serialize)]
pub struct AggregateProgress {
    pub overall_percent: f32,
    pub current_stage: String,
    pub stage_percent: f32,
    pub elapsed_secs: f64,
    pub eta_secs: Option<f64>,
}

impl AggregateProgress {
    /// Calculate overall progress with weighted stages
    pub fn calculate(
        stage_weights: &[(&str, f32)],
        current_stage_idx: usize,
        stage_percent: f32,
        elapsed_secs: f64,
    ) -> Self {
        let total_weight: f32 = stage_weights.iter().map(|(_, w)| w).sum();
        let completed_weight: f32 = stage_weights[..current_stage_idx]
            .iter()
            .map(|(_, w)| w)
            .sum();
        let current_weight = stage_weights
            .get(current_stage_idx)
            .map(|(_, w)| w)
            .unwrap_or(&0.0);

        let overall =
            (completed_weight + current_weight * stage_percent / 100.0) / total_weight * 100.0;

        // Estimate ETA
        let eta = if overall > 0.0 {
            Some(elapsed_secs * (100.0 - overall) as f64 / overall as f64)
        } else {
            None
        };

        Self {
            overall_percent: overall,
            current_stage: stage_weights
                .get(current_stage_idx)
                .map(|(name, _)| name.to_string())
                .unwrap_or_default(),
            stage_percent,
            elapsed_secs,
            eta_secs: eta,
        }
    }
}
