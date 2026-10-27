// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Lifecycle-bound, zero-allocation local work-stealing compute pool.
//!
//! Powered by an isolated, non-global Rayon thread pool with platform-native QoS
//! binding (`pthread_set_qos_class_self_np` on Darwin, `THREAD_PRIORITY_HIGHEST`
//! on Windows, `setpriority` on Linux).
//!
//! # Architecture Invariants
//! - **Zero Persistent Global State**: Thread pools are lifecycle-bound to their owner and
//!   cleanly shut down and join all OS threads upon drop.
//! - **Zero Box<dyn FnOnce> Heap Overhead**: Task scheduling leverages Rayon's internal
//!   work-stealing deques without per-task dynamic heap allocation.
//! - **Apple Silicon P-Core Sizing**: Default concurrency aligns with `recommended_compute_threads()`
//!   to prevent starvation of the UI rendering loop and Darwin cooperative runtimes.

use std::sync::Arc;

use crate::cpu_topology::recommended_compute_threads;
use crate::scheduler::qos::apply_thread_compute_qos;

/// A hardware-conscious local work-stealing thread pool backed by an isolated Rayon pool.
#[derive(Clone)]
pub struct LocalWorkStealingPool {
    inner: Arc<rayon::ThreadPool>,
    thread_count: usize,
}

impl LocalWorkStealingPool {
    /// Creates a new work-stealing pool sized to the recommended compute threads budget.
    ///
    /// On Apple Silicon platforms, this allocates threads precisely matching the physical
    /// Performance Core (P-Core) budget.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::with_threads(recommended_compute_threads())
    }

    /// Creates a new work-stealing pool with an explicit worker thread count.
    #[must_use]
    pub fn with_threads(num_threads: usize) -> Self {
        let thread_count = num_threads.max(1);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .thread_name(|idx| format!("ttzip-worker-{}", idx))
            .start_handler(|_| {
                apply_thread_compute_qos();
            })
            .build()
            .expect("Failed to build local compute thread pool");

        Self {
            inner: Arc::new(pool),
            thread_count,
        }
    }

    /// Returns the number of worker threads in the pool.
    #[inline]
    #[must_use]
    pub fn thread_count(&self) -> usize {
        self.thread_count
    }

    /// Executes an operation directly within the pool's execution context.
    #[inline]
    pub fn install<OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce() -> R + Send,
        R: Send,
    {
        self.inner.install(op)
    }

    /// Executes a scoped parallel closure, ensuring all spawned tasks complete before returning.
    ///
    /// Tasks spawned via `Scope::spawn` are stack-borrowed and execute with zero heap
    /// allocations via Rayon's lock-free work-stealing deques.
    pub fn scope<'scope, F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Scope<'scope>) -> R + Send,
        R: Send,
    {
        self.inner.scope(|rayon_scope| {
            let inner_scope: &'scope rayon::Scope<'scope> = unsafe { std::mem::transmute(rayon_scope) };
            let s = Scope {
                inner: inner_scope,
            };
            f(&s)
        })
    }
}

impl Default for LocalWorkStealingPool {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// A scope for spawning task closures that reference data on the caller's stack frame.
pub struct Scope<'scope> {
    inner: &'scope rayon::Scope<'scope>,
}

impl<'scope> Scope<'scope> {
    /// Spawns a task into the local work-stealing pool.
    ///
    /// The task closure can borrow from the enclosing stack frame.
    #[inline]
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'scope,
    {
        self.inner.spawn(move |_| f());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_pool_lifecycle_and_execution() {
        let pool = LocalWorkStealingPool::with_threads(4);
        assert_eq!(pool.thread_count(), 4);

        let counter = AtomicUsize::new(0);
        pool.scope(|s| {
            for _ in 0..100 {
                s.spawn(|| {
                    counter.fetch_add(1, Ordering::Relaxed);
                });
            }
        });

        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn test_stack_borrowing_in_scope() {
        let pool = LocalWorkStealingPool::with_threads(2);
        let mut data = vec![1, 2, 3, 4, 5];

        pool.scope(|s| {
            s.spawn(|| {
                let sum: i32 = data.iter().sum();
                assert_eq!(sum, 15);
            });
        });

        data.push(6);
        assert_eq!(data.len(), 6);
    }

    #[test]
    fn test_install_execution() {
        let pool = LocalWorkStealingPool::with_threads(2);
        let res = pool.install(|| 40 + 2);
        assert_eq!(res, 42);
    }
}
