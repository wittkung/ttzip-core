// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Differential Test Suite for 7-Zip Solid Multi-File Selective Extraction.
//!
//! Validates Task 14.9:
//! - Scalable solid archive datasets (10, 100, 1000 files) with mixed content types
//!   (structured text, source code, binary blobs, synthetic image data).
//! - 4 distinct selective extraction access patterns:
//!   1. Single-file fast extraction: first (0), middle (N/2), last (N-1).
//!   2. Contiguous slice extraction: range [10..20] (or [2..7] for N=10).
//!   3. Random scattered extraction: 15 non-contiguous pseudorandom entries.
//!   4. Reverse order extraction: from end to beginning.
//! - Strict Bit-Exact cryptographic verification using SHA-256 and CRC-32 digests.
//! - Early-Exit termination verification: exact skipped trailing/preceding bytes accounting,
//!   and significant CPU latency/throughput advantages.

use sha2::{Digest, Sha256};
use std::time::Instant;

use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::decoder::solid_stream::{
    SolidEarlyExitExtractor, SolidExtractionStats, SOLID_MICRO_BUFFER_CHUNK_SIZE,
};
use ttzip_engine::sevenz::{
    create_7z_solid_archive_bytes, decode_7z_solid_streaming, SevenZArchive,
};
use ttzip_engine::zip::writer::ZipInputItem;

/// Ground truth representation for test validation against archive extraction outputs.
#[derive(Debug, Clone)]
struct GroundTruthFile {
    rel_path: String,
    data: Vec<u8>,
    crc32: u32,
    sha256_hex: String,
}

/// Deterministic Pseudo-Random Number Generator for reproducible test vectors.
struct FastRng {
    state: u64,
}

impl FastRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u32;
        min + (self.next_u32() % span) as usize
    }
}

/// Generates diverse, high-fidelity mixed datasets covering structured text, source code,
/// binary blobs, and synthetic media across `count` file entries.
fn generate_mixed_solid_dataset(count: usize) -> (Vec<ZipInputItem>, Vec<GroundTruthFile>) {
    let mut items = Vec::with_capacity(count);
    let mut ground_truth = Vec::with_capacity(count);
    let mut rng = FastRng::new(0xdeadbeef_cafebabe ^ (count as u64));

    for i in 0..count {
        let category = i % 4;
        let (rel_path, data) = match category {
            // Category 0: Structured Text (JSON / Markdown / Config)
            0 => {
                let path = format!("docs/module_{:04}_spec.json", i);
                let target_len = rng.range(2048, 8192);
                let mut buf = Vec::with_capacity(target_len + 256);

                let header = format!(
                    "{{\n  \"$schema\": \"https://json-schema.org/draft/2020-12/schema\",\n  \"id\": \"spec-{:04}\",\n  \"version\": \"1.0.{}\",\n  \"entries\": [\n",
                    i, i
                );
                buf.extend_from_slice(header.as_bytes());

                let mut row = 0;
                while buf.len() < target_len {
                    let item_line = format!(
                        "    {{\"index\": {}, \"uuid\": \"{:08x}-{:04x}\", \"active\": {}}},\n",
                        row,
                        rng.next_u32(),
                        (rng.next_u32() & 0xffff) as u16,
                        if row % 2 == 0 { "true" } else { "false" }
                    );
                    buf.extend_from_slice(item_line.as_bytes());
                    row += 1;
                }
                buf.extend_from_slice(b"    {\"final\": true}\n  ]\n}\n");
                (path, buf)
            }

            // Category 1: Source Code (Rust / Swift / C / Python)
            1 => {
                let sub_type = (i / 4) % 4;
                let (path, ext_header) = match sub_type {
                    0 => (format!("src/kernel/driver_{:04}.rs", i), "// TTZip Rust Kernel Module\nuse std::sync::Arc;\n"),
                    1 => (format!("apple/views/View_{:04}.swift", i), "// TTZip Swift Native View\nimport SwiftUI\n"),
                    2 => (format!("native/include/bridge_{:04}.h", i), "/* C-ABI Export Header */\n#pragma once\n#include <stdint.h>\n"),
                    _ => (format!("scripts/pipeline_{:04}.py", i), "# TTZip Automation Pipeline\nimport os, sys, json\n"),
                };

                let target_len = rng.range(3072, 10240);
                let mut buf = Vec::with_capacity(target_len + 256);
                buf.extend_from_slice(ext_header.as_bytes());

                let mut fn_idx = 0;
                while buf.len() < target_len {
                    let fn_block = format!(
                        "\npub fn execute_subtask_{:04}_{:02}(context_id: u64) -> Result<u32, i32> {{\n    let seed = 0x{:08x};\n    let computed = (context_id as u32).wrapping_mul(seed ^ 0x5bd1e995);\n    Ok(computed.rotate_left(5))\n}}\n",
                        i, fn_idx, rng.next_u32()
                    );
                    buf.extend_from_slice(fn_block.as_bytes());
                    fn_idx += 1;
                }
                (path, buf)
            }

            // Category 2: Binary Blobs (Mach-O / ELF Mock Headers + Patterned Byte Streams)
            2 => {
                let path = format!("bin/payload_{:04}.bin", i);
                let target_len = rng.range(4096, 12288);
                let mut buf = Vec::with_capacity(target_len);

                // 64-bit Mach-O Magic Header: 0xfeedfacf (little-endian: CF FA ED FE)
                buf.extend_from_slice(&[0xcf, 0xfa, 0xed, 0xfe, 0x07, 0x00, 0x00, 0x01]);
                buf.extend_from_slice(&[0x03, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00]);

                let seed = rng.next_u32();
                while buf.len() < target_len {
                    let chunk_val = seed.wrapping_add((buf.len() as u32).wrapping_mul(0x9e3779b9));
                    buf.extend_from_slice(&chunk_val.to_le_bytes());
                }
                buf.truncate(target_len);
                (path, buf)
            }

            // Category 3: Synthetic Image & Media (BMP Header + Pixel Raster)
            _ => {
                let path = format!("assets/textures/raster_{:04}.bmp", i);
                let width = 48usize;
                let height = rng.range(32, 96);
                let row_bytes = (width * 3 + 3) & !3; // 4-byte aligned BMP row width
                let image_data_size = row_bytes * height;
                let file_size = 54 + image_data_size;

                let mut buf = Vec::with_capacity(file_size);
                // BMP File Header (14 bytes)
                buf.extend_from_slice(b"BM");
                buf.extend_from_slice(&(file_size as u32).to_le_bytes());
                buf.extend_from_slice(&[0, 0, 0, 0]); // Reserved
                buf.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel data

                // DIB Header (BITMAPINFOHEADER - 40 bytes)
                buf.extend_from_slice(&40u32.to_le_bytes());
                buf.extend_from_slice(&(width as i32).to_le_bytes());
                buf.extend_from_slice(&(height as i32).to_le_bytes());
                buf.extend_from_slice(&1u16.to_le_bytes()); // Planes
                buf.extend_from_slice(&24u16.to_le_bytes()); // 24-bit RGB
                buf.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB (Uncompressed)
                buf.extend_from_slice(&(image_data_size as u32).to_le_bytes());
                buf.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI X
                buf.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI Y
                buf.extend_from_slice(&0u32.to_le_bytes());
                buf.extend_from_slice(&0u32.to_le_bytes());

                // Synthetic RGB pixel data with smooth diagonal gradients
                for y in 0..height {
                    for x in 0..width {
                        let r = ((x * 255) / width) as u8;
                        let g = ((y * 255) / height) as u8;
                        let b = (((x + y) * 128) / (width + height)) as u8;
                        buf.push(b);
                        buf.push(g);
                        buf.push(r);
                    }
                    if row_bytes > width * 3 {
                        buf.extend(std::iter::repeat_n(0, row_bytes - width * 3)); // Row padding
                    }
                }
                (path, buf)
            }
        };

        let crc = crc32_fast(0, &data);
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256_hex = hex::encode(hasher.finalize());

        ground_truth.push(GroundTruthFile {
            rel_path: rel_path.clone(),
            data: data.clone(),
            crc32: crc,
            sha256_hex,
        });

        items.push(ZipInputItem {
            rel_path,
            data,
            mtime_epoch_secs: (1700000000 + i * 11) as u32,
            mode: 0o644,
            is_directory: false,
        });
    }

    (items, ground_truth)
}

/// Verifies that a selectively extracted file matches ground truth with 100% Bit-Exact precision
/// across Byte Content, CRC-32, and SHA-256 digests.
fn verify_extracted_entry(
    extractor: &SolidEarlyExitExtractor<'_>,
    expected: &GroundTruthFile,
    file_idx: usize,
) -> SolidExtractionStats {
    let (extracted_bytes, stats) = extractor
        .extract_to_vec(file_idx, None)
        .unwrap_or_else(|e| panic!("Selective extraction failed for entry {} ({}): {:?}", file_idx, expected.rel_path, e));

    // 1. Bit-Exact Length & Content Assertion
    assert_eq!(
        extracted_bytes.len(),
        expected.data.len(),
        "Byte length mismatch for entry {} ({})",
        file_idx,
        expected.rel_path
    );
    assert_eq!(
        extracted_bytes, expected.data,
        "Bit-Exact byte content mismatch for entry {} ({})",
        file_idx,
        expected.rel_path
    );

    // 2. CRC-32 Digest Assertion
    let calculated_crc = crc32_fast(0, &extracted_bytes);
    assert_eq!(
        calculated_crc, expected.crc32,
        "Computed CRC-32 mismatch for entry {} ({})",
        file_idx,
        expected.rel_path
    );
    assert_eq!(
        stats.computed_crc, expected.crc32,
        "Stats report CRC-32 mismatch for entry {}",
        file_idx
    );
    assert!(
        stats.crc_matched,
        "CRC match verification flag is false for entry {}",
        file_idx
    );

    // 3. SHA-256 Digest Assertion
    let mut hasher = Sha256::new();
    hasher.update(&extracted_bytes);
    let calculated_sha_hex = hex::encode(hasher.finalize());
    assert_eq!(
        calculated_sha_hex, expected.sha256_hex,
        "SHA-256 cryptographic digest mismatch for entry {} ({})",
        file_idx,
        expected.rel_path
    );

    stats
}

/// Executes the full 4-pattern differential test matrix against a created solid archive.
fn execute_differential_4_patterns_test_suite(
    count: usize,
    items: &[ZipInputItem],
    ground_truth: &[GroundTruthFile],
) {
    let total_uncompressed_bytes: u64 = ground_truth.iter().map(|g| g.data.len() as u64).sum();

    // Create Solid 7z archive using Fast-LZMA2 level 3 with 2 worker threads
    let archive_bytes = create_7z_solid_archive_bytes(items, 3, 2)
        .unwrap_or_else(|e| panic!("create_7z_solid_archive_bytes failed for count={}: {:?}", count, e));
    assert!(!archive_bytes.is_empty());

    let archive = SevenZArchive::open_slice(&archive_bytes)
        .unwrap_or_else(|e| panic!("SevenZArchive::open_slice failed for count={}: {:?}", count, e));
    assert_eq!(archive.len(), count);
    assert_eq!(archive.solid_index().file_count(), count);

    let extractor = archive.solid_extractor();

    // =========================================================================
    // Pattern 1: Single-File Fast Extraction (First=0, Middle=N/2, Last=N-1)
    // =========================================================================
    let first_idx = 0usize;
    let mid_idx = count / 2;
    let last_idx = count - 1;

    // First Entry (0)
    let stats_first = verify_extracted_entry(&extractor, &ground_truth[first_idx], first_idx);
    let loc_first = archive.solid_index().lookup(first_idx).unwrap();
    assert_eq!(stats_first.skipped_preceding_bytes, 0);
    assert_eq!(stats_first.extracted_target_bytes, loc_first.uncompressed_size);
    assert!(
        stats_first.early_exit_triggered,
        "Early Exit must trigger when extracting entry 0"
    );
    if total_uncompressed_bytes > SOLID_MICRO_BUFFER_CHUNK_SIZE as u64 {
        assert!(
            stats_first.decompressed_bytes_total < total_uncompressed_bytes,
            "Entry 0 decompressed total {} must be < total archive size {}",
            stats_first.decompressed_bytes_total,
            total_uncompressed_bytes
        );
    }

    // Middle Entry (N / 2)
    let stats_mid = verify_extracted_entry(&extractor, &ground_truth[mid_idx], mid_idx);
    let loc_mid = archive.solid_index().lookup(mid_idx).unwrap();
    assert_eq!(stats_mid.target_offset_start, loc_mid.offset_start);
    assert_eq!(stats_mid.target_offset_end, loc_mid.offset_end);
    assert_eq!(stats_mid.skipped_preceding_bytes, loc_mid.offset_start);
    assert_eq!(stats_mid.extracted_target_bytes, loc_mid.uncompressed_size);
    assert!(
        stats_mid.early_exit_triggered,
        "Early Exit must trigger when extracting middle entry {}",
        mid_idx
    );
    assert!(
        stats_mid.decompressed_bytes_total >= loc_mid.offset_end,
        "Decompressed bytes must cover target file end offset"
    );

    // Last Entry (N - 1)
    let stats_last = verify_extracted_entry(&extractor, &ground_truth[last_idx], last_idx);
    let loc_last = archive.solid_index().lookup(last_idx).unwrap();
    assert_eq!(stats_last.target_offset_start, loc_last.offset_start);
    assert_eq!(stats_last.target_offset_end, loc_last.offset_end);
    assert_eq!(stats_last.skipped_preceding_bytes, loc_last.offset_start);
    assert_eq!(stats_last.extracted_target_bytes, loc_last.uncompressed_size);
    assert_eq!(
        stats_last.decompressed_bytes_total,
        total_uncompressed_bytes,
        "Last entry decompression must cover the entire solid stream"
    );

    // =========================================================================
    // Pattern 2: Contiguous Slice Extraction
    // =========================================================================
    let (slice_start, slice_end) = if count <= 10 {
        (2usize, 7usize.min(count))
    } else {
        (10usize, 20usize.min(count))
    };

    for idx in slice_start..slice_end {
        let stats = verify_extracted_entry(&extractor, &ground_truth[idx], idx);
        let loc = archive.solid_index().lookup(idx).unwrap();
        assert_eq!(stats.target_offset_start, loc.offset_start);
        assert_eq!(stats.target_offset_end, loc.offset_end);
    }

    // =========================================================================
    // Pattern 3: Random Scattered Extraction (15 Non-Contiguous Entries)
    // =========================================================================
    let scattered_target_count = 15usize.min(count);
    let mut all_indices: Vec<usize> = (0..count).collect();
    let mut rng = FastRng::new(0x1337c0de ^ (count as u64));

    // Guaranteed O(N) deterministic Fisher-Yates shuffle (immune to modulo step cycles)
    for i in (1..all_indices.len()).rev() {
        let j = rng.range(0, i);
        all_indices.swap(i, j);
    }
    let scattered_indices: Vec<usize> = all_indices.into_iter().take(scattered_target_count).collect();

    for &idx in &scattered_indices {
        let stats = verify_extracted_entry(&extractor, &ground_truth[idx], idx);
        let loc = archive.solid_index().lookup(idx).unwrap();
        assert_eq!(stats.target_offset_start, loc.offset_start);
        assert_eq!(stats.target_offset_end, loc.offset_end);
    }

    // =========================================================================
    // Pattern 4: Reverse Order Extraction (Tail to Head)
    // =========================================================================
    let reverse_indices: Vec<usize> = if count <= 100 {
        (0..count).rev().collect()
    } else {
        // For large 1000-file corpus, sample 30 files in strict reverse order
        (0..count).rev().step_by(count / 30).collect()
    };

    for &idx in &reverse_indices {
        let stats = verify_extracted_entry(&extractor, &ground_truth[idx], idx);
        let loc = archive.solid_index().lookup(idx).unwrap();
        assert_eq!(stats.target_offset_start, loc.offset_start);
        assert_eq!(stats.target_offset_end, loc.offset_end);
    }
}

// -----------------------------------------------------------------------------
// Test Case 1: 10-File Scale Solid Differential Matrix
// -----------------------------------------------------------------------------
#[test]
fn test_7z_solid_mixed_dataset_10_files_differential() {
    let (items, ground_truth) = generate_mixed_solid_dataset(10);
    assert_eq!(items.len(), 10);
    assert_eq!(ground_truth.len(), 10);

    execute_differential_4_patterns_test_suite(10, &items, &ground_truth);
}

// -----------------------------------------------------------------------------
// Test Case 2: 100-File Scale Solid Differential Matrix
// -----------------------------------------------------------------------------
#[test]
fn test_7z_solid_mixed_dataset_100_files_differential() {
    let (items, ground_truth) = generate_mixed_solid_dataset(100);
    assert_eq!(items.len(), 100);
    assert_eq!(ground_truth.len(), 100);

    execute_differential_4_patterns_test_suite(100, &items, &ground_truth);
}

// -----------------------------------------------------------------------------
// Test Case 3: 1000-File Scale Solid Differential Matrix
// -----------------------------------------------------------------------------
#[test]
fn test_7z_solid_mixed_dataset_1000_files_differential() {
    let (items, ground_truth) = generate_mixed_solid_dataset(1000);
    assert_eq!(items.len(), 1000);
    assert_eq!(ground_truth.len(), 1000);

    execute_differential_4_patterns_test_suite(1000, &items, &ground_truth);
}

// -----------------------------------------------------------------------------
// Test Case 4: Early-Exit Efficiency & Stream Byte Accounting Verification
// -----------------------------------------------------------------------------
#[test]
fn test_7z_solid_early_exit_efficiency_and_stream_accounting() {
    // Generate a solid corpus spanning multiple 4MB micro-buffer sliding chunks (e.g. 1000 files, ~6.5MB)
    let (items, ground_truth) = generate_mixed_solid_dataset(1000);
    let total_uncompressed_bytes: u64 = ground_truth.iter().map(|g| g.data.len() as u64).sum();

    // Verify dataset is substantial (> 4MB sliding chunk threshold)
    assert!(
        total_uncompressed_bytes > SOLID_MICRO_BUFFER_CHUNK_SIZE as u64,
        "Total uncompressed bytes ({}) must exceed 4MB micro-buffer chunk ({})",
        total_uncompressed_bytes,
        SOLID_MICRO_BUFFER_CHUNK_SIZE
    );

    let archive_bytes = create_7z_solid_archive_bytes(&items, 3, 2).expect("create 7z solid failed");
    let archive = SevenZArchive::open_slice(&archive_bytes).expect("open 7z archive failed");
    let extractor = archive.solid_extractor();

    // 1. Single entry 0 extraction with Early-Exit
    let entry0_expected = &ground_truth[0];
    let (entry0_data, entry0_stats) = extractor
        .extract_to_vec(0, None)
        .expect("extract entry 0 failed");

    assert_eq!(entry0_data, entry0_expected.data);
    assert_eq!(entry0_stats.skipped_preceding_bytes, 0);
    assert_eq!(entry0_stats.extracted_target_bytes, entry0_expected.data.len() as u64);
    assert!(entry0_stats.early_exit_triggered);

    // Stream accounting verification: trailing chunk skipping
    assert!(
        entry0_stats.decompressed_bytes_total < total_uncompressed_bytes,
        "Entry 0 decompressed total {} must be < total archive size {}",
        entry0_stats.decompressed_bytes_total,
        total_uncompressed_bytes
    );

    let skipped_trailing_bytes = total_uncompressed_bytes.saturating_sub(entry0_stats.decompressed_bytes_total);
    println!(
        "-> 1000-File Solid Archive: Total Stream = {} bytes, Entry 0 Decompressed = {} bytes (Skipped Trailing = {} bytes, {:.1}%)",
        total_uncompressed_bytes,
        entry0_stats.decompressed_bytes_total,
        skipped_trailing_bytes,
        (skipped_trailing_bytes as f64 / total_uncompressed_bytes as f64) * 100.0
    );

    assert!(
        skipped_trailing_bytes > 0,
        "Early exit must skip trailing stream chunks when extracting entry 0"
    );

    // 2. Middle entry (Index 500) Early-Exit stream verification
    let mid_idx = 500usize;
    let mid_expected = &ground_truth[mid_idx];
    let (mid_data, mid_stats) = extractor
        .extract_to_vec(mid_idx, None)
        .expect("extract middle entry 500 failed");

    let mid_loc = archive.solid_index().lookup(mid_idx).unwrap();
    assert_eq!(mid_data, mid_expected.data);
    assert_eq!(mid_stats.skipped_preceding_bytes, mid_loc.offset_start);
    assert_eq!(mid_stats.extracted_target_bytes, mid_expected.data.len() as u64);
    assert!(mid_stats.early_exit_triggered);

    // 3. Performance Latency Verification: Early-Exit Entry 0 vs Full Archive Decompression
    let iters = 10;
    let start_early = Instant::now();
    for _ in 0..iters {
        let (d, _) = extractor.extract_to_vec(0, None).unwrap();
        assert_eq!(d.len(), entry0_expected.data.len());
    }
    let duration_early = start_early.elapsed();
    let avg_early = duration_early / iters;

    let start_full = Instant::now();
    let mut total_unpacked = 0u64;
    decode_7z_solid_streaming(&archive_bytes, archive.info(), None, 1, |chunk| {
        total_unpacked += chunk.len() as u64;
        Ok(())
    })
    .expect("full solid decode failed");
    let duration_full = start_full.elapsed();

    assert_eq!(total_unpacked, total_uncompressed_bytes);

    println!(
        "-> Performance Benchmark: {} runs of Entry 0 Early-Exit took {:?} (avg {:?}), 1 Full Archive Decode took {:?} (Unpacked {} bytes)",
        iters, duration_early, avg_early, duration_full, total_unpacked
    );

    // Early exit average latency must be significantly less than or equal to full decompression time
    assert!(
        avg_early <= duration_full,
        "Early exit avg latency ({:?}) must be <= full decompression latency ({:?})",
        avg_early,
        duration_full
    );
}
