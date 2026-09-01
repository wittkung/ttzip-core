// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 5: Global E-book Memory Budget & Viewport Stream Watchdog.
//!
//! Enforces the TTZip systemic engineering invariant of <= 64 MB resident memory per task
//! across manifest indices, viewport chapter decodes, raster images, and font caches.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    EbookDefenseError, DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET, DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
};

/// Guard enforcing global and modular resident memory budget ceilings for e-book tasks.
#[derive(Debug)]
pub struct EbookMemoryBudgetGuard {
    current_usage: AtomicUsize,
    max_budget: usize,
    max_chapter_viewport: usize,
}

impl Default for EbookMemoryBudgetGuard {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
            DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET,
        )
    }
}

impl EbookMemoryBudgetGuard {
    /// Creates a new memory budget guard with configured maximum bytes and chapter limits.
    pub const fn new(max_budget: usize, max_chapter_viewport: usize) -> Self {
        Self {
            current_usage: AtomicUsize::new(0),
            max_budget,
            max_chapter_viewport,
        }
    }

    /// Atomically attempts to allocate a bounded block of memory, returning a RAII permit.
    pub fn allocate(&self, size: usize) -> Result<MemoryPermit<'_>, EbookDefenseError> {
        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            let new_usage = current.saturating_add(size);
            if new_usage > self.max_budget {
                return Err(EbookDefenseError::MemoryBudgetExceeded {
                    requested: size,
                    current_allocated: current,
                    limit: self.max_budget,
                });
            }

            match self.current_usage.compare_exchange_weak(
                current,
                new_usage,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(MemoryPermit { guard: self, size }),
                Err(actual) => current = actual,
            }
        }
    }

    /// Validates whether a chapter uncompressed size fits within the single-chapter viewport limit.
    pub fn validate_chapter_size(&self, size: usize) -> Result<(), EbookDefenseError> {
        if size > self.max_chapter_viewport {
            Err(EbookDefenseError::ChapterExceedsViewportLimit {
                size,
                limit: self.max_chapter_viewport,
            })
        } else {
            Ok(())
        }
    }

    /// Atomically reserves memory without returning a RAII permit.
    pub fn try_reserve(&self, size: usize) -> Result<(), EbookDefenseError> {
        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            let new_usage = current.saturating_add(size);
            if new_usage > self.max_budget {
                return Err(EbookDefenseError::MemoryBudgetExceeded {
                    requested: size,
                    current_allocated: current,
                    limit: self.max_budget,
                });
            }

            match self.current_usage.compare_exchange_weak(
                current,
                new_usage,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => current = actual,
            }
        }
    }

    /// Atomically releases previously reserved memory.
    pub fn release(&self, size: usize) {
        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            let new_usage = current.saturating_sub(size);
            match self.current_usage.compare_exchange_weak(
                current,
                new_usage,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Returns the currently allocated resident memory in bytes.
    #[inline]
    pub fn current_bytes(&self) -> usize {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Returns the remaining available memory budget in bytes.
    #[inline]
    pub fn remaining_bytes(&self) -> usize {
        let current = self.current_usage.load(Ordering::Relaxed);
        self.max_budget.saturating_sub(current)
    }

    /// Returns the configured maximum global budget ceiling in bytes.
    #[inline]
    pub fn max_budget(&self) -> usize {
        self.max_budget
    }

    /// Returns the configured maximum chapter viewport size in bytes.
    #[inline]
    pub fn max_chapter_viewport(&self) -> usize {
        self.max_chapter_viewport
    }

    /// Resets the memory usage counter to zero.
    #[inline]
    pub fn reset(&self) {
        self.current_usage.store(0, Ordering::Release);
    }
}

/// A RAII permit that releases allocated memory upon Drop.
#[derive(Debug)]
pub struct MemoryPermit<'a> {
    guard: &'a EbookMemoryBudgetGuard,
    size: usize,
}

impl<'a> MemoryPermit<'a> {
    /// Returns the allocated byte size associated with this permit.
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }
}

impl<'a> Drop for MemoryPermit<'a> {
    fn drop(&mut self) {
        self.guard.release(self.size);
    }
}
