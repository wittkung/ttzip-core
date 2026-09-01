// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Memory budget watchdog and RAII quota reservation guard.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{ImageDefenseError, DEFAULT_MAX_RESIDENT_MEMORY_BUDGET};

/// RAII reservation handle that releases memory quota back to the watchdog on drop.
#[derive(Debug)]
pub struct MemoryReservation {
    bytes: usize,
    counter: Arc<AtomicUsize>,
}

impl MemoryReservation {
    /// Number of bytes held by this reservation.
    #[inline]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Explicitly releases the memory reservation ahead of drop.
    pub fn release(mut self) {
        if self.bytes > 0 {
            self.counter.fetch_sub(self.bytes, Ordering::Release);
            self.bytes = 0;
        }
    }
}

impl Drop for MemoryReservation {
    fn drop(&mut self) {
        if self.bytes > 0 {
            self.counter.fetch_sub(self.bytes, Ordering::Release);
        }
    }
}

/// Task-level memory watchdog enforcing an upper ceiling on transient and resident allocations.
#[derive(Debug, Clone)]
pub struct MemoryBudgetWatchdog {
    max_budget: usize,
    allocated: Arc<AtomicUsize>,
}

impl Default for MemoryBudgetWatchdog {
    fn default() -> Self {
        Self {
            max_budget: DEFAULT_MAX_RESIDENT_MEMORY_BUDGET,
            allocated: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl MemoryBudgetWatchdog {
    /// Creates a new watchdog with the specified maximum byte quota.
    pub fn new(max_budget: usize) -> Self {
        Self {
            max_budget,
            allocated: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Attempts to reserve `bytes` against the task budget. Returns an RAII reservation guard on success.
    pub fn reserve(&self, bytes: usize) -> Result<MemoryReservation, ImageDefenseError> {
        let mut current = self.allocated.load(Ordering::Acquire);
        loop {
            let next = match current.checked_add(bytes) {
                Some(n) => n,
                None => {
                    return Err(ImageDefenseError::MemoryBudgetExceeded {
                        allocated_bytes: usize::MAX,
                        budget_bytes: self.max_budget,
                    });
                }
            };

            if next > self.max_budget {
                return Err(ImageDefenseError::MemoryBudgetExceeded {
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
                    return Ok(MemoryReservation {
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

    /// Returns currently allocated bytes.
    #[inline]
    pub fn current_allocated(&self) -> usize {
        self.allocated.load(Ordering::Acquire)
    }

    /// Returns the maximum allowed budget ceiling.
    #[inline]
    pub fn max_budget(&self) -> usize {
        self.max_budget
    }

    /// Returns remaining available memory before tripping the budget limit.
    #[inline]
    pub fn remaining_budget(&self) -> usize {
        self.max_budget
            .saturating_sub(self.allocated.load(Ordering::Acquire))
    }

    /// Resets the current allocated count to zero.
    pub fn reset(&self) {
        self.allocated.store(0, Ordering::Release);
    }
}
