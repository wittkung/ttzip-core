// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Brotli 7-Layer Defense-in-Depth Guard and Decompression Bomb Circuit Breaker Subsystem.
//!
//! Enforces deterministic memory bounds and strict protocol-level defenses against malicious Brotli bitstreams:
//! 1. **Sliding Window Memory Quota & Large Window Protection**: Rejects unauthorized large window (1 GiB) requests.
//! 2. **Exuberant Nibbles Validation**: Rejects non-canonical redundant zero nibbles in meta-block size headers.
//! 3. **Distance Arithmetic Overflow Guard**: Enforces `0x7FFFFFFC` maximum backward distance hard ceiling.
//! 4. **Decompression Bomb Quota Breaker**: Real-time cumulative output budget and expansion ratio limiter (100:1).
//! 5. **Non-Zero Padding Bit Invariant**: Validates byte-boundary alignment padding bits are strictly zero.
//! 6. **Metadata Block Header Validation**: Rejects malformed and exuberant metadata block lengths.
//! 7. **Zero-Panic & Bounded State Invariant**: Guarantees panic-free execution and deterministic error propagation.

use crate::types::TTZipStatus;

/// Minimum allowable sliding window bits exponent (10 = 1 KiB - 16 B).
pub const BROTLI_MIN_WINDOW_BITS: u8 = 10;

/// Maximum allowable sliding window bits exponent in standard RFC 7932 (24 = 16 MiB - 16 B).
pub const BROTLI_MAX_WINDOW_BITS: u8 = 24;

/// Maximum allowable sliding window bits exponent in Large Window extension (30 = 1 GiB - 16 B).
pub const BROTLI_LARGE_MAX_WINDOW_BITS: u8 = 30;

/// Hard ceiling for backward distance references (`(1 << 31) - 4 = 0x7FFFFFFC`).
///
/// Prevents signed 32-bit integer overflows during ring buffer address calculations
/// per RFC 7932 and Google Brotli C reference decoder specifications.
pub const BROTLI_MAX_ALLOWED_DISTANCE: usize = 0x7FFF_FFFC;

/// Configuration parameters for Brotli decompression defense and resource budget enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrotliDefenseConfig {
    /// Maximum cumulative uncompressed output size in bytes (default: 512 MiB).
    pub max_output_limit: u64,
    /// Maximum allowable sliding window bits exponent (default: 24, corresponding to 16 MiB).
    pub max_window_bits: u8,
    /// Maximum allowable decompression expansion ratio (e.g. 100 for 100:1, default: 100).
    pub max_expansion_ratio: u32,
    /// Whether RFC 9841 Large Window Brotli extension (> 24 bits up to 30 bits) is permitted.
    pub allow_large_window: bool,
    /// Threshold in uncompressed bytes before expansion ratio check is enforced (default: 1 MiB).
    pub threshold_bytes: u64,
}

impl Default for BrotliDefenseConfig {
    #[inline]
    fn default() -> Self {
        Self::default_limits()
    }
}

impl BrotliDefenseConfig {
    /// Default maximum cumulative uncompressed limit (512 MiB).
    pub const DEFAULT_MAX_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;
    /// Default maximum sliding window bits exponent (24 = 16 MiB).
    pub const DEFAULT_MAX_WINDOW_BITS: u8 = 24;
    /// Default maximum decompression expansion ratio (100:1).
    pub const DEFAULT_MAX_EXPANSION_RATIO: u32 = 100;
    /// Default large window authorization policy (false).
    pub const DEFAULT_ALLOW_LARGE_WINDOW: bool = false;
    /// Default threshold before ratio enforcement starts (1 MiB).
    pub const DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;

    /// Creates a new `BrotliDefenseConfig` with explicit core parameters.
    #[must_use]
    pub const fn new(
        max_output_limit: u64,
        max_window_bits: u8,
        max_expansion_ratio: u32,
        allow_large_window: bool,
    ) -> Self {
        Self {
            max_output_limit,
            max_window_bits,
            max_expansion_ratio,
            allow_large_window,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
        }
    }

    /// Creates default production security limits.
    #[must_use]
    pub const fn default_limits() -> Self {
        Self {
            max_output_limit: Self::DEFAULT_MAX_OUTPUT_LIMIT,
            max_window_bits: Self::DEFAULT_MAX_WINDOW_BITS,
            max_expansion_ratio: Self::DEFAULT_MAX_EXPANSION_RATIO,
            allow_large_window: Self::DEFAULT_ALLOW_LARGE_WINDOW,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
        }
    }

    /// Sets custom cumulative decompressed output limit.
    #[must_use]
    pub const fn with_max_output_limit(mut self, max_output_limit: u64) -> Self {
        self.max_output_limit = max_output_limit;
        self
    }

    /// Sets custom sliding window bits ceiling.
    #[must_use]
    pub const fn with_max_window_bits(mut self, max_window_bits: u8) -> Self {
        self.max_window_bits = max_window_bits;
        self
    }

    /// Sets custom expansion ratio breaker limit.
    #[must_use]
    pub const fn with_max_expansion_ratio(mut self, max_expansion_ratio: u32) -> Self {
        self.max_expansion_ratio = max_expansion_ratio;
        self
    }

    /// Sets whether Large Window extension is allowed.
    #[must_use]
    pub const fn with_allow_large_window(mut self, allow_large_window: bool) -> Self {
        self.allow_large_window = allow_large_window;
        self
    }

    /// Sets custom warmup threshold in bytes before ratio check activates.
    #[must_use]
    pub const fn with_threshold_bytes(mut self, threshold_bytes: u64) -> Self {
        self.threshold_bytes = threshold_bytes;
        self
    }
}

/// Active 7-layer defense guard and decompression bomb circuit breaker for Brotli streams.
#[derive(Debug, Clone)]
pub struct BrotliDefenseGuard {
    /// Defense configuration parameters and quota policies.
    pub config: BrotliDefenseConfig,
    /// Cumulative compressed bytes consumed from the input stream.
    pub bytes_read: u64,
    /// Cumulative uncompressed bytes produced to the output sink.
    pub bytes_written: u64,
}

impl Default for BrotliDefenseGuard {
    #[inline]
    fn default() -> Self {
        Self::new(BrotliDefenseConfig::default())
    }
}

impl BrotliDefenseGuard {
    /// Creates a new guard bound to the specified defense configuration.
    #[must_use]
    pub const fn new(config: BrotliDefenseConfig) -> Self {
        Self {
            config,
            bytes_read: 0,
            bytes_written: 0,
        }
    }

    /// Creates a guard with default configuration overriding only `max_output_limit`.
    #[must_use]
    pub const fn with_output_limit(max_output_limit: u64) -> Self {
        Self {
            config: BrotliDefenseConfig::default_limits().with_max_output_limit(max_output_limit),
            bytes_read: 0,
            bytes_written: 0,
        }
    }

    /// Validates requested sliding window bits against RFC 7932 / RFC 9841 bounds and authorization.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if:
    /// - `wbits < BROTLI_MIN_WINDOW_BITS` (10)
    /// - `wbits > BROTLI_LARGE_MAX_WINDOW_BITS` (30)
    /// - `wbits > self.config.max_window_bits`
    /// - `wbits > BROTLI_MAX_WINDOW_BITS` (24) when `allow_large_window` is false
    pub fn validate_window_bits(&self, wbits: u8) -> Result<(), TTZipStatus> {
        if !(BROTLI_MIN_WINDOW_BITS..=BROTLI_LARGE_MAX_WINDOW_BITS).contains(&wbits) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if !self.config.allow_large_window && wbits > BROTLI_MAX_WINDOW_BITS {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if wbits > self.config.max_window_bits {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(())
    }

    /// Validates meta-block size nibble count and highest nibble value against Exuberant Nibble attacks.
    ///
    /// In RFC 7932 Section 9.2, uncompressed/compressed meta-block headers encode size in 4..=7 nibbles.
    /// If `size_nibbles > 4`, the highest (last decoded) nibble MUST NOT be zero. A value of zero
    /// constitutes an "Exuberant Nibble" (non-canonical representation) and MUST be rejected.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `size_nibbles` is out of 4..=7 range,
    /// if `high_nibble > 0x0F`, or if `size_nibbles > 4 && high_nibble == 0`.
    pub fn validate_meta_block_nibbles(
        &self,
        size_nibbles: u8,
        high_nibble: u8,
    ) -> Result<(), TTZipStatus> {
        if !(4..=7).contains(&size_nibbles) || high_nibble > 0x0F {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if size_nibbles > 4 && high_nibble == 0 {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(())
    }

    /// Validates metadata block byte count and highest byte value against Exuberant Meta Nibble attacks.
    ///
    /// In RFC 7932 Section 9.2, metadata block headers encode size in 1..=4 bytes.
    /// If `size_bytes > 1`, the highest byte MUST NOT be zero.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `size_bytes` is out of 1..=4 range
    /// or if `size_bytes > 1 && high_byte == 0`.
    pub fn validate_metadata_nibbles(
        &self,
        size_bytes: u8,
        high_byte: u8,
    ) -> Result<(), TTZipStatus> {
        if !(1..=4).contains(&size_bytes) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if size_bytes > 1 && high_byte == 0 {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(())
    }

    /// Validates backward reference distance against the `0x7FFFFFFC` hard ceiling.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `distance > BROTLI_MAX_ALLOWED_DISTANCE`
    /// or if `distance == 0`.
    pub fn validate_distance(&self, distance: usize) -> Result<(), TTZipStatus> {
        if distance == 0 || distance > BROTLI_MAX_ALLOWED_DISTANCE {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(())
    }

    /// Validates that padding bits used when jumping to a byte boundary are strictly zero.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `padding_bits > 7` or `padding_value != 0`.
    pub fn validate_padding(&self, padding_bits: u8, padding_value: u8) -> Result<(), TTZipStatus> {
        if padding_bits > 7 || padding_value != 0 {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(())
    }

    /// Tracks incremental decompression chunks, enforcing cumulative output limits and expansion ratio circuit breakers.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if:
    /// 1. Cumulative `bytes_written` exceeds `config.max_output_limit`.
    /// 2. Beyond `threshold_bytes` warmup, the cumulative expansion ratio (`bytes_written / bytes_read`)
    ///    exceeds `config.max_expansion_ratio`.
    pub fn track_decompression(
        &mut self,
        compressed_chunk: usize,
        decompressed_chunk: usize,
    ) -> Result<(), TTZipStatus> {
        self.bytes_read = self.bytes_read.saturating_add(compressed_chunk as u64);
        self.bytes_written = self.bytes_written.saturating_add(decompressed_chunk as u64);

        // 1. Enforce hard cumulative uncompressed output budget
        if self.bytes_written > self.config.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // 2. Enforce decompression bomb expansion ratio circuit breaker once past warmup threshold
        if self.bytes_written > self.config.threshold_bytes {
            let comp = self.bytes_read.max(1) as f64;
            let uncomp = self.bytes_written as f64;
            let ratio = uncomp / comp;
            if ratio > self.config.max_expansion_ratio as f64 {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }

    /// Returns the current cumulative decompression expansion ratio.
    #[must_use]
    pub fn current_ratio(&self) -> f64 {
        let comp = self.bytes_read.max(1) as f64;
        (self.bytes_written as f64) / comp
    }

    /// Returns the cumulative compressed bytes consumed so far.
    #[inline]
    #[must_use]
    pub const fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Returns the cumulative decompressed bytes produced so far.
    #[inline]
    #[must_use]
    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Resets decompression tracking byte counters to zero while preserving configuration.
    pub fn reset(&mut self) {
        self.bytes_read = 0;
        self.bytes_written = 0;
    }
}
