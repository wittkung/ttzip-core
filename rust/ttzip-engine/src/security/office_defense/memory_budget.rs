// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 5: Global Office Memory Budget & Viewport Watchdog.
//!
//! Enforces the TTZip systemic engineering invariant of <= 64 MB resident task memory
//! across workbook models, XML parsers, SST caches, and document body viewports.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    OfficeDefenseError, DEFAULT_MAX_DOCUMENT_BODY_BUDGET, DEFAULT_MAX_OFFICE_BUDGET,
    DEFAULT_MAX_SHEET_VIEWPORT_BUDGET,
};

/// Guard enforcing global and modular resident memory budget ceilings for Office tasks.
#[derive(Debug)]
pub struct OfficeMemoryBudgetGuard {
    current_usage: AtomicUsize,
    max_budget: usize,
    max_sheet_viewport: usize,
    max_document_body: usize,
}

impl Default for OfficeMemoryBudgetGuard {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_OFFICE_BUDGET,
            DEFAULT_MAX_SHEET_VIEWPORT_BUDGET,
            DEFAULT_MAX_DOCUMENT_BODY_BUDGET,
        )
    }
}

impl OfficeMemoryBudgetGuard {
    /// Creates a new memory budget guard with configured maximum bytes and viewports.
    pub const fn new(
        max_budget: usize,
        max_sheet_viewport: usize,
        max_document_body: usize,
    ) -> Self {
        Self {
            current_usage: AtomicUsize::new(0),
            max_budget,
            max_sheet_viewport,
            max_document_body,
        }
    }

    /// Atomically attempts to allocate a bounded block of memory, returning an RAII permit.
    pub fn allocate(&self, size: usize) -> Result<OfficeMemoryPermit<'_>, OfficeDefenseError> {
        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            let new_usage = current.saturating_add(size);
            if new_usage > self.max_budget {
                return Err(OfficeDefenseError::MemoryBudgetExceeded {
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
                Ok(_) => return Ok(OfficeMemoryPermit { guard: self, size }),
                Err(actual) => current = actual,
            }
        }
    }

    /// Validates whether a worksheet uncompressed size fits within the sheet viewport limit.
    pub fn validate_sheet_size(&self, size: usize) -> Result<(), OfficeDefenseError> {
        if size > self.max_sheet_viewport {
            Err(OfficeDefenseError::SheetExceedsViewportLimit {
                size,
                limit: self.max_sheet_viewport,
            })
        } else {
            Ok(())
        }
    }

    /// Validates whether a document body uncompressed size fits within the document limit.
    pub fn validate_document_size(&self, size: usize) -> Result<(), OfficeDefenseError> {
        if size > self.max_document_body {
            Err(OfficeDefenseError::DocumentExceedsLimit {
                size,
                limit: self.max_document_body,
            })
        } else {
            Ok(())
        }
    }

    /// Atomically reserves memory without returning an RAII permit.
    pub fn try_reserve(&self, size: usize) -> Result<(), OfficeDefenseError> {
        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            let new_usage = current.saturating_add(size);
            if new_usage > self.max_budget {
                return Err(OfficeDefenseError::MemoryBudgetExceeded {
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

    /// Atomically releases previously allocated memory bytes.
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
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Returns the currently allocated memory bytes.
    #[inline]
    pub fn allocated_bytes(&self) -> usize {
        self.current_usage.load(Ordering::Acquire)
    }

    /// Returns the maximum allowable global budget.
    #[inline]
    pub const fn max_budget(&self) -> usize {
        self.max_budget
    }

    /// Returns remaining available memory bytes in the budget.
    #[inline]
    pub fn available_bytes(&self) -> usize {
        self.max_budget.saturating_sub(self.allocated_bytes())
    }

    /// Resets the memory allocation watchdog back to zero.
    pub fn reset(&self) {
        self.current_usage.store(0, Ordering::Release);
    }
}

/// RAII memory permit that automatically releases allocated bytes upon drop.
#[derive(Debug)]
pub struct OfficeMemoryPermit<'a> {
    guard: &'a OfficeMemoryBudgetGuard,
    size: usize,
}

impl<'a> OfficeMemoryPermit<'a> {
    /// Returns the byte size tracked by this permit.
    #[inline]
    pub const fn size(&self) -> usize {
        self.size
    }
}

impl<'a> Drop for OfficeMemoryPermit<'a> {
    fn drop(&mut self) {
        self.guard.release(self.size);
    }
}
