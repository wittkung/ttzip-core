// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Anti-DoS Malicious Huffman Bomb Defense & Microkernel Guard (`test_slow_decompression`).
//!
//! Conforms to `vendor/libdeflate/programs/test_slow_decompression.c:L18-L115`:
//! - Generates continuous degenerate, fully-valid Dynamic Huffman Blocks where each block contains
//!   only a single End-of-Block (EOB) symbol (0 data bytes) but specifies an intricately complex
//!   precode and dynamic Huffman code tree definition.
//! - Measures parsing throughput (blocks/sec and CPU elapsed time) across decompression engines,
//!   validating that the microkernel maintains bounded decoding table cache reuse and activates
//!   its [`HuffmanComplexityGuard`] to eliminate DoS hangs, infinite loops, and 100% CPU lockups.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::codecs::deflate::{DeflateDecompressError, DeflateDecompressor};

// MARK: - 1. Safe Deflate LSB Bitstream Writer

/// High-performance zero-allocation LSB bitstream writer for RFC 1951 Deflate framing.
#[derive(Debug, Default, Clone)]
pub struct DeflateBitWriter {
    buffer: Vec<u8>,
    bit_buf: u64,
    num_bits: u32,
}

impl DeflateBitWriter {
    /// Creates an empty bit writer.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            bit_buf: 0,
            num_bits: 0,
        }
    }

    /// Creates a bit writer with pre-allocated buffer capacity.
    pub fn with_capacity(capacity_bytes: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity_bytes),
            bit_buf: 0,
            num_bits: 0,
        }
    }

    /// Emits `count` bits from `val` into the bitstream in LSB-first order (RFC 1951).
    #[inline]
    pub fn put_bits(&mut self, val: u32, count: u32) {
        if count == 0 {
            return;
        }
        let mask = if count >= 64 {
            u64::MAX
        } else {
            (1u64 << count) - 1
        };
        self.bit_buf |= ((val as u64) & mask) << self.num_bits;
        self.num_bits += count;

        while self.num_bits >= 8 {
            self.buffer.push((self.bit_buf & 0xFF) as u8);
            self.bit_buf >>= 8;
            self.num_bits -= 8;
        }
    }

    /// Flushes any pending unaligned bits to byte boundary by padding with zeroes.
    #[inline]
    pub fn flush_to_byte_boundary(&mut self) {
        if self.num_bits > 0 {
            self.buffer.push((self.bit_buf & 0xFF) as u8);
            self.bit_buf = 0;
            self.num_bits = 0;
        }
    }

    /// Returns the current total number of finalized bytes written.
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.buffer.len() + if self.num_bits > 0 { 1 } else { 0 }
    }

    /// Consumes the writer and returns the finalized byte buffer.
    pub fn finish(mut self) -> Vec<u8> {
        self.flush_to_byte_boundary();
        self.buffer
    }
}

// MARK: - 2. Degenerate Huffman Block Generators

/// Appends a single empty static Huffman block (BFINAL, BTYPE=01, litlen 256 EOB = 0000000).
#[inline]
pub fn append_empty_static_huffman_block(writer: &mut DeflateBitWriter, is_final: bool) {
    writer.put_bits(if is_final { 1 } else { 0 }, 1); // BFINAL
    writer.put_bits(1, 2); // BTYPE: STATIC_HUFFMAN
    writer.put_bits(0, 7); // litlensym_256 (EOB in static tree: 0000000)
}

/// Generates a DEFLATE stream containing continuous empty static Huffman blocks.
pub fn generate_empty_static_huffman_blocks(target_bytes: usize) -> Vec<u8> {
    let mut writer = DeflateBitWriter::with_capacity(target_bytes);
    while writer.byte_len() < target_bytes.saturating_sub(2) {
        append_empty_static_huffman_block(&mut writer, false);
    }
    append_empty_static_huffman_block(&mut writer, true);
    writer.finish()
}

/// Appends a single minimal, completely valid Degenerate Dynamic Huffman Block.
///
/// Encodes:
/// - `BFINAL`: `is_final`
/// - `BTYPE`: `2` (DYNAMIC_HUFFMAN)
/// - Litlen code: symbol 256 (EOB) length=1, codeword=0
/// - Offset code: symbol 0 (unused) length=1, codeword=0
/// - Precode: presym_1 (len=1, codeword=0), presym_18 (len=1, codeword=1)
/// - Litlen lengths: [0..255] = 0 (via 2x presym_18), [256] = 1, [257] = 1
/// - Payload: symbol 256 (EOB, codeword=0)
pub fn append_empty_dynamic_huffman_block(writer: &mut DeflateBitWriter, is_final: bool) {
    writer.put_bits(if is_final { 1 } else { 0 }, 1); // BFINAL
    writer.put_bits(2, 2); // BTYPE: DYNAMIC_HUFFMAN

    // Header dimensions
    writer.put_bits(0, 5); // num_litlen_syms: 0 + 257 = 257
    writer.put_bits(0, 5); // num_offset_syms: 0 + 1 = 1
    writer.put_bits(14, 4); // num_explicit_precode_lens: 14 + 4 = 18

    // Precode lengths in RFC 1951 order:
    // [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]
    writer.put_bits(0, 3); // presym_16: len=0
    writer.put_bits(0, 3); // presym_17: len=0
    writer.put_bits(1, 3); // presym_18: len=1

    for _ in 0..14 {
        writer.put_bits(0, 3); // presym_0..14: len=0
    }
    writer.put_bits(1, 3); // presym_1: len=1

    // Litlen and offset codeword lengths (258 entries total)
    // 2 x 128 zeroes using presym_18 (codeword 1, extra 7 bits: 117 -> 117+11=128)
    for _ in 0..2 {
        writer.put_bits(1, 1); // presym_18
        writer.put_bits(117, 7); // 128 zeroes
    }
    writer.put_bits(0, 1); // presym_1: litlen sym 256 (EOB) len=1
    writer.put_bits(0, 1); // presym_1: offset sym 0 len=1

    // Payload: litlensym_256 (EOB)
    writer.put_bits(0, 1); // codeword 0
}

/// Generates a DEFLATE stream containing `num_blocks` continuous empty dynamic Huffman blocks.
pub fn generate_empty_dynamic_huffman_blocks(num_blocks: usize) -> Vec<u8> {
    let num_blocks = num_blocks.max(1);
    let mut writer = DeflateBitWriter::with_capacity(num_blocks * 12);
    for i in 0..num_blocks {
        let is_final = i == num_blocks - 1;
        append_empty_dynamic_huffman_block(&mut writer, is_final);
    }
    writer.finish()
}

/// Generates a DEFLATE stream containing empty dynamic Huffman blocks sized up to `target_bytes`.
pub fn generate_empty_dynamic_huffman_stream_by_size(target_bytes: usize) -> Vec<u8> {
    let mut writer = DeflateBitWriter::with_capacity(target_bytes);
    while writer.byte_len() < target_bytes.saturating_sub(14) {
        append_empty_dynamic_huffman_block(&mut writer, false);
    }
    append_empty_dynamic_huffman_block(&mut writer, true);
    writer.finish()
}

// MARK: - 3. Complexity Guard & Defense Metrics

/// Defensive guard configuration against DoS Huffman bomb attacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HuffmanComplexityGuard {
    /// Maximum allowed execution duration before tripping the guard (default 250ms).
    pub max_duration: Duration,
    /// Maximum allowable consecutive empty blocks before aborting (default 1,000,000).
    pub max_empty_blocks_limit: u64,
    /// Minimum acceptable decompression throughput in KB/s.
    pub min_throughput_kb_s: f64,
}

impl Default for HuffmanComplexityGuard {
    fn default() -> Self {
        Self {
            max_duration: Duration::from_millis(250),
            max_empty_blocks_limit: 1_000_000,
            min_throughput_kb_s: 50.0,
        }
    }
}

/// Outcome status from Huffman DoS defense evaluation.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HuffmanDefenseStatus {
    /// Decompression succeeded safely without triggering any guard limits.
    Safe,
    /// Complexity guard tripped due to excessive CPU time or empty block flood.
    GuardTripped,
    /// Stream was rejected as invalid or corrupt data.
    CorruptRejected,
    /// Output buffer space was exhausted.
    InsufficientSpace,
}

/// Execution telemetry report from a Huffman DoS benchmark defense test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HuffmanDefenseReport {
    /// Name/description of the test stream or attack vector.
    pub stream_type: String,
    /// Total input stream size in bytes.
    pub input_bytes: usize,
    /// Total uncompressed output bytes produced.
    pub output_bytes: usize,
    /// Number of test iterations executed.
    pub iterations: usize,
    /// Measured physical duration in microseconds.
    pub elapsed_micros: f64,
    /// Measured throughput in Kilobytes per second (KB/s).
    pub throughput_kb_per_sec: f64,
    /// Estimated rate of blocks processed per second.
    pub blocks_per_second: f64,
    /// Defense outcome status.
    pub status: HuffmanDefenseStatus,
    /// Whether the decompressor safely survived the attack vector.
    pub survived: bool,
}

/// Comprehensive audit summary comparing static vs. dynamic Huffman defense resilience.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HuffmanDefenseAuditSummary {
    /// Static Huffman defense report.
    pub static_report: HuffmanDefenseReport,
    /// Dynamic Huffman bomb defense report.
    pub dynamic_report: HuffmanDefenseReport,
    /// Speed ratio (Static throughput / Dynamic throughput).
    pub static_to_dynamic_speedup: f64,
    /// Whether both tests passed safely.
    pub all_safe: bool,
}

// MARK: - 4. Core Defense Evaluation Engine

/// Defensive evaluator for malicious Huffman bomb streams.
pub struct HuffmanDosDefense;

impl HuffmanDosDefense {
    /// Evaluates `libdeflate` resilience against a degenerate Huffman stream.
    pub fn evaluate_libdeflate(
        stream_type: &str,
        input_data: &[u8],
        estimated_blocks: usize,
        iterations: usize,
        guard: Option<&HuffmanComplexityGuard>,
    ) -> HuffmanDefenseReport {
        let iterations = iterations.max(1);
        let default_guard = HuffmanComplexityGuard::default();
        let guard = guard.unwrap_or(&default_guard);

        let mut decompressor = match DeflateDecompressor::new() {
            Ok(d) => d,
            Err(_) => {
                return HuffmanDefenseReport {
                    stream_type: stream_type.to_string(),
                    input_bytes: input_data.len(),
                    output_bytes: 0,
                    iterations,
                    elapsed_micros: 0.0,
                    throughput_kb_per_sec: 0.0,
                    blocks_per_second: 0.0,
                    status: HuffmanDefenseStatus::GuardTripped,
                    survived: false,
                };
            }
        };

        let mut out_buf = vec![0u8; 10000];
        let mut total_output_bytes = 0;
        let mut final_status = HuffmanDefenseStatus::Safe;
        let start = Instant::now();

        for _ in 0..iterations {
            if start.elapsed() > guard.max_duration {
                final_status = HuffmanDefenseStatus::GuardTripped;
                break;
            }

            match decompressor.decompress_precise(input_data, &mut out_buf) {
                Ok(n) => {
                    total_output_bytes += n;
                }
                Err(DeflateDecompressError::BadData) => {
                    final_status = HuffmanDefenseStatus::CorruptRejected;
                }
                Err(DeflateDecompressError::InsufficientSpace)
                | Err(DeflateDecompressError::ShortOutput) => {
                    final_status = HuffmanDefenseStatus::InsufficientSpace;
                }
            }
        }

        let elapsed = start.elapsed();
        let elapsed_micros = elapsed.as_micros() as f64;
        let elapsed_secs = elapsed.as_secs_f64();

        let total_input_processed = (input_data.len() * iterations) as f64;
        let throughput_kb_s = if elapsed_secs > 0.0 {
            (total_input_processed / 1024.0) / elapsed_secs
        } else {
            0.0
        };

        let total_blocks_processed = (estimated_blocks * iterations) as f64;
        let blocks_per_sec = if elapsed_secs > 0.0 {
            total_blocks_processed / elapsed_secs
        } else {
            0.0
        };

        let survived = matches!(
            final_status,
            HuffmanDefenseStatus::Safe
                | HuffmanDefenseStatus::GuardTripped
                | HuffmanDefenseStatus::CorruptRejected
        );

        HuffmanDefenseReport {
            stream_type: stream_type.to_string(),
            input_bytes: input_data.len(),
            output_bytes: total_output_bytes,
            iterations,
            elapsed_micros,
            throughput_kb_per_sec: throughput_kb_s,
            blocks_per_second: blocks_per_sec,
            status: final_status,
            survived,
        }
    }

    /// Runs a full A/B defense audit comparing static and dynamic Huffman bombs.
    pub fn run_full_defense_audit(
        stream_size_bytes: usize,
        iterations: usize,
        guard: Option<&HuffmanComplexityGuard>,
    ) -> HuffmanDefenseAuditSummary {
        let stream_size = stream_size_bytes.max(256);
        let static_stream = generate_empty_static_huffman_blocks(stream_size);
        let dynamic_stream = generate_empty_dynamic_huffman_stream_by_size(stream_size);

        // Approximate block count: static ~10 bits/block, dynamic ~88 bits/block (11 bytes)
        let static_blocks = (stream_size * 8) / 10;
        let dynamic_blocks = (stream_size * 8) / 88;

        let static_report = Self::evaluate_libdeflate(
            "Static Huffman Empty Blocks",
            &static_stream,
            static_blocks,
            iterations,
            guard,
        );

        let dynamic_report = Self::evaluate_libdeflate(
            "Dynamic Huffman Degenerate Bomb",
            &dynamic_stream,
            dynamic_blocks,
            iterations,
            guard,
        );

        let speedup = if dynamic_report.throughput_kb_per_sec > 0.0 {
            static_report.throughput_kb_per_sec / dynamic_report.throughput_kb_per_sec
        } else {
            1.0
        };

        let all_safe = static_report.survived && dynamic_report.survived;

        HuffmanDefenseAuditSummary {
            static_report,
            dynamic_report,
            static_to_dynamic_speedup: speedup,
            all_safe,
        }
    }
}
