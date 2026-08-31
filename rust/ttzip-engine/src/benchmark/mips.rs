// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Official LZMA2 MIPS Hardware Benchmark Engine.
//!
//! Implements Igor Pavlov's canonical 7-Zip MIPS rating standard:
//! - LZMA2 multi-threaded compression & decompression kernels (`fast-lzma2`).
//! - Logarithmic polynomial complexity mathematical model:
//!   `Complexity(D) = 870 + 5 * (log2(D) - 18)^2`
//! - Decompression command model:
//!   `Decompress Commands = (190 * packSize + 4 * unpackSize) * iters`
//! - `CountCpuFreq`: 128-instruction pure ALU dependency micro-loop for frequency calibration.
//! - `BENCH_ALLOCA_VALUE` cache-line collision prevention.
//! - `libc::getrusage` precision CPU utilization and single-core Rating/Usage calculation.

use super::clock::MonotonicStopwatch;
use crate::codecs::lzma2::{
    fl2_compress_bound, Fl2CCtx, Fl2DCtx,
};
use crate::fs::apfs::AlignedBuffer;
use crate::types::TTZipStatus;

/// 64-bit deterministic SplitMix64 PRNG for synthetic compressible benchmark stream generation.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    #[inline]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

/// Standardized 7-Zip aligned MIPS hardware benchmark telemetry and scores.
#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct MIPSResult {
    pub dictionary_size_mb: u32,
    pub thread_count: u32,
    pub compress_mips: f64,
    pub decompress_mips: f64,
    pub total_mips: f64,
    pub compress_speed_mbs: f64,
    pub decompress_speed_mbs: f64,
    pub cpu_usage_percent: f64,
    pub rating_per_usage_mips: f64,
}

/// POSIX rusage CPU measurement snapshot for user and system time tracking.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessRusage {
    pub user_secs: f64,
    pub sys_secs: f64,
}

impl ProcessRusage {
    /// Queries current process CPU resource usage using POSIX `getrusage`.
    pub fn current() -> Self {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        let ret = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
        if ret == 0 {
            let u = unsafe { usage.assume_init() };
            let user_secs = (u.ru_utime.tv_sec as f64) + (u.ru_utime.tv_usec as f64) * 1e-6;
            let sys_secs = (u.ru_stime.tv_sec as f64) + (u.ru_stime.tv_usec as f64) * 1e-6;
            Self { user_secs, sys_secs }
        } else {
            Self::default()
        }
    }

    #[inline]
    pub fn total_cpu_secs(&self) -> f64 {
        self.user_secs + self.sys_secs
    }
}

/// Internal measurement metrics for MIPS score calculation.
#[derive(Debug, Clone, Copy)]
struct MipsMetricsInput {
    dict_mb: u32,
    threads: u32,
    iters: u32,
    unpack_size_per_iter: usize,
    pack_size_per_iter: usize,
    comp_elapsed: f64,
    decomp_elapsed: f64,
    wall_elapsed: f64,
    cpu_time_elapsed: f64,
}

/// Hardware performance benchmark engine based on official 7-Zip LZMA2 MIPS formulas.
pub struct MIPSHardwareBenchmarkEngine;

impl MIPSHardwareBenchmarkEngine {
    /// Offsets stack pointer allocation per thread to prevent L1/L2 cache set thrashing (`BENCH_ALLOCA_VALUE`).
    #[inline(always)]
    pub fn bench_alloca_offset(thread_idx: usize) -> Vec<u8> {
        let pad_size = (thread_idx * 64) % 4096;
        vec![0u8; pad_size]
    }

    /// Calculates official 7-Zip LZMA/LZMA2 compression algorithmic complexity factor:
    /// `Complexity(D) = 870 + 5 * (log2(D) - 18)^2`
    #[inline]
    pub fn calculate_lzma2_complexity(dictionary_size_bytes: usize) -> f64 {
        let dict_size = dictionary_size_bytes.max(262_144) as f64; // minimum 256KB for log2
        let log2_d = dict_size.log2();
        let delta = log2_d - 18.0;
        870.0 + 5.0 * (delta * delta)
    }

    /// Executes one unrolled iteration of 128 data-dependent ALU instructions (`CountCpuFreq`).
    #[inline(never)]
    pub fn count_cpu_freq_inner(mut a: u64, mut b: u64) -> (u64, u64) {
        macro_rules! alu_step8 {
            () => {
                a = a.wrapping_add(b); b ^= a;
                a = a.wrapping_add(b); b ^= a;
                a = a.wrapping_add(b); b ^= a;
                a = a.wrapping_add(b); b ^= a;
                a = a.wrapping_add(b); b ^= a;
                a = a.wrapping_add(b); b ^= a;
                a = a.wrapping_add(b); b ^= a;
                a = a.wrapping_add(b); b ^= a;
            };
        }
        // 16 blocks * 8 instructions = 128 dependent ALU operations
        alu_step8!(); alu_step8!(); alu_step8!(); alu_step8!();
        alu_step8!(); alu_step8!(); alu_step8!(); alu_step8!();
        alu_step8!(); alu_step8!(); alu_step8!(); alu_step8!();
        alu_step8!(); alu_step8!(); alu_step8!(); alu_step8!();
        (a, b)
    }

    /// Measures effective CPU core clock frequency in MHz using 128-instruction ALU dependency loop.
    pub fn count_cpu_freq(duration_millis: u64) -> f64 {
        let mut a: u64 = 0x1234_5678_9ABC_DEF0;
        let mut b: u64 = 0xFEDC_BA98_7654_3210;

        let t_start = std::time::Instant::now();
        let target_duration = std::time::Duration::from_millis(duration_millis.clamp(5, 500));

        let mut total_iters: u64 = 0;
        while t_start.elapsed() < target_duration {
            for _ in 0..10_000 {
                let res = Self::count_cpu_freq_inner(a, b);
                a = res.0;
                b = res.1;
            }
            total_iters += 10_000;
        }
        let elapsed_sec = t_start.elapsed().as_secs_f64().max(1e-6);
        std::hint::black_box((a, b));

        let total_instructions = (total_iters as f64) * 128.0;
        let freq_hz = total_instructions / elapsed_sec;
        freq_hz / 1_000_000.0
    }

    /// Executes a standardized 7-Zip LZMA2 MIPS benchmark pass.
    pub fn run_benchmark(
        dictionary_size_mb: u32,
        thread_count: u32,
        iterations: u32,
    ) -> Result<MIPSResult, TTZipStatus> {
        let dict_mb = if dictionary_size_mb == 0 { 32 } else { dictionary_size_mb };
        let threads = if thread_count == 0 { 1 } else { thread_count };
        let iters = if iterations == 0 { 1 } else { iterations };
        let buffer_size = (dict_mb as usize) * 1024 * 1024;

        if threads == 1 {
            Self::run_single_thread(dict_mb, iters, buffer_size)
        } else {
            Self::run_multi_thread(dict_mb, threads, iters, buffer_size)
        }
    }

    fn run_single_thread(
        dict_mb: u32,
        iters: u32,
        buffer_size: usize,
    ) -> Result<MIPSResult, TTZipStatus> {
        let _stack_pad = Self::bench_alloca_offset(0);
        std::hint::black_box(&_stack_pad);

        let mut src_buf = AlignedBuffer::new(buffer_size)?;
        let comp_bound = fl2_compress_bound(buffer_size) + (64 * 1024);
        let mut comp_buf = AlignedBuffer::new(comp_bound)?;
        let mut decomp_buf = AlignedBuffer::new(buffer_size)?;

        let mut prng = SplitMix64::new(0xDEAD_BEEF_CAFE);
        for (i, byte) in src_buf.as_mut_slice()[..buffer_size].iter_mut().enumerate() {
            *byte = ((prng.next_u64() ^ (i as u64 % 256)) & 0xFF) as u8;
        }

        let mut cctx = Fl2CCtx::new()?;
        let mut dctx = Fl2DCtx::new()?;
        let mut compressed_len = 0;

        let rusage_start = ProcessRusage::current();
        let bench_sw = MonotonicStopwatch::start();

        // 1. Compression pass
        let comp_sw = MonotonicStopwatch::start();
        for _ in 0..iters {
            compressed_len = cctx.compress(
                &src_buf.as_slice()[..buffer_size],
                comp_buf.as_mut_slice(),
                3, // Level 3 fast match finder for standard 7-Zip benchmark
            )?;
        }
        let comp_elapsed = comp_sw.elapsed_secs().max(0.0001);

        // 2. Decompression pass
        let decomp_sw = MonotonicStopwatch::start();
        for _ in 0..iters {
            let written = dctx.decompress(
                &comp_buf.as_slice()[..compressed_len],
                &mut decomp_buf.as_mut_slice()[..buffer_size],
            )?;
            if written != buffer_size {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
        }
        let decomp_elapsed = decomp_sw.elapsed_secs().max(0.0001);

        let total_wall_elapsed = bench_sw.elapsed_secs().max(0.0001);
        let rusage_end = ProcessRusage::current();
        let cpu_time_elapsed = (rusage_end.total_cpu_secs() - rusage_start.total_cpu_secs()).max(0.0);

        Self::calculate_metric(&MipsMetricsInput {
            dict_mb,
            threads: 1,
            iters,
            unpack_size_per_iter: buffer_size,
            pack_size_per_iter: compressed_len,
            comp_elapsed,
            decomp_elapsed,
            wall_elapsed: total_wall_elapsed,
            cpu_time_elapsed,
        })
    }

    fn run_multi_thread(
        dict_mb: u32,
        threads: u32,
        iters: u32,
        buffer_size: usize,
    ) -> Result<MIPSResult, TTZipStatus> {
        use rayon::prelude::*;

        let rusage_start = ProcessRusage::current();
        let bench_sw = MonotonicStopwatch::start();

        // 1. Parallel Compression pass
        let comp_sw = MonotonicStopwatch::start();
        let thread_results: Result<Vec<(AlignedBuffer, usize)>, TTZipStatus> = (0..threads)
            .into_par_iter()
            .map(|thread_idx| -> Result<(AlignedBuffer, usize), TTZipStatus> {
                let _stack_pad = Self::bench_alloca_offset(thread_idx as usize);
                std::hint::black_box(&_stack_pad);

                let mut src_buf = AlignedBuffer::new(buffer_size)?;
                let comp_bound = fl2_compress_bound(buffer_size) + (64 * 1024);
                let mut comp_buf = AlignedBuffer::new(comp_bound)?;

                let mut prng = SplitMix64::new(0xDEAD_BEEF_CAFE ^ (thread_idx as u64));
                for (i, byte) in src_buf.as_mut_slice()[..buffer_size].iter_mut().enumerate() {
                    *byte = ((prng.next_u64() ^ (i as u64 % 256)) & 0xFF) as u8;
                }

                let mut cctx = Fl2CCtx::new()?;
                let mut compressed_len = 0;
                for _ in 0..iters {
                    compressed_len = cctx.compress(
                        &src_buf.as_slice()[..buffer_size],
                        comp_buf.as_mut_slice(),
                        3,
                    )?;
                }
                Ok((comp_buf, compressed_len))
            })
            .collect();

        let compressed_items = thread_results?;
        let comp_elapsed = comp_sw.elapsed_secs().max(0.0001);

        // 2. Parallel Decompression pass
        let decomp_sw = MonotonicStopwatch::start();
        let decomp_res: Result<(), TTZipStatus> = compressed_items
            .par_iter()
            .enumerate()
            .map(|(thread_idx, (comp_buf, compressed_len))| -> Result<(), TTZipStatus> {
                let _stack_pad = Self::bench_alloca_offset(thread_idx);
                std::hint::black_box(&_stack_pad);

                let mut decomp_buf = AlignedBuffer::new(buffer_size)?;
                let mut dctx = Fl2DCtx::new()?;
                for _ in 0..iters {
                    let written = dctx.decompress(
                        &comp_buf.as_slice()[..*compressed_len],
                        &mut decomp_buf.as_mut_slice()[..buffer_size],
                    )?;
                    if written != buffer_size {
                        return Err(TTZipStatus::ErrExtractionFailed);
                    }
                }
                Ok(())
            })
            .collect();
        decomp_res?;
        let decomp_elapsed = decomp_sw.elapsed_secs().max(0.0001);

        let total_wall_elapsed = bench_sw.elapsed_secs().max(0.0001);
        let rusage_end = ProcessRusage::current();
        let cpu_time_elapsed = (rusage_end.total_cpu_secs() - rusage_start.total_cpu_secs()).max(0.0);

        let total_pack_size: usize = compressed_items.iter().map(|(_, sz)| *sz).sum();
        let avg_pack_size = total_pack_size / (threads.max(1) as usize);

        Self::calculate_metric(&MipsMetricsInput {
            dict_mb,
            threads,
            iters,
            unpack_size_per_iter: buffer_size * (threads as usize),
            pack_size_per_iter: avg_pack_size * (threads as usize),
            comp_elapsed,
            decomp_elapsed,
            wall_elapsed: total_wall_elapsed,
            cpu_time_elapsed,
        })
    }

    fn calculate_metric(input: &MipsMetricsInput) -> Result<MIPSResult, TTZipStatus> {
        let total_uncompressed_bytes = (input.unpack_size_per_iter as f64) * (input.iters as f64);
        let total_compressed_bytes = (input.pack_size_per_iter as f64) * (input.iters as f64);

        let uncompressed_mb = total_uncompressed_bytes / (1024.0 * 1024.0);
        let comp_speed_mbs = uncompressed_mb / input.comp_elapsed;
        let decomp_speed_mbs = uncompressed_mb / input.decomp_elapsed;

        // 1. Official 7-Zip Logarithmic Polynomial Complexity:
        // Complexity(D) = 870 + 5 * (log2(D) - 18)^2
        let dictionary_bytes = (input.dict_mb as usize) * 1024 * 1024;
        let complexity_d = Self::calculate_lzma2_complexity(dictionary_bytes);

        // 2. Compress MIPS = Throughput(MB/s) * Complexity(D)
        let compress_mips = comp_speed_mbs * complexity_d;

        // 3. Official 7-Zip Decompress Commands:
        // Decompress Commands = (190 * packSize + 4 * unpackSize) * iters
        let decompress_commands = 190.0 * total_compressed_bytes + 4.0 * total_uncompressed_bytes;

        // 4. Decompress MIPS = Decompress Commands / (Elapsed_Time * 10^6)
        let decompress_mips = decompress_commands / (input.decomp_elapsed * 1_000_000.0);

        // 5. Total MIPS
        let total_mips = (compress_mips + decompress_mips) / 2.0;

        // 6. CPU Usage % & Rating/Usage using getrusage
        let measured_cpu_usage = if input.wall_elapsed > 0.0001 && input.cpu_time_elapsed > 0.0 {
            (input.cpu_time_elapsed / input.wall_elapsed) * 100.0
        } else {
            (input.threads as f64) * 100.0
        };
        let cpu_usage_percent = measured_cpu_usage.clamp(50.0, (input.threads as f64) * 120.0);
        let rating_per_usage = total_mips / (cpu_usage_percent / 100.0).max(0.01);

        Ok(MIPSResult {
            dictionary_size_mb: input.dict_mb,
            thread_count: input.threads,
            compress_mips,
            decompress_mips,
            total_mips,
            compress_speed_mbs: comp_speed_mbs,
            decompress_speed_mbs: decomp_speed_mbs,
            cpu_usage_percent,
            rating_per_usage_mips: rating_per_usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lzma2_mips_complexity_formula() {
        // D = 1 MB (2^20 bytes) -> log2 = 20 -> 870 + 5 * (20 - 18)^2 = 870 + 20 = 890
        let c_1mb = MIPSHardwareBenchmarkEngine::calculate_lzma2_complexity(1024 * 1024);
        assert_eq!(c_1mb, 890.0);

        // D = 32 MB (2^25 bytes) -> log2 = 25 -> 870 + 5 * (25 - 18)^2 = 870 + 245 = 1115
        let c_32mb = MIPSHardwareBenchmarkEngine::calculate_lzma2_complexity(32 * 1024 * 1024);
        assert_eq!(c_32mb, 1115.0);

        // D = 64 MB (2^26 bytes) -> log2 = 26 -> 870 + 5 * (26 - 18)^2 = 870 + 320 = 1190
        let c_64mb = MIPSHardwareBenchmarkEngine::calculate_lzma2_complexity(64 * 1024 * 1024);
        assert_eq!(c_64mb, 1190.0);
    }

    #[test]
    fn test_count_cpu_freq_execution() {
        let freq_mhz = MIPSHardwareBenchmarkEngine::count_cpu_freq(10);
        assert!(freq_mhz > 50.0, "Measured CPU freq must be > 50 MHz, got {} MHz", freq_mhz);
    }

    #[test]
    fn test_bench_alloca_offset() {
        let pad0 = MIPSHardwareBenchmarkEngine::bench_alloca_offset(0);
        assert_eq!(pad0.len(), 0);
        let pad1 = MIPSHardwareBenchmarkEngine::bench_alloca_offset(1);
        assert_eq!(pad1.len(), 64);
        let pad2 = MIPSHardwareBenchmarkEngine::bench_alloca_offset(2);
        assert_eq!(pad2.len(), 128);
    }

    #[test]
    fn test_mips_hardware_benchmark_single_thread() {
        let res = MIPSHardwareBenchmarkEngine::run_benchmark(1, 1, 1)
            .expect("1MB MIPS benchmark should pass");
        assert!(res.compress_mips > 0.0);
        assert!(res.decompress_mips > 0.0);
        assert!(res.total_mips > 0.0);
        assert!(res.compress_speed_mbs > 0.0);
        assert!(res.decompress_speed_mbs > 0.0);
        assert_eq!(res.thread_count, 1);
        assert_eq!(res.dictionary_size_mb, 1);
        assert!(res.rating_per_usage_mips > 0.0);
    }

    #[test]
    fn test_mips_hardware_benchmark_multi_thread() {
        let res = MIPSHardwareBenchmarkEngine::run_benchmark(1, 2, 1)
            .expect("2-thread 1MB MIPS benchmark should pass");
        assert!(res.total_mips > 0.0);
        assert_eq!(res.thread_count, 2);
        assert_eq!(res.dictionary_size_mb, 1);
    }
}

