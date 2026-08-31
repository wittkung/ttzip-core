// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` and `LZVN` 6-Layer Defense-in-Depth Guard and Decompression Bomb Circuit Breaker.
//!
//! Enforces deterministic memory bounds and strict protocol-level defenses against malicious
//! LZFSE and LZVN bitstreams across 6 orthogonal security layers:
//!
//! 1. **Magic & Block Header Validation**: Validates container magic identifiers (`bvx-`, `bvx1`,
//!    `bvx2`, `bvxn`, `bvx$`), payload size bounds, header sizes, and match/literal counts.
//! 2. **Decompression Bomb Quota Breaker**: Real-time cumulative output budget (default: 512 MiB),
//!    per-block uncompressed size ceiling (1 MiB), and expansion ratio breaker (100:1).
//! 3. **Match Distance Underflow Defense**: Intercepts zero distance ($D == 0$) and backward distance
//!    underflows ($D > \text{dst\_pos}$) to prevent heap/buffer underflow attacks.
//! 4. **FSE Frequency Table Conservation & State Bounds**: Validates symbol frequency sums
//!    $\sum \text{freq} \le N_{\text{states}}$ for L (64), M (64), D (256), and Literal (1024) tables,
//!    and ensures initial/runtime states remain strictly bounded.
//! 5. **Reverse LIFO Bitstream Boundary Sentinel**: Bounds backward-reading bitstream cursors,
//!    prevents negative indexing past buffer heads, and validates bitstream accumulator invariants.
//! 6. **Zero-Panic & Deterministic Error Mapping**: Guarantees panic-free execution under arbitrary
//!    hostile or corrupted payloads, mapping all anomalies deterministically to [`TTZipStatus`].

use crate::codecs::lzfse::block::{
    BvxMagic, LzfseBlockHeader, LzfseFreqTables, LZFSE_ENCODE_D_STATES,
    LZFSE_ENCODE_LITERAL_STATES, LZFSE_ENCODE_L_STATES, LZFSE_ENCODE_M_STATES,
    LZFSE_LITERALS_PER_BLOCK, LZFSE_MATCHES_PER_BLOCK, LZFSE_V2_HEADER_FIXED_SIZE,
};
use crate::types::TTZipStatus;
use std::panic::{catch_unwind, AssertUnwindSafe, UnwindSafe};

// MARK: - Constants & Security Defaults

/// Default maximum cumulative uncompressed output budget (512 MiB).
pub const LZFSE_DEFAULT_MAX_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;

/// Default maximum allowable decompression expansion ratio (100:1).
pub const LZFSE_DEFAULT_MAX_EXPANSION_RATIO: u32 = 100;

/// Default maximum uncompressed size allowed for a single block (1 MiB).
pub const LZFSE_DEFAULT_MAX_BLOCK_UNCOMPRESSED_SIZE: usize = 1024 * 1024;

/// Default uncompressed output threshold before expansion ratio checks activate (1 MiB).
pub const LZFSE_DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;

/// Standard Apple LZFSE block uncompressed capacity (256 KiB).
pub const LZFSE_STANDARD_BLOCK_SIZE: usize = 256 * 1024;

/// Maximum allowable backward reference distance (`0x7FFFFFFF` = 2 GiB).
pub const LZFSE_MAX_BACKWARD_DISTANCE: usize = 0x7FFF_FFFF;

// MARK: - Security Limits Configuration

/// Configuration limits for LZFSE and LZVN decompression defense and resource budget enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LzfseSecurityLimits {
    /// Maximum cumulative uncompressed output size in bytes (default: 512 MiB).
    pub max_output_limit: u64,
    /// Maximum allowable decompression expansion ratio (default: 100 for 100:1).
    pub max_expansion_ratio: u32,
    /// Maximum uncompressed bytes permitted per single block (default: 1 MiB).
    pub max_block_uncompressed_size: usize,
    /// Threshold in uncompressed bytes before expansion ratio check is enforced (default: 1 MiB).
    pub threshold_bytes: u64,
    /// Maximum number of matches permitted per compressed block (default: 10,000).
    pub max_matches_per_block: usize,
    /// Maximum number of literals permitted per compressed block (default: 40,000).
    pub max_literals_per_block: usize,
}

impl Default for LzfseSecurityLimits {
    #[inline]
    fn default() -> Self {
        Self::default_limits()
    }
}

impl LzfseSecurityLimits {
    /// Creates a new `LzfseSecurityLimits` with explicit core parameters.
    #[must_use]
    pub const fn new(
        max_output_limit: u64,
        max_expansion_ratio: u32,
        max_block_uncompressed_size: usize,
    ) -> Self {
        Self {
            max_output_limit,
            max_expansion_ratio,
            max_block_uncompressed_size,
            threshold_bytes: LZFSE_DEFAULT_THRESHOLD_BYTES,
            max_matches_per_block: LZFSE_MATCHES_PER_BLOCK,
            max_literals_per_block: LZFSE_LITERALS_PER_BLOCK,
        }
    }

    /// Creates default production security limits.
    #[must_use]
    pub const fn default_limits() -> Self {
        Self {
            max_output_limit: LZFSE_DEFAULT_MAX_OUTPUT_LIMIT,
            max_expansion_ratio: LZFSE_DEFAULT_MAX_EXPANSION_RATIO,
            max_block_uncompressed_size: LZFSE_DEFAULT_MAX_BLOCK_UNCOMPRESSED_SIZE,
            threshold_bytes: LZFSE_DEFAULT_THRESHOLD_BYTES,
            max_matches_per_block: LZFSE_MATCHES_PER_BLOCK,
            max_literals_per_block: LZFSE_LITERALS_PER_BLOCK,
        }
    }

    /// Sets custom cumulative decompressed output limit.
    #[must_use]
    pub const fn with_max_output_limit(mut self, limit: u64) -> Self {
        self.max_output_limit = limit;
        self
    }

    /// Sets custom decompression expansion ratio breaker.
    #[must_use]
    pub const fn with_max_expansion_ratio(mut self, ratio: u32) -> Self {
        self.max_expansion_ratio = ratio;
        self
    }

    /// Sets custom per-block uncompressed size ceiling.
    #[must_use]
    pub const fn with_max_block_uncompressed_size(mut self, size: usize) -> Self {
        self.max_block_uncompressed_size = size;
        self
    }

    /// Sets custom warmup threshold in bytes before ratio check activates.
    #[must_use]
    pub const fn with_threshold_bytes(mut self, threshold: u64) -> Self {
        self.threshold_bytes = threshold;
        self
    }

    /// Sets custom maximum matches per block.
    #[must_use]
    pub const fn with_max_matches_per_block(mut self, max_matches: usize) -> Self {
        self.max_matches_per_block = max_matches;
        self
    }

    /// Sets custom maximum literals per block.
    #[must_use]
    pub const fn with_max_literals_per_block(mut self, max_literals: usize) -> Self {
        self.max_literals_per_block = max_literals;
        self
    }
}

/// Alias for `LzfseSecurityLimits` matching unified defense naming conventions.
pub type LzfseDefenseConfig = LzfseSecurityLimits;

// MARK: - Active Defense Guard & Circuit Breaker

/// Active runtime guard tracking decompression progress, enforcing quotas, and breaking circuits on attacks.
#[derive(Debug, Clone)]
pub struct LzfseDefenseGuard {
    limits: LzfseSecurityLimits,
    bytes_read: u64,
    bytes_written: u64,
    blocks_processed: u64,
}

impl Default for LzfseDefenseGuard {
    #[inline]
    fn default() -> Self {
        Self::new(LzfseSecurityLimits::default())
    }
}

impl LzfseDefenseGuard {
    /// Creates a new `LzfseDefenseGuard` bound to the specified limits.
    #[must_use]
    pub const fn new(limits: LzfseSecurityLimits) -> Self {
        Self {
            limits,
            bytes_read: 0,
            bytes_written: 0,
            blocks_processed: 0,
        }
    }

    /// Creates a guard with default limits overriding only `max_output_limit`.
    #[must_use]
    pub const fn with_output_limit(max_output_limit: u64) -> Self {
        Self {
            limits: LzfseSecurityLimits::default_limits().with_max_output_limit(max_output_limit),
            bytes_read: 0,
            bytes_written: 0,
            blocks_processed: 0,
        }
    }

    /// Returns the active security limits.
    #[inline]
    pub const fn limits(&self) -> &LzfseSecurityLimits {
        &self.limits
    }

    /// Returns cumulative input bytes read.
    #[inline]
    pub const fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Returns cumulative output bytes written.
    #[inline]
    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Returns the number of blocks processed.
    #[inline]
    pub const fn blocks_processed(&self) -> u64 {
        self.blocks_processed
    }

    /// Returns the current cumulative decompression expansion ratio.
    #[must_use]
    pub fn current_ratio(&self) -> f64 {
        let comp = self.bytes_read.max(1) as f64;
        (self.bytes_written as f64) / comp
    }

    /// Resets runtime byte counters while preserving configuration limits.
    pub fn reset(&mut self) {
        self.bytes_read = 0;
        self.bytes_written = 0;
        self.blocks_processed = 0;
    }

    // MARK: - Layer 1: Magic & Block Header Validation

    /// Validates 4-byte container magic identifier against standard LZFSE magic values.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if magic does not match any valid BVX identifier.
    pub fn validate_magic(magic_u32: u32) -> Result<BvxMagic, TTZipStatus> {
        BvxMagic::from_u32(magic_u32).ok_or(TTZipStatus::ErrCorruptHeader)
    }

    /// Validates block header structural parameters against container limits.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` or `Err(TTZipStatus::ErrCorruptHeader)` if
    /// parameters exceed security boundaries or violate protocol constraints.
    pub fn validate_block_header(&self, header: &LzfseBlockHeader) -> Result<(), TTZipStatus> {
        // 1. Validate uncompressed raw bytes against per-block limit
        if (header.n_raw_bytes as usize) > self.limits.max_block_uncompressed_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // 2. Validate block-specific constraints
        match header.magic {
            BvxMagic::RawUncompressed => {
                if header.header_size < 8 {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            BvxMagic::CompressedLZVN => {
                if header.header_size < 12 {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            BvxMagic::EndOfStream => {
                if header.header_size < 4 {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                if (header.n_literals as usize) > self.limits.max_literals_per_block {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                if (header.n_matches as usize) > self.limits.max_matches_per_block {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
                if header.magic == BvxMagic::CompressedV2
                    && (header.header_size as usize) < LZFSE_V2_HEADER_FIXED_SIZE
                {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }

                // Validate FSE states and frequency tables
                Self::validate_fse_states(
                    header.l_state,
                    header.m_state,
                    header.d_state,
                    &header.literal_state,
                )?;
                if let Some(tables) = &header.freq_tables {
                    Self::validate_fse_freq_tables(tables)?;
                }
            }
        }

        Ok(())
    }

    /// Validates raw uncompressed block size against single-block maximum capacity.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `raw_bytes` exceeds `max_block_uncompressed_size`.
    pub fn validate_raw_block_size(&self, raw_bytes: usize) -> Result<(), TTZipStatus> {
        if raw_bytes > self.limits.max_block_uncompressed_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    // MARK: - Layer 2: Decompression Bomb & Quota Circuit Breaker

    /// Tracks decompressed chunk progress and enforces cumulative output budgets and expansion ratios.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if cumulative uncompressed size exceeds
    /// `max_output_limit` or if expansion ratio exceeds `max_expansion_ratio` past warmup.
    pub fn track_decompression(
        &mut self,
        compressed_chunk: usize,
        decompressed_chunk: usize,
    ) -> Result<(), TTZipStatus> {
        self.bytes_read = self
            .bytes_read
            .checked_add(compressed_chunk as u64)
            .ok_or(TTZipStatus::ErrSecurityViolation)?;
        self.bytes_written = self
            .bytes_written
            .checked_add(decompressed_chunk as u64)
            .ok_or(TTZipStatus::ErrSecurityViolation)?;

        // 1. Cumulative physical uncompressed output budget breaker
        if self.bytes_written > self.limits.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // 2. Expansion ratio circuit breaker once past warmup threshold
        if self.bytes_written > self.limits.threshold_bytes {
            let max_allowed = self
                .bytes_read
                .max(1)
                .saturating_mul(self.limits.max_expansion_ratio as u64);
            if self.bytes_written > max_allowed {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }

    /// Tracks a completed block decompression and increments block counter.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if quotas are exceeded.
    pub fn track_block(
        &mut self,
        compressed_len: usize,
        uncompressed_len: usize,
    ) -> Result<(), TTZipStatus> {
        self.validate_raw_block_size(uncompressed_len)?;
        self.track_decompression(compressed_len, uncompressed_len)?;
        self.blocks_processed += 1;
        Ok(())
    }

    /// Pre-validates declared uncompressed size against policy limit and pre-flight expansion ratio.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if declared output violates security boundaries.
    pub fn validate_declared_decompressed_size(
        &self,
        declared_size: u64,
        compressed_size: u64,
    ) -> Result<(), TTZipStatus> {
        if declared_size > self.limits.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if declared_size > self.limits.threshold_bytes && compressed_size > 0 {
            let max_allowed = compressed_size.saturating_mul(self.limits.max_expansion_ratio as u64);
            if declared_size > max_allowed {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }

    // MARK: - Layer 3: Match Distance Underflow Defense

    /// Validates a backward match distance reference against the current destination write position.
    ///
    /// # Invariants
    /// 1. Zero distance ($D == 0$) is invalid and MUST be rejected.
    /// 2. Backward distance MUST NOT exceed current write position ($D \le \text{dst\_pos}$).
    /// 3. Backward distance MUST NOT exceed global distance limit ($D \le \text{LZFSE\_MAX\_BACKWARD\_DISTANCE}$).
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if distance is 0, exceeds position, or exceeds maximum limit.
    #[inline]
    pub fn validate_match_distance(distance: usize, current_dst_pos: usize) -> Result<(), TTZipStatus> {
        if distance == 0 || distance > current_dst_pos || distance > LZFSE_MAX_BACKWARD_DISTANCE {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Validates an LZVN backward match distance reference against current destination cursor.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if distance is 0 or exceeds current destination cursor.
    #[inline]
    pub fn validate_lzvn_distance(distance: usize, current_dst_pos: usize) -> Result<(), TTZipStatus> {
        Self::validate_match_distance(distance, current_dst_pos)
    }

    // MARK: - Layer 4: FSE Frequency Table Conservation & State Bounds

    /// Validates that an FSE normalized frequency table respects the probability conservation law:
    /// $\sum_{i=0}^{N-1} \text{freq}[i] \le N_{\text{states}}$.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if the sum of symbol frequencies exceeds state capacity.
    pub fn validate_fse_freq_table(freqs: &[u16], max_states: usize) -> Result<(), TTZipStatus> {
        let sum: usize = freqs.iter().map(|&v| v as usize).sum();
        if sum > max_states {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(())
    }

    /// Validates all four FSE normalized frequency tables (L, M, D, Literal) against standard state capacities.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if any frequency sum exceeds its corresponding state capacity.
    pub fn validate_fse_freq_tables(tables: &LzfseFreqTables) -> Result<(), TTZipStatus> {
        Self::validate_fse_freq_table(&tables.l_freq, LZFSE_ENCODE_L_STATES)?;
        Self::validate_fse_freq_table(&tables.m_freq, LZFSE_ENCODE_M_STATES)?;
        Self::validate_fse_freq_table(&tables.d_freq, LZFSE_ENCODE_D_STATES)?;
        Self::validate_fse_freq_table(&tables.literal_freq, LZFSE_ENCODE_LITERAL_STATES)?;
        Ok(())
    }

    /// Validates that initial or runtime FSE states do not exceed maximum table capacities.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if any state index is out of bounds.
    pub fn validate_fse_states(
        l_state: u16,
        m_state: u16,
        d_state: u16,
        literal_states: &[u16; 4],
    ) -> Result<(), TTZipStatus> {
        if (l_state as usize) >= LZFSE_ENCODE_L_STATES
            || (m_state as usize) >= LZFSE_ENCODE_M_STATES
            || (d_state as usize) >= LZFSE_ENCODE_D_STATES
        {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        for &state in literal_states {
            if (state as usize) >= LZFSE_ENCODE_LITERAL_STATES {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }

        Ok(())
    }

    // MARK: - Layer 5: Reverse LIFO Bitstream Boundary Sentinel

    /// Validates LIFO reverse bitstream payload boundaries and initial bit count offset.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if payload size is insufficient or `initial_bits`
    /// is outside allowable range `[-7, 0]`.
    pub fn validate_lifo_stream_bounds(
        payload_len: usize,
        initial_bits: i32,
        min_header_bytes: usize,
    ) -> Result<(), TTZipStatus> {
        if !(-7..=0).contains(&initial_bits) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let required_bytes = if initial_bits != 0 { 8 } else { 7 };
        if payload_len < required_bytes.max(min_header_bytes) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(())
    }

    /// Validates that a reverse LIFO cursor retreat operation will not underflow past buffer head.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `bytes_to_retreat > current_cursor`.
    #[inline]
    pub fn validate_lifo_cursor_retreat(
        current_cursor: usize,
        bytes_to_retreat: usize,
    ) -> Result<usize, TTZipStatus> {
        current_cursor
            .checked_sub(bytes_to_retreat)
            .ok_or(TTZipStatus::ErrSecurityViolation)
    }

    /// Validates that a 64-bit reverse bitstream accumulator has valid bit count and no dirty upper bits.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if bit count is not in `56..64` or upper bits are non-zero.
    pub fn validate_accumulator_state(accum: u64, accum_nbits: i32) -> Result<(), TTZipStatus> {
        if !(56..64).contains(&accum_nbits) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if accum_nbits < 64 && (accum >> accum_nbits) != 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(())
    }

    // MARK: - Layer 6: Zero-Panic & Deterministic Error Mapping

    /// Executes a closure inside a panic catch barrier, converting any unexpected panics into deterministic `TTZipStatus`.
    ///
    /// # Invariants
    /// Guarantees that hostile, corrupt, or fuzz-generated inputs never crash or abort the host process.
    pub fn guarantee_zero_panic<F, T>(f: F) -> Result<T, TTZipStatus>
    where
        F: FnOnce() -> Result<T, TTZipStatus> + UnwindSafe,
    {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(res) => res,
            Err(_) => Err(TTZipStatus::ErrSecurityViolation),
        }
    }
}
