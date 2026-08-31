// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Cross-Language Asynchronous Progress & Cancellation Bridge.
//!
//! Provides thread-safe rate-limited progress dispatching, ETA computation,
//! and lock-free hierarchical cancellation tokens across FFI boundaries.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;

/// Cross-language asynchronous progress callback interface protocol implemented in Swift / Kotlin / Python.
#[uniffi::export(callback_interface)]
pub trait UniFFIProgressCallback: Send + Sync {
    /// Dispatches a progress event. Return `false` to request immediate operation cancellation.
    fn on_progress(&self, processed_bytes: u64, total_bytes: u64, current_entry: Option<String>) -> bool;
}

/// Thread-safe lock-free hierarchical cancellation token.
#[derive(uniffi::Object)]
pub struct UniFFICancellationToken {
    cancelled: AtomicBool,
    parent: Option<Arc<UniFFICancellationToken>>,
}

#[uniffi::export]
impl UniFFICancellationToken {
    /// Creates a new root cancellation token in the non-cancelled state.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cancelled: AtomicBool::new(false),
            parent: None,
        })
    }

    /// Creates a child cancellation token linked to this parent.
    ///
    /// If either the parent or the child is cancelled, the child reports cancelled.
    pub fn create_child(self: &Arc<Self>) -> Arc<UniFFICancellationToken> {
        Arc::new(UniFFICancellationToken {
            cancelled: AtomicBool::new(false),
            parent: Some(Arc::clone(self)),
        })
    }

    /// Triggers cancellation for this token (and any of its children).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Checks whether cancellation has been requested on this token or its ancestors.
    pub fn is_cancelled(&self) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            return true;
        }
        if let Some(ref p) = self.parent {
            if p.is_cancelled() {
                return true;
            }
        }
        false
    }

    /// Resets the cancellation flag back to active (false).
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }
}

/// Internal state for progress rate-limiting and ETA calculation.
struct ReporterState {
    last_dispatch: Instant,
    start_time: Instant,
    last_processed_bytes: u64,
}

/// High-throughput smooth progress reporter with adaptive throttling and throughput computation.
#[derive(uniffi::Object)]
pub struct UniFFIProgressReporter {
    total_bytes: AtomicU64,
    processed_bytes: AtomicU64,
    throttle_millis: u64,
    callback: Option<Box<dyn UniFFIProgressCallback>>,
    cancellation_token: Option<Arc<UniFFICancellationToken>>,
    state: Mutex<ReporterState>,
}

#[uniffi::export]
impl UniFFIProgressReporter {
    /// Creates a new progress reporter with specified byte quota, throttle interval, and optional callback.
    #[uniffi::constructor]
    pub fn new(
        total_bytes: u64,
        throttle_millis: u32,
        callback: Option<Box<dyn UniFFIProgressCallback>>,
        cancellation_token: Option<Arc<UniFFICancellationToken>>,
    ) -> Arc<Self> {
        let now = Instant::now();
        Arc::new(Self {
            total_bytes: AtomicU64::new(total_bytes),
            processed_bytes: AtomicU64::new(0),
            throttle_millis: throttle_millis as u64,
            callback,
            cancellation_token,
            state: Mutex::new(ReporterState {
                last_dispatch: now,
                start_time: now,
                last_processed_bytes: 0,
            }),
        })
    }

    /// Updates processed byte count and triggers throttled callback.
    ///
    /// Returns `true` if processing should continue, or `false` if cancelled.
    pub fn update(&self, processed: u64, current_entry: Option<String>) -> bool {
        if let Some(ref token) = self.cancellation_token {
            if token.is_cancelled() {
                return false;
            }
        }

        self.processed_bytes.store(processed, Ordering::Release);
        let total = self.total_bytes.load(Ordering::Acquire);

        if let Some(ref cb) = self.callback {
            let mut state = self.state.lock();
            let now = Instant::now();
            let elapsed_since_last = now.duration_since(state.last_dispatch).as_millis() as u64;

            let is_terminal = processed >= total && total > 0;
            if elapsed_since_last >= self.throttle_millis || is_terminal {
                state.last_dispatch = now;
                state.last_processed_bytes = processed;
                drop(state);

                let cont = cb.on_progress(processed, total, current_entry);
                if !cont {
                    if let Some(ref token) = self.cancellation_token {
                        token.cancel();
                    }
                    return false;
                }
            }
        }

        true
    }

    /// Advances processed byte count by a delta chunk.
    pub fn advance(&self, delta: u64, current_entry: Option<String>) -> bool {
        let new_val = self.processed_bytes.fetch_add(delta, Ordering::Relaxed) + delta;
        self.update(new_val, current_entry)
    }

    /// Computes current completion percentage in range [0.0, 100.0].
    pub fn percentage(&self) -> f64 {
        let total = self.total_bytes.load(Ordering::Acquire);
        if total == 0 {
            return 0.0;
        }
        let processed = self.processed_bytes.load(Ordering::Acquire);
        ((processed as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
    }

    /// Computes instantaneous throughput in megabytes per second (MB/s).
    pub fn throughput_mbs(&self) -> f64 {
        let state = self.state.lock();
        let elapsed = state.start_time.elapsed().as_secs_f64();
        if elapsed <= 0.001 {
            return 0.0;
        }
        let processed = self.processed_bytes.load(Ordering::Acquire);
        (processed as f64 / (1024.0 * 1024.0)) / elapsed
    }

    /// Computes estimated remaining time in seconds (ETA).
    pub fn estimated_remaining_seconds(&self) -> f64 {
        let total = self.total_bytes.load(Ordering::Acquire);
        let processed = self.processed_bytes.load(Ordering::Acquire);
        if processed == 0 || processed >= total {
            return 0.0;
        }

        let state = self.state.lock();
        let elapsed = state.start_time.elapsed().as_secs_f64();
        if elapsed <= 0.001 {
            return 0.0;
        }

        let bytes_per_sec = processed as f64 / elapsed;
        let remaining_bytes = (total - processed) as f64;
        remaining_bytes / bytes_per_sec
    }

    /// Checks if cancellation has occurred.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.as_ref().is_some_and(|t| t.is_cancelled())
    }

    /// Flushes final 100% completion event to the callback.
    pub fn finish(&self) {
        let total = self.total_bytes.load(Ordering::Acquire);
        self.processed_bytes.store(total, Ordering::Release);
        if let Some(ref cb) = self.callback {
            let _ = cb.on_progress(total, total, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct MockCallback {
        call_count: AtomicUsize,
        should_continue: AtomicBool,
    }

    impl MockCallback {
        fn new(should_cont: bool) -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                should_continue: AtomicBool::new(should_cont),
            }
        }
    }

    impl UniFFIProgressCallback for MockCallback {
        fn on_progress(&self, _processed: u64, _total: u64, _entry: Option<String>) -> bool {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            self.should_continue.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn test_cancellation_token_hierarchy() {
        let parent = UniFFICancellationToken::new();
        let child = parent.create_child();

        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());

        // Cancel parent, child should reflect cancelled
        parent.cancel();
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());

        // Reset parent
        parent.reset();
        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());

        // Cancel child independently
        child.cancel();
        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());
    }

    #[test]
    fn test_progress_reporter_metrics_and_callback() {
        let token = UniFFICancellationToken::new();
        let reporter = UniFFIProgressReporter::new(
            1000,
            0, // 0 throttle for testing
            Some(Box::new(MockCallback::new(true))),
            Some(Arc::clone(&token)),
        );

        assert_eq!(reporter.percentage(), 0.0);
        let cont = reporter.update(500, Some("file.txt".to_string()));
        assert!(cont);
        assert!((reporter.percentage() - 50.0).abs() < 0.001);

        let cont2 = reporter.advance(250, None);
        assert!(cont2);
        assert!((reporter.percentage() - 75.0).abs() < 0.001);

        reporter.finish();
        assert_eq!(reporter.percentage(), 100.0);
    }

    #[test]
    fn test_progress_reporter_cancellation_abort() {
        let token = UniFFICancellationToken::new();
        let reporter = UniFFIProgressReporter::new(
            1000,
            0,
            Some(Box::new(MockCallback::new(false))), // Aborts immediately
            Some(Arc::clone(&token)),
        );

        let cont = reporter.update(100, None);
        assert!(!cont, "Callback requesting stop must return false");
        assert!(reporter.is_cancelled());
        assert!(token.is_cancelled());
    }
}
