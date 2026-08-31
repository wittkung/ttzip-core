// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Deflate-NG / zlib-ng 6-Layer Defense-in-Depth Guard and Protocol Invariant Enforcement Subsystem.
//!
//! Enforces deterministic memory bounds and strict protocol-level defenses against malicious Deflate bitstreams:
//! 1. **Sliding Window 32KB Ring Bounds Guard (`WindowBoundsGuard`)**: Validates backward distances (1..=32768),
//!    match lengths (3..=258), and wraps ring buffer indices safely.
//! 2. **Hash Chain Degeneration & Loop Breaker (`HashChainLoopGuard`)**: Protects matchfinder hash chains
//!    against algorithmic complexity attacks, self-loops, and enforces `max_chain` step truncation.
//! 3. **Dynamic Level Mutation & State Machine Integrity Guard (`DynamicLevelIntegrityGuard`)**: Enforces
//!    valid compression level (-1..=12) and strategy mutations only at safe block boundaries.
//! 4. **1032x Decompression Bomb Quota Guard (`DecompressionBombGuard`)**: 1032x theoretical maximum expansion
//!    ratio ceiling and cumulative uncompressed size limits (`max_output_limit`).
//! 5. **Stored / Raw Block Escape & Zero-Length Loop Guard (`StoredBlockEscapeGuard`)**: Validates `LEN == !NLEN`
//!    invariants, rejects payload boundary escapes, and breaks infinite consecutive zero-length block loops.
//! 6. **Sensitive Memory Scrubbing & Zeroize Protection (`DeflateZeroizeScratchpad`)**: Uses `zeroize` and
//!    `ZeroizeOnDrop` to scrub internal state, temporary tables, and history scratchpads on drop.

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::security::path_sanitizer::{sanitize_path, PathSanitizationResult};
use crate::types::TTZipStatus;

// MARK: - Constants & Security Defaults

/// RFC 1951 Deflate sliding window size in bytes (32 KiB = 32,768 bytes).
pub const DEFLATE_NG_MAX_WINDOW_SIZE: usize = 32 * 1024;

/// RFC 1951 minimum match length in bytes.
pub const DEFLATE_NG_MIN_MATCH_LEN: usize = 3;

/// RFC 1951 maximum match length in bytes.
pub const DEFLATE_NG_MAX_MATCH_LEN: usize = 258;

/// Theoretical maximum single-stream expansion ratio in RFC 1951 Deflate (1032:1).
///
/// 258 uncompressed match bytes produced from a 2-bit fixed Huffman code = 1032x expansion.
pub const DEFLATE_NG_MAX_EXPANSION_RATIO: u32 = 1032;

/// Default maximum cumulative uncompressed output budget (512 MiB).
pub const DEFLATE_NG_DEFAULT_MAX_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;

/// Default uncompressed output threshold before expansion ratio checks activate (1 MiB).
pub const DEFLATE_NG_DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;

/// Default maximum hash chain search depth before truncation.
pub const DEFLATE_NG_DEFAULT_MAX_HASH_CHAIN: usize = 4096;

/// Default maximum consecutive zero-length stored blocks before triggering DoS defense.
pub const DEFLATE_NG_DEFAULT_MAX_CONSECUTIVE_STORED_BLOCKS: usize = 1024;

// MARK: - 1. Window Bounds Guard

/// Guard validating sliding window ring buffer distances, match lengths, and wrap-around offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowBoundsGuard {
    window_size: usize,
    window_mask: usize,
}

impl Default for WindowBoundsGuard {
    #[inline]
    fn default() -> Self {
        Self::new(DEFLATE_NG_MAX_WINDOW_SIZE)
    }
}

impl WindowBoundsGuard {
    /// Creates a new `WindowBoundsGuard` with the specified window capacity.
    #[must_use]
    pub const fn new(window_size: usize) -> Self {
        let size = if window_size == 0 || window_size > DEFLATE_NG_MAX_WINDOW_SIZE {
            DEFLATE_NG_MAX_WINDOW_SIZE
        } else {
            window_size
        };
        Self {
            window_size: size,
            window_mask: size.saturating_sub(1),
        }
    }

    /// Validates backward reference distance against destination buffer cursor and window ceiling.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `distance == 0`, `distance > current_pos`,
    /// or `distance > window_size`.
    #[inline]
    pub fn validate_distance(&self, distance: usize, current_pos: usize) -> Result<(), TTZipStatus> {
        if distance == 0 || distance > self.window_size || distance > current_pos {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Validates match distance and length against RFC 1951 bounds (length: 3..=258).
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` on distance violation or invalid match length.
    pub fn validate_match(
        &self,
        distance: usize,
        length: usize,
        current_pos: usize,
    ) -> Result<(), TTZipStatus> {
        self.validate_distance(distance, current_pos)?;
        if !(DEFLATE_NG_MIN_MATCH_LEN..=DEFLATE_NG_MAX_MATCH_LEN).contains(&length) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Safely wraps a linear ring buffer index within window capacity.
    #[inline]
    #[must_use]
    pub const fn wrap_index(&self, index: usize) -> usize {
        index & self.window_mask
    }

    /// Clamps and validates log2 window bits (8..=15) into byte size (256..=32,768).
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrInvalidParam)` if `window_bits` is not in `8..=15`.
    pub fn clamp_window_size(window_bits: u8) -> Result<usize, TTZipStatus> {
        if !(8..=15).contains(&window_bits) {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        Ok(1usize << window_bits)
    }

    /// Returns configured window size.
    #[inline]
    #[must_use]
    pub const fn window_size(&self) -> usize {
        self.window_size
    }
}

// MARK: - 2. Hash Chain Loop Breaker Guard

/// Guard protecting matchfinder hash chains against pathological degeneration and algorithmic complexity loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashChainLoopGuard {
    max_chain: usize,
    current_chain_steps: usize,
}

impl Default for HashChainLoopGuard {
    #[inline]
    fn default() -> Self {
        Self::new(DEFLATE_NG_DEFAULT_MAX_HASH_CHAIN)
    }
}

impl HashChainLoopGuard {
    /// Creates a new `HashChainLoopGuard` with explicit max chain search depth.
    #[must_use]
    pub const fn new(max_chain: usize) -> Self {
        let max = if max_chain == 0 {
            DEFLATE_NG_DEFAULT_MAX_HASH_CHAIN
        } else {
            max_chain
        };
        Self {
            max_chain: max,
            current_chain_steps: 0,
        }
    }

    /// Records one traversal step along a hash chain.
    ///
    /// Returns `Ok(true)` to continue search, or `Ok(false)` if `max_chain` limit is reached
    /// and the matchfinder should safely truncate traversal.
    #[inline]
    pub fn record_step(&mut self) -> Result<bool, TTZipStatus> {
        self.current_chain_steps = self.current_chain_steps.saturating_add(1);
        if self.current_chain_steps > self.max_chain {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// Validates chain link monotonic ordering to detect self-loops and circular pointers.
    ///
    /// In Deflate matchfinders, links point strictly backwards (`next_idx < cur_idx`).
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `next_idx >= cur_idx` and `next_idx != 0`.
    #[inline]
    pub fn check_cycle(&self, cur_idx: usize, next_idx: usize) -> Result<(), TTZipStatus> {
        if next_idx != 0 && next_idx >= cur_idx {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Resets traversal step counter for the next symbol position.
    #[inline]
    pub fn reset_step_count(&mut self) {
        self.current_chain_steps = 0;
    }

    /// Returns the maximum allowed chain length.
    #[inline]
    #[must_use]
    pub const fn max_chain(&self) -> usize {
        self.max_chain
    }
}

// MARK: - 3. Dynamic Level & State Machine Integrity Guard

/// Compression strategy matching zlib / zlib-ng standards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeflateCompressionStrategy {
    #[default]
    Default = 0,
    Filtered = 1,
    HuffmanOnly = 2,
    Rle = 3,
    Fixed = 4,
}

/// Deflate encoder stream lifecycle state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeflateStreamState {
    #[default]
    Uninitialized = 0,
    Ready = 1,
    BlockHeader = 2,
    BlockEncoding = 3,
    BlockFlushing = 4,
    Finished = 5,
    Poisoned = 6,
}

/// Guard enforcing state machine transitions and verifying dynamic parameter mutations occur only at safe block boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicLevelIntegrityGuard {
    level: i32,
    strategy: DeflateCompressionStrategy,
    state: DeflateStreamState,
    mutation_count: u64,
}

impl Default for DynamicLevelIntegrityGuard {
    #[inline]
    fn default() -> Self {
        Self::new(6, DeflateCompressionStrategy::Default)
            .unwrap_or(Self {
                level: 6,
                strategy: DeflateCompressionStrategy::Default,
                state: DeflateStreamState::Ready,
                mutation_count: 0,
            })
    }
}

impl DynamicLevelIntegrityGuard {
    /// Creates a new guard verifying compression level (-1..=12) and strategy.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrInvalidParam)` if `level` is outside `-1..=12`.
    pub fn new(level: i32, strategy: DeflateCompressionStrategy) -> Result<Self, TTZipStatus> {
        if !(-1..=12).contains(&level) {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        Ok(Self {
            level,
            strategy,
            state: DeflateStreamState::Ready,
            mutation_count: 0,
        })
    }

    /// Transitions the state machine to the target state.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` on invalid state transitions.
    pub fn transition_to(&mut self, next: DeflateStreamState) -> Result<(), TTZipStatus> {
        if self.state == DeflateStreamState::Poisoned {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        let valid = matches!(
            (self.state, next),
            (DeflateStreamState::Uninitialized, DeflateStreamState::Ready)
                | (DeflateStreamState::Ready, DeflateStreamState::BlockHeader)
                | (DeflateStreamState::Ready, DeflateStreamState::Finished)
                | (DeflateStreamState::BlockHeader, DeflateStreamState::BlockEncoding)
                | (DeflateStreamState::BlockEncoding, DeflateStreamState::BlockFlushing)
                | (DeflateStreamState::BlockFlushing, DeflateStreamState::Ready)
                | (DeflateStreamState::BlockFlushing, DeflateStreamState::Finished)
                | (DeflateStreamState::Finished, DeflateStreamState::Ready)
                | (_, DeflateStreamState::Poisoned)
        );

        if valid {
            self.state = next;
            Ok(())
        } else {
            self.state = DeflateStreamState::Poisoned;
            Err(TTZipStatus::ErrSecurityViolation)
        }
    }

    /// Dynamically mutates compression level and strategy.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if mutation is attempted during active `BlockEncoding`
    /// without being at a clean block boundary (`is_block_boundary == true`).
    /// Returns `Err(TTZipStatus::ErrInvalidParam)` if `new_level` is outside `-1..=12`.
    pub fn mutate_params(
        &mut self,
        new_level: i32,
        new_strategy: DeflateCompressionStrategy,
        is_block_boundary: bool,
    ) -> Result<(), TTZipStatus> {
        if !(-1..=12).contains(&new_level) {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        if self.state == DeflateStreamState::BlockEncoding && !is_block_boundary {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        self.level = new_level;
        self.strategy = new_strategy;
        self.mutation_count = self.mutation_count.saturating_add(1);
        Ok(())
    }

    /// Returns `true` if dynamic parameters can safely mutate in the current state.
    #[inline]
    #[must_use]
    pub fn can_mutate_now(&self) -> bool {
        matches!(
            self.state,
            DeflateStreamState::Ready | DeflateStreamState::Finished | DeflateStreamState::Uninitialized
        )
    }

    /// Returns the active compression level.
    #[inline]
    #[must_use]
    pub const fn level(&self) -> i32 {
        self.level
    }

    /// Returns the active strategy.
    #[inline]
    #[must_use]
    pub const fn strategy(&self) -> DeflateCompressionStrategy {
        self.strategy
    }

    /// Returns the current lifecycle state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> DeflateStreamState {
        self.state
    }
}

// MARK: - 4. 1032x Decompression Bomb Quota Guard

/// Guard tracking decompression progress and enforcing 1032x ratio and cumulative output limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecompressionBombGuard {
    max_output_limit: u64,
    max_expansion_ratio: u32,
    threshold_bytes: u64,
    bytes_read: u64,
    bytes_written: u64,
}

impl Default for DecompressionBombGuard {
    #[inline]
    fn default() -> Self {
        Self::new(
            DEFLATE_NG_DEFAULT_MAX_OUTPUT_LIMIT,
            DEFLATE_NG_MAX_EXPANSION_RATIO,
            DEFLATE_NG_DEFAULT_THRESHOLD_BYTES,
        )
    }
}

impl DecompressionBombGuard {
    /// Creates a new `DecompressionBombGuard` with custom limits.
    #[must_use]
    pub const fn new(
        max_output_limit: u64,
        max_expansion_ratio: u32,
        threshold_bytes: u64,
    ) -> Self {
        Self {
            max_output_limit,
            max_expansion_ratio,
            threshold_bytes,
            bytes_read: 0,
            bytes_written: 0,
        }
    }

    /// Tracks incremental input and output bytes, enforcing expansion ratio and cumulative quotas.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` on quota or expansion ratio breach.
    pub fn track_progress(
        &mut self,
        compressed_chunk: usize,
        decompressed_chunk: usize,
    ) -> Result<(), TTZipStatus> {
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
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` on quota or expansion ratio breach.
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
    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

// MARK: - 5. Stored Block Escape & Zero-Length Loop Guard

/// Guard validating uncompressed (stored/raw) blocks against header corruption, payload escapes, and zero-length spin loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlockEscapeGuard {
    max_consecutive_zero_len: usize,
    consecutive_zero_len: usize,
}

impl Default for StoredBlockEscapeGuard {
    #[inline]
    fn default() -> Self {
        Self::new(DEFLATE_NG_DEFAULT_MAX_CONSECUTIVE_STORED_BLOCKS)
    }
}

impl StoredBlockEscapeGuard {
    /// Creates a new `StoredBlockEscapeGuard` with custom consecutive zero-length threshold.
    #[must_use]
    pub const fn new(max_consecutive_zero_len: usize) -> Self {
        Self {
            max_consecutive_zero_len,
            consecutive_zero_len: 0,
        }
    }

    /// Validates RFC 1951 uncompressed block header inverted length integrity (`LEN == !NLEN`).
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if `(len as u16) != !(nlen as u16)`.
    #[inline]
    pub fn validate_stored_header(&self, len: u16, nlen: u16) -> Result<(), TTZipStatus> {
        if len != !nlen {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(())
    }

    /// Validates complete stored block header, available payload size, and zero-length spin breaker.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if header check fails or available input is insufficient.
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if consecutive zero-length blocks exceed threshold.
    pub fn validate_stored_block(
        &mut self,
        len: u16,
        nlen: u16,
        available_input: usize,
    ) -> Result<(), TTZipStatus> {
        self.validate_stored_header(len, nlen)?;

        if (len as usize) > available_input {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        if len == 0 {
            self.consecutive_zero_len = self.consecutive_zero_len.saturating_add(1);
            if self.consecutive_zero_len > self.max_consecutive_zero_len {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        } else {
            self.consecutive_zero_len = 0;
        }

        Ok(())
    }

    /// Resets consecutive zero-length block counter.
    #[inline]
    pub fn reset(&mut self) {
        self.consecutive_zero_len = 0;
    }
}

// MARK: - 6. Sensitive Memory Scrubbing & Path Sanitizer

/// Zeroizing sensitive scratchpad for Deflate cryptographic keys, history buffers, and tables.
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct DeflateZeroizeScratchpad {
    /// Zeroized temporary scratchpad.
    pub scratch: [u8; 64],
}

impl Default for DeflateZeroizeScratchpad {
    #[inline]
    fn default() -> Self {
        Self { scratch: [0u8; 64] }
    }
}

/// Zero-allocation Deflate container path sanitizer protecting against directory traversal (Zip-Slip).
#[inline]
#[must_use]
pub fn sanitize_deflate_entry_path(raw_path: &str) -> PathSanitizationResult {
    sanitize_path(raw_path)
}

// MARK: - Composite Deflate-NG Defense Guard

/// Composite 6-layer defense guard unifying all Deflate-NG runtime safety checks.
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct DeflateNgDefenseGuard {
    #[zeroize(skip)]
    pub window_guard: WindowBoundsGuard,
    #[zeroize(skip)]
    pub hash_guard: HashChainLoopGuard,
    #[zeroize(skip)]
    pub level_guard: DynamicLevelIntegrityGuard,
    #[zeroize(skip)]
    pub bomb_guard: DecompressionBombGuard,
    #[zeroize(skip)]
    pub stored_guard: StoredBlockEscapeGuard,
    pub sensitive_pad: DeflateZeroizeScratchpad,
}

impl Default for DeflateNgDefenseGuard {
    #[inline]
    fn default() -> Self {
        Self {
            window_guard: WindowBoundsGuard::default(),
            hash_guard: HashChainLoopGuard::default(),
            level_guard: DynamicLevelIntegrityGuard::default(),
            bomb_guard: DecompressionBombGuard::default(),
            stored_guard: StoredBlockEscapeGuard::default(),
            sensitive_pad: DeflateZeroizeScratchpad::default(),
        }
    }
}

impl DeflateNgDefenseGuard {
    /// Creates a composite guard with default security policies.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a composite guard with custom output limit.
    #[must_use]
    pub fn with_output_limit(max_output_limit: u64) -> Self {
        Self {
            window_guard: WindowBoundsGuard::default(),
            hash_guard: HashChainLoopGuard::default(),
            level_guard: DynamicLevelIntegrityGuard::default(),
            bomb_guard: DecompressionBombGuard::new(
                max_output_limit,
                DEFLATE_NG_MAX_EXPANSION_RATIO,
                DEFLATE_NG_DEFAULT_THRESHOLD_BYTES,
            ),
            stored_guard: StoredBlockEscapeGuard::default(),
            sensitive_pad: DeflateZeroizeScratchpad::default(),
        }
    }

    /// Resets internal counters across stream resets while keeping configuration intact.
    pub fn reset(&mut self) {
        self.hash_guard.reset_step_count();
        self.bomb_guard.reset();
        self.stored_guard.reset();
        self.sensitive_pad.scratch.zeroize();
    }
}

// MARK: - Unit Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_bounds_guard_invariants() {
        let guard = WindowBoundsGuard::new(32768);
        assert!(guard.validate_distance(1, 1).is_ok());
        assert!(guard.validate_distance(32768, 40000).is_ok());
        assert_eq!(guard.validate_distance(0, 10), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_distance(32769, 40000), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_distance(100, 50), Err(TTZipStatus::ErrSecurityViolation));

        assert!(guard.validate_match(10, 3, 20).is_ok());
        assert!(guard.validate_match(10, 258, 300).is_ok());
        assert_eq!(guard.validate_match(10, 2, 20), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.validate_match(10, 259, 300), Err(TTZipStatus::ErrSecurityViolation));

        assert_eq!(guard.wrap_index(32768), 0);
        assert_eq!(guard.wrap_index(32769), 1);
        assert_eq!(guard.wrap_index(100), 100);

        assert_eq!(WindowBoundsGuard::clamp_window_size(15), Ok(32768));
        assert_eq!(WindowBoundsGuard::clamp_window_size(8), Ok(256));
        assert_eq!(WindowBoundsGuard::clamp_window_size(7), Err(TTZipStatus::ErrInvalidParam));
        assert_eq!(WindowBoundsGuard::clamp_window_size(16), Err(TTZipStatus::ErrInvalidParam));
    }

    #[test]
    fn test_hash_chain_loop_guard() {
        let mut guard = HashChainLoopGuard::new(3);
        assert_eq!(guard.record_step(), Ok(true));
        assert_eq!(guard.record_step(), Ok(true));
        assert_eq!(guard.record_step(), Ok(true));
        assert_eq!(guard.record_step(), Ok(false)); // Truncate cleanly

        assert!(guard.check_cycle(100, 50).is_ok());
        assert!(guard.check_cycle(100, 0).is_ok());
        assert_eq!(guard.check_cycle(100, 100), Err(TTZipStatus::ErrSecurityViolation));
        assert_eq!(guard.check_cycle(100, 105), Err(TTZipStatus::ErrSecurityViolation));
    }

    #[test]
    fn test_dynamic_level_integrity_guard() {
        let mut guard = DynamicLevelIntegrityGuard::new(6, DeflateCompressionStrategy::Default).unwrap();
        assert!(guard.can_mutate_now());

        assert!(guard.transition_to(DeflateStreamState::BlockHeader).is_ok());
        assert!(guard.transition_to(DeflateStreamState::BlockEncoding).is_ok());
        assert!(!guard.can_mutate_now());

        // Forbid mutation mid-block without boundary
        assert_eq!(
            guard.mutate_params(9, DeflateCompressionStrategy::HuffmanOnly, false),
            Err(TTZipStatus::ErrSecurityViolation)
        );

        // Allow mutation with boundary flag
        assert!(guard.mutate_params(9, DeflateCompressionStrategy::HuffmanOnly, true).is_ok());
        assert_eq!(guard.level(), 9);
        assert_eq!(guard.strategy(), DeflateCompressionStrategy::HuffmanOnly);

        // Transition through flushing to finish
        assert!(guard.transition_to(DeflateStreamState::BlockFlushing).is_ok());
        assert!(guard.transition_to(DeflateStreamState::Ready).is_ok());
        assert!(guard.can_mutate_now());
    }

    #[test]
    fn test_decompression_bomb_guard() {
        let mut guard = DecompressionBombGuard::new(1024 * 1024, 1032, 1024);
        assert!(guard.track_progress(100, 500).is_ok());

        // Quota exceed
        assert_eq!(
            guard.track_progress(100, 2 * 1024 * 1024),
            Err(TTZipStatus::ErrSecurityViolation)
        );

        // Ratio exceed past threshold
        let mut ratio_guard = DecompressionBombGuard::new(10 * 1024 * 1024, 10, 100);
        assert_eq!(
            ratio_guard.track_progress(10, 200),
            Err(TTZipStatus::ErrSecurityViolation)
        );
    }

    #[test]
    fn test_stored_block_escape_guard() {
        let mut guard = StoredBlockEscapeGuard::new(2);
        assert!(guard.validate_stored_header(0x1234, !0x1234).is_ok());
        assert_eq!(
            guard.validate_stored_header(0x1234, 0x1234),
            Err(TTZipStatus::ErrCorruptHeader)
        );

        assert!(guard.validate_stored_block(10, !10, 10).is_ok());
        assert_eq!(
            guard.validate_stored_block(10, !10, 5),
            Err(TTZipStatus::ErrCorruptHeader)
        );

        // Zero length loop test
        assert!(guard.validate_stored_block(0, !0, 0).is_ok());
        assert!(guard.validate_stored_block(0, !0, 0).is_ok());
        assert_eq!(
            guard.validate_stored_block(0, !0, 0),
            Err(TTZipStatus::ErrSecurityViolation)
        );
    }

    #[test]
    fn test_sensitive_scratchpad_and_path_sanitizer() {
        let mut guard = DeflateNgDefenseGuard::new();
        guard.sensitive_pad.scratch[0] = 0xAA;
        guard.reset();
        assert_eq!(guard.sensitive_pad.scratch[0], 0);

        let sanitized = sanitize_deflate_entry_path("../../../etc/passwd");
        assert_eq!(sanitized.normalized_path, "etc/passwd");
        assert!(sanitized.has_traversal_attack);
    }
}
