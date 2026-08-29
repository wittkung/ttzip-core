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
    bzip2::{bzip2_compress, bzip2_compress_bound, bzip2_decompress},
    deflate::{
        deflate_compress, deflate_compress_bound, deflate_decompress, DeflateCompressor,
    },
    lz4::{lz4_compress_bound, lz4_compress_fast, lz4_compress_hc, lz4_decompress},
    lzfse::{
        lzfse_compress, lzfse_compress_bound, lzfse_decompress, lzvn_compress,
        lzvn_compress_bound, lzvn_decompress,
    },
    lzma2::{fl2_compress, fl2_compress_bound, fl2_decompress},
    ppmd::{ppmd_compress_to_vec, ppmd_decompress},
    snappy::{
        is_framed_snappy, snappy_compress, snappy_compress_bound as snappy_max_compressed_length,
        snappy_decompress, snappy_frame_decode_to_vec, snappy_frame_encode_to_vec,
    },
    zstd::{
        fse_compress, fse_compress_bound, fse_decompress, huf0_compress1x, huf0_compress4x,
        huf0_compress_bound, huf0_decompress1x, huf0_decompress4x, zstd_compress,
        zstd_compress_advanced, zstd_compress_bound, zstd_decompress, ZstdConfig, ZstdDCtx,
        ZstdDictionaryManager,
    },
};

use crate::types::TTZipStatus;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

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

// MARK: - 2b. Zstandard LDM Driver (64MB..2GB Windows)

/// Zstandard Long Distance Matching benchmark driver supporting 64MB..2GB sliding windows.
pub struct ZstdLdmBenchmarkDriver;

impl CodecBenchmarkDriver for ZstdLdmBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Zstd-LDM"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![26, 27, 28, 29, 30, 31]
    }

    fn display_name(&self, level: i32) -> String {
        match level {
            26 => "Zstd LDM (64MB Window)".to_string(),
            27 => "Zstd LDM (128MB Window)".to_string(),
            28 => "Zstd LDM (256MB Window)".to_string(),
            29 => "Zstd LDM (512MB Window)".to_string(),
            30 => "Zstd LDM (1GB Window)".to_string(),
            31 => "Zstd LDM (2GB Window)".to_string(),
            _ => format!("Zstd LDM (Log {})", level),
        }
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = zstd_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let config = ZstdConfig::ldm(9, level as u32).with_ldm_tuning(18, 32, 3, 2);
        let written = zstd_compress_advanced(src, &mut dst, &config)?;
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let mut dctx = ZstdDCtx::new()?;
        dctx.set_max_window_log(31)?;
        let written = dctx.decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 2c. Zstandard Dictionary Driver (112KB Shared Corpus)

/// Zstandard pre-trained dictionary benchmark driver.
pub struct ZstdDictBenchmarkDriver;

impl CodecBenchmarkDriver for ZstdDictBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Zstd-Dict"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![1, 3, 6, 9]
    }

    fn display_name(&self, level: i32) -> String {
        format!("Zstd Dict 112KB (L{})", level)
    }

    fn bench_compress(&self, src: &[u8], _level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let dict = ZstdDictionaryManager::global().ensure_standard_112kb();
        let bound = zstd_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let written = dict.compress_small(src, &mut dst)?;
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let dict = ZstdDictionaryManager::global().ensure_standard_112kb();
        let mut dst = vec![0u8; orig_len];
        let written = dict.decompress_small(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 2d. FSE (Finite State Entropy / tANS) Driver

/// Finite State Entropy (tANS) benchmark driver.
pub struct FseBenchmarkDriver;

impl CodecBenchmarkDriver for FseBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "FSE"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![1]
    }

    fn display_name(&self, _level: i32) -> String {
        "FSE (Finite State Entropy / tANS)".to_string()
    }

    fn bench_compress(&self, src: &[u8], _level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = fse_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let written = fse_compress(src, &mut dst)?;
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = fse_decompress(compressed, &mut dst)?;
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 2e. Huff0 (4-Stream & 1-Stream Huffman) Driver

/// Huff0 benchmark driver supporting 1-Stream sequential and 4-Stream parallel modes.
pub struct Huff0BenchmarkDriver;

impl CodecBenchmarkDriver for Huff0BenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "Huff0"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![1, 4]
    }

    fn display_name(&self, level: i32) -> String {
        if level == 1 {
            "Huff0 1-Stream".to_string()
        } else {
            "Huff0 4-Stream".to_string()
        }
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        let bound = huf0_compress_bound(src.len()) + 1024;
        let mut dst = vec![0u8; bound];
        let written = if level == 1 {
            huf0_compress1x(src, &mut dst)?
        } else {
            huf0_compress4x(src, &mut dst)?
        };
        dst.truncate(written);
        Ok(dst)
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = if compressed.is_empty() {
            0
        } else {
            // Test 4X first, fallback to 1X if needed
            match huf0_decompress4x(compressed, &mut dst) {
                Ok(w) => w,
                Err(_) => huf0_decompress1x(compressed, &mut dst)?,
            }
        };
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
        let bound = bzip2_compress_bound(src.len());
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

/// Apple LZFSE / LZVN benchmark driver with thread-private 2MB scratch buffer.
pub struct LzfseBenchmarkDriver;

impl CodecBenchmarkDriver for LzfseBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "LZFSE"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![1, 2]
    }

    fn display_name(&self, level: i32) -> String {
        if level == 2 {
            "Apple LZVN".to_string()
        } else {
            "Apple LZFSE".to_string()
        }
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        if level == 2 {
            let bound = lzvn_compress_bound(src.len());
            let mut dst = vec![0u8; bound];
            let written = lzvn_compress(src, &mut dst)?;
            dst.truncate(written);
            Ok(dst)
        } else {
            let bound = lzfse_compress_bound(src.len());
            let mut dst = vec![0u8; bound];
            let written = lzfse_compress(src, &mut dst)?;
            dst.truncate(written);
            Ok(dst)
        }
    }

    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut dst = vec![0u8; orig_len];
        let written = if orig_len == 0 {
            0
        } else {
            match lzfse_decompress(compressed, &mut dst) {
                Ok(n) => n,
                Err(_) => lzvn_decompress(compressed, &mut dst)?,
            }
        };
        if written != orig_len {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        Ok(dst)
    }
}

// MARK: - 9. PPMd Driver (Order 2..=16, 16MB budget)

/// PPMd statistical benchmark driver wrapping Pure Safe Rust PPMd Model H.
pub struct PpmdBenchmarkDriver;

impl CodecBenchmarkDriver for PpmdBenchmarkDriver {
    fn algorithm_id(&self) -> &'static str {
        "PPMd"
    }

    fn available_levels(&self) -> Vec<i32> {
        vec![2, 4, 6, 8, 12, 16]
    }

    fn display_name(&self, level: i32) -> String {
        format!("PPMd Order {}", level)
    }

    fn bench_compress(&self, src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
        if src.is_empty() {
            return Ok(Vec::new());
        }
        let order = (level as u32).clamp(2, 16);
        let mut comp = ppmd_compress_to_vec(src, order, 16 * 1024 * 1024)?;
        let mut out = Vec::with_capacity(comp.len() + 1);
        out.push(order as u8);
        out.append(&mut comp);
        Ok(out)
    }


    fn bench_decompress(&self, compressed: &[u8], orig_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        if orig_len == 0 {
            return Ok(Vec::new());
        }
        if compressed.is_empty() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let order = (compressed[0] as u32).clamp(2, 16);
        let mut dst = vec![0u8; orig_len];
        let written = ppmd_decompress(&compressed[1..], &mut dst, order, 16 * 1024 * 1024)?;
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
    /// Returns static instances of all 13 benchmark drivers.
    pub fn drivers() -> &'static [Box<dyn CodecBenchmarkDriver>] {
        static DRIVERS: OnceLock<Vec<Box<dyn CodecBenchmarkDriver>>> = OnceLock::new();
        DRIVERS.get_or_init(|| {
            vec![
                Box::new(DeflateBenchmarkDriver),
                Box::new(ZstdBenchmarkDriver),
                Box::new(ZstdLdmBenchmarkDriver),
                Box::new(ZstdDictBenchmarkDriver),
                Box::new(FseBenchmarkDriver),
                Box::new(Huff0BenchmarkDriver),
                Box::new(Lzma2BenchmarkDriver),
                Box::new(BrotliBenchmarkDriver),
                Box::new(Bzip2BenchmarkDriver),
                Box::new(SnappyBenchmarkDriver),
                Box::new(Lz4BenchmarkDriver),
                Box::new(LzfseBenchmarkDriver),
                Box::new(PpmdBenchmarkDriver),
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
                || (name_lower == "zstd-ldm" && id_lower == "zstd-ldm")
                || (name_lower == "zstd-dict" && id_lower == "zstd-dict")
                || (name_lower == "fse" && id_lower == "fse")
                || (name_lower == "huff0" && id_lower == "huff0")
                || (name_lower == "ppmd" && id_lower == "ppmd")
                || (name_lower == "bzip2" && id_lower == "bzip2")
            {
                return Some(&**driver);
            }
        }
        None
    }

    /// Generates all standard benchmark configurations covering 60+ points (90+ total points).
    pub fn all_matrix_configs() -> Vec<MatrixCodecConfig> {
        let mut configs = Vec::with_capacity(100);
        configs.push(MatrixCodecConfig::new("Libdeflate", 0, "Libdeflate Store (L0)"));
        for lvl in 1..=12 { configs.push(MatrixCodecConfig::new("Libdeflate", lvl, format!("Libdeflate L{}", lvl))); }
        for lvl in 1..=19 { configs.push(MatrixCodecConfig::new("Zstd", lvl, format!("Zstd L{}", lvl))); }
        configs.push(MatrixCodecConfig::new("Zstd", 22, "Zstd Ultra L22"));
        configs.push(MatrixCodecConfig::new("Zstd", 100, "Zstd L19 + LDM"));
        for (w, lbl) in [(26, "64MB"), (27, "128MB"), (28, "256MB"), (29, "512MB"), (30, "1GB"), (31, "2GB")] {
            configs.push(MatrixCodecConfig::new("Zstd-LDM", w, format!("Zstd LDM {}", lbl)));
        }
        for lvl in [1, 3, 6, 9] { configs.push(MatrixCodecConfig::new("Zstd-Dict", lvl, format!("Zstd Dict 112KB (L{})", lvl))); }
        configs.push(MatrixCodecConfig::new("FSE", 1, "FSE (tANS)"));
        configs.push(MatrixCodecConfig::new("Huff0", 1, "Huff0 1-Stream"));
        configs.push(MatrixCodecConfig::new("Huff0", 4, "Huff0 4-Stream"));
        for lvl in 1..=9 { configs.push(MatrixCodecConfig::new("LZMA2", lvl, format!("LZMA2 L{}", lvl))); }
        for (lvl, lbl) in [(1, "Fast 1"), (3, "Fast 3"), (9, "Fast 9"), (19, "HC 9"), (22, "HC 12")] {
            configs.push(MatrixCodecConfig::new("LZ4", lvl, format!("LZ4 {}", lbl)));
        }
        configs.push(MatrixCodecConfig::new("Snappy", 1, "Snappy Raw"));
        configs.push(MatrixCodecConfig::new("Snappy", 2, "Snappy Framed"));
        configs.push(MatrixCodecConfig::new("LZFSE", 1, "Apple LZFSE"));
        configs.push(MatrixCodecConfig::new("LZFSE", 2, "Apple LZVN"));
        for q in 0..=11 { configs.push(MatrixCodecConfig::new("Brotli", q, format!("Brotli Q{}", q))); }
        for lvl in 1..=9 { configs.push(MatrixCodecConfig::new("Bzip2", lvl, format!("Bzip2 L{}", lvl))); }
        for ord in [2, 4, 6, 8, 12, 16] { configs.push(MatrixCodecConfig::new("PPMd", ord, format!("PPMd Order {}", ord))); }
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
#[path = "codecs_driver_tests.rs"]
mod codecs_driver_tests;


