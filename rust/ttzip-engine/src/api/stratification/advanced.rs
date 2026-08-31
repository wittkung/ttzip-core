// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Layer 4: Advanced API (Fine-Grained Hyperparameters & Hardware Control).

use crate::api::stratification::simple::{
    simple_compress_bound, simple_compress_to_slice, simple_decompress_to_slice,
};
use crate::codecs::{
    zstd_compress_advanced, CDict, DDict, ZstdCCtx, ZstdConfig, ZstdDCtx,
};
use crate::types::{
    resolve_thread_budget, TTZipArchiveFormat, TTZipCompressionLevel, TTZipStatus,
};

/// Comprehensive hyperparameter configuration for advanced compression workflows.
#[derive(Debug, Clone)]
pub struct AdvancedCompressionConfig {
    pub format: TTZipArchiveFormat,
    pub level: TTZipCompressionLevel,
    pub threads: u32,
    pub window_log: Option<u32>,
    pub dictionary: Option<Vec<u8>>,
    pub enable_checksum: bool,
    pub enable_hardware_simd: bool,
    pub block_size_bytes: usize,
    pub memory_budget_mb: usize,
}

impl Default for AdvancedCompressionConfig {
    fn default() -> Self {
        Self {
            format: TTZipArchiveFormat::Zstd,
            level: TTZipCompressionLevel::Normal,
            threads: 0, // Auto parallelism
            window_log: None,
            dictionary: None,
            enable_checksum: true,
            enable_hardware_simd: true,
            block_size_bytes: 1024 * 1024,
            memory_budget_mb: 256,
        }
    }
}

/// Fluent builder for constructing and executing advanced custom compression pipelines.
#[derive(Debug, Clone, Default)]
pub struct AdvancedCompressorBuilder {
    config: AdvancedCompressionConfig,
}

impl AdvancedCompressorBuilder {
    /// Creates a new advanced compressor builder for the specified format.
    #[must_use]
    pub fn new(format: TTZipArchiveFormat) -> Self {
        Self {
            config: AdvancedCompressionConfig {
                format,
                ..Default::default()
            },
        }
    }

    /// Configures compression level.
    #[must_use]
    pub fn level(mut self, level: TTZipCompressionLevel) -> Self {
        self.config.level = level;
        self
    }

    /// Configures worker thread budget (0 for automatic detection).
    #[must_use]
    pub fn threads(mut self, threads: u32) -> Self {
        self.config.threads = threads;
        self
    }

    /// Configures custom log2 sliding window size.
    #[must_use]
    pub fn window_log(mut self, window_log: u32) -> Self {
        self.config.window_log = Some(window_log);
        self
    }

    /// Attaches a pre-trained dictionary buffer.
    #[must_use]
    pub fn dictionary(mut self, dictionary: Vec<u8>) -> Self {
        self.config.dictionary = Some(dictionary);
        self
    }

    /// Toggles data block checksum calculation.
    #[must_use]
    pub fn checksum(mut self, enable: bool) -> Self {
        self.config.enable_checksum = enable;
        self
    }

    /// Toggles hardware SIMD vector acceleration.
    #[must_use]
    pub fn hardware_simd(mut self, enable: bool) -> Self {
        self.config.enable_hardware_simd = enable;
        self
    }

    /// Configures chunk block size in bytes.
    #[must_use]
    pub fn block_size(mut self, size_bytes: usize) -> Self {
        self.config.block_size_bytes = size_bytes;
        self
    }

    /// Configures maximum memory allocation budget in megabytes.
    #[must_use]
    pub fn memory_budget_mb(mut self, mb: usize) -> Self {
        self.config.memory_budget_mb = mb;
        self
    }

    /// Builds the advanced compressor instance.
    #[must_use]
    pub fn build(self) -> AdvancedCompressor {
        AdvancedCompressor {
            config: self.config,
        }
    }
}

/// Fully configured advanced compressor instance.
pub struct AdvancedCompressor {
    config: AdvancedCompressionConfig,
}

impl AdvancedCompressor {
    /// Returns the underlying hyperparameter configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &AdvancedCompressionConfig {
        &self.config
    }

    /// Compresses input slice using configured advanced hyperparameters into destination slice.
    pub fn compress(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        let threads = resolve_thread_budget(self.config.threads) as u32;

        if let Some(ref dict) = self.config.dictionary {
            if self.config.format == TTZipArchiveFormat::Zstd
                || self.config.format == TTZipArchiveFormat::TarZstd
            {
                let cdict = CDict::create(dict, self.config.level as i32)?;
                let mut cctx = ZstdCCtx::new()?;
                return cctx.compress_using_cdict_raw(src, dst, cdict.as_ptr());
            }
        }

        if self.config.format == TTZipArchiveFormat::Zstd
            || self.config.format == TTZipArchiveFormat::TarZstd
        {
            let win_log = self.config.window_log.unwrap_or(0);
            let zconfig = ZstdConfig {
                level: self.config.level as i32,
                nb_workers: threads,
                window_log: win_log,
                enable_checksum: self.config.enable_checksum,
                ..Default::default()
            };
            return zstd_compress_advanced(src, dst, &zconfig);
        }

        simple_compress_to_slice(src, dst, self.config.format, self.config.level)
    }

    /// Compresses input slice into a newly allocated vector using configured hyperparameters.
    pub fn compress_to_vec(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let bound = simple_compress_bound(src.len(), self.config.format, self.config.level).max(64);
        let mut out = vec![0u8; bound];
        let written = self.compress(src, &mut out)?;
        out.truncate(written);
        Ok(out)
    }
}

/// Hyperparameter configuration for advanced decompression workflows.
#[derive(Debug, Clone)]
pub struct AdvancedDecompressionConfig {
    pub format: TTZipArchiveFormat,
    pub threads: u32,
    pub dictionary: Option<Vec<u8>>,
    pub memory_budget_mb: usize,
    pub verify_checksum: bool,
}

impl Default for AdvancedDecompressionConfig {
    fn default() -> Self {
        Self {
            format: TTZipArchiveFormat::Zstd,
            threads: 0,
            dictionary: None,
            memory_budget_mb: 256,
            verify_checksum: true,
        }
    }
}

/// Fluent builder for constructing advanced custom decompressor pipelines.
#[derive(Debug, Clone, Default)]
pub struct AdvancedDecompressorBuilder {
    config: AdvancedDecompressionConfig,
}

impl AdvancedDecompressorBuilder {
    /// Creates a new advanced decompressor builder for the given format.
    #[must_use]
    pub fn new(format: TTZipArchiveFormat) -> Self {
        Self {
            config: AdvancedDecompressionConfig {
                format,
                ..Default::default()
            },
        }
    }

    /// Sets thread budget for parallel decompression.
    #[must_use]
    pub fn threads(mut self, threads: u32) -> Self {
        self.config.threads = threads;
        self
    }

    /// Attaches custom dictionary.
    #[must_use]
    pub fn dictionary(mut self, dictionary: Vec<u8>) -> Self {
        self.config.dictionary = Some(dictionary);
        self
    }

    /// Configures memory budget in MB.
    #[must_use]
    pub fn memory_budget_mb(mut self, mb: usize) -> Self {
        self.config.memory_budget_mb = mb;
        self
    }

    /// Toggles checksum verification.
    #[must_use]
    pub fn verify_checksum(mut self, verify: bool) -> Self {
        self.config.verify_checksum = verify;
        self
    }

    /// Builds the advanced decompressor instance.
    #[must_use]
    pub fn build(self) -> AdvancedDecompressor {
        AdvancedDecompressor {
            config: self.config,
        }
    }
}

/// Fully configured advanced decompressor instance.
pub struct AdvancedDecompressor {
    config: AdvancedDecompressionConfig,
}

impl AdvancedDecompressor {
    /// Returns the underlying decompression configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &AdvancedDecompressionConfig {
        &self.config
    }

    /// Decompresses input slice using advanced parameters directly into destination slice.
    pub fn decompress(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        if let Some(ref dict) = self.config.dictionary {
            if self.config.format == TTZipArchiveFormat::Zstd
                || self.config.format == TTZipArchiveFormat::TarZstd
            {
                let ddict = DDict::create(dict)?;
                let mut dctx = ZstdDCtx::new()?;
                return dctx.decompress_using_ddict_raw(src, dst, ddict.as_ptr());
            }
        }
        simple_decompress_to_slice(src, dst, self.config.format)
    }

    /// Decompresses input slice into a newly allocated vector using configured hyperparameters.
    pub fn decompress_to_vec(&self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let estimated_cap = src.len().saturating_mul(4).max(1024);
        let mut out = vec![0u8; estimated_cap];

        for _ in 0..6 {
            match self.decompress(src, &mut out) {
                Ok(written) => {
                    out.truncate(written);
                    return Ok(out);
                }
                Err(TTZipStatus::ErrExtractionFailed) | Err(TTZipStatus::ErrInvalidParam) => {
                    let cur_len = out.len();
                    let new_len = cur_len.saturating_mul(2).min(1024 * 1024 * 1024);
                    if new_len == cur_len {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                    out.resize(new_len, 0);
                }
                Err(status) => return Err(status),
            }
        }
        Err(TTZipStatus::ErrExtractionFailed)
    }
}
