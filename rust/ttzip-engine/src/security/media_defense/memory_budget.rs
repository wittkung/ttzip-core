// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Video Task Resident Memory Budget Watchdog and RAII Reservation Guard.
//!
//! Enforces deterministic resident memory ceilings (<= 64MB per video parsing/demuxing task)
//! across demuxer ring buffers, subtitle tracks, packet queues, and index tables,
//! preventing uncontrolled heap consumption and OOM crashes on constrained platforms.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{VideoDefenseError, DEFAULT_MAX_VIDEO_RESIDENT_MEMORY_BUDGET};

/// RAII memory reservation handle that automatically releases its quota on drop.
#[derive(Debug)]
pub struct VideoMemoryReservation {
    bytes: usize,
    counter: Arc<AtomicUsize>,
}

impl VideoMemoryReservation {
    /// Number of bytes held by this active reservation.
    #[inline]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Explicitly releases the memory reservation early before dropping.
    pub fn release(mut self) {
        if self.bytes > 0 {
            self.counter.fetch_sub(self.bytes, Ordering::Release);
            self.bytes = 0;
        }
    }
}

impl Drop for VideoMemoryReservation {
    fn drop(&mut self) {
        if self.bytes > 0 {
            self.counter.fetch_sub(self.bytes, Ordering::Release);
        }
    }
}

/// Task-level memory watchdog tracking transient and resident video allocations.
#[derive(Debug, Clone)]
pub struct VideoMemoryBudgetGuard {
    max_budget: usize,
    allocated: Arc<AtomicUsize>,
}

impl Default for VideoMemoryBudgetGuard {
    fn default() -> Self {
        Self {
            max_budget: DEFAULT_MAX_VIDEO_RESIDENT_MEMORY_BUDGET,
            allocated: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl VideoMemoryBudgetGuard {
    /// Creates a new watchdog with the specified byte budget.
    pub fn new(max_budget: usize) -> Self {
        Self {
            max_budget,
            allocated: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Attempts to reserve `bytes` against the task memory budget. Returns an RAII reservation guard on success.
    pub fn reserve(&self, bytes: usize) -> Result<VideoMemoryReservation, VideoDefenseError> {
        let mut current = self.allocated.load(Ordering::Acquire);
        loop {
            let next = match current.checked_add(bytes) {
                Some(n) => n,
                None => {
                    return Err(VideoDefenseError::MemoryBudgetExceeded {
                        allocated_bytes: usize::MAX,
                        budget_bytes: self.max_budget,
                    });
                }
            };

            if next > self.max_budget {
                return Err(VideoDefenseError::MemoryBudgetExceeded {
                    allocated_bytes: next,
                    budget_bytes: self.max_budget,
                });
            }

            match self.allocated.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(VideoMemoryReservation {
                        bytes,
                        counter: Arc::clone(&self.allocated),
                    });
                }
                Err(actual) => {
                    current = actual;
                }
            }
        }
    }

    /// Returns the currently allocated resident memory in bytes.
    #[inline]
    pub fn allocated(&self) -> usize {
        self.allocated.load(Ordering::Acquire)
    }

    /// Returns the maximum allowed memory budget in bytes.
    #[inline]
    pub const fn budget(&self) -> usize {
        self.max_budget
    }

    /// Returns the remaining unallocated memory budget in bytes.
    #[inline]
    pub fn available(&self) -> usize {
        let current = self.allocated();
        self.max_budget.saturating_sub(current)
    }

    /// Resets the internal allocation counter to zero.
    pub fn reset(&self) {
        self.allocated.store(0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_reservation_and_raii_release() {
        let guard = VideoMemoryBudgetGuard::new(1024 * 1024); // 1 MiB budget
        assert_eq!(guard.allocated(), 0);
        assert_eq!(guard.available(), 1024 * 1024);

        {
            let res1 = guard.reserve(512 * 1024).unwrap();
            assert_eq!(res1.bytes(), 512 * 1024);
            assert_eq!(guard.allocated(), 512 * 1024);
            assert_eq!(guard.available(), 512 * 1024);

            {
                let res2 = guard.reserve(256 * 1024).unwrap();
                assert_eq!(guard.allocated(), 768 * 1024);
                assert_eq!(guard.available(), 256 * 1024);

                // Attempt to reserve 512 KiB when only 256 KiB is available -> Error
                let err = guard.reserve(512 * 1024).unwrap_err();
                match err {
                    VideoDefenseError::MemoryBudgetExceeded { allocated_bytes, budget_bytes } => {
                        assert_eq!(allocated_bytes, (768 + 512) * 1024);
                        assert_eq!(budget_bytes, 1024 * 1024);
                    }
                    _ => panic!("Expected MemoryBudgetExceeded"),
                }

                // res2 dropped here
                drop(res2);
            }

            assert_eq!(guard.allocated(), 512 * 1024);
        }

        // res1 dropped here
        assert_eq!(guard.allocated(), 0);
        assert_eq!(guard.available(), 1024 * 1024);
    }

    #[test]
    fn test_explicit_release() {
        let guard = VideoMemoryBudgetGuard::new(1000);
        let res = guard.reserve(600).unwrap();
        assert_eq!(guard.allocated(), 600);

        res.release();
        assert_eq!(guard.allocated(), 0);
    }
}
