// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified Trait-driven Codec Benchmark Driver & 60-Point Orthogonal Matrix Suite.
//!
//! Exposes the `CodecBenchmarkDriver` abstraction and concrete implementations for:
//! 1. Deflate (`libdeflate` 0..=12: Store mode + SIMD libdeflate)
//! 2. Zstandard (`zstd` 1..=22 + Long Distance Matching LDM)
//! 3. LZMA2 (`fast-lzma2` 0..=9 multi-threaded/single-threaded)
//! 4. Brotli (`brotli` 0..=11 quality)
//! 5. Bzip2 (`libbz2` 1..=9 block sizes)
//! 6. Snappy (`snap` raw block & framed streaming)
//! 7. LZ4 (`lz4` fast acceleration & HC levels)
//! 8. LZFSE (Apple `lzfse` with thread-private 2MB scratch buffer)

use crate::codecs::{
    brotli::{brotli_compress, brotli_compress_bound, brotli_decompress},
    deflate::{
        deflate_compress, deflate_compress_bound, deflate_decompress, DeflateCompressor,
    },
    fast_blocks::{
        lz4_compress_bound, lz4_compress_fast, lz4_compress_hc, lz4_decompress, lzfse_compress,
        lzfse_decompress, snappy_compress, snappy_decompress, snappy_max_compressed_length,
    },
    lzma2::{fl2_compress, fl2_compress_bound, fl2_decompress},
    snappy::{is_framed_snappy, snappy_frame_decode_to_vec, snappy_frame_encode_to_vec},
    zstd::{
        zstd_compress, zstd_compress_advanced, zstd_compress_bound, zstd_decompress, ZstdConfig,
    },
};
use crate::types::TTZipStatus;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

// MARK: - Native C Bzip2 Declarations

extern "C" {
    fn BZ2_bzBuffToBuffCompress(
        dest: *mut libc::c_char,
        destLen: *mut libc::c_uint,
        source: *const libc::c_char,
        sourceLen: libc::c_uint,
        blockSize100k: libc::c_int,
        verbosity: libc::c_int,
        workFactor: libc::c_int,
    ) -> libc::c_int;

    fn BZ2_bzBuffToBuffDecompress(
        dest: *mut libc::c_char,
        destLen: *mut libc::c_uint,
        source: *const libc::c_char,
        sourceLen: libc::c_uint,
        small: libc::c_int,
        verbosity: libc::c_int,
    ) -> libc::c_int;
}

fn bzip2_compress(src: &[u8], dst: &mut [u8], level: i32) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let mut dst_len = dst.len() as libc::c_uint;
    let ret = unsafe {
        BZ2_bzBuffToBuffCompress(
            dst.as_mut_ptr() as *mut libc::c_char,
            &mut dst_len,
            src.as_ptr() as *const libc::c_char,
            src.len() as libc::c_uint,
            level.clamp(1, 9) as libc::c_int,
            0,
            30,
        )
    };
    if ret == 0 {
        Ok(dst_len as usize)
    } else {
        Err(TTZipStatus::ErrCompressionFailed)
    }
}

fn bzip2_decompress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    let mut dst_len = dst.len() as libc::c_uint;
    let ret = unsafe {
        BZ2_bzBuffToBuffDecompress(
            dst.as_mut_ptr() as *mut libc::c_char,
            &mut dst_len,
            src.as_ptr() as *const libc::c_char,
            src.len() as libc::c_uint,
            0,
            0,
        )
    };
    if ret == 0 {
        Ok(dst_len as usize)
    } else {
        Err(TTZipStatus::ErrExtractionFailed)
    }
}

// MARK: - Codec Benchmark Driver Trait

/// Unified trait for codec benchmarking drivers.
pub trait CodecBenchmarkDriver: Send + Sync {
    /// Canonical algorithm identifier (e.g. "Deflate", "Zstd", "LZMA2", "Brotli", etc.).
    fn algorithm_id(&self) -> &'static str;

    /// Returns the sequence of benchmarkable compression levels or operating modes.
    fn available_levels(&self) -> Vec<i32>;

    /// Human-readable display label for a specific level.
    fn display_name(&self, level: i32) -> String;

    /// Executes single-pass in-memory compression of `src` at the given level.
    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus>;

    /// Executes single-pass in-memory decompression of `compressed` to restore `orig_len` bytes.
    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus>;
}

// MARK: - 1. Deflate Driver (libdeflate 0..=12)

/// Deflate benchmark driver supporting Level 0 (Store) through Level 12 (Ultra).
pub struct DeflateBenchmarkDriver;

impl CodecBenchmarkDriver for DeflateBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Deflate"
    }

    fn available_levels(&self) -> Vec<i32> {
        (0..=12).collect()
    }

    fn display_name(&self, level: i32) -> String {
        if level == 0 {
            "Deflate Store (L0)".to_string()
        } else {
            format!("Deflate L{}", level)
        }
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        if level == 0 {
            let mut compressor = DeflateCompressor::new(0)?;
            let bound = compressor.compress_bound(src.len());
            let mut dst = vec![0u8; bound];
            let written = compressor.compress(src, &mut dst)?;
            dst.truncate(written);
            Ok(dst)
        } else {
            let bound = deflate_compress_bound(src.len(), level) + 1024;
            let mut dst = vec![0u8; bound];
            let written = deflate_compress(src, &mut dst, level)?;
            dst.truncate(written);
            Ok(dst)
        }
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = deflate_decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 2. Zstandard Driver (zstd 1..=22 + LDM)

/// Zstandard benchmark driver supporting levels 1..=22 and Long Distance Matching (LDM).
pub struct ZstdBenchmarkDriver;

impl CodecBenchmarkDriver for ZstdBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Zstd"
    }

    fn available_levels(&self) -> Vec<i32> {
        let mut levels: Vec<i32> = (1..=19).collect();
        levels.push(22);
        levels.push(100); // 100 encodes LDM (Level 19 + Long Distance Matching)
        levels
    }

    fn display_name(&self, level: i32) -> String {
        if level == 100 {
            "Zstd L19 + LDM".to_string()
        } else if level == 22 {
            "Zstd Ultra L22".to_string()
        } else {
            format!("Zstd L{}", level)
        }
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = zstd_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let written = if level == 100 {
            let config = ZstdConfig {
                level: 19,
                enable_ldm: true,
                ..Default::default()
            };
            zstd_compress_advanced(src, &mut dst, &config)?
        } else {
            zstd_compress(src, &mut dst, level)?
        };
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = zstd_decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 3. LZMA2 Driver (fast-lzma2 0..=9)

/// LZMA2 benchmark driver wrapping `fast-lzma2`.
pub struct Lzma2BenchmarkDriver;

impl CodecBenchmarkDriver for Lzma2BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "LZMA2"
    }

    fn available_levels(&self) -> Vec<i32> {
        (0..=9).collect()
    }

    fn display_name(&self, level: i32) -> String {
        format!("LZMA2 L{}", level)
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = fl2_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let written = fl2_compress(src, &mut dst, level.clamp(0, 9), 1)?;
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = fl2_decompress(compressed, &mut dst, 1)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 4. Brotli Driver (brotli 0..=11)

/// Brotli benchmark driver wrapping pure safe Rust `brotli`.
pub struct BrotliBenchmarkDriver;

impl CodecBenchmarkDriver for BrotliBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Brotli"
    }

    fn available_levels(&self) -> Vec<i32> {
        (0..=11).collect()
    }

    fn display_name(&self, level: i32) -> String {
        format!("Brotli Q{}", level)
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = brotli_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let written = brotli_compress(src, &mut dst, level.clamp(0, 11) as u32, 22)?;
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = brotli_decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 5. Bzip2 Driver (libbz2 1..=9)

/// Bzip2 benchmark driver wrapping native `libbz2`.
pub struct Bzip2BenchmarkDriver;

impl CodecBenchmarkDriver for Bzip2BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Bzip2"
    }

    fn available_levels(&self) -> Vec<i32> {
        (1..=9).collect()
    }

    fn display_name(&self, level: i32) -> String {
        format!("Bzip2 L{}", level)
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = src.len() + (src.len() / 100) + 1024;
        let mut dst = vec![0u8; bound];
        let written = bzip2_compress(src, &mut dst, level)?;
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = bzip2_decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 6. Snappy Driver (raw/framed)

/// Snappy benchmark driver supporting Raw Block (Level 1) and Framed Streaming (Level 2).
pub struct SnappyBenchmarkDriver;

impl CodecBenchmarkDriver for SnappyBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Snappy"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![1, 2]
    }

    fn display_name(&self, level: i32) -> String {
        if level == 1 {
            "Snappy Raw".to_string()
        } else {
            "Snappy Framed".to_string()
        }
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        if level == 1 {
            let bound = snappy_max_compressed_length(src.len()) + 1024;
            let mut dst = vec![0u8; bound];
            let written = snappy_compress(src, &mut dst)?;
            dst.truncate(written);
            Ok(dst)
        } else {
            snappy_frame_encode_to_vec(src)
        }
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        if is_framed_snappy(compressed) {
            let dst = snappy_frame_decode_to_vec(compressed, orig_len + 1024)?;
            if dst.len() != orig_len {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            Ok(dst)
        } else {
            let mut dst = vec![0u8; orig_len];
            let written = snappy_decompress(compressed, &mut dst)?;
            if written != orig_len {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
            Ok(dst)
        }
    }
}

// MARK: - 7. LZ4 Driver (fast / hc)

/// LZ4 benchmark driver supporting Fast acceleration factors (1, 3, 9) and High Compression HC (19 -> HC 9, 22 -> HC 12).
pub struct Lz4BenchmarkDriver;

impl CodecBenchmarkDriver for Lz4BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "LZ4"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![1, 3, 9, 19, 22]
    }

    fn display_name(&self, level: i32) -> String {
        match level {
            1 => "LZ4 Fast 1".to_string(),
            3 => "LZ4 Fast 3".to_string(),
            9 => "LZ4 Fast 9".to_string(),
            19 => "LZ4 HC 9".to_string(),
            22 => "LZ4 HC 12".to_string(),
            _ => format!("LZ4 L{}", level),
        }
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = lz4_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let written = if level > 9 {
            let hc_level = if level == 19 { 9 } else if level == 22 { 12 } else { level - 10 };
            lz4_compress_hc(src, &mut dst, hc_level)?
        } else {
            lz4_compress_fast(src, &mut dst, level)?
        };
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = lz4_decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 8. LZFSE Driver (Apple 2MB scratch buffer)

/// Apple LZFSE benchmark driver with thread-private 2MB scratch buffer.
pub struct LzfseBenchmarkDriver;

impl CodecBenchmarkDriver for LzfseBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "LZFSE"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![1]
    }

    fn display_name(&self, _level: i32) -> String {
        "Apple LZFSE".to_string()
    }

    fn bench_compress(&self, src: &[u8], _level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = src.len() + 4096;
        let mut dst = vec![0u8; bound];
        let written = lzfse_compress(src, &mut dst)?;
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = lzfse_decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - Matrix Codec Configuration & Dispatcher

/// Codec configuration descriptor for matrix gate benchmarks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatrixCodecConfig {
    pub algorithm: String,
    pub level: i32,
    pub display_name: String,
}

impl MatrixCodecConfig {
    pub fn new(algorithm: impl Into<String>, level: i32, display_name: impl Into<String>) -> Self {
        Self {
            algorithm: algorithm.into(),
            level,
            display_name: display_name.into(),
        }
    }
}

/// Unified codec driver dispatcher for matrix gate benchmarks.
pub struct MatrixCodecDriver;

impl MatrixCodecDriver {
    /// Returns static instances of all 8 core benchmark drivers.
    pub fn drivers() -> &'static [Box<dyn CodecBenchmarkDriver>] {
        static DRIVERS: OnceLock<Vec<Box<dyn CodecBenchmarkDriver>>> = OnceLock::new();
        DRIVERS.get_or_init(|| {
            vec![
                Box::new(DeflateBenchmarkDriver),
                Box::new(ZstdBenchmarkDriver),
                Box::new(Lzma2BenchmarkDriver),
                Box::new(BrotliBenchmarkDriver),
                Box::new(Bzip2BenchmarkDriver),
                Box::new(SnappyBenchmarkDriver),
                Box::new(Lz4BenchmarkDriver),
                Box::new(LzfseBenchmarkDriver),
            ]
        })
    }

    /// Resolves driver by canonical name (case-insensitive, alias-tolerant).
    pub fn find_driver(name: &str) -> Option<&'static (dyn CodecBenchmarkDriver + 'static)> {
        let name_lower = name.to_ascii_lowercase();
        for driver in Self::drivers() {
            let id_lower = driver.algorithm_id().to_ascii_lowercase();
            if id_lower == name_lower
                || (name_lower == "libdeflate" && id_lower == "deflate")
                || (name_lower == "deflate" && id_lower == "deflate")
                || (name_lower == "fast-lzma2" && id_lower == "lzma2")
                || (name_lower == "zstandard" && id_lower == "zstd")
            {
                return Some(&**driver);
            }
        }
        None
    }

    /// Generates all standard benchmark configurations covering 60+ points (72 total points).
    pub fn all_matrix_configs() -> Vec<MatrixCodecConfig> {
        let mut configs = Vec::with_capacity(75);

        // 1. Deflate: Levels 0..=12 (13 points)
        configs.push(MatrixCodecConfig::new("Libdeflate", 0, "Libdeflate Store (L0)"));
        for lvl in 1..=12 {
            configs.push(MatrixCodecConfig::new("Libdeflate", lvl, format!("Libdeflate L{}", lvl)));
        }

        // 2. Zstandard: Levels 1..=19 + L22 + LDM (21 points)
        for lvl in 1..=19 {
            configs.push(MatrixCodecConfig::new("Zstd", lvl, format!("Zstd L{}", lvl)));
        }
        configs.push(MatrixCodecConfig::new("Zstd", 22, "Zstd Ultra L22"));
        configs.push(MatrixCodecConfig::new("Zstd", 100, "Zstd L19 + LDM"));

        // 3. LZMA2: Levels 1..=9 (9 points)
        for lvl in 1..=9 {
            configs.push(MatrixCodecConfig::new("LZMA2", lvl, format!("LZMA2 L{}", lvl)));
        }

        // 4. LZ4: Fast (1, 3, 9) + HC (9, 12) (5 points)
        configs.push(MatrixCodecConfig::new("LZ4", 1, "LZ4 Fast 1"));
        configs.push(MatrixCodecConfig::new("LZ4", 3, "LZ4 Fast 3"));
        configs.push(MatrixCodecConfig::new("LZ4", 9, "LZ4 Fast 9"));
        configs.push(MatrixCodecConfig::new("LZ4", 19, "LZ4 HC 9"));
        configs.push(MatrixCodecConfig::new("LZ4", 22, "LZ4 HC 12"));

        // 5. Snappy: Raw + Framed (2 points)
        configs.push(MatrixCodecConfig::new("Snappy", 1, "Snappy Raw"));
        configs.push(MatrixCodecConfig::new("Snappy", 2, "Snappy Framed"));

        // 6. Apple LZFSE (1 point)
        configs.push(MatrixCodecConfig::new("LZFSE", 1, "Apple LZFSE"));

        // 7. Brotli: Quality 0..=11 (12 points)
        for q in 0..=11 {
            configs.push(MatrixCodecConfig::new("Brotli", q, format!("Brotli Q{}", q)));
        }

        // 8. Bzip2: Levels 1..=9 (9 points)
        for lvl in 1..=9 {
            configs.push(MatrixCodecConfig::new("Bzip2", lvl, format!("Bzip2 L{}", lvl)));
        }

        configs
    }

    /// Compresses source slice using the given configuration.
    pub fn compress(cfg: &MatrixCodecConfig, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let driver = Self::find_driver(&cfg.algorithm).ok_or(TTZipStatus::ErrInvalidParam)?;
        driver.bench_compress(src, cfg.level)
    }

    /// Decompresses compressed slice using the given configuration.
    pub fn decompress(
        cfg: &MatrixCodecConfig,
        compressed: &[u8],
        orig_len: usize,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let driver = Self::find_driver(&cfg.algorithm).ok_or(TTZipStatus::ErrInvalidParam)?;
        driver.bench_decompress(compressed, orig_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_8_drivers_registered_and_identified() {
        let drivers = MatrixCodecDriver::drivers();
        assert_eq!(drivers.len(), 8);

        let ids: Vec<&str> = drivers.iter().map(|d| d.algorithm_id()).collect();
        assert_eq!(
            ids,
            vec!["Deflate", "Zstd", "LZMA2", "Brotli", "Bzip2", "Snappy", "LZ4", "LZFSE"]
        );
    }

    #[test]
    fn test_all_8_drivers_roundtrip() {
        let payload = b"TTZip 2026 High-Performance Multi-Codec Architecture Verification Payload.";

        for driver in MatrixCodecDriver::drivers() {
            let levels = driver.available_levels();
            assert!(!levels.is_empty(), "Driver {} must have levels", driver.algorithm_id());

            let test_level = levels[levels.len() / 2];
            let compressed = driver
                .bench_compress(payload, test_level)
                .unwrap_or_else(|e| panic!("Compress failed for {} (L{}): {:?}", driver.algorithm_id(), test_level, e));
            assert!(!compressed.is_empty());

            let decompressed = driver
                .bench_decompress(&compressed, payload.len())
                .unwrap_or_else(|e| panic!("Decompress failed for {} (L{}): {:?}", driver.algorithm_id(), test_level, e));
            assert_eq!(
                decompressed.as_slice(),
                payload.as_slice(),
                "Roundtrip mismatch for {}",
                driver.algorithm_id()
            );
        }
    }

    #[test]
    fn test_matrix_configs_count_and_orthogonality() {
        let configs = MatrixCodecDriver::all_matrix_configs();
        assert!(
            configs.len() >= 60,
            "Expected at least 60 matrix configurations, got {}",
            configs.len()
        );

        let payload = b"Verification of 60+ point matrix configurations in TTZip benchmark engine.";
        for cfg in configs.iter().take(10) {
            let comp = MatrixCodecDriver::compress(cfg, payload)
                .unwrap_or_else(|e| panic!("Matrix compress failed for {}: {:?}", cfg.display_name, e));
            let decomp = MatrixCodecDriver::decompress(cfg, &comp, payload.len())
                .unwrap_or_else(|e| panic!("Matrix decompress failed for {}: {:?}", cfg.display_name, e));
            assert_eq!(decomp.as_slice(), payload.as_slice());
        }
    }
}

