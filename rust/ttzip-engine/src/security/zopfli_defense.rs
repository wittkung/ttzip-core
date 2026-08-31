// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zopfli 6-Layer Defense-in-Depth Security Guard and Algorithmic Complexity Circuit Breaker.
//!
//! Enforces deterministic resource bounds and strict protocol-level defenses for Zopfli Deflate optimization:
//! 1. **DAG State Transition Graph Depth & Loop Breaker (`DagRecursionGuard`)**: Validates strictly forward-directed
//!    transitions (`from_pos < to_pos <= block_size`), bounds graph relaxation steps, and detects cyclic paths.
//! 2. **Squeeze Iteration Quota & Adaptive Timeout Fuse (`SqueezeIterationGuard`)**: Enforces hard ceiling on
//!    optimization passes (`max_iterations <= 500`), monitors wall-clock elapsed time, and early-terminates stagnant runs.
//! 3. **Block Split Recursion Depth Circuit Breaker (`BlockSplitRecursionGuard`)**: Restricts recursive entropy
//!    block-splitting depth (`depth <= 16`) and caps total split blocks (`max_blocks <= 2048`).
//! 4. **1032x Decompression Bomb & Memory Quota Guard (`ZopfliDecompressionBombGuard`)**: 1032x theoretical maximum
//!    expansion ratio ceiling and cumulative uncompressed size limits (`max_output_limit`).
//! 5. **Sensitive Memory Scrubbing & Zeroize Protection (`ZopfliZeroizeScratchpad`)**: Uses `zeroize` and
//!    `ZeroizeOnDrop` to scrub symbol tables, cost arrays, hash chains, and entropy state on drop.
//! 6. **Atomic Cancellation Token & Cooperative Interruption Guard (`ZopfliCancellationGuard`)**: Provides
//!    low-overhead periodic atomic polling to abort long-running compression jobs cleanly.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::security::path_sanitizer::{sanitize_path, PathSanitizationResult};
use crate::types::TTZipStatus;

// MARK: - Constants & Security Defaults

/// Theoretical maximum single-stream expansion ratio in RFC 1951 Deflate (1032:1).
pub const ZOPFLI_MAX_EXPANSION_RATIO: u32 = 1032;
/// Hard ceiling for Zopfli squeeze iterations to prevent CPU exhaustion DoS.
pub const ZOPFLI_MAX_NUM_ITERATIONS: u32 = 500;
/// Default recommended squeeze iterations for standard high-compression profile.
pub const ZOPFLI_DEFAULT_NUM_ITERATIONS: u32 = 15;
/// Hard ceiling for block splitting recursion depth (2^16 = 65,536 max potential segments).
pub const ZOPFLI_MAX_BLOCK_SPLIT_DEPTH: usize = 16;
/// Default maximum number of dynamic entropy blocks allowed in a single input stream.
pub const ZOPFLI_DEFAULT_MAX_SPLIT_BLOCKS: usize = 2048;
/// Default maximum block size for DAG shortest-path relaxation (128 KiB).
pub const ZOPFLI_DEFAULT_MAX_BLOCK_SIZE: usize = 128 * 1024;
/// Default maximum relaxation steps allowed per block to prevent algorithmic graph DoS.
pub const ZOPFLI_DEFAULT_MAX_RELAXATION_STEPS: usize = 2_000_000;
/// Default maximum cumulative uncompressed output budget (512 MiB).
pub const ZOPFLI_DEFAULT_MAX_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;
/// Default uncompressed output threshold before expansion ratio checks activate (1 MiB).
pub const ZOPFLI_DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;
/// Default periodic cancellation check interval (every 256 symbol / iteration steps).
pub const ZOPFLI_DEFAULT_CANCEL_CHECK_INTERVAL: u32 = 256;

// MARK: - 1. DAG State Transition Graph Depth & Loop Breaker Guard

/// Guard validating shortest-path DAG node bounds, edge monotonicity, and graph relaxation step budgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagRecursionGuard {
    max_block_size: usize,
    max_relaxation_steps: usize,
    relaxation_steps: usize,
    current_block_size: usize,
}

impl Default for DagRecursionGuard {
    #[inline]
    fn default() -> Self {
        Self::new(ZOPFLI_DEFAULT_MAX_BLOCK_SIZE, ZOPFLI_DEFAULT_MAX_RELAXATION_STEPS)
    }
}

impl DagRecursionGuard {
    /// Creates a new `DagRecursionGuard` with specified block size ceiling and relaxation step quota.
    #[must_use]
    pub const fn new(max_block_size: usize, max_relaxation_steps: usize) -> Self {
        let block_sz = if max_block_size == 0 { ZOPFLI_DEFAULT_MAX_BLOCK_SIZE } else { max_block_size };
        let relax_steps = if max_relaxation_steps == 0 { ZOPFLI_DEFAULT_MAX_RELAXATION_STEPS } else { max_relaxation_steps };
        Self {
            max_block_size: block_sz,
            max_relaxation_steps: relax_steps,
            relaxation_steps: 0,
            current_block_size: 0,
        }
    }

    /// Initializes DAG bounds for a newly submitted block.
    pub fn begin_block(&mut self, block_size: usize) -> Result<(), TTZipStatus> {
        if block_size > self.max_block_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        self.current_block_size = block_size;
        self.relaxation_steps = 0;
        Ok(())
    }

    /// Validates a single directed state transition edge from `from_pos` to `to_pos`.
    #[inline]
    pub fn validate_transition(&mut self, from_pos: usize, to_pos: usize) -> Result<(), TTZipStatus> {
        if from_pos >= to_pos || to_pos > self.current_block_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        if self.relaxation_steps >= self.max_relaxation_steps {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        self.relaxation_steps = self.relaxation_steps.saturating_add(1);
        Ok(())
    }

    /// Validates an entire shortest-path backtrack trace sequence for strict monotonicity and boundary integrity.
    pub fn validate_path_trace(&self, path: &[usize], block_size: usize) -> Result<(), TTZipStatus> {
        if path.is_empty() {
            return if block_size == 0 { Ok(()) } else { Err(TTZipStatus::ErrSecurityViolation) };
        }
        if path[0] != 0 || *path.last().unwrap_or(&0) != block_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        for window in path.windows(2) {
            if window[0] >= window[1] || window[1] > block_size {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
        Ok(())
    }

    /// Resets relaxation counters for the next block.
    #[inline]
    pub fn reset(&mut self) {
        self.relaxation_steps = 0;
        self.current_block_size = 0;
    }

    /// Returns the maximum allowed block size.
    #[inline]
    #[must_use]
    pub const fn max_block_size(&self) -> usize { self.max_block_size }

    /// Returns current relaxation step count.
    #[inline]
    #[must_use]
    pub const fn relaxation_steps(&self) -> usize { self.relaxation_steps }
}

// MARK: - 2. Squeeze Iteration Quota & Adaptive Timeout Fuse Guard

/// Guard tracking Zopfli squeeze iterations, wall-clock timeout fuses, and adaptive convergence stagnation.
#[derive(Debug, Clone, PartialEq)]
pub struct SqueezeIterationGuard {
    max_iterations: u32,
    current_iteration: u32,
    timeout_ms: Option<u64>,
    start_time: Option<Instant>,
    min_cost_improvement: f64,
    last_cost: f64,
    stagnant_iterations: u32,
    max_stagnant_iterations: u32,
}

impl Default for SqueezeIterationGuard {
    #[inline]
    fn default() -> Self {
        Self::new(ZOPFLI_DEFAULT_NUM_ITERATIONS, None).unwrap_or(Self {
            max_iterations: ZOPFLI_DEFAULT_NUM_ITERATIONS,
            current_iteration: 0,
            timeout_ms: None,
            start_time: None,
            min_cost_improvement: 0.0001,
            last_cost: f64::INFINITY,
            stagnant_iterations: 0,
            max_stagnant_iterations: 5,
        })
    }
}

impl SqueezeIterationGuard {
    /// Creates a new `SqueezeIterationGuard` with explicit iteration ceiling (`<= 500`) and optional timeout.
    pub fn new(max_iterations: u32, timeout_ms: Option<u64>) -> Result<Self, TTZipStatus> {
        if max_iterations == 0 || max_iterations > ZOPFLI_MAX_NUM_ITERATIONS {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        Ok(Self {
            max_iterations,
            current_iteration: 0,
            timeout_ms,
            start_time: None,
            min_cost_improvement: 0.0001,
            last_cost: f64::INFINITY,
            stagnant_iterations: 0,
            max_stagnant_iterations: 5,
        })
    }

    /// Arm the squeeze timer and reset iteration progress.
    pub fn begin_squeeze(&mut self) {
        self.current_iteration = 0;
        self.start_time = Some(Instant::now());
        self.last_cost = f64::INFINITY;
        self.stagnant_iterations = 0;
    }

    /// Records completion of a single squeeze iteration with calculated bitstream cost.
    pub fn record_iteration(&mut self, current_cost: f64) -> Result<bool, TTZipStatus> {
        if self.current_iteration >= self.max_iterations {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        self.current_iteration = self.current_iteration.saturating_add(1);

        if let (Some(limit_ms), Some(start)) = (self.timeout_ms, self.start_time) {
            if start.elapsed().as_millis() as u64 >= limit_ms {
                return Ok(false);
            }
        }

        if current_cost < self.last_cost {
            let improvement = self.last_cost - current_cost;
            if improvement < self.min_cost_improvement {
                self.stagnant_iterations = self.stagnant_iterations.saturating_add(1);
            } else {
                self.stagnant_iterations = 0;
            }
            self.last_cost = current_cost;
        } else {
            self.stagnant_iterations = self.stagnant_iterations.saturating_add(1);
        }

        if self.stagnant_iterations >= self.max_stagnant_iterations || self.current_iteration >= self.max_iterations {
            return Ok(false);
        }
        Ok(true)
    }

    /// Returns configured maximum iteration ceiling.
    #[inline]
    #[must_use]
    pub const fn max_iterations(&self) -> u32 { self.max_iterations }

    /// Returns count of completed iterations.
    #[inline]
    #[must_use]
    pub const fn current_iteration(&self) -> u32 { self.current_iteration }

    /// Resets iteration tracking state.
    pub fn reset(&mut self) {
        self.current_iteration = 0;
        self.start_time = None;
        self.last_cost = f64::INFINITY;
        self.stagnant_iterations = 0;
    }
}

// MARK: - 3. Block Split Recursion Depth Circuit Breaker Guard

/// Guard enforcing recursive block-splitting depth limits and total split block ceilings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSplitRecursionGuard {
    max_depth: usize,
    max_blocks: usize,
    current_depth: usize,
    total_blocks: usize,
}

impl Default for BlockSplitRecursionGuard {
    #[inline]
    fn default() -> Self {
        Self::new(ZOPFLI_MAX_BLOCK_SPLIT_DEPTH, ZOPFLI_DEFAULT_MAX_SPLIT_BLOCKS).unwrap_or(Self {
            max_depth: ZOPFLI_MAX_BLOCK_SPLIT_DEPTH,
            max_blocks: ZOPFLI_DEFAULT_MAX_SPLIT_BLOCKS,
            current_depth: 0,
            total_blocks: 1,
        })
    }
}

impl BlockSplitRecursionGuard {
    /// Creates a new `BlockSplitRecursionGuard` verifying depth (`<= 16`) and total block quotas.
    pub fn new(max_depth: usize, max_blocks: usize) -> Result<Self, TTZipStatus> {
        if max_depth == 0 || max_depth > ZOPFLI_MAX_BLOCK_SPLIT_DEPTH {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let blocks = if max_blocks == 0 { ZOPFLI_DEFAULT_MAX_SPLIT_BLOCKS } else { max_blocks };
        Ok(Self {
            max_depth,
            max_blocks: blocks,
            current_depth: 0,
            total_blocks: 1,
        })
    }

    /// Increments recursion depth when descending into child block splitting.
    pub fn enter_depth(&mut self) -> Result<(), TTZipStatus> {
        if self.current_depth >= self.max_depth {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        self.current_depth = self.current_depth.saturating_add(1);
        Ok(())
    }

    /// Decrements recursion depth upon returning from child block splitting.
    #[inline]
    pub fn leave_depth(&mut self) {
        self.current_depth = self.current_depth.saturating_sub(1);
    }

    /// Validates proposed split point within `[start, end]` range and tracks total block allocations.
    pub fn validate_split_point(&mut self, start: usize, split_pos: usize, end: usize) -> Result<(), TTZipStatus> {
        if split_pos <= start || split_pos >= end {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        if self.total_blocks >= self.max_blocks {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        self.total_blocks = self.total_blocks.saturating_add(1);
        Ok(())
    }

    /// Returns current recursion depth.
    #[inline]
    #[must_use]
    pub const fn current_depth(&self) -> usize { self.current_depth }

    /// Returns cumulative total blocks generated.
    #[inline]
    #[must_use]
    pub const fn total_blocks(&self) -> usize { self.total_blocks }

    /// Resets tracking counters.
    #[inline]
    pub fn reset(&mut self) {
        self.current_depth = 0;
        self.total_blocks = 1;
    }
}

// MARK: - 4. 1032x Decompression Bomb Quota Guard

/// Guard tracking decompression progress and enforcing 1032x ratio and cumulative output limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZopfliDecompressionBombGuard {
    max_output_limit: u64,
    max_expansion_ratio: u32,
    threshold_bytes: u64,
    bytes_read: u64,
    bytes_written: u64,
}

impl Default for ZopfliDecompressionBombGuard {
    #[inline]
    fn default() -> Self {
        Self::new(ZOPFLI_DEFAULT_MAX_OUTPUT_LIMIT, ZOPFLI_MAX_EXPANSION_RATIO, ZOPFLI_DEFAULT_THRESHOLD_BYTES)
    }
}

impl ZopfliDecompressionBombGuard {
    /// Creates a new `ZopfliDecompressionBombGuard` with custom limits.
    #[must_use]
    pub const fn new(max_output_limit: u64, max_expansion_ratio: u32, threshold_bytes: u64) -> Self {
        Self {
            max_output_limit,
            max_expansion_ratio,
            threshold_bytes,
            bytes_read: 0,
            bytes_written: 0,
        }
    }

    /// Tracks incremental input and output bytes, enforcing expansion ratio and cumulative quotas.
    pub fn track_progress(&mut self, compressed_chunk: usize, decompressed_chunk: usize) -> Result<(), TTZipStatus> {
        self.bytes_read = self.bytes_read.saturating_add(compressed_chunk as u64);
        self.bytes_written = self.bytes_written.saturating_add(decompressed_chunk as u64);

        if self.bytes_written > self.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if self.bytes_written > self.threshold_bytes {
            let comp = self.bytes_read.max(1) as f64;
            let uncomp = self.bytes_written as f64;
            let ratio = uncomp / comp;
            if ratio > self.max_expansion_ratio as f64 {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
        Ok(())
    }

    /// Validates standalone total bytes against expansion policies.
    pub fn validate_expansion_ratio(&self, bytes_in: u64, bytes_out: u64) -> Result<(), TTZipStatus> {
        if bytes_out > self.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        if bytes_out > self.threshold_bytes {
            let comp = bytes_in.max(1) as f64;
            let uncomp = bytes_out as f64;
            let ratio = uncomp / comp;
            if ratio > self.max_expansion_ratio as f64 {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
        Ok(())
    }

    /// Returns current expansion ratio.
    #[must_use]
    pub fn current_ratio(&self) -> f64 {
        let comp = self.bytes_read.max(1) as f64;
        (self.bytes_written as f64) / comp
    }

    /// Resets tracking counters.
    #[inline]
    pub fn reset(&mut self) {
        self.bytes_read = 0;
        self.bytes_written = 0;
    }

    /// Returns cumulative decompressed bytes written.
    #[inline]
    #[must_use]
    pub const fn bytes_written(&self) -> u64 { self.bytes_written }
}

// MARK: - 5. Sensitive Memory Scrubbing & Zeroize Protection

/// Zeroizing sensitive scratchpad for Zopfli symbols, dynamic costs, hash tables, and entropy tables.
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct ZopfliZeroizeScratchpad {
    /// Secure symbol buffer wiped on drop.
    pub symbol_scratch: Vec<u16>,
    /// Intermediate cost calculations wiped on drop.
    pub cost_scratch: Vec<f64>,
    /// Hash chain cache wiped on drop.
    pub hash_scratch: Vec<u32>,
    /// Entropy state salt.
    pub secure_salt: [u8; 64],
}

impl Default for ZopfliZeroizeScratchpad {
    #[inline]
    fn default() -> Self {
        Self::new(1024)
    }
}

impl ZopfliZeroizeScratchpad {
    /// Creates a pre-allocated scratchpad wiped on drop.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            symbol_scratch: Vec::with_capacity(capacity),
            cost_scratch: Vec::with_capacity(capacity),
            hash_scratch: Vec::with_capacity(capacity),
            secure_salt: [0u8; 64],
        }
    }

    /// Clears and zeroes all memory buffers.
    pub fn reset(&mut self) {
        self.symbol_scratch.zeroize();
        self.symbol_scratch.clear();
        self.cost_scratch.zeroize();
        self.cost_scratch.clear();
        self.hash_scratch.zeroize();
        self.hash_scratch.clear();
        self.secure_salt.zeroize();
    }
}

// MARK: - 6. Atomic Cancellation Token & Cooperative Interruption Guard

/// Guard facilitating periodic cooperative cancellation checks during compute-intensive Zopfli optimization.
#[derive(Debug, Clone)]
pub struct ZopfliCancellationGuard {
    cancel_flag: Option<Arc<AtomicBool>>,
    check_interval: u32,
    counter: u32,
}

impl Default for ZopfliCancellationGuard {
    #[inline]
    fn default() -> Self {
        Self::new(None, ZOPFLI_DEFAULT_CANCEL_CHECK_INTERVAL)
    }
}

impl ZopfliCancellationGuard {
    /// Creates a new `ZopfliCancellationGuard` bound to an optional atomic boolean flag.
    #[must_use]
    pub const fn new(cancel_flag: Option<Arc<AtomicBool>>, check_interval: u32) -> Self {
        let interval = if check_interval == 0 { ZOPFLI_DEFAULT_CANCEL_CHECK_INTERVAL } else { check_interval };
        Self {
            cancel_flag,
            check_interval: interval,
            counter: 0,
        }
    }

    /// Directly checks whether cancellation has been requested.
    #[inline]
    pub fn check_cancelled(&self) -> Result<(), TTZipStatus> {
        if let Some(ref flag) = self.cancel_flag {
            if flag.load(Ordering::Relaxed) {
                return Err(TTZipStatus::Cancelled);
            }
        }
        Ok(())
    }

    /// Ticks operation counter and performs cancellation check every `check_interval` steps.
    #[inline]
    pub fn tick_check(&mut self) -> Result<(), TTZipStatus> {
        self.counter = self.counter.wrapping_add(1);
        if self.counter.is_multiple_of(self.check_interval) {
            self.check_cancelled()?;
        }
        Ok(())
    }

    /// Requests cooperative cancellation.
    #[inline]
    pub fn cancel(&self) {
        if let Some(ref flag) = self.cancel_flag {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Returns `true` if cancellation flag is active.
    #[inline]
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::Relaxed))
    }

    /// Binds an external atomic cancellation token.
    pub fn set_cancel_flag(&mut self, flag: Option<Arc<AtomicBool>>) {
        self.cancel_flag = flag;
    }
}

// MARK: - Composite Zopfli Defense Guard

/// Composite 6-layer defense guard unifying all Zopfli runtime safety invariants.
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct ZopfliDefenseGuard {
    #[zeroize(skip)]
    pub dag_guard: DagRecursionGuard,
    #[zeroize(skip)]
    pub squeeze_guard: SqueezeIterationGuard,
    #[zeroize(skip)]
    pub split_guard: BlockSplitRecursionGuard,
    #[zeroize(skip)]
    pub bomb_guard: ZopfliDecompressionBombGuard,
    pub scratchpad: ZopfliZeroizeScratchpad,
    #[zeroize(skip)]
    pub cancel_guard: ZopfliCancellationGuard,
}

impl Default for ZopfliDefenseGuard {
    #[inline]
    fn default() -> Self {
        Self {
            dag_guard: DagRecursionGuard::default(),
            squeeze_guard: SqueezeIterationGuard::default(),
            split_guard: BlockSplitRecursionGuard::default(),
            bomb_guard: ZopfliDecompressionBombGuard::default(),
            scratchpad: ZopfliZeroizeScratchpad::default(),
            cancel_guard: ZopfliCancellationGuard::default(),
        }
    }
}

impl ZopfliDefenseGuard {
    /// Creates a composite guard with default security policies.
    #[must_use]
    pub fn new() -> Self { Self::default() }

    /// Creates a composite guard with custom iteration and cancellation bounds.
    pub fn with_options(
        max_iterations: u32,
        timeout_ms: Option<u64>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<Self, TTZipStatus> {
        let squeeze = SqueezeIterationGuard::new(max_iterations, timeout_ms)?;
        let cancel = ZopfliCancellationGuard::new(cancel_flag, ZOPFLI_DEFAULT_CANCEL_CHECK_INTERVAL);
        Ok(Self {
            dag_guard: DagRecursionGuard::default(),
            squeeze_guard: squeeze,
            split_guard: BlockSplitRecursionGuard::default(),
            bomb_guard: ZopfliDecompressionBombGuard::default(),
            scratchpad: ZopfliZeroizeScratchpad::default(),
            cancel_guard: cancel,
        })
    }

    /// Resets all internal guards for the next compression job while preserving policies.
    pub fn reset(&mut self) {
        self.dag_guard.reset();
        self.squeeze_guard.reset();
        self.split_guard.reset();
        self.bomb_guard.reset();
        self.scratchpad.reset();
    }
}

/// Zero-allocation Zopfli container path sanitizer protecting against directory traversal (Zip-Slip).
#[inline]
#[must_use]
pub fn sanitize_zopfli_entry_path(raw_path: &str) -> PathSanitizationResult {
    sanitize_path(raw_path)
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_recursion_guard_invariants() {
        let mut guard = DagRecursionGuard::new(1024, 10);
        assert!(guard.begin_block(500).is_ok());
        assert_eq!(guard.begin_block(2000), Err(TTZipStatus::ErrSecurityViolation));

        assert!(guard.begin_block(500).is_ok());
        assert!(guard.validate_transition(0, 10).is_ok());
        assert!(guard.validate_transition(10, 20).is_ok());

        assert_eq!(guard.validate_transition(20, 10), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_transition(20, 20), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_transition(20, 600), Err(TTZipStatus::ErrSecurityViolation));

        let mut step_guard = DagRecursionGuard::new(1024, 2);
        assert!(step_guard.begin_block(100).is_ok());
        assert!(step_guard.validate_transition(0, 1).is_ok());
        assert!(step_guard.validate_transition(1, 2).is_ok());
        assert_eq!(step_guard.validate_transition(2, 3), Err(TTZipStatus::ErrSecurityViolation));

        assert!(guard.validate_path_trace(&[0, 10, 50, 100], 100).is_ok());
        assert_eq!(guard.validate_path_trace(&[1, 10, 50, 100], 100), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_path_trace(&[0, 10, 50, 99], 100), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_path_trace(&[0, 50, 10, 100], 100), Err(TTZipStatus::ErrSecurityViolation));
    }

    #[test]
    fn test_squeeze_iteration_guard_invariants() {
        assert_eq!(SqueezeIterationGuard::new(0, None).unwrap_err(), TTZipStatus::ErrInvalidParam);
        assert_eq!(SqueezeIterationGuard::new(501, None).unwrap_err(), TTZipStatus::ErrInvalidParam);

        let mut guard = SqueezeIterationGuard::new(3, None).unwrap();
        guard.begin_squeeze();
        assert_eq!(guard.record_iteration(100.0), Ok(true));
        assert_eq!(guard.record_iteration(90.0), Ok(true));
        assert_eq!(guard.record_iteration(80.0), Ok(false));
        assert_eq!(guard.record_iteration(70.0), Err(TTZipStatus::ErrSecurityViolation));

        let mut stag_guard = SqueezeIterationGuard::new(50, None).unwrap();
        stag_guard.begin_squeeze();
        for _ in 0..5 {
            assert_eq!(stag_guard.record_iteration(100.0), Ok(true));
        }
        assert_eq!(stag_guard.record_iteration(100.0), Ok(false));
    }

    #[test]
    fn test_block_split_recursion_guard_invariants() {
        assert_eq!(BlockSplitRecursionGuard::new(0, 100), Err(TTZipStatus::ErrInvalidParam));
        assert_eq!(BlockSplitRecursionGuard::new(17, 100), Err(TTZipStatus::ErrInvalidParam));

        let mut guard = BlockSplitRecursionGuard::new(2, 3).unwrap();
        assert!(guard.enter_depth().is_ok());
        assert!(guard.enter_depth().is_ok());
        assert_eq!(guard.enter_depth(), Err(TTZipStatus::ErrSecurityViolation));
        guard.leave_depth();
        assert_eq!(guard.current_depth(), 1);

        assert!(guard.validate_split_point(0, 50, 100).is_ok());
        assert!(guard.validate_split_point(50, 75, 100).is_ok());
        assert_eq!(guard.validate_split_point(75, 90, 100), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_split_point(0, 0, 100), Err(TTZipStatus::ErrSecurityViolation));
    }

    #[test]
    fn test_decompression_bomb_guard_invariants() {
        let mut guard = ZopfliDecompressionBombGuard::new(1024 * 1024, 1032, 1024);
        assert!(guard.track_progress(100, 500).is_ok());

        assert_eq!(guard.track_progress(100, 2 * 1024 * 1024), Err(TTZipStatus::ErrSecurityViolation));

        let mut ratio_guard = ZopfliDecompressionBombGuard::new(10 * 1024 * 1024, 10, 100);
        assert_eq!(ratio_guard.track_progress(10, 200), Err(TTZipStatus::ErrSecurityViolation));
    }

    #[test]
    fn test_zeroize_scratchpad_and_cancellation() {
        let mut scratch = ZopfliZeroizeScratchpad::new(64);
        scratch.symbol_scratch.push(0x1234);
        scratch.cost_scratch.push(42.0);
        scratch.hash_scratch.push(999);
        scratch.secure_salt[0] = 0xAA;
        scratch.reset();
        assert!(scratch.symbol_scratch.is_empty());
        assert!(scratch.cost_scratch.is_empty());
        assert!(scratch.hash_scratch.is_empty());
        assert_eq!(scratch.secure_salt[0], 0);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut cancel_guard = ZopfliCancellationGuard::new(Some(cancel_flag.clone()), 2);
        assert!(cancel_guard.check_cancelled().is_ok());
        assert!(cancel_guard.tick_check().is_ok());
        cancel_flag.store(true, Ordering::SeqCst);
        assert_eq!(cancel_guard.check_cancelled(), Err(TTZipStatus::Cancelled));
        assert_eq!(cancel_guard.tick_check(), Err(TTZipStatus::Cancelled));
    }

    #[test]
    fn test_composite_guard_and_path_sanitizer() {
        let guard = ZopfliDefenseGuard::new();
        assert_eq!(guard.dag_guard.max_block_size(), ZOPFLI_DEFAULT_MAX_BLOCK_SIZE);
        assert_eq!(guard.squeeze_guard.max_iterations(), ZOPFLI_DEFAULT_NUM_ITERATIONS);

        let res = sanitize_zopfli_entry_path("../../../secrets.txt");
        assert_eq!(res.normalized_path, "secrets.txt");
        assert!(res.has_traversal_attack);
    }
}
