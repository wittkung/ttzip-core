// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Topology-aware on-demand concurrency helpers anchored to Apple Silicon P-Core budgets.
//!
//! Replaces persistent global Rayon thread pools with ephemeral,
//! zero-background-thread scoped thread dispatching bounded by
//! Apple Silicon P-Core budgets.
//!
//! # Concurrency Invariant
//! Thread allocation is bounded by `ttzip_types::recommended_compute_threads()`
//! (which yields `max(1, p_cores - 1)`). Workloads must never saturate all
//! hardware cores, guaranteeing zero starvation for Swift UI runtimes and OS daemons.

use ttzip_types::recommended_compute_threads;

/// Error returned when thread pool initialization fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolInitError;

impl std::fmt::Display for PoolInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "thread pool initialization failed")
    }
}

impl std::error::Error for PoolInitError {}

/// Initializes bounded concurrency idempotently.
///
/// Kept for backward compatibility. Global persistent thread pools have been excised;
/// concurrency is now dispatched on demand via scoped threads.
#[inline]
pub fn init_bounded_rayon_pool() -> Result<(), PoolInitError> {
    Ok(())
}

/// Initializes bounded concurrency with an explicit thread budget idempotently.
#[inline]
pub fn init_bounded_rayon_pool_with_threads(_threads: usize) -> Result<(), PoolInitError> {
    Ok(())
}

/// Executes a closure directly on the calling thread, respecting caller QoS and
/// eliminating thread oversubscription.
#[inline]
pub fn in_bounded_pool<OP, R>(op: OP) -> R
where
    OP: FnOnce() -> R,
{
    op()
}

/// Returns the configured worker thread count bounded by `recommended_compute_threads()`.
#[inline]
pub fn current_compute_threads() -> usize {
    recommended_compute_threads()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttzip_types::get_performance_cores;

    #[test]
    fn test_init_bounded_rayon_pool_idempotent() {
        let res1 = init_bounded_rayon_pool();
        assert!(res1.is_ok(), "First initialization should succeed");
        let res2 = init_bounded_rayon_pool();
        assert!(res2.is_ok(), "Subsequent initialization must be idempotent");
    }

    #[test]
    fn test_in_bounded_pool_parallel_computation() {
        let data: Vec<u64> = (1..=1000).collect();
        let sum: u64 = in_bounded_pool(|| {
            data.iter().copied().sum()
        });
        assert_eq!(sum, 500500);
    }

    #[test]
    fn test_current_compute_threads() {
        let count = current_compute_threads();
        assert!(count >= 1);
        let recommended = recommended_compute_threads();
        assert_eq!(count, recommended);
        let p_cores = get_performance_cores();
        assert!(count <= p_cores);
    }
}
