// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio Memory Budget Watchdog and RAII Reservation Guard.
//!
//! Enforces deterministic resident memory ceilings (<= 64MB per audio task)
//! across stream buffers, PCM ring buffers, and decoded audio samples,
//! preventing out-of-memory crashes on resource-constrained devices.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{AudioDefenseError, DEFAULT_MAX_AUDIO_RESIDENT_MEMORY_BUDGET};

/// RAII reservation handle that releases allocated audio memory quota on drop.
#[derive(Debug)]
pub struct AudioMemoryReservation {
    bytes: usize,
    counter: Arc<AtomicUsize>,
}

impl AudioMemoryReservation {
    /// Number of bytes held by this reservation.
    #[inline]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Explicitly releases the memory reservation before dropping.
    pub fn release(mut self) {
        if self.bytes > 0 {
            self.counter.fetch_sub(self.bytes, Ordering::Release);
            self.bytes = 0;
        }
    }
}

impl Drop for AudioMemoryReservation {
    fn drop(&mut self) {
        if self.bytes > 0 {
            self.counter.fetch_sub(self.bytes, Ordering::Release);
        }
    }
}

/// Task-level memory watchdog tracking transient and resident audio allocations.
#[derive(Debug, Clone)]
pub struct AudioMemoryBudgetGuard {
    max_budget: usize,
    allocated: Arc<AtomicUsize>,
}

impl Default for AudioMemoryBudgetGuard {
    fn default() -> Self {
        Self {
            max_budget: DEFAULT_MAX_AUDIO_RESIDENT_MEMORY_BUDGET,
            allocated: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl AudioMemoryBudgetGuard {
    /// Creates a new watchdog with the specified byte budget.
    pub fn new(max_budget: usize) -> Self {
        Self {
            max_budget,
            allocated: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Attempts to reserve `bytes` against the task memory budget. Returns an RAII reservation guard on success.
    pub fn reserve(&self, bytes: usize) -> Result<AudioMemoryReservation, AudioDefenseError> {
        let mut current = self.allocated.load(Ordering::Acquire);
        loop {
            let next = match current.checked_add(bytes) {
                Some(n) => n,
                None => {
                    return Err(AudioDefenseError::MemoryBudgetExceeded {
                        allocated_bytes: usize::MAX,
                        budget_bytes: self.max_budget,
                    });
                }
            };

            if next > self.max_budget {
                return Err(AudioDefenseError::MemoryBudgetExceeded {
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
                    return Ok(AudioMemoryReservation {
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

    /// Returns the currently allocated byte volume.
    #[inline]
    pub fn current_allocated(&self) -> usize {
        self.allocated.load(Ordering::Acquire)
    }

    /// Returns the maximum allowed budget ceiling in bytes.
    #[inline]
    pub const fn max_budget(&self) -> usize {
        self.max_budget
    }

    /// Returns remaining available memory before tripping the budget limit.
    #[inline]
    pub fn remaining_budget(&self) -> usize {
        self.max_budget
            .saturating_sub(self.allocated.load(Ordering::Acquire))
    }

    /// Resets the current allocated counter to zero.
    pub fn reset(&self) {
        self.allocated.store(0, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_budget_reservation_and_raii_drop() {
        let watchdog = AudioMemoryBudgetGuard::new(1024);
        assert_eq!(watchdog.remaining_budget(), 1024);
        assert_eq!(watchdog.current_allocated(), 0);

        {
            let res1 = watchdog.reserve(512).unwrap();
            assert_eq!(res1.bytes(), 512);
            assert_eq!(watchdog.current_allocated(), 512);
            assert_eq!(watchdog.remaining_budget(), 512);

            let res2 = watchdog.reserve(256).unwrap();
            assert_eq!(res2.bytes(), 256);
            assert_eq!(watchdog.current_allocated(), 768);

            // Exceeds remaining 256 bytes
            let err = watchdog.reserve(300).unwrap_err();
            assert_eq!(
                err,
                AudioDefenseError::MemoryBudgetExceeded {
                    allocated_bytes: 1068,
                    budget_bytes: 1024
                }
            );
        }

        // RAII drop should restore entire quota
        assert_eq!(watchdog.current_allocated(), 0);
        assert_eq!(watchdog.remaining_budget(), 1024);
    }

    #[test]
    fn test_explicit_release() {
        let watchdog = AudioMemoryBudgetGuard::new(1024);
        let res = watchdog.reserve(500).unwrap();
        assert_eq!(watchdog.current_allocated(), 500);

        res.release();
        assert_eq!(watchdog.current_allocated(), 0);
    }

    #[test]
    fn test_concurrent_reservations() {
        use std::thread;

        let watchdog = AudioMemoryBudgetGuard::new(100_000);
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let wd = watchdog.clone();
                thread::spawn(move || {
                    let mut held = Vec::new();
                    for _ in 0..100 {
                        if let Ok(res) = wd.reserve(50) {
                            held.push(res);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // All threads finished, reservations dropped
        assert_eq!(watchdog.current_allocated(), 0);
    }
}
