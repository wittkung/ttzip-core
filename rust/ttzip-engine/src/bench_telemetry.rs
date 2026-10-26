// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-Overhead Memory RSS and Table of Contents (TOC) Benchmark Telemetry Engine.
//!
//! Provides hardware-level resident memory tracking and high-resolution latency/throughput
//! probes for archive container indexing and decompression pipelines:
//! - macOS Mach Kernel `mach_task_basic_info` / `task_info` zero-heap resident metrics.
//! - Cross-platform POSIX `rusage` fallback for non-Darwin environments.
//! - `MemoryTracker` recording baseline RSS, peak RSS, and resident delta bytes.
//! - `TocBenchmarkMetrics` measuring TOC extraction latency (microseconds) and entry throughput.
//! - Structured JSON serialization compatible with CI quality gates and statistical tests.

use std::time::Duration;
use serde::{Deserialize, Serialize};

// ============================================================================
// Platform Native Memory Diagnostics (Darwin Mach / POSIX rusage)
// ============================================================================

#[cfg(target_os = "macos")]
mod darwin_abi {
    use libc::{integer_t, mach_msg_type_number_t, natural_t};

    #[repr(C)]
    #[derive(Debug, Copy, Clone, Default)]
    pub struct mach_task_basic_info {
        pub virtual_size: u64,
        pub resident_size: u64,
        pub resident_size_max: u64,
        pub user_time: [i32; 2],
        pub system_time: [i32; 2],
        pub policy: i32,
        pub suspend_count: i32,
    }

    pub const MACH_TASK_BASIC_INFO: u32 = 20;

    extern "C" {
        pub fn mach_task_self() -> libc::mach_port_t;
        pub fn task_info(
            target_task: libc::mach_port_t,
            flavor: libc::c_uint,
            task_info_out: *mut integer_t,
            task_info_outCnt: *mut mach_msg_type_number_t,
        ) -> libc::kern_return_t;
    }

    /// Queries Darwin Mach kernel task information without heap allocation.
    #[inline]
    pub fn read_task_info() -> Option<(u64, u64, u64)> {
        unsafe {
            let mut info = mach_task_basic_info::default();
            let mut count = (std::mem::size_of::<mach_task_basic_info>()
                / std::mem::size_of::<natural_t>()) as mach_msg_type_number_t;
            let kr = task_info(
                mach_task_self(),
                MACH_TASK_BASIC_INFO,
                &mut info as *mut _ as *mut integer_t,
                &mut count,
            );
            if kr == libc::KERN_SUCCESS {
                Some((info.resident_size, info.resident_size_max, info.virtual_size))
            } else {
                None
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod darwin_abi {
    /// Non-Darwin POSIX fallback querying getrusage.
    #[inline]
    pub fn read_task_info() -> Option<(u64, u64, u64)> {
        unsafe {
            let mut usage = std::mem::zeroed::<libc::rusage>();
            if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
                let max_rss_bytes = (usage.ru_maxrss.max(0) as u64) * 1024;
                Some((max_rss_bytes, max_rss_bytes, 0))
            } else {
                None
            }
        }
    }
}

// ============================================================================
// Memory Snapshot & Metrics
// ============================================================================

/// Raw point-in-time resident and virtual memory snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// Current resident set size in bytes.
    pub resident_bytes: u64,
    /// Peak lifetime resident set size (high-water mark) in bytes.
    pub peak_resident_bytes: u64,
    /// Current virtual memory size in bytes.
    pub virtual_bytes: u64,
}

impl MemorySnapshot {
    /// Captures the current memory usage snapshot from the host operating system.
    #[inline]
    pub fn capture() -> Self {
        if let Some((resident, peak, virt)) = darwin_abi::read_task_info() {
            Self {
                resident_bytes: resident,
                peak_resident_bytes: peak,
                virtual_bytes: virt,
            }
        } else {
            Self::default()
        }
    }
}

/// Delta and peak memory consumption metrics over an execution interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Baseline resident set size before operation in bytes.
    pub baseline_rss_bytes: u64,
    /// Current resident set size after operation in bytes.
    pub current_rss_bytes: u64,
    /// Peak resident set size observed in bytes.
    pub peak_rss_bytes: u64,
    /// Net change in resident memory (current - baseline) in bytes.
    pub resident_delta_bytes: i64,
    /// Peak expansion above baseline (peak - baseline) in bytes.
    pub peak_delta_bytes: i64,
}

/// Thread-safe resident memory tracker with zero heap allocation.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryTracker {
    baseline_rss: u64,
    baseline_peak: u64,
    current_rss: u64,
    peak_rss: u64,
}

impl MemoryTracker {
    /// Initializes and starts a new memory tracking session.
    #[inline]
    pub fn start() -> Self {
        let snap = MemorySnapshot::capture();
        Self {
            baseline_rss: snap.resident_bytes,
            baseline_peak: snap.peak_resident_bytes,
            current_rss: snap.resident_bytes,
            peak_rss: snap.peak_resident_bytes,
        }
    }

    /// Samples current operating system memory metrics, updating the peak water mark.
    #[inline]
    pub fn sample(&mut self) {
        let snap = MemorySnapshot::capture();
        self.current_rss = snap.resident_bytes;
        self.peak_rss = self.peak_rss.max(snap.resident_bytes).max(snap.peak_resident_bytes);
    }

    /// Stops tracking and materializes immutable `MemoryMetrics`.
    #[inline]
    pub fn finish(&mut self) -> MemoryMetrics {
        self.sample();
        let resident_delta = self.current_rss as i64 - self.baseline_rss as i64;
        let peak_delta = (self.peak_rss as i64 - self.baseline_rss as i64).max(0);
        MemoryMetrics {
            baseline_rss_bytes: self.baseline_rss,
            current_rss_bytes: self.current_rss,
            peak_rss_bytes: self.peak_rss,
            resident_delta_bytes: resident_delta,
            peak_delta_bytes: peak_delta,
        }
    }

    /// Returns the baseline RSS in bytes recorded at tracker inception.
    #[inline]
    pub fn baseline_rss(&self) -> u64 {
        self.baseline_rss
    }

    /// Returns the baseline peak RSS in bytes recorded at tracker inception.
    #[inline]
    pub fn baseline_peak_rss(&self) -> u64 {
        self.baseline_peak
    }

    /// Returns the highest peak RSS in bytes observed.
    #[inline]
    pub fn peak_rss(&self) -> u64 {
        self.peak_rss
    }

    /// Returns the instantaneous net resident delta in bytes.
    #[inline]
    pub fn resident_delta(&self) -> i64 {
        self.current_rss as i64 - self.baseline_rss as i64
    }

    /// Standalone query returning current resident size in bytes.
    #[inline]
    pub fn current_resident_bytes() -> u64 {
        MemorySnapshot::capture().resident_bytes
    }

    /// Standalone query returning lifetime peak resident size in bytes.
    #[inline]
    pub fn peak_resident_bytes() -> u64 {
        MemorySnapshot::capture().peak_resident_bytes
    }
}

// ============================================================================
// TOC Extraction Benchmark Telemetry
// ============================================================================

/// Table of Contents (TOC) extraction performance and resource consumption telemetry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TocBenchmarkMetrics {
    /// Canonical container format identifier (e.g. "ZIP", "7Z", "TAR", "TAR.GZ", "TAR.XZ", "TAR.ZST", "SPARSE").
    pub container_format: String,
    /// Total number of indexed directory entries.
    pub entry_count: usize,
    /// Physical size of the archive container file or memory buffer in bytes.
    pub archive_size_bytes: u64,
    /// Sum of uncompressed file payload sizes declared across all catalog entries.
    pub uncompressed_size_bytes: u64,
    /// Exact wall-clock latency to locate, parse, and materialize the TOC in nanoseconds.
    pub latency_nanos: u64,
    /// Wall-clock latency to locate, parse, and materialize the TOC in microseconds.
    pub latency_micros: f64,
    /// TOC parsing throughput in catalog entries per second.
    pub throughput_entries_per_sec: f64,
    /// Container processing throughput in MB/s based on compressed archive size.
    pub throughput_archive_mb_sec: f64,
    /// Catalog data throughput in MB/s based on total uncompressed content.
    pub throughput_uncompressed_mb_sec: f64,
    /// Baseline resident memory at TOC extraction start in bytes.
    pub memory_baseline_rss: u64,
    /// Peak resident memory observed during TOC extraction in bytes.
    pub memory_peak_rss: u64,
    /// Net resident memory delta induced by the catalog parser in bytes.
    pub memory_delta_bytes: i64,
}

impl TocBenchmarkMetrics {
    /// Constructs a new `TocBenchmarkMetrics` record from measurement readings.
    pub fn new(
        container_format: impl Into<String>,
        entry_count: usize,
        archive_size_bytes: u64,
        uncompressed_size_bytes: u64,
        duration: Duration,
        memory: MemoryMetrics,
    ) -> Self {
        let latency_nanos = duration.as_nanos() as u64;
        let latency_micros = duration.as_secs_f64() * 1_000_000.0;
        let secs = duration.as_secs_f64().max(1e-9);
        let throughput_entries_per_sec = (entry_count as f64) / secs;
        let throughput_archive_mb_sec = (archive_size_bytes as f64 / 1_000_000.0) / secs;
        let throughput_uncompressed_mb_sec = (uncompressed_size_bytes as f64 / 1_000_000.0) / secs;

        Self {
            container_format: container_format.into(),
            entry_count,
            archive_size_bytes,
            uncompressed_size_bytes,
            latency_nanos,
            latency_micros,
            throughput_entries_per_sec,
            throughput_archive_mb_sec,
            throughput_uncompressed_mb_sec,
            memory_baseline_rss: memory.baseline_rss_bytes,
            memory_peak_rss: memory.peak_rss_bytes,
            memory_delta_bytes: memory.resident_delta_bytes,
        }
    }
}

// ============================================================================
// Decompression & Multi-Format Telemetry Records
// ============================================================================

/// Single-stage decompression performance and memory consumption record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecompressionBenchmarkMetrics {
    /// Target container format.
    pub container_format: String,
    /// Compression codec or algorithm name.
    pub codec: String,
    /// Streaming mode: "sequential" or "chunked".
    pub mode: String,
    /// Compressed input payload size in bytes.
    pub input_size_bytes: u64,
    /// Extracted uncompressed payload size in bytes.
    pub uncompressed_size_bytes: u64,
    /// Total decompression wall-clock latency in nanoseconds.
    pub latency_nanos: u64,
    /// Effective throughput in MB/s based on uncompressed bytes produced.
    pub throughput_mb_sec: f64,
    /// Resident memory before decompression in bytes.
    pub memory_baseline_rss: u64,
    /// Peak resident memory reached during decompression in bytes.
    pub memory_peak_rss: u64,
    /// Net resident memory delta in bytes.
    pub memory_delta_bytes: i64,
}

/// Unified multi-format container benchmark record aggregating TOC and dual-mode decompression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiFormatBenchmarkRecord {
    /// Container identifier (ZIP, 7Z, TAR, GZIP, XZ, ZSTD, APFS-SPARSE).
    pub container: String,
    /// Specific codec or profile (Deflate, Zstd, LZMA2, etc.).
    pub codec: String,
    /// Input size before compression in bytes.
    pub input_size_bytes: u64,
    /// Output compressed archive size in bytes.
    pub compressed_size_bytes: u64,
    /// Compression ratio (compressed / uncompressed).
    pub compression_ratio: f64,
    /// Table of Contents extraction telemetry.
    pub toc_metrics: Option<TocBenchmarkMetrics>,
    /// Full sequential decompression telemetry.
    pub seq_decompression: DecompressionBenchmarkMetrics,
    /// Chunked (e.g. 64KB bounded buffer) decompression telemetry.
    pub chunked_decompression: DecompressionBenchmarkMetrics,
}

/// Statistical sample series for automated A/B hypothesis testing gates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkMetricSamples {
    /// Benchmark operation key.
    pub name: String,
    /// Measured metric name (e.g. "throughput_mb_s", "toc_latency_micros").
    pub metric: String,
    /// Display unit.
    pub unit: String,
    /// Raw observation values across benchmark repetitions.
    pub samples: Vec<f64>,
    /// Sample arithmetic mean.
    pub mean: f64,
    /// Minimum observation.
    pub min: f64,
    /// Maximum observation.
    pub max: f64,
    /// Sample standard deviation.
    pub std_dev: f64,
}

impl BenchmarkMetricSamples {
    /// Constructs a sample distribution from an observation vector.
    pub fn from_samples(name: impl Into<String>, metric: impl Into<String>, unit: impl Into<String>, samples: Vec<f64>) -> Self {
        let n = samples.len() as f64;
        if samples.is_empty() {
            return Self {
                name: name.into(),
                metric: metric.into(),
                unit: unit.into(),
                samples,
                mean: 0.0,
                min: 0.0,
                max: 0.0,
                std_dev: 0.0,
            };
        }
        let mean = samples.iter().sum::<f64>() / n;
        let min = samples.iter().copied().fold(f64::INFINITY, f64::min);
        let max = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let variance = if samples.len() > 1 {
            samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
        } else {
            0.0
        };
        let std_dev = variance.sqrt();
        Self {
            name: name.into(),
            metric: metric.into(),
            unit: unit.into(),
            samples,
            mean,
            min,
            max,
            std_dev,
        }
    }
}

/// Full multi-format benchmark run report conforming to `ab_stat_gate.py` input contracts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiFormatBenchmarkSuiteReport {
    /// Suite title identifier.
    pub suite_name: String,
    /// Execution timestamp in Unix epoch seconds.
    pub timestamp_epoch_secs: u64,
    /// Host CPU microarchitecture.
    pub host_arch: String,
    /// Name and description of the synthetic workload corpus.
    pub corpus_name: String,
    /// Theoretical Shannon entropy of the test payload.
    pub corpus_entropy: f64,
    /// Multi-format container benchmark records.
    pub records: Vec<MultiFormatBenchmarkRecord>,
    /// Statistical sample distributions indexed for automated regression gating.
    pub distributions: Vec<BenchmarkMetricSamples>,
    /// True if all benchmark targets satisfied operational throughput and latency constraints.
    pub summary_overall_passed: bool,
}

// Compile-time static thread safety and Send + Sync verification.
const fn assert_send_sync<T: Send + Sync>() {}
const _: () = {
    assert_send_sync::<MemorySnapshot>();
    assert_send_sync::<MemoryMetrics>();
    assert_send_sync::<MemoryTracker>();
    assert_send_sync::<TocBenchmarkMetrics>();
    assert_send_sync::<DecompressionBenchmarkMetrics>();
    assert_send_sync::<MultiFormatBenchmarkRecord>();
    assert_send_sync::<BenchmarkMetricSamples>();
    assert_send_sync::<MultiFormatBenchmarkSuiteReport>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker_lifecycle() {
        let mut tracker = MemoryTracker::start();
        assert!(tracker.baseline_rss() > 0 || cfg!(not(target_os = "macos")));

        // Allocate transient memory buffer to induce resident footprint change
        let mut dummy = vec![0xAAu8; 1024 * 1024 * 4];
        dummy[0] = 0x55;
        let last_idx = dummy.len() - 1;
        dummy[last_idx] = 0x55;
        std::hint::black_box(&dummy);

        tracker.sample();
        let metrics = tracker.finish();

        assert!(metrics.baseline_rss_bytes > 0 || cfg!(not(target_os = "macos")));
        assert!(metrics.peak_rss_bytes >= metrics.baseline_rss_bytes);
    }

    #[test]
    fn test_toc_benchmark_metrics_computation() {
        let metrics = MemoryMetrics {
            baseline_rss_bytes: 10_000_000,
            current_rss_bytes: 12_000_000,
            peak_rss_bytes: 15_000_000,
            resident_delta_bytes: 2_000_000,
            peak_delta_bytes: 5_000_000,
        };

        let toc = TocBenchmarkMetrics::new(
            "ZIP",
            1000,
            50_000_000,
            100_000_000,
            Duration::from_millis(50),
            metrics,
        );

        assert_eq!(toc.container_format, "ZIP");
        assert_eq!(toc.entry_count, 1000);
        assert_eq!(toc.archive_size_bytes, 50_000_000);
        assert!((toc.latency_micros - 50_000.0).abs() < 100.0);
        assert!(toc.throughput_entries_per_sec > 19_000.0);
        assert_eq!(toc.memory_delta_bytes, 2_000_000);
    }

    #[test]
    fn test_benchmark_metric_samples_statistics() {
        let samples = vec![100.0, 102.0, 98.0, 104.0, 96.0];
        let dist = BenchmarkMetricSamples::from_samples("test_decomp", "throughput_mb_s", "MB/s", samples);

        assert_eq!(dist.name, "test_decomp");
        assert!((dist.mean - 100.0).abs() < 1e-6);
        assert_eq!(dist.min, 96.0);
        assert_eq!(dist.max, 104.0);
        assert!(dist.std_dev > 0.0);
    }
}
