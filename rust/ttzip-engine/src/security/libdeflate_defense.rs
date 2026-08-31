// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Libdeflate 6-Layer Defense-in-Depth Guard and Decompression Bomb Circuit Breaker Subsystem.
//!
//! Enforces deterministic memory bounds and strict protocol-level defenses against malicious Deflate bitstreams:
//! 1. **Output Quota Circuit Breaker & Decompression Bomb Defense**: 1032x theoretical maximum expansion
//!    ratio ceiling and cumulative uncompressed size limits (`max_output_limit`).
//! 2. **Backward Reference Distance Underflow Defense**: Intercepts `offset == 0` and `offset > dst_pos`
//!    out-of-bounds backward references to prevent buffer underflow and memory traversal attacks.
//! 3. **Malformed Huffman Tree & Codespace Overload Defense**: Enforces Kraft-McMillan inequality and
//!    canonical completeness checks on literal/length, offset, and precode trees.
//! 4. **Bitstream Overread & Boundary Guard**: Restricts maximum bitstream overread (`overread_count <= 8`)
//!    and bounds stream refills.
//! 5. **Uncompressed Block Inverted Length Invariant**: Enforces `LEN != !NLEN` check on uncompressed blocks
//!    to reject tampered block headers.
//! 6. **Sensitive Memory Scrubbing & Zeroize Protection**: Uses `zeroize` and `ZeroizeOnDrop` to scrub
//!    internal state, temporary buffers, and decoded tables on drop.

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::codecs::deflate::deflate_decompress;
use crate::codecs::libdeflate::container::{
    gzip_decompress, zlib_decompress, ContainerFormat, GZIP_CM_DEFLATE, GZIP_FRESERVED, GZIP_ID1,
    GZIP_ID2, GZIP_MIN_HEADER_SIZE, ZLIB_CINFO_32K_WINDOW, ZLIB_CM_DEFLATE, ZLIB_MIN_HEADER_SIZE,
};
use crate::types::TTZipStatus;

// MARK: - Constants & Security Defaults

/// Theoretical maximum single-stream expansion ratio in RFC 1951 Deflate (1032:1).
///
/// 258 uncompressed match bytes produced from a 2-bit fixed Huffman code = 1032x expansion.
pub const LIBDEFLATE_MAX_EXPANSION_RATIO: u32 = 1032;

/// Default maximum cumulative uncompressed output budget (512 MiB).
pub const LIBDEFLATE_DEFAULT_MAX_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;

/// Default uncompressed output threshold before expansion ratio checks activate (1 MiB).
pub const LIBDEFLATE_DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;

/// Maximum allowable sliding window backward distance in standard Deflate (32 KiB).
pub const LIBDEFLATE_MAX_ALLOWED_DISTANCE: usize = 32768;

/// Maximum allowable bitstream overread count in 64-bit refill buffers (8 bytes).
pub const LIBDEFLATE_MAX_OVERREAD_BYTES: usize = 8;

// MARK: - Security Configuration

/// Configuration parameters for Libdeflate decompression defense and resource budget enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibdeflateDefenseConfig {
    /// Maximum cumulative uncompressed output size in bytes (default: 512 MiB).
    pub max_output_limit: u64,
    /// Maximum allowable decompression expansion ratio (default: 1032 for 1032:1).
    pub max_expansion_ratio: u32,
    /// Threshold in uncompressed bytes before expansion ratio check is enforced (default: 1 MiB).
    pub threshold_bytes: u64,
    /// Maximum allowable backward reference distance in bytes (default: 32,768).
    pub max_distance: usize,
}

impl Default for LibdeflateDefenseConfig {
    #[inline]
    fn default() -> Self {
        Self::default_limits()
    }
}

impl LibdeflateDefenseConfig {
    /// Creates a new `LibdeflateDefenseConfig` with explicit core parameters.
    #[must_use]
    pub const fn new(max_output_limit: u64, max_expansion_ratio: u32) -> Self {
        Self {
            max_output_limit,
            max_expansion_ratio,
            threshold_bytes: LIBDEFLATE_DEFAULT_THRESHOLD_BYTES,
            max_distance: LIBDEFLATE_MAX_ALLOWED_DISTANCE,
        }
    }

    /// Creates default production security limits.
    #[must_use]
    pub const fn default_limits() -> Self {
        Self {
            max_output_limit: LIBDEFLATE_DEFAULT_MAX_OUTPUT_LIMIT,
            max_expansion_ratio: LIBDEFLATE_MAX_EXPANSION_RATIO,
            threshold_bytes: LIBDEFLATE_DEFAULT_THRESHOLD_BYTES,
            max_distance: LIBDEFLATE_MAX_ALLOWED_DISTANCE,
        }
    }

    /// Sets custom cumulative uncompressed output limit.
    #[must_use]
    pub const fn with_max_output_limit(mut self, max_output_limit: u64) -> Self {
        self.max_output_limit = max_output_limit;
        self
    }

    /// Sets custom expansion ratio circuit breaker ceiling.
    #[must_use]
    pub const fn with_max_expansion_ratio(mut self, max_expansion_ratio: u32) -> Self {
        self.max_expansion_ratio = max_expansion_ratio;
        self
    }

    /// Sets custom warmup threshold in bytes before expansion ratio check activates.
    #[must_use]
    pub const fn with_threshold_bytes(mut self, threshold_bytes: u64) -> Self {
        self.threshold_bytes = threshold_bytes;
        self
    }

    /// Sets custom maximum allowable backward reference distance.
    #[must_use]
    pub const fn with_max_distance(mut self, max_distance: usize) -> Self {
        self.max_distance = max_distance;
        self
    }
}

// MARK: - Active Security Guard

/// Active 6-layer defense guard and decompression bomb circuit breaker for Deflate streams.
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct LibdeflateSecurityGuard {
    /// Defense configuration parameters and quota policies.
    #[zeroize(skip)]
    pub config: LibdeflateDefenseConfig,
    /// Cumulative compressed bytes consumed from the input stream.
    pub bytes_read: u64,
    /// Cumulative uncompressed bytes produced to the output sink.
    pub bytes_written: u64,
    /// Secure internal scratchpad wiped on drop.
    pub secure_scratch: [u8; 64],
}

impl Default for LibdeflateSecurityGuard {
    #[inline]
    fn default() -> Self {
        Self::new(LibdeflateDefenseConfig::default())
    }
}

impl LibdeflateSecurityGuard {
    /// Creates a new guard bound to the specified defense configuration.
    #[must_use]
    pub const fn new(config: LibdeflateDefenseConfig) -> Self {
        Self {
            config,
            bytes_read: 0,
            bytes_written: 0,
            secure_scratch: [0u8; 64],
        }
    }

    /// Creates a guard with default configuration overriding only `max_output_limit`.
    #[must_use]
    pub const fn with_output_limit(max_output_limit: u64) -> Self {
        Self {
            config: LibdeflateDefenseConfig::default_limits().with_max_output_limit(max_output_limit),
            bytes_read: 0,
            bytes_written: 0,
            secure_scratch: [0u8; 64],
        }
    }

    /// Returns a reference to the active defense configuration.
    #[inline]
    #[must_use]
    pub const fn config(&self) -> &LibdeflateDefenseConfig {
        &self.config
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

    /// Returns the current cumulative decompression expansion ratio.
    #[must_use]
    pub fn current_ratio(&self) -> f64 {
        let comp = self.bytes_read.max(1) as f64;
        (self.bytes_written as f64) / comp
    }

    /// Resets decompression tracking byte counters to zero while preserving configuration.
    pub fn reset(&mut self) {
        self.bytes_read = 0;
        self.bytes_written = 0;
        self.secure_scratch.zeroize();
    }

    // MARK: - Layer 1: Output Quota & Expansion Ratio Circuit Breaker

    /// Validates incremental decompression progress against cumulative quota and expansion ratio ceiling.
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

    /// Validates standalone input/output byte counts against expansion ratio policies.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if uncompressed bytes exceed quota or expansion limits.
    pub fn validate_expansion_ratio(&self, bytes_in: u64, bytes_out: u64) -> Result<(), TTZipStatus> {
        if bytes_out > self.config.max_output_limit {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if bytes_out > self.config.threshold_bytes {
            let comp = bytes_in.max(1) as f64;
            let uncomp = bytes_out as f64;
            let ratio = uncomp / comp;
            if ratio > self.config.max_expansion_ratio as f64 {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }

    // MARK: - Layer 2: Match Distance Underflow & Window Ceiling Guard

    /// Validates backward reference distance against destination buffer cursor and maximum window ceiling.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if `offset == 0`, `offset > dst_pos`,
    /// or `offset > config.max_distance`.
    pub fn validate_distance(&self, offset: usize, dst_pos: usize) -> Result<(), TTZipStatus> {
        if offset == 0 || offset > dst_pos || offset > self.config.max_distance {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        Ok(())
    }

    // MARK: - Layer 3: Kraft Inequality & Huffman Codespace Defense

    /// Validates canonical Huffman codeword lengths against Kraft-McMillan inequality and completeness bounds.
    ///
    /// Prevents over-subscribed Huffman codes (where $\sum 2^{-\text{len}} > 1$) and malformed
    /// incomplete dynamic trees from causing decoder state corruption or infinite loops.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrSecurityViolation)` if the Kraft inequality is violated (over-subscribed code).
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if codeword lengths exceed maximum bounds or contain invalid incomplete codes.
    pub fn validate_huffman_codespace(
        &self,
        lens: &[u8],
        max_codeword_len: usize,
    ) -> Result<(), TTZipStatus> {
        if lens.is_empty() || max_codeword_len == 0 || max_codeword_len > 15 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let mut len_counts = [0u32; 16];
        for &l in lens {
            let len = l as usize;
            if len > max_codeword_len {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            len_counts[len] += 1;
        }

        let mut max_len = max_codeword_len;
        while max_len > 1 && len_counts[max_len] == 0 {
            max_len -= 1;
        }

        if len_counts[0] as usize == lens.len() {
            // Valid empty tree
            return Ok(());
        }

        let mut codespace_used: u32 = 0;
        for len in 1..max_len {
            codespace_used = (codespace_used << 1).saturating_add(len_counts[len]);
        }
        codespace_used = (codespace_used << 1).saturating_add(len_counts[max_len]);

        // Kraft-McMillan inequality check: sum(2^-len) <= 1  <=>  codespace_used <= 2^max_len
        if codespace_used > (1u32 << max_len) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // Incomplete code check
        if codespace_used < (1u32 << max_len) {
            if codespace_used == 0 {
                return Ok(());
            }
            // RFC 1951: Incomplete code allowed only if exactly 1 symbol with length 1
            if codespace_used != (1u32 << (max_len - 1)) || len_counts[1] != 1 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }

        Ok(())
    }

    /// Alias for [`validate_huffman_codespace`](Self::validate_huffman_codespace).
    #[inline]
    pub fn validate_codespace(
        &self,
        lens: &[u8],
        max_codeword_len: usize,
    ) -> Result<(), TTZipStatus> {
        self.validate_huffman_codespace(lens, max_codeword_len)
    }

    // MARK: - Layer 4: Bitstream Overread Boundary Guard

    /// Validates bitstream refill overread counter against the 8-byte hard threshold.
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if `overread_count > LIBDEFLATE_MAX_OVERREAD_BYTES`.
    pub fn validate_overread(&self, overread_count: usize) -> Result<(), TTZipStatus> {
        if overread_count > LIBDEFLATE_MAX_OVERREAD_BYTES {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(())
    }

    // MARK: - Layer 5: Uncompressed Block Inverted Length Invariant

    /// Validates RFC 1951 uncompressed block header inverted length integrity (`LEN == !NLEN`).
    ///
    /// # Errors
    /// Returns `Err(TTZipStatus::ErrCorruptHeader)` if `len != !nlen`.
    pub fn validate_uncompressed_block(&self, len: u16, nlen: u16) -> Result<(), TTZipStatus> {
        if len != !nlen {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(())
    }
}

// MARK: - Standalone Stream Validation & Guarded Decompression APIs

/// Validates container framing header for raw, zlib, or gzip compressed streams.
///
/// # Container Specifications
/// - **Raw**: No framing header required; returns `Ok(())`.
/// - **Zlib (RFC 1950)**: Requires valid 2-byte header satisfying $(CMF \times 256 + FLG) \bmod 31 == 0$,
///   $CM = 8$ (Deflate), $CINFO \le 7$ (32KB window), and $FDICT = 0$.
/// - **Gzip (RFC 1952)**: Requires 10-byte header with $ID1 = 0x1F$, $ID2 = 0x8B$, $CM = 8$, and reserved flags $= 0$.
///
/// # Errors
/// Returns `Err(TTZipStatus::ErrCorruptHeader)` if header bytes violate container framing invariants.
pub fn validate_stream_header(header: &[u8], format: ContainerFormat) -> Result<(), TTZipStatus> {
    match format {
        ContainerFormat::Raw => Ok(()),
        ContainerFormat::Zlib => {
            if header.len() < ZLIB_MIN_HEADER_SIZE {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            let hdr = u16::from_be_bytes([header[0], header[1]]);

            // FCHECK validation: (CMF * 256 + FLG) % 31 == 0
            if !hdr.is_multiple_of(31) {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            // Compression method must be DEFLATE (CM = 8)
            let cm = (hdr >> 8) & 0x0F;
            if (cm as u8) != ZLIB_CM_DEFLATE {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            // Window size must not exceed 32KB (CINFO <= 7)
            let cinfo = (hdr >> 12) & 0x0F;
            if (cinfo as u8) > ZLIB_CINFO_32K_WINDOW {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            // Preset dictionary (FDICT) is not supported
            if ((hdr >> 5) & 1) != 0 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            Ok(())
        }
        ContainerFormat::Gzip => {
            if header.len() < GZIP_MIN_HEADER_SIZE {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            // Magic bytes ID1 and ID2, and CM = 8
            if header[0] != GZIP_ID1 || header[1] != GZIP_ID2 || header[2] != GZIP_CM_DEFLATE {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            let flg = header[3];
            if (flg & GZIP_FRESERVED) != 0 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            Ok(())
        }
    }
}

/// Decompresses source stream with 6-layer defense invariants, quota enforcement, and circuit breakers.
///
/// # Parameters
/// - `src`: Compressed source byte slice.
/// - `dst`: Destination output buffer.
/// - `format`: Framing format ([`ContainerFormat::Raw`], [`ContainerFormat::Zlib`], or [`ContainerFormat::Gzip`]).
/// - `limit`: Maximum allowable uncompressed bytes (hard quota ceiling).
///
/// # Errors
/// Returns:
/// - `Err(TTZipStatus::ErrSecurityViolation)` if `limit == 0`, uncompressed output exceeds `limit`,
///   or expansion ratio exceeds 1032:1 beyond warmup threshold.
/// - `Err(TTZipStatus::ErrCorruptHeader)` if container header or bitstream is malformed.
/// - `Err(TTZipStatus::ErrExtractionFailed)` if decompression encounters unexpected payload truncation.
pub fn guarded_decompress(
    src: &[u8],
    dst: &mut [u8],
    format: ContainerFormat,
    limit: usize,
) -> Result<usize, TTZipStatus> {
    if limit == 0 {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    validate_stream_header(src, format)?;

    let mut guard = LibdeflateSecurityGuard::with_output_limit(limit as u64);

    let effective_dst_len = dst.len().min(limit);
    let target_slice = &mut dst[..effective_dst_len];

    let decompressed_len = match format {
        ContainerFormat::Raw => deflate_decompress(src, target_slice)?,
        ContainerFormat::Zlib => zlib_decompress(src, target_slice)?,
        ContainerFormat::Gzip => gzip_decompress(src, target_slice)?,
    };

    if decompressed_len > limit {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    guard.track_decompression(src.len(), decompressed_len)?;

    Ok(decompressed_len)
}
