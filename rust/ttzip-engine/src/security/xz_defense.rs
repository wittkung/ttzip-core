// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XZ Memory Quota Budget Breaker, Stream Flags Validator, and Malformed Block Defense Subsystem.
//!
//! Enforces deterministic memory bounds and defense-in-depth protections against malicious .xz inputs:
//! - LZMA2 dictionary size pre-flight memory estimation and OOM circuit breaker.
//! - Filter chain topology validation (1..=4 filters, unique LZMA2 at terminal position, max 1 Delta).
//! - Stream header / footer flags reserved bit and check type validation.
//! - Stream Index record count and memory exhaustion breaker.
//! - Cumulative decompression expansion ratio (Zip-Bomb) breaker.

use crate::types::TTZipStatus;
use crate::xz::block::{
    XzBlockHeader, XzFilterConfig, FILTER_ID_ARM, FILTER_ID_ARM64, FILTER_ID_ARMTHUMB,
    FILTER_ID_DELTA, FILTER_ID_IA64, FILTER_ID_LZMA2, FILTER_ID_POWERPC, FILTER_ID_RISCV,
    FILTER_ID_SPARC, FILTER_ID_X86,
};
use crate::xz::header::XzStreamFlags;
use crate::xz::payload::lzma2_dict_size_from_prop;
use crate::xz::types::XzCheckType;

/// Base memory overhead allocated by LZMA2 decoder states and stream buffers (2 MiB).
pub const LZMA2_DECODER_OVERHEAD_BYTES: u64 = 2 * 1024 * 1024;

/// Estimated internal buffer memory for BCJ / Delta branch filters (64 KiB).
pub const FILTER_BUFFER_OVERHEAD_BYTES: u64 = 64 * 1024;

/// Estimated in-memory metadata footprint per `XzRecord` (including prefix sum tables).
pub const INDEX_RECORD_MEMORY_ESTIMATE_BYTES: u64 = 48;

/// Configuration options for XZ stream decoding defense and quota management.
#[derive(Debug, Clone, PartialEq)]
pub struct XzDefenseConfig {
    /// Maximum allowed physical memory budget in bytes for decompression (default: 256 MiB).
    pub max_memlimit: u64,
    /// Maximum cumulative uncompressed output size in bytes (default: 10 GiB).
    pub max_decompressed_size: u64,
    /// Maximum allowed decompression expansion ratio (e.g. 100 for 100:1, default: 100).
    pub max_ratio: u32,
    /// Threshold in uncompressed bytes before expansion ratio check is enforced (default: 1 MiB).
    pub threshold_bytes: u64,
    /// Maximum allowed number of records in an XZ Stream Index (default: 1,000,000).
    pub max_index_records: u64,
    /// Maximum number of filters allowed in a single Block Header filter chain (default: 4).
    pub max_filter_count: usize,
}

impl Default for XzDefenseConfig {
    #[inline]
    fn default() -> Self {
        Self::default_limits()
    }
}

impl XzDefenseConfig {
    /// Default maximum memory limit (256 MiB).
    pub const DEFAULT_MAX_MEMLIMIT: u64 = 256 * 1024 * 1024;
    /// Default maximum cumulative uncompressed limit (10 GiB).
    pub const DEFAULT_MAX_DECOMPRESSED_SIZE: u64 = 10 * 1024 * 1024 * 1024;
    /// Default maximum decompression expansion ratio (100:1).
    pub const DEFAULT_MAX_RATIO: u32 = 100;
    /// Default threshold before ratio enforcement starts (1 MiB).
    pub const DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;
    /// Default maximum index record count (1,000,000 records).
    pub const DEFAULT_MAX_INDEX_RECORDS: u64 = 1_000_000;
    /// Default maximum filter chain count (4 filters).
    pub const DEFAULT_MAX_FILTER_COUNT: usize = 4;

    /// Creates a new `XzDefenseConfig` with custom core parameters.
    #[must_use]
    pub const fn new(max_memlimit: u64, max_decompressed_size: u64, max_ratio: u32) -> Self {
        Self {
            max_memlimit,
            max_decompressed_size,
            max_ratio,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
            max_index_records: Self::DEFAULT_MAX_INDEX_RECORDS,
            max_filter_count: Self::DEFAULT_MAX_FILTER_COUNT,
        }
    }

    /// Creates default production security limits.
    #[must_use]
    pub const fn default_limits() -> Self {
        Self {
            max_memlimit: Self::DEFAULT_MAX_MEMLIMIT,
            max_decompressed_size: Self::DEFAULT_MAX_DECOMPRESSED_SIZE,
            max_ratio: Self::DEFAULT_MAX_RATIO,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
            max_index_records: Self::DEFAULT_MAX_INDEX_RECORDS,
            max_filter_count: Self::DEFAULT_MAX_FILTER_COUNT,
        }
    }

    /// Sets custom memory limit budget.
    #[must_use]
    pub const fn with_memlimit(mut self, max_memlimit: u64) -> Self {
        self.max_memlimit = max_memlimit;
        self
    }

    /// Sets custom cumulative decompressed size quota.
    #[must_use]
    pub const fn with_max_decompressed_size(mut self, max_decompressed_size: u64) -> Self {
        self.max_decompressed_size = max_decompressed_size;
        self
    }

    /// Sets custom expansion ratio breaker limit.
    #[must_use]
    pub const fn with_max_ratio(mut self, max_ratio: u32) -> Self {
        self.max_ratio = max_ratio;
        self
    }

    /// Sets custom threshold bytes before ratio check activates.
    #[must_use]
    pub const fn with_threshold_bytes(mut self, threshold_bytes: u64) -> Self {
        self.threshold_bytes = threshold_bytes;
        self
    }

    /// Sets custom maximum stream index records limit.
    #[must_use]
    pub const fn with_max_index_records(mut self, max_index_records: u64) -> Self {
        self.max_index_records = max_index_records;
        self
    }
}

/// Active security guard and memory quota enforcement engine for XZ streams.
#[derive(Debug, Clone)]
pub struct XzDefenseGuard {
    /// Defense configuration rules.
    pub config: XzDefenseConfig,
    /// Cumulative compressed bytes consumed so far.
    pub cumulative_compressed_bytes: u64,
    /// Cumulative decompressed output bytes produced so far.
    pub cumulative_decompressed_bytes: u64,
}

impl Default for XzDefenseGuard {
    #[inline]
    fn default() -> Self {
        Self::new(XzDefenseConfig::default())
    }
}

impl XzDefenseGuard {
    /// Creates a new guard bound to the specified configuration.
    #[must_use]
    pub const fn new(config: XzDefenseConfig) -> Self {
        Self {
            config,
            cumulative_compressed_bytes: 0,
            cumulative_decompressed_bytes: 0,
        }
    }

    /// Creates a guard with default configuration overriding only `max_memlimit`.
    #[must_use]
    pub const fn with_memlimit(max_memlimit: u64) -> Self {
        Self {
            config: XzDefenseConfig::default_limits().with_memlimit(max_memlimit),
            cumulative_compressed_bytes: 0,
            cumulative_decompressed_bytes: 0,
        }
    }

    /// Validates raw 2-byte stream flags against XZ specification §2.1.1.2.
    ///
    /// # Errors
    /// - `ErrSecurityViolation` if reserved bits are set.
    /// - `ErrUnsupportedFeature` if check type ID is not standardized.
    pub fn validate_raw_flags(raw_flags: [u8; 2]) -> Result<XzStreamFlags, TTZipStatus> {
        let byte0 = raw_flags[0];
        let byte1 = raw_flags[1];
        let reserved_bits = byte1 & 0xF0;

        if byte0 != 0x00 || reserved_bits != 0x00 {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        let check_id = byte1 & 0x0F;
        let check_type = match check_id {
            0x00 => XzCheckType::None,
            0x01 => XzCheckType::Crc32,
            0x04 => XzCheckType::Crc64,
            0x0A => XzCheckType::Sha256,
            _ => return Err(TTZipStatus::ErrUnsupportedFeature),
        };

        Ok(XzStreamFlags::new(check_type))
    }

    /// Validates `XzStreamFlags` integrity check type compatibility.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if the check type is not recognized or illegal.
    pub fn validate_header_flags(flags: &XzStreamFlags) -> Result<(), TTZipStatus> {
        match flags.check_type {
            XzCheckType::None | XzCheckType::Crc32 | XzCheckType::Crc64 | XzCheckType::Sha256 => {
                Ok(())
            }
        }
    }

    /// Validates the structure and filter properties of a Block Header filter chain.
    ///
    /// # Rules (§3.1.2 & §5.3):
    /// 1. `1 <= filters.len() <= max_filter_count` (1..=4).
    /// 2. The terminal (last) filter MUST be LZMA2 (`0x21`).
    /// 3. Non-terminal filters MUST NOT be LZMA2.
    /// 4. At most 1 Delta filter (`0x03`) is permitted.
    /// 5. Delta filter properties must be exactly 1 byte.
    /// 6. BCJ filter properties must be 0 or 4 bytes.
    /// 7. LZMA2 filter properties must be 1 byte with `prop <= 39`.
    /// 8. Unsupported filter IDs return `ErrUnsupportedFeature`.
    pub fn validate_filter_chain(
        &self,
        filters: &[XzFilterConfig],
    ) -> Result<(), TTZipStatus> {
        if filters.is_empty() || filters.len() > self.config.max_filter_count {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        let last_filter = filters.last().unwrap();
        if last_filter.filter_id != FILTER_ID_LZMA2 {
            // Last filter in XZ block MUST be LZMA2
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        let mut delta_count = 0usize;

        for (idx, filter) in filters.iter().enumerate() {
            let is_last = idx == filters.len() - 1;

            match filter.filter_id {
                FILTER_ID_LZMA2 => {
                    if !is_last {
                        // LZMA2 is only permitted at terminal position
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                    if filter.properties.len() != 1 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                    if filter.properties[0] > 39 {
                        // Dictionary property > 39 is reserved and invalid
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                }
                FILTER_ID_DELTA => {
                    if is_last {
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                    delta_count += 1;
                    if delta_count > 1 {
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                    if filter.properties.len() != 1 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                }
                FILTER_ID_X86
                | FILTER_ID_POWERPC
                | FILTER_ID_IA64
                | FILTER_ID_ARM
                | FILTER_ID_ARMTHUMB
                | FILTER_ID_SPARC
                | FILTER_ID_ARM64
                | FILTER_ID_RISCV => {
                    if is_last {
                        return Err(TTZipStatus::ErrSecurityViolation);
                    }
                    if !filter.properties.is_empty() && filter.properties.len() != 4 {
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                }
                _ => return Err(TTZipStatus::ErrUnsupportedFeature),
            }
        }

        Ok(())
    }

    /// Accurately estimates physical memory required to decompress the block described by `header`.
    ///
    /// Pre-flight calculation takes into account:
    /// - LZMA2 dictionary size decoded from property byte.
    /// - Fixed LZMA2 decoder state and output buffer overhead.
    /// - Secondary transform buffers for BCJ and Delta filters.
    ///
    /// # Errors
    /// Returns `ErrOutOfMemory` if total required memory exceeds `config.max_memlimit`.
    /// Returns `ErrSecurityViolation`, `ErrCorruptHeader`, or `ErrUnsupportedFeature` if the
    /// filter chain is malformed.
    pub fn estimate_block_memory(&self, header: &XzBlockHeader) -> Result<u64, TTZipStatus> {
        self.validate_filter_chain(&header.filters)?;

        let mut total_memory: u64 = 0;

        for filter in &header.filters {
            match filter.filter_id {
                FILTER_ID_LZMA2 => {
                    let prop = filter.properties.first().copied().unwrap_or(0);
                    let dict_size = lzma2_dict_size_from_prop(prop);
                    let lzma2_needed = dict_size.saturating_add(LZMA2_DECODER_OVERHEAD_BYTES);
                    total_memory = total_memory.saturating_add(lzma2_needed);
                }
                FILTER_ID_DELTA
                | FILTER_ID_X86
                | FILTER_ID_POWERPC
                | FILTER_ID_IA64
                | FILTER_ID_ARM
                | FILTER_ID_ARMTHUMB
                | FILTER_ID_SPARC
                | FILTER_ID_ARM64
                | FILTER_ID_RISCV => {
                    total_memory = total_memory.saturating_add(FILTER_BUFFER_OVERHEAD_BYTES);
                }
                _ => return Err(TTZipStatus::ErrUnsupportedFeature),
            }
        }

        if total_memory > self.config.max_memlimit {
            return Err(TTZipStatus::ErrOutOfMemory);
        }

        Ok(total_memory)
    }

    /// Tracks decompression progress and enforces total size limits and expansion ratio circuit breakers.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if total uncompressed bytes exceed `max_decompressed_size`
    /// or if the cumulative expansion ratio exceeds `max_ratio` beyond `threshold_bytes`.
    pub fn track_decompression(
        &mut self,
        compressed_bytes: u64,
        decompressed_bytes: u64,
    ) -> Result<(), TTZipStatus> {
        self.cumulative_compressed_bytes =
            self.cumulative_compressed_bytes.saturating_add(compressed_bytes);
        self.cumulative_decompressed_bytes =
            self.cumulative_decompressed_bytes.saturating_add(decompressed_bytes);

        // 1. Enforce hard total decompressed size quota
        if self.cumulative_decompressed_bytes > self.config.max_decompressed_size {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // 2. Enforce expansion ratio breaker once past warmup threshold
        if self.cumulative_decompressed_bytes > self.config.threshold_bytes {
            let comp = self.cumulative_compressed_bytes.max(1) as f64;
            let decomp = self.cumulative_decompressed_bytes as f64;
            let ratio = decomp / comp;
            if ratio > self.config.max_ratio as f64 {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }

    /// Validates Index record count and prevents memory exhaustion attacks via synthetic Index headers.
    ///
    /// # Errors
    /// - `ErrSecurityViolation` if record count exceeds `max_index_records`.
    /// - `ErrOutOfMemory` if estimated index memory exceeds `max_memlimit`.
    pub fn validate_index_memory(&self, record_count: u64) -> Result<(), TTZipStatus> {
        if record_count > self.config.max_index_records {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        let estimated_mem = record_count.saturating_mul(INDEX_RECORD_MEMORY_ESTIMATE_BYTES);
        if estimated_mem > self.config.max_memlimit {
            return Err(TTZipStatus::ErrOutOfMemory);
        }

        Ok(())
    }

    /// Returns the current cumulative expansion ratio.
    #[must_use]
    pub fn current_ratio(&self) -> f64 {
        let comp = self.cumulative_compressed_bytes.max(1) as f64;
        (self.cumulative_decompressed_bytes as f64) / comp
    }

    /// Resets cumulative decompression tracking counters to zero.
    pub fn reset(&mut self) {
        self.cumulative_compressed_bytes = 0;
        self.cumulative_decompressed_bytes = 0;
    }
}
