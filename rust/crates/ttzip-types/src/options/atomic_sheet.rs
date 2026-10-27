// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Lock-free, 128-byte cache-line aligned atomic progress sheet and cancellation probe.
//!
//! # Microarchitecture & Memory Invariants
//! High-throughput compression/decompression pipelines update progress counters on
//! performance-critical worker threads. Traditional cross-FFI callbacks or mutex-guarded
//! progress structures incur heavy lock contention, thread descheduling, and cache line
//! bouncing.
//!
//! `ProgressAtomicSheet` aligns to 128 bytes (matching Apple Silicon / ARMv8.5+ big-core
//! cache line topologies) and pads unused bytes, completely preventing false sharing.
//! Updates utilize single-instruction `Ordering::Release` stores (`stlr`), while cancellation
//! probes execute relaxed single-cycle loads (`ldrb`), achieving sub-nanosecond read latency
//! with zero lock overhead.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// POD representation of a progress snapshot taken at a single point in time.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    /// Number of bytes processed so far.
    pub processed_bytes: u64,
    /// Total bytes to process (0 if indeterminate/streaming).
    pub total_bytes: u64,
    /// Instantaneous transfer rate in bytes per second.
    pub instant_bps: u64,
    /// Whether cancellation has been triggered.
    pub is_cancelled: bool,
    /// Whether the operation has reached terminal completion.
    pub is_finished: bool,
}

impl ProgressSnapshot {
    /// Creates a new progress snapshot with the given values.
    #[inline]
    pub const fn new(
        processed_bytes: u64,
        total_bytes: u64,
        instant_bps: u64,
        is_cancelled: bool,
        is_finished: bool,
    ) -> Self {
        Self {
            processed_bytes,
            total_bytes,
            instant_bps,
            is_cancelled,
            is_finished,
        }
    }

    /// Creates an empty/initial snapshot.
    #[inline]
    pub const fn empty() -> Self {
        Self {
            processed_bytes: 0,
            total_bytes: 0,
            instant_bps: 0,
            is_cancelled: false,
            is_finished: false,
        }
    }

    /// Calculates completion fraction clamped in `[0.0, 1.0]`.
    /// Returns `0.0` if `total_bytes` is zero.
    #[inline]
    pub fn fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.processed_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
        }
    }

    /// Calculates completion percentage clamped in `[0.0, 100.0]`.
    #[inline]
    pub fn percentage(&self) -> f64 {
        self.fraction() * 100.0
    }

    /// Returns `true` if the operation is actively progressing (neither finished nor cancelled).
    #[inline]
    pub fn is_active(&self) -> bool {
        !self.is_finished && !self.is_cancelled
    }
}

impl Default for ProgressSnapshot {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

/// 128-byte cache-line aligned lock-free atomic progress sheet.
///
/// Designed to reside in shared memory or behind an `Arc`, allowing worker threads
/// to store progress without locking, and consumer/sampling threads (such as UI
/// DisplayLink / VSync dispatchers) to take atomic snapshots without impeding execution.
#[repr(align(128))]
pub struct ProgressAtomicSheet {
    /// Total bytes processed so far.
    pub processed_bytes: AtomicU64,
    /// Total expected payload bytes.
    pub total_bytes: AtomicU64,
    /// Instantaneous throughput in bytes per second.
    pub instant_bps: AtomicU64,
    /// Atomic cancellation flag polled by worker loops.
    pub cancel_flag: AtomicBool,
    /// Atomic completion flag indicating operation termination.
    pub is_finished: AtomicBool,
    /// Explicit padding to ensure the structure occupies exactly 128 bytes,
    /// preventing false sharing across CPU cores.
    /// 3 * 8 (AtomicU64) + 2 * 1 (AtomicBool) = 26 bytes.
    /// 128 - 26 = 102 bytes.
    _pad: [u8; 102],
}

impl ProgressAtomicSheet {
    /// Creates a new uncancelled, empty progress sheet.
    pub const fn new() -> Self {
        Self {
            processed_bytes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            instant_bps: AtomicU64::new(0),
            cancel_flag: AtomicBool::new(false),
            is_finished: AtomicBool::new(false),
            _pad: [0; 102],
        }
    }

    /// Creates a new progress sheet with preconfigured `total_bytes`.
    pub const fn with_total(total_bytes: u64) -> Self {
        Self {
            processed_bytes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(total_bytes),
            instant_bps: AtomicU64::new(0),
            cancel_flag: AtomicBool::new(false),
            is_finished: AtomicBool::new(false),
            _pad: [0; 102],
        }
    }

    /// Updates processed bytes and instantaneous transfer rate using release ordering.
    #[inline(always)]
    pub fn update_progress(&self, processed: u64, instant_bps: u64) {
        self.processed_bytes.store(processed, Ordering::Release);
        self.instant_bps.store(instant_bps, Ordering::Release);
    }

    /// Sets the total expected bytes using release ordering.
    #[inline(always)]
    pub fn set_total(&self, total: u64) {
        self.total_bytes.store(total, Ordering::Release);
    }

    /// Sets the instantaneous throughput in bytes per second using release ordering.
    #[inline(always)]
    pub fn set_instant_bps(&self, bps: u64) {
        self.instant_bps.store(bps, Ordering::Release);
    }

    /// Atomically increments the processed bytes counter and returns the new value.
    #[inline(always)]
    pub fn add_processed(&self, delta: u64) -> u64 {
        self.processed_bytes.fetch_add(delta, Ordering::Release) + delta
    }

    /// Marks the operation as terminal completed.
    #[inline(always)]
    pub fn mark_finished(&self) {
        self.is_finished.store(true, Ordering::Release);
    }

    /// Signals collaborative cancellation to all worker threads.
    #[inline(always)]
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Release);
    }

    /// Ultra-fast non-blocking probe to check if cancellation was requested.
    ///
    /// Uses `Ordering::Relaxed` to emit a single load instruction on ARM64 (`ldrb`)
    /// without pipeline barriers, ideal for inner decompression/compression loops.
    #[inline(always)]
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    /// Checks if the operation has been marked finished.
    #[inline(always)]
    pub fn is_finished(&self) -> bool {
        self.is_finished.load(Ordering::Relaxed)
    }

    /// Reads an instantaneous snapshot of all progress counters using acquire ordering.
    #[inline]
    pub fn read_snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            processed_bytes: self.processed_bytes.load(Ordering::Acquire),
            total_bytes: self.total_bytes.load(Ordering::Acquire),
            instant_bps: self.instant_bps.load(Ordering::Acquire),
            is_cancelled: self.cancel_flag.load(Ordering::Acquire),
            is_finished: self.is_finished.load(Ordering::Acquire),
        }
    }

    /// Resets all counters and flags back to initial states.
    #[inline]
    pub fn reset(&self) {
        self.processed_bytes.store(0, Ordering::Release);
        self.total_bytes.store(0, Ordering::Release);
        self.instant_bps.store(0, Ordering::Release);
        self.cancel_flag.store(false, Ordering::Release);
        self.is_finished.store(false, Ordering::Release);
    }
}

impl Default for ProgressAtomicSheet {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time static assertions ensuring 128-byte cache line alignment, size, and thread safety.
const _: () = {
    assert!(std::mem::align_of::<ProgressAtomicSheet>() == 128);
    assert!(std::mem::size_of::<ProgressAtomicSheet>() == 128);
    assert!(std::mem::size_of::<ProgressSnapshot>() <= 64);
};

const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
const _: () = {
    assert_send_sync_static::<ProgressAtomicSheet>();
    assert_send_sync_static::<ProgressSnapshot>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_atomic_sheet_alignment_and_size() {
        assert_eq!(std::mem::align_of::<ProgressAtomicSheet>(), 128);
        assert_eq!(std::mem::size_of::<ProgressAtomicSheet>(), 128);
        assert!(std::mem::size_of::<ProgressSnapshot>() <= 64);
    }

    #[test]
    fn test_atomic_sheet_lifecycle() {
        let sheet = ProgressAtomicSheet::with_total(1000);
        assert!(!sheet.is_cancelled());
        assert!(!sheet.is_finished());

        sheet.update_progress(500, 250_000);
        let snapshot = sheet.read_snapshot();
        assert_eq!(snapshot.processed_bytes, 500);
        assert_eq!(snapshot.total_bytes, 1000);
        assert_eq!(snapshot.instant_bps, 250_000);
        assert_eq!(snapshot.fraction(), 0.5);
        assert_eq!(snapshot.percentage(), 50.0);
        assert!(snapshot.is_active());

        sheet.mark_finished();
        assert!(sheet.is_finished());
        assert!(sheet.read_snapshot().is_finished);
        assert!(!sheet.read_snapshot().is_active());

        sheet.reset();
        assert_eq!(sheet.read_snapshot(), ProgressSnapshot::empty());
    }

    #[test]
    fn test_cancellation_probe_propagation() {
        let sheet = Arc::new(ProgressAtomicSheet::new());
        let sheet_clone = Arc::clone(&sheet);

        let handle = thread::spawn(move || {
            let mut iterations = 0u64;
            while !sheet_clone.is_cancelled() {
                iterations += 1;
                std::hint::spin_loop();
            }
            iterations
        });

        // Give the worker thread a moment to enter the spin loop.
        thread::sleep(Duration::from_millis(2));
        sheet.cancel();

        let iterations = handle.join().expect("Worker thread panicked");
        assert!(iterations > 0);
        assert!(sheet.is_cancelled());
        assert!(sheet.read_snapshot().is_cancelled);
    }

    #[test]
    fn test_high_concurrency_stress() {
        let sheet = Arc::new(ProgressAtomicSheet::with_total(100_000));
        let num_threads = 8;
        let iters_per_thread = 10_000;

        let mut handles = Vec::new();
        for _ in 0..num_threads {
            let sheet_producer = Arc::clone(&sheet);
            handles.push(thread::spawn(move || {
                for _ in 0..iters_per_thread {
                    sheet_producer.add_processed(1);
                    sheet_producer.set_instant_bps(10_000_000);
                }
            }));
        }

        // Concurrent reader thread polling snapshots.
        let sheet_reader = Arc::clone(&sheet);
        let reader_handle = thread::spawn(move || {
            let mut max_processed = 0;
            while !sheet_reader.is_finished() {
                let snap = sheet_reader.read_snapshot();
                if snap.processed_bytes > max_processed {
                    max_processed = snap.processed_bytes;
                }
                std::hint::spin_loop();
            }
            max_processed
        });

        for h in handles {
            h.join().expect("Producer thread panicked");
        }
        sheet.mark_finished();

        let max_read = reader_handle.join().expect("Reader thread panicked");
        let final_snapshot = sheet.read_snapshot();
        assert_eq!(
            final_snapshot.processed_bytes,
            (num_threads * iters_per_thread) as u64
        );
        assert!(max_read <= final_snapshot.processed_bytes);
        assert!(final_snapshot.is_finished);
    }

    #[test]
    fn test_single_producer_monotonic_progress() {
        let sheet = Arc::new(ProgressAtomicSheet::with_total(1_000_000));
        let sheet_producer = Arc::clone(&sheet);

        let producer_handle = thread::spawn(move || {
            for i in 1..=10_000 {
                sheet_producer.update_progress(i * 100, i * 50);
            }
            sheet_producer.mark_finished();
        });

        let sheet_reader = Arc::clone(&sheet);
        let reader_handle = thread::spawn(move || {
            let mut last_seen = 0;
            while !sheet_reader.is_finished() {
                let snap = sheet_reader.read_snapshot();
                assert!(snap.processed_bytes >= last_seen);
                last_seen = snap.processed_bytes;
                std::hint::spin_loop();
            }
        });

        producer_handle.join().expect("Producer panicked");
        reader_handle.join().expect("Reader panicked");
        let final_snap = sheet.read_snapshot();
        assert_eq!(final_snap.processed_bytes, 1_000_000);
        assert!(final_snap.is_finished);
    }

    #[test]
    fn test_snapshot_edge_cases() {
        let snap_zero = ProgressSnapshot::new(0, 0, 0, false, false);
        assert_eq!(snap_zero.fraction(), 0.0);
        assert_eq!(snap_zero.percentage(), 0.0);

        let snap_overflow = ProgressSnapshot::new(200, 100, 50, false, false);
        assert_eq!(snap_overflow.fraction(), 1.0);
        assert_eq!(snap_overflow.percentage(), 100.0);
    }
}
