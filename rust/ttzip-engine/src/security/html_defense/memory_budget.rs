// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 5: HTML Resident Memory Budget Watchdog & Truncation Guard.
//!
//! Enforces the TTZip systemic engineering invariant of <= 64 MiB resident memory per task:
//! - Hard resident memory allocation ceiling <= 64 MiB (67,108,864 bytes)
//! - Safe preview truncation threshold at 50 MiB (52,428,800 bytes) with security notice banner
//! - RAII `HtmlMemoryPermit` for deterministic resource release and leak prevention.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    HtmlDefenseError, DEFAULT_HTML_TRUNCATION_THRESHOLD, DEFAULT_MAX_HTML_MEMORY_BUDGET,
};

/// Prominent security notice banner injected when HTML content exceeds 50 MiB threshold.
pub const HTML_TRUNCATION_BANNER: &str = r#"<div id="ttzip-security-truncated-banner" style="background-color:#fff3cd;color:#856404;border:1px solid #ffeeba;padding:12px 16px;margin:12px 0;font-family:system-ui,-apple-system,sans-serif;font-size:13px;line-height:1.5;border-radius:6px;box-shadow:0 1px 3px rgba(0,0,0,0.08);"><strong>[TTZip Security Notice]</strong> This HTML preview exceeded the 50 MiB resident memory budget threshold and was safely truncated to protect system performance.</div>"#;

/// RAII memory permit that automatically decrements the memory watchdog upon drop.
#[derive(Debug)]
pub struct HtmlMemoryPermit<'a> {
    guard: &'a HtmlMemoryBudgetGuard,
    size: usize,
}

impl<'a> HtmlMemoryPermit<'a> {
    /// Creates a new active memory permit.
    #[must_use]
    pub const fn new(guard: &'a HtmlMemoryBudgetGuard, size: usize) -> Self {
        Self { guard, size }
    }

    /// Returns the allocated byte size tracked by this permit.
    #[inline]
    #[must_use]
    pub const fn size(&self) -> usize {
        self.size
    }
}

impl Drop for HtmlMemoryPermit<'_> {
    fn drop(&mut self) {
        self.guard.release(self.size);
    }
}


/// Global memory budget guard enforcing resident memory quotas and safe truncation.
#[derive(Debug)]
pub struct HtmlMemoryBudgetGuard {
    current_usage: AtomicUsize,
    max_budget: usize,
    truncation_threshold: usize,
}

impl Clone for HtmlMemoryBudgetGuard {
    fn clone(&self) -> Self {
        Self {
            current_usage: AtomicUsize::new(self.current_usage.load(Ordering::Relaxed)),
            max_budget: self.max_budget,
            truncation_threshold: self.truncation_threshold,
        }
    }
}

impl Default for HtmlMemoryBudgetGuard {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_HTML_MEMORY_BUDGET,
            DEFAULT_HTML_TRUNCATION_THRESHOLD,
        )
    }
}

impl HtmlMemoryBudgetGuard {
    /// Creates a new memory budget guard with custom hard ceiling and truncation limit.
    #[must_use]
    pub const fn new(max_budget: usize, truncation_threshold: usize) -> Self {
        Self {
            current_usage: AtomicUsize::new(0),
            max_budget,
            truncation_threshold,
        }
    }

    /// Atomically attempts to allocate a block of memory, returning a RAII permit on success.
    pub fn allocate(&self, size: usize) -> Result<HtmlMemoryPermit<'_>, HtmlDefenseError> {
        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            let new_usage = current.saturating_add(size);
            if new_usage > self.max_budget {
                return Err(HtmlDefenseError::MemoryBudgetExceeded {
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
                Ok(_) => return Ok(HtmlMemoryPermit::new(self, size)),
                Err(actual) => current = actual,
            }
        }
    }

    /// Atomically reserves memory without RAII tracking.
    pub fn try_reserve(&self, size: usize) -> Result<(), HtmlDefenseError> {
        let mut current = self.current_usage.load(Ordering::Relaxed);
        loop {
            let new_usage = current.saturating_add(size);
            if new_usage > self.max_budget {
                return Err(HtmlDefenseError::MemoryBudgetExceeded {
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

    /// Releases a previously allocated or reserved amount of memory.
    pub fn release(&self, size: usize) {
        self.current_usage.fetch_sub(size, Ordering::Release);
    }

    /// Returns the currently allocated memory in bytes.
    #[inline]
    #[must_use]
    pub fn current_usage(&self) -> usize {
        self.current_usage.load(Ordering::Relaxed)
    }

    /// Checks if a given input size exceeds the 50 MiB truncation threshold.
    #[inline]
    #[must_use]
    pub fn should_truncate(&self, size: usize) -> bool {
        size > self.truncation_threshold
    }

    /// Returns the maximum allowed memory budget ceiling.
    #[inline]
    #[must_use]
    pub const fn max_budget(&self) -> usize {
        self.max_budget
    }

    /// Returns the 50 MiB truncation threshold.
    #[inline]
    #[must_use]
    pub const fn truncation_threshold(&self) -> usize {
        self.truncation_threshold
    }

    /// Truncates an HTML string at the 50 MiB threshold and appends the safety banner.
    #[must_use]
    pub fn truncate_with_banner(&self, input: &str) -> (String, bool) {
        if !self.should_truncate(input.len()) {
            return (input.to_string(), false);
        }

        // Truncate at character boundary before threshold
        let mut truncate_len = self.truncation_threshold;
        while truncate_len > 0 && !input.is_char_boundary(truncate_len) {
            truncate_len -= 1;
        }

        let slice = &input[..truncate_len];
        let mut truncated = String::with_capacity(slice.len() + HTML_TRUNCATION_BANNER.len() + 64);
        truncated.push_str(slice);
        truncated.push_str("\n");
        truncated.push_str(HTML_TRUNCATION_BANNER);

        (truncated, true)
    }
}
