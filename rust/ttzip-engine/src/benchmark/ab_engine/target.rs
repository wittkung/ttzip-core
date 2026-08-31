// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Declarative A/B Benchmarking Engine - Layer 1: Target Registry & Driver Adapters.
//!
//! Provides the core abstraction `BenchmarkTarget` along with zero-overhead adapters
//! wrapping the 13 matrix codecs, 11 cryptographic/hashing primitives, and 8 container drivers.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::benchmark::clock::MonotonicStopwatch;
use crate::benchmark::codecs_driver::{
    BrotliBenchmarkDriver, Bzip2BenchmarkDriver, CodecBenchmarkDriver, DeflateBenchmarkDriver,
    FseBenchmarkDriver, Huff0BenchmarkDriver, Lz4BenchmarkDriver, LzfseBenchmarkDriver,
    Lzma2BenchmarkDriver, PpmdBenchmarkDriver, SnappyBenchmarkDriver, ZstdBenchmarkDriver,
    ZstdDictBenchmarkDriver, ZstdLdmBenchmarkDriver,
};
use crate::benchmark::container_driver::{
    AarContainerDriver, ContainerBenchmarkDriver, SevenZContainerDriver, TarBrotliContainerDriver,
    TarContainerDriver, TarGzContainerDriver, TarSnappyContainerDriver, TarZstContainerDriver,
    ZipContainerDriver,
};
use crate::benchmark::crypto_driver::{
    Adler32BenchmarkDriver, Blake3BenchmarkDriver, Crc32BenchmarkDriver, Crc64BenchmarkDriver,
    CryptoBenchmarkDriver, SevenZAes256BenchmarkDriver, VaultAesGcmBenchmarkDriver,
    VaultChaChaPolyBenchmarkDriver, WinZipAes256BenchmarkDriver, Xxh3_128BenchmarkDriver,
    Xxh3_64BenchmarkDriver, ZipCryptoBenchmarkDriver,
};
use crate::platform::memory::get_peak_rss_bytes;
use crate::types::TTZipStatus;
use crate::zip::writer::ZipInputItem;

// MARK: - Enums & Descriptors

/// Functional category of the benchmark target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetCategory {
    /// Lossless compression/decompression codec algorithm.
    Codec,
    /// Cryptographic cipher, stream cipher, or digest checksum primitive.
    Crypto,
    /// Multi-file archive container format pack/unpack driver.
    Container,
    /// End-to-end composite real-world workload scenario.
    Scenario,
    /// Cross-language foreign function interface bridge boundary.
    Ffi,
}

impl TargetCategory {
    /// Returns the canonical URI namespace prefix.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codec => "codec",
            Self::Crypto => "crypto",
            Self::Container => "container",
            Self::Scenario => "scenario",
            Self::Ffi => "ffi",
        }
    }
}

/// Primary metric unit produced by target execution passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricUnit {
    /// High-throughput streaming data rate in bytes per second.
    BytesPerSec,
    /// High-precision latency per execution pass in nanoseconds.
    NanosecondsPerOp,
    /// Instruction throughput in Million Instructions Per Second.
    Mips,
    /// Frame rate in frames per second.
    Fps,
}

impl MetricUnit {
    /// Returns human-readable metric unit representation.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BytesPerSec => "bytes/sec",
            Self::NanosecondsPerOp => "ns/op",
            Self::Mips => "mips",
            Self::Fps => "fps",
        }
    }
}

/// Fully qualified metadata descriptor for an executable benchmark target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetDescriptor {
    /// Canonical URI identifier (e.g. `codec/zstd/compress/l3`, `crypto/blake3/digest`).
    pub uri: String,
    /// Target category.
    pub category: TargetCategory,
    /// Descriptive human-readable algorithm or component name.
    pub name: String,
    /// Compression level, operating mode, or parameter configuration string.
    pub level_or_mode: Option<String>,
    /// Default metric unit produced by this target.
    pub unit: MetricUnit,
}

/// Telemetry output produced by a single execution pass of a benchmark target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetExecutionOutput {
    /// Monotonic wall-clock execution duration in nanoseconds.
    pub duration_nanos: u64,
    /// Size of processed or generated output payload in bytes.
    pub output_bytes: usize,
    /// Peak resident set size (RSS) recorded during execution in bytes.
    pub peak_rss_bytes: usize,
    /// Optional secondary metric (e.g. compression ratio, throughput MB/s, entry count).
    pub extra_metric: Option<f64>,
}

// MARK: - Benchmark Target Trait

/// Unified trait implemented by all declarative benchmark target adapters.
pub trait BenchmarkTarget: Send + Sync {
    /// Returns the target's immutable metadata descriptor.
    fn descriptor(&self) -> &TargetDescriptor;

    /// Executes a single timed benchmark pass against the provided byte input buffer.
    fn execute_pass(&self, input: &[u8]) -> Result<TargetExecutionOutput, TTZipStatus>;
}

// MARK: - Helper Functions

/// Normalizes arbitrary names into URI-safe slug strings.
#[inline]
fn slugify(name: &str) -> String {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "crc-32" | "crc32" => "crc32".to_string(),
        "adler-32" | "adler32" => "adler32".to_string(),
        "crc-64-ecma" | "crc-64" | "crc64" => "crc64".to_string(),
        _ => lower
            .replace(['.', '-', ' '], "_"),
    }
}

/// Matches a string against a glob pattern supporting `*` and `?`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p_bytes = pattern.as_bytes();
    let t_bytes = text.as_bytes();
    let mut p_idx = 0;
    let mut t_idx = 0;
    let mut star_idx = None;
    let mut match_idx = 0;

    while t_idx < t_bytes.len() {
        if p_idx < p_bytes.len() && (p_bytes[p_idx] == b'?' || p_bytes[p_idx] == t_bytes[t_idx]) {
            p_idx += 1;
            t_idx += 1;
        } else if p_idx < p_bytes.len() && p_bytes[p_idx] == b'*' {
            star_idx = Some(p_idx);
            match_idx = t_idx;
            p_idx += 1;
        } else if let Some(star) = star_idx {
            p_idx = star + 1;
            match_idx += 1;
            t_idx = match_idx;
        } else {
            return false;
        }
    }

    while p_idx < p_bytes.len() && p_bytes[p_idx] == b'*' {
        p_idx += 1;
    }

    p_idx == p_bytes.len()
}

// MARK: - Codec Target Adapter

/// Operating mode for codec benchmarking passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodecMode {
    /// Compression from raw source data to compressed payload.
    Compress,
    /// Decompression from compressed payload back to original data.
    Decompress,
}

impl CodecMode {
    /// Returns mode slug for URI paths.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compress => "compress",
            Self::Decompress => "decompress",
        }
    }
}

/// Target adapter wrapping a `CodecBenchmarkDriver` implementation.
pub struct CodecTargetAdapter {
    descriptor: TargetDescriptor,
    driver: Arc<dyn CodecBenchmarkDriver>,
    level: i32,
    mode: CodecMode,
}

impl CodecTargetAdapter {
    /// Creates a new codec target adapter.
    pub fn new(driver: Arc<dyn CodecBenchmarkDriver>, level: i32, mode: CodecMode) -> Self {
        let algo_slug = slugify(driver.algorithm_id());
        let mode_str = mode.as_str();
        let uri = format!("codec/{}/{}/l{}", algo_slug, mode_str, level);
        let name = format!("{} [{}]", driver.display_name(level), mode_str);
        let level_or_mode = Some(format!("{}_l{}", mode_str, level));

        let descriptor = TargetDescriptor {
            uri,
            category: TargetCategory::Codec,
            name,
            level_or_mode,
            unit: MetricUnit::BytesPerSec,
        };

        Self {
            descriptor,
            driver,
            level,
            mode,
        }
    }
}

impl BenchmarkTarget for CodecTargetAdapter {
    fn descriptor(&self) -> &TargetDescriptor {
        &self.descriptor
    }

    fn execute_pass(&self, input: &[u8]) -> Result<TargetExecutionOutput, TTZipStatus> {
        match self.mode {
            CodecMode::Compress => {
                let stopwatch = MonotonicStopwatch::start();
                let compressed = self.driver.bench_compress(input, self.level)?;
                let duration_nanos = stopwatch.elapsed_nanos();
                let peak_rss_bytes = get_peak_rss_bytes() as usize;
                let ratio = if !input.is_empty() {
                    compressed.len() as f64 / input.len() as f64
                } else {
                    1.0
                };

                Ok(TargetExecutionOutput {
                    duration_nanos,
                    output_bytes: compressed.len(),
                    peak_rss_bytes,
                    extra_metric: Some(ratio),
                })
            }
            CodecMode::Decompress => {
                // Compress once outside the timed pass to create legitimate test payload
                let compressed = self.driver.bench_compress(input, self.level)?;
                let stopwatch = MonotonicStopwatch::start();
                let decompressed = self
                    .driver
                    .bench_decompress(&compressed, input.len())?;
                let duration_nanos = stopwatch.elapsed_nanos();
                let peak_rss_bytes = get_peak_rss_bytes() as usize;
                let ratio = if !compressed.is_empty() {
                    decompressed.len() as f64 / compressed.len() as f64
                } else {
                    1.0
                };

                Ok(TargetExecutionOutput {
                    duration_nanos,
                    output_bytes: decompressed.len(),
                    peak_rss_bytes,
                    extra_metric: Some(ratio),
                })
            }
        }
    }
}

// MARK: - Crypto Target Adapter

/// Operating mode for cryptographic primitive benchmarking passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CryptoMode {
    /// Digest generation or encryption.
    Process,
    /// Digest verification or decryption.
    VerifyOrDecrypt,
}

impl CryptoMode {
    /// Returns mode slug for URI paths.
    #[inline]
    pub fn as_str(&self, is_encryption: bool) -> &'static str {
        match self {
            Self::Process => {
                if is_encryption {
                    "encrypt"
                } else {
                    "digest"
                }
            }
            Self::VerifyOrDecrypt => {
                if is_encryption {
                    "decrypt"
                } else {
                    "verify"
                }
            }
        }
    }
}

/// Target adapter wrapping a `CryptoBenchmarkDriver` implementation.
pub struct CryptoTargetAdapter {
    descriptor: TargetDescriptor,
    driver: Arc<dyn CryptoBenchmarkDriver>,
    mode: CryptoMode,
}

impl CryptoTargetAdapter {
    /// Creates a new crypto target adapter.
    pub fn new(driver: Arc<dyn CryptoBenchmarkDriver>, mode: CryptoMode) -> Self {
        let algo_slug = slugify(driver.algorithm_id());
        let is_enc = driver.is_encryption();
        let mode_str = mode.as_str(is_enc);
        let uri = format!("crypto/{}/{}", algo_slug, mode_str);
        let name = format!("{} [{}]", driver.display_name(), mode_str);
        let level_or_mode = Some(mode_str.to_string());

        let descriptor = TargetDescriptor {
            uri,
            category: TargetCategory::Crypto,
            name,
            level_or_mode,
            unit: MetricUnit::BytesPerSec,
        };

        Self {
            descriptor,
            driver,
            mode,
        }
    }
}

impl BenchmarkTarget for CryptoTargetAdapter {
    fn descriptor(&self) -> &TargetDescriptor {
        &self.descriptor
    }

    fn execute_pass(&self, input: &[u8]) -> Result<TargetExecutionOutput, TTZipStatus> {
        match self.mode {
            CryptoMode::Process => {
                let stopwatch = MonotonicStopwatch::start();
                let output = self.driver.bench_process(input)?;
                let duration_nanos = stopwatch.elapsed_nanos();
                let peak_rss_bytes = get_peak_rss_bytes() as usize;

                Ok(TargetExecutionOutput {
                    duration_nanos,
                    output_bytes: output.len(),
                    peak_rss_bytes,
                    extra_metric: None,
                })
            }
            CryptoMode::VerifyOrDecrypt => {
                // Compute processed digest/ciphertext outside the timed pass
                let processed = self.driver.bench_process(input)?;
                let stopwatch = MonotonicStopwatch::start();
                let verified = self
                    .driver
                    .bench_verify_or_decrypt(&processed, input)?;
                let duration_nanos = stopwatch.elapsed_nanos();
                let peak_rss_bytes = get_peak_rss_bytes() as usize;

                if !verified {
                    return Err(TTZipStatus::ErrExtractionFailed);
                }

                Ok(TargetExecutionOutput {
                    duration_nanos,
                    output_bytes: input.len(),
                    peak_rss_bytes,
                    extra_metric: None,
                })
            }
        }
    }
}

// MARK: - Container Target Adapter

/// Operating mode for archive container format benchmarking passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContainerMode {
    /// Pack input items into archive stream.
    Create,
    /// Unpack and parse archive stream.
    Extract,
}

impl ContainerMode {
    /// Returns mode slug for URI paths.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Extract => "extract",
        }
    }
}

/// Target adapter wrapping a `ContainerBenchmarkDriver` implementation.
pub struct ContainerTargetAdapter {
    descriptor: TargetDescriptor,
    driver: Arc<dyn ContainerBenchmarkDriver>,
    mode: ContainerMode,
    level: i32,
    algorithm: Option<String>,
    password: Option<String>,
}

impl ContainerTargetAdapter {
    /// Creates a new container target adapter.
    pub fn new(
        driver: Arc<dyn ContainerBenchmarkDriver>,
        mode: ContainerMode,
        level: i32,
        algorithm: Option<&str>,
        password: Option<&str>,
    ) -> Self {
        let container_slug = slugify(driver.container_id());
        let mode_str = mode.as_str();
        let uri = format!("container/{}/{}", container_slug, mode_str);
        let name = format!("{} Container [{}]", driver.container_id(), mode_str);
        let level_or_mode = Some(format!("{}_l{}", mode_str, level));

        let descriptor = TargetDescriptor {
            uri,
            category: TargetCategory::Container,
            name,
            level_or_mode,
            unit: MetricUnit::BytesPerSec,
        };

        Self {
            descriptor,
            driver,
            mode,
            level,
            algorithm: algorithm.map(|s| s.to_string()),
            password: password.map(|s| s.to_string()),
        }
    }
}

impl BenchmarkTarget for ContainerTargetAdapter {
    fn descriptor(&self) -> &TargetDescriptor {
        &self.descriptor
    }

    fn execute_pass(&self, input: &[u8]) -> Result<TargetExecutionOutput, TTZipStatus> {
        let item = ZipInputItem {
            rel_path: "benchmark_payload.bin".to_string(),
            data: input.to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o100644,
            is_directory: false,
        };
        let items = [item];

        match self.mode {
            ContainerMode::Create => {
                let stopwatch = MonotonicStopwatch::start();
                let archive = self.driver.create_archive(
                    &items,
                    self.level,
                    self.algorithm.as_deref(),
                    self.password.as_deref(),
                )?;
                let duration_nanos = stopwatch.elapsed_nanos();
                let peak_rss_bytes = get_peak_rss_bytes() as usize;
                let ratio = if !input.is_empty() {
                    archive.len() as f64 / input.len() as f64
                } else {
                    1.0
                };

                Ok(TargetExecutionOutput {
                    duration_nanos,
                    output_bytes: archive.len(),
                    peak_rss_bytes,
                    extra_metric: Some(ratio),
                })
            }
            ContainerMode::Extract => {
                // Create archive representation outside the timed pass
                let archive = self.driver.create_archive(
                    &items,
                    self.level,
                    self.algorithm.as_deref(),
                    self.password.as_deref(),
                )?;
                let stopwatch = MonotonicStopwatch::start();
                let extracted_count = self
                    .driver
                    .extract_archive(&archive, self.password.as_deref())?;
                let duration_nanos = stopwatch.elapsed_nanos();
                let peak_rss_bytes = get_peak_rss_bytes() as usize;

                Ok(TargetExecutionOutput {
                    duration_nanos,
                    output_bytes: input.len(),
                    peak_rss_bytes,
                    extra_metric: Some(extracted_count as f64),
                })
            }
        }
    }
}

// MARK: - Target Registry

/// Central declarative registry for benchmark targets, supporting URI lookup and Glob filtering.
#[derive(Default, Clone)]
pub struct TargetRegistry {
    targets: Vec<Arc<dyn BenchmarkTarget>>,
    by_uri: HashMap<String, Arc<dyn BenchmarkTarget>>,
}

impl TargetRegistry {
    /// Creates an empty target registry.
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
            by_uri: HashMap::new(),
        }
    }

    /// Registers a benchmark target into the registry.
    pub fn register_target(&mut self, target: Arc<dyn BenchmarkTarget>) {
        let uri = target.descriptor().uri.clone();
        self.by_uri.insert(uri.clone(), Arc::clone(&target));
        // Register canonical alias variants for resilient URI lookup
        if uri.contains("crc32") {
            self.by_uri.insert(uri.replace("crc32", "crc_32"), Arc::clone(&target));
        } else if uri.contains("crc_32") {
            self.by_uri.insert(uri.replace("crc_32", "crc32"), Arc::clone(&target));
        }
        if uri.contains("adler32") {
            self.by_uri.insert(uri.replace("adler32", "adler_32"), Arc::clone(&target));
        } else if uri.contains("adler_32") {
            self.by_uri.insert(uri.replace("adler_32", "adler32"), Arc::clone(&target));
        }
        if uri.contains("crc64") {
            self.by_uri.insert(uri.replace("crc64", "crc_64"), Arc::clone(&target));
            self.by_uri.insert(uri.replace("crc64", "crc_64_ecma"), Arc::clone(&target));
        }
        self.targets.push(target);
    }

    /// Retrieves a target by its exact URI.
    pub fn get_target(&self, uri: &str) -> Option<Arc<dyn BenchmarkTarget>> {
        self.by_uri.get(uri).cloned()
    }

    /// Filters targets using wildcard glob patterns (supporting comma-separated lists e.g. `codec/lzfse/*,crypto/chacha*`).
    pub fn filter_targets(&self, glob_pattern: &str) -> Vec<Arc<dyn BenchmarkTarget>> {
        let patterns: Vec<&str> = glob_pattern
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if patterns.is_empty() || patterns.iter().any(|&p| p == "*" || p == "**") {
            return self.targets.clone();
        }
        self.targets
            .iter()
            .filter(|t| {
                let uri = &t.descriptor().uri;
                patterns.iter().any(|&p| {
                    glob_match(p, uri)
                        || (p.contains("crc32") && glob_match(&p.replace("crc32", "crc_32"), uri))
                        || (p.contains("crc_32") && glob_match(&p.replace("crc_32", "crc32"), uri))
                        || (p.contains("adler32") && glob_match(&p.replace("adler32", "adler_32"), uri))
                        || (p.contains("adler_32") && glob_match(&p.replace("adler_32", "adler32"), uri))
                        || (p.contains("crc64") && (glob_match(&p.replace("crc64", "crc_64"), uri) || glob_match(&p.replace("crc64", "crc_64_ecma"), uri)))
                        || (p.contains("crc_64") && glob_match(&p.replace("crc_64", "crc64"), uri))
                })
            })
            .cloned()
            .collect()
    }

    /// Returns a slice of all registered targets.
    #[inline]
    pub fn all_targets(&self) -> &[Arc<dyn BenchmarkTarget>] {
        &self.targets
    }

    /// Returns the total count of registered targets.
    #[inline]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Returns whether the registry contains no targets.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Creates and returns a core representative registry (~48 targets across all 13 codecs, 11 cryptos, 8 containers).
    pub fn default_core() -> Self {
        let mut registry = Self::new();

        let core_cfgs: Vec<(Arc<dyn CodecBenchmarkDriver>, &[i32])> = vec![
            (Arc::new(DeflateBenchmarkDriver), &[1, 6, 9]),
            (Arc::new(ZstdBenchmarkDriver), &[1, 3, 9, 19]),
            (Arc::new(ZstdLdmBenchmarkDriver), &[3]),
            (Arc::new(ZstdDictBenchmarkDriver), &[3]),
            (Arc::new(FseBenchmarkDriver), &[1]),
            (Arc::new(Huff0BenchmarkDriver), &[1]),
            (Arc::new(Lzma2BenchmarkDriver), &[1, 6, 9]),
            (Arc::new(BrotliBenchmarkDriver), &[1, 6, 11]),
            (Arc::new(Bzip2BenchmarkDriver), &[1, 9]),
            (Arc::new(SnappyBenchmarkDriver), &[1, 2]),
            (Arc::new(Lz4BenchmarkDriver), &[1, 9, 109, 112]),
            (Arc::new(LzfseBenchmarkDriver), &[1, 2]),
            (Arc::new(PpmdBenchmarkDriver), &[1, 6]),
        ];

        for (driver, levels) in core_cfgs {
            for &level in levels {
                registry.register_codec_pair(driver.clone(), level);
            }
        }

        Self::register_crypto_and_containers(&mut registry);
        registry
    }

    /// Creates and returns a fully populated default registry containing all matrix drivers (326 targets).
    pub fn default_full() -> Self {
        let mut registry = Self::new();
        let drivers: [Arc<dyn CodecBenchmarkDriver>; 13] = [
            Arc::new(DeflateBenchmarkDriver),
            Arc::new(ZstdBenchmarkDriver),
            Arc::new(ZstdLdmBenchmarkDriver),
            Arc::new(ZstdDictBenchmarkDriver),
            Arc::new(FseBenchmarkDriver),
            Arc::new(Huff0BenchmarkDriver),
            Arc::new(Lzma2BenchmarkDriver),
            Arc::new(BrotliBenchmarkDriver),
            Arc::new(Bzip2BenchmarkDriver),
            Arc::new(SnappyBenchmarkDriver),
            Arc::new(Lz4BenchmarkDriver),
            Arc::new(LzfseBenchmarkDriver),
            Arc::new(PpmdBenchmarkDriver),
        ];

        for driver in &drivers {
            for level in driver.available_levels() {
                registry.register_codec_pair(driver.clone(), level);
            }
        }

        Self::register_crypto_and_containers(&mut registry);
        registry
    }

    fn register_codec_pair(&mut self, driver: Arc<dyn CodecBenchmarkDriver>, level: i32) {
        self.register_target(Arc::new(CodecTargetAdapter::new(driver.clone(), level, CodecMode::Compress)));
        self.register_target(Arc::new(CodecTargetAdapter::new(driver, level, CodecMode::Decompress)));
    }

    fn register_crypto_and_containers(registry: &mut Self) {
        let crypto_drivers: [Arc<dyn CryptoBenchmarkDriver>; 11] = [
            Arc::new(Adler32BenchmarkDriver), Arc::new(Crc32BenchmarkDriver), Arc::new(Crc64BenchmarkDriver),
            Arc::new(Xxh3_64BenchmarkDriver), Arc::new(Xxh3_128BenchmarkDriver), Arc::new(Blake3BenchmarkDriver),
            Arc::new(WinZipAes256BenchmarkDriver), Arc::new(SevenZAes256BenchmarkDriver),
            Arc::new(ZipCryptoBenchmarkDriver), Arc::new(VaultAesGcmBenchmarkDriver),
            Arc::new(VaultChaChaPolyBenchmarkDriver),
        ];
        for d in &crypto_drivers {
            registry.register_target(Arc::new(CryptoTargetAdapter::new(d.clone(), CryptoMode::Process)));
            registry.register_target(Arc::new(CryptoTargetAdapter::new(d.clone(), CryptoMode::VerifyOrDecrypt)));
        }

        let container_cfgs: [(Arc<dyn ContainerBenchmarkDriver>, i32); 8] = [
            (Arc::new(ZipContainerDriver), 6), (Arc::new(TarContainerDriver), 0),
            (Arc::new(TarGzContainerDriver), 6), (Arc::new(TarZstContainerDriver), 3),
            (Arc::new(SevenZContainerDriver), 3), (Arc::new(AarContainerDriver), 1),
            (Arc::new(TarBrotliContainerDriver), 4), (Arc::new(TarSnappyContainerDriver), 1),
        ];
        for (d, lvl) in container_cfgs {
            registry.register_target(Arc::new(ContainerTargetAdapter::new(d.clone(), ContainerMode::Create, lvl, None, None)));
            registry.register_target(Arc::new(ContainerTargetAdapter::new(d, ContainerMode::Extract, lvl, None, None)));
        }
    }
}

/// Returns the adaptive recommended payload size for a target to prevent excessive execution times on ultra-heavy algorithms.
#[inline]
pub fn target_recommended_payload_size(uri: &str, requested_size: usize) -> usize {
    if uri.starts_with("crypto/") {
        return requested_size;
    }
    let is_ultra_heavy = uri.contains("ppmd")
        || uri.contains("lzma2/compress/l7")
        || uri.contains("lzma2/compress/l8")
        || uri.contains("lzma2/compress/l9")
        || uri.contains("bzip2/compress/l7")
        || uri.contains("bzip2/compress/l8")
        || uri.contains("bzip2/compress/l9")
        || uri.contains("zstd/compress/l19")
        || uri.contains("zstd/compress/l20")
        || uri.contains("zstd/compress/l21")
        || uri.contains("zstd/compress/l22")
        || uri.contains("zstd_ldm/compress/l19")
        || uri.contains("zstd_ldm/compress/l22")
        || uri.contains("brotli/compress/q10")
        || uri.contains("brotli/compress/q11")
        || uri.contains("container/7z")
        || uri.contains("container/tar_brotli");

    if is_ultra_heavy {
        return requested_size.min(131072); // 128KB
    }

    let is_heavy = uri.contains("lzma2")
        || uri.contains("bzip2")
        || uri.contains("brotli/compress/q7")
        || uri.contains("brotli/compress/q8")
        || uri.contains("brotli/compress/q9")
        || uri.contains("zstd/compress/l10")
        || uri.contains("zstd/compress/l11")
        || uri.contains("zstd/compress/l12")
        || uri.contains("zstd/compress/l13")
        || uri.contains("zstd/compress/l14")
        || uri.contains("zstd/compress/l15")
        || uri.contains("zstd/compress/l16")
        || uri.contains("zstd/compress/l17")
        || uri.contains("zstd/compress/l18");

    if is_heavy {
        return requested_size.min(524288); // 512KB
    }

    requested_size
}
