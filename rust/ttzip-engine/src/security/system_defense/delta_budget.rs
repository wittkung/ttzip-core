// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Binary Delta Memory Budget & Resource Circuit Breaker Guard (`BinaryDeltaMemoryBudgetGuard`).
//!
//! Enforces deterministic resource ceilings, decompression explosion tripwires,
//! instruction count limits, and single-task resident memory watchdogs for binary delta patching.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::SystemDefenseError;

/// Default maximum allowable single task resident memory budget for delta operations (64 MiB).
pub const DEFAULT_MAX_DELTA_MEMORY_BUDGET: usize = 64 * 1024 * 1024;

/// Default maximum allowable input delta patch file size (512 MiB).
pub const DEFAULT_MAX_DELTA_PATCH_SIZE: usize = 512 * 1024 * 1024;

/// Default maximum allowable decompression expansion ratio (1,000x).
pub const DEFAULT_MAX_DELTA_EXPANSION_RATIO: usize = 1000;

/// Default maximum allowable control triplet instruction quota (100,000).
pub const DEFAULT_MAX_DELTA_INSTRUCTIONS: usize = 100_000;

/// Configuration options for binary delta memory and resource budgeting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryDeltaBudgetOptions {
    /// Maximum allowable resident memory budget in bytes.
    pub max_memory_budget: usize,
    /// Maximum allowable compressed patch file size in bytes.
    pub max_patch_size: usize,
    /// Maximum allowable expansion ratio (uncompressed size / compressed patch size).
    pub max_expansion_ratio: usize,
    /// Maximum allowable control instructions before tripping circuit breaker.
    pub max_instructions: usize,
}

impl Default for BinaryDeltaBudgetOptions {
    fn default() -> Self {
        Self {
            max_memory_budget: DEFAULT_MAX_DELTA_MEMORY_BUDGET,
            max_patch_size: DEFAULT_MAX_DELTA_PATCH_SIZE,
            max_expansion_ratio: DEFAULT_MAX_DELTA_EXPANSION_RATIO,
            max_instructions: DEFAULT_MAX_DELTA_INSTRUCTIONS,
        }
    }
}

/// RAII Permit tracking allocated memory against a `BinaryDeltaMemoryBudgetGuard`.
#[derive(Debug)]
pub struct DeltaMemoryPermit {
    allocated_bytes: usize,
    counter: Arc<AtomicUsize>,
}

impl DeltaMemoryPermit {
    /// Returns the number of bytes held by this permit.
    #[inline]
    #[must_use]
    pub const fn allocated_bytes(&self) -> usize {
        self.allocated_bytes
    }
}

impl Drop for DeltaMemoryPermit {
    fn drop(&mut self) {
        if self.allocated_bytes > 0 {
            self.counter.fetch_sub(self.allocated_bytes, Ordering::Relaxed);
        }
    }
}

/// Watchdog and resource circuit breaker for binary delta patching.
#[derive(Debug, Clone)]
pub struct BinaryDeltaMemoryBudgetGuard {
    options: BinaryDeltaBudgetOptions,
    current_allocated: Arc<AtomicUsize>,
}

impl BinaryDeltaMemoryBudgetGuard {
    /// Creates a new guard with specified options.
    #[inline]
    #[must_use]
    pub fn new(options: BinaryDeltaBudgetOptions) -> Self {
        Self {
            options,
            current_allocated: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Creates a guard with default options (64MB budget, 512MB max patch, 1000x ratio, 100k instructions).
    #[inline]
    #[must_use]
    pub fn with_default_budget() -> Self {
        Self::new(BinaryDeltaBudgetOptions::default())
    }

    /// Returns the active configuration options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &BinaryDeltaBudgetOptions {
        &self.options
    }

    /// Returns the currently allocated memory bytes under this guard.
    #[inline]
    #[must_use]
    pub fn current_usage(&self) -> usize {
        self.current_allocated.load(Ordering::Relaxed)
    }

    /// Validates the input patch file size against maximum allowable threshold.
    pub fn validate_patch_size(&self, patch_size: usize) -> Result<(), SystemDefenseError> {
        if patch_size > self.options.max_patch_size {
            return Err(SystemDefenseError::DeltaPatchSizeExceeded {
                size: patch_size,
                max_size: self.options.max_patch_size,
            });
        }
        Ok(())
    }

    /// Validates the patch expansion ratio against zip bomb / decompression explosion ceilings.
    pub fn validate_expansion_ratio(
        &self,
        patch_size: usize,
        uncompressed_size: usize,
    ) -> Result<(), SystemDefenseError> {
        // Enforce absolute memory budget limit
        if uncompressed_size > self.options.max_memory_budget {
            return Err(SystemDefenseError::DeltaMemoryBudgetExceeded {
                allocated: uncompressed_size,
                max_budget: self.options.max_memory_budget,
            });
        }

        // For non-zero patch sizes, evaluate expansion multiplier
        if patch_size > 0 {
            let ratio = uncompressed_size / patch_size;
            if ratio > self.options.max_expansion_ratio {
                return Err(SystemDefenseError::DeltaExpansionRatioExceeded {
                    ratio,
                    max_ratio: self.options.max_expansion_ratio,
                });
            }
        }

        Ok(())
    }

    /// Validates and reserves memory within the watchdog budget, returning an RAII permit.
    pub fn acquire_permit(&self, bytes: usize) -> Result<DeltaMemoryPermit, SystemDefenseError> {
        let prev = self.current_allocated.fetch_add(bytes, Ordering::SeqCst);
        let new_total = prev.saturating_add(bytes);

        if new_total > self.options.max_memory_budget {
            self.current_allocated.fetch_sub(bytes, Ordering::SeqCst);
            return Err(SystemDefenseError::DeltaMemoryBudgetExceeded {
                allocated: new_total,
                max_budget: self.options.max_memory_budget,
            });
        }

        Ok(DeltaMemoryPermit {
            allocated_bytes: bytes,
            counter: Arc::clone(&self.current_allocated),
        })
    }

    /// Validates the instruction count against the circuit breaker threshold.
    pub fn validate_instruction_count(&self, count: usize) -> Result<(), SystemDefenseError> {
        if count > self.options.max_instructions {
            return Err(SystemDefenseError::DeltaInstructionQuotaExceeded {
                count,
                max_quota: self.options.max_instructions,
            });
        }
        Ok(())
    }
}
