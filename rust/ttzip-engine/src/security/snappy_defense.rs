// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Snappy 6-Layer Defense-in-Depth Guard and Decompression Bomb Circuit Breaker Subsystem.
//!
//! Enforces deterministic memory bounds and strict protocol-level defenses against malicious Snappy bitstreams:
//! 1. **Varint-32 Overflow & Header Truncation Guard**: Rejects 5-byte continuation overflows and high 4-bit non-zero attacks.
//! 2. **Zero Offset & Out-of-Bounds Copy Guard**: Intercepts `offset == 0` loops and `offset > produced_bytes` underflow exploits.
//! 3. **Decompression Bomb Quota Breaker**: Enforces cumulative output size limit (default: 512 MiB) and expansion ratio limits.
//! 4. **Framed Chunk 64KB Bound Guard**: Enforces strict 65,536-byte max uncompressed chunk limits per spec.
//! 5. **Masked Castagnoli CRC-32C Guard**: Rejects tampered payloads via hardware-accelerated CRC-32C verification.
//! 6. **Zero-Panic & Deterministic Error Mapping Invariant**: Guarantees panic-free execution on any arbitrary corrupted input.

use crate::codecs::snappy::error::SnappyError;
use crate::codecs::snappy::frame::MAX_UNCOMPRESSED_CHUNK_SIZE;
use crate::types::TTZipStatus;

/// Maximum allowable uncompressed size for a single raw Snappy block (default: 512 MiB).
pub const SNAPPY_MAX_ALLOWED_BLOCK_SIZE: usize = 512 * 1024 * 1024;

/// Configuration parameters for Snappy decompression defense and resource budget enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnappyDefenseConfig {
    /// Maximum cumulative uncompressed output size in bytes (default: 512 MiB).
    pub max_output_limit: u64,
    /// Maximum allowable uncompressed chunk size (default: 65,536 bytes / 64 KiB).
    pub max_chunk_size: usize,
    /// Maximum allowable decompression expansion ratio (e.g. 100 for 100:1, default: 100).
    pub max_expansion_ratio: u32,
    /// Threshold in uncompressed bytes before expansion ratio check is enforced (default: 1 MiB).
    pub threshold_bytes: u64,
}

impl Default for SnappyDefenseConfig {
    #[inline]
    fn default() -> Self {
        Self::default_limits()
    }
}

impl SnappyDefenseConfig {
    /// Default maximum cumulative uncompressed limit (512 MiB).
    pub const DEFAULT_MAX_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;
    /// Default maximum uncompressed chunk size (64 KiB).
    pub const DEFAULT_MAX_CHUNK_SIZE: usize = MAX_UNCOMPRESSED_CHUNK_SIZE;
    /// Default maximum decompression expansion ratio (100:1).
    pub const DEFAULT_MAX_EXPANSION_RATIO: u32 = 100;
    /// Default threshold before ratio enforcement starts (1 MiB).
    pub const DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;

    /// Creates a new `SnappyDefenseConfig` with explicit core parameters.
    #[must_use]
    pub const fn new(
        max_output_limit: u64,
        max_chunk_size: usize,
        max_expansion_ratio: u32,
    ) -> Self {
        Self {
            max_output_limit,
            max_chunk_size,
            max_expansion_ratio,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
        }
    }

    /// Creates default production security limits.
    #[must_use]
    pub const fn default_limits() -> Self {
        Self {
            max_output_limit: Self::DEFAULT_MAX_OUTPUT_LIMIT,
            max_chunk_size: Self::DEFAULT_MAX_CHUNK_SIZE,
            max_expansion_ratio: Self::DEFAULT_MAX_EXPANSION_RATIO,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
        }
    }

    /// Sets custom cumulative decompressed output limit.
    #[must_use]
    pub const fn with_max_output_limit(mut self, limit: u64) -> Self {
        self.max_output_limit = limit;
        self
    }
}

/// Active runtime guard tracking decompression progress, verifying invariants, and breaking circuits on attacks.
#[derive(Debug, Clone)]
pub struct SnappyDefenseGuard {
    config: SnappyDefenseConfig,
    bytes_read: u64,
    bytes_written: u64,
    chunk_count: u64,
}

impl Default for SnappyDefenseGuard {
    #[inline]
    fn default() -> Self {
        Self::new(SnappyDefenseConfig::default())
    }
}

impl SnappyDefenseGuard {
    /// Creates a new `SnappyDefenseGuard` with specified configuration.
    #[must_use]
    pub const fn new(config: SnappyDefenseConfig) -> Self {
        Self {
            config,
            bytes_read: 0,
            bytes_written: 0,
            chunk_count: 0,
        }
    }

    /// Returns the active security configuration.
    #[inline]
    pub const fn config(&self) -> &SnappyDefenseConfig {
        &self.config
    }

    /// Returns cumulative input bytes read.
    #[inline]
    pub const fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    /// Returns cumulative output bytes produced.
    #[inline]
    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Returns the number of chunks processed.
    #[inline]
    pub const fn chunk_count(&self) -> u64 {
        self.chunk_count
    }

    /// Validates raw declared uncompressed size against policy limit.
    pub fn validate_raw_uncompressed_length(&self, declared_len: usize) -> Result<(), TTZipStatus> {
        if (declared_len as u64) > self.config.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Validates a framed chunk size against the 64KB spec boundary.
    pub fn validate_chunk_size(&self, chunk_len: usize) -> Result<(), TTZipStatus> {
        if chunk_len > self.config.max_chunk_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Validates a copy offset against the current decompressed cursor.
    #[inline]
    pub fn validate_copy_offset(&self, offset: usize, current_pos: usize) -> Result<(), SnappyError> {
        if offset == 0 {
            return Err(SnappyError::InvalidOffset {
                offset: 0,
                position: current_pos,
            });
        }
        if offset > current_pos {
            return Err(SnappyError::OffsetOutOfBounds {
                offset,
                current_pos,
            });
        }
        Ok(())
    }

    /// Tracks decompressed chunk progress and verifies cumulative budget and expansion ratio invariants.
    pub fn track_decompression(
        &mut self,
        compressed_bytes: usize,
        decompressed_bytes: usize,
    ) -> Result<(), TTZipStatus> {
        self.bytes_read = self
            .bytes_read
            .checked_add(compressed_bytes as u64)
            .ok_or(TTZipStatus::ErrSecurityViolation)?;
        self.bytes_written = self
            .bytes_written
            .checked_add(decompressed_bytes as u64)
            .ok_or(TTZipStatus::ErrSecurityViolation)?;
        self.chunk_count += 1;

        // 1. Enforce cumulative uncompressed output budget
        if self.bytes_written > self.config.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // 2. Enforce expansion ratio limit if output exceeds threshold
        if self.bytes_written > self.config.threshold_bytes && self.bytes_read > 0 {
            let max_allowed = self
                .bytes_read
                .saturating_mul(self.config.max_expansion_ratio as u64);
            if self.bytes_written > max_allowed {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }
}
