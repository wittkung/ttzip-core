// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 100-Point Enterprise Scenario Matrix Definitions and Execution Logic.

use crate::benchmark::container_driver::*;
use crate::benchmark::scenario_driver::evaluators::*;
use crate::benchmark::scenario_driver::ScenarioBenchmarkPoint;
use crate::types::{TTZipArchiveFormat, TTZipStatus};
use crate::zip::writer::ZipInputItem;

/// Executes all 100 enterprise benchmark scenarios across 7 industrial categories.
pub fn execute_100_scenario_matrix() -> Result<Vec<ScenarioBenchmarkPoint>, TTZipStatus> {
    let mut points = Vec::with_capacity(100);

    // Standard baseline synthetic items
    let std_items = generate_synthetic_items(10, 256 * 1024);
    let raw_sample = generate_raw_bytes(128 * 1024);

    let zip_driver = ZipContainerDriver;
    let tar_driver = TarContainerDriver;
    let targz_driver = TarGzContainerDriver;
    let tarzst_driver = TarZstContainerDriver;
    let aar_driver = AarContainerDriver;
    let tar_br_driver = TarBrotliContainerDriver;
    let tar_sz_driver = TarSnappyContainerDriver;

    // =========================================================================
    // 1. Cryptographic Security & Key Derivation (SEC-01 .. SEC-15, 15 scenarios)
    // =========================================================================
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-01", category: "Security", display_name: "ZIP Plaintext Deflate L1",
        options_summary: "Fastest Deflate Stream, No Encryption", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-02", category: "Security", display_name: "ZIP Plaintext Deflate L6",
        options_summary: "Standard Deflate Stream, No Encryption", level: 6,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-03", category: "Security", display_name: "ZIP Plaintext Deflate L9",
        options_summary: "Maximum Deflate Stream, No Encryption", level: 9,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-04", category: "Security", display_name: "ZIP WinZip AES-256 Deflate",
        options_summary: "PBKDF2-HMAC-SHA1 + AES-256 CTR + 0x9901 Extra Header", level: 6,
        algorithm: Some("WinZip-AES256"), password: Some("P@ssw0rdEnterprise2026"), is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-05", category: "Security", display_name: "ZIP WinZip AES-256 Store",
        options_summary: "Zero-Compression AES-256 CTR Encrypted", level: 0,
        algorithm: Some("Store"), password: Some("P@ssw0rdEnterprise2026"), is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-06", category: "Security", display_name: "ZIP ZipCrypto Legacy Deflate",
        options_summary: "Traditional 3-Key CRC Cipher Stream", level: 6,
        algorithm: Some("ZipCrypto"), password: Some("LegacyZipPass"), is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-07", category: "Security", display_name: "ZIP ZipCrypto Legacy Store",
        options_summary: "Legacy Uncompressed Encrypted Stream", level: 0,
        algorithm: Some("Store"), password: Some("LegacyZipPass"), is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SEC-08", category: "Security", display_name: "7z AES-256 LZMA2 Encrypted",
        options_summary: "AES-256-CBC Solid Block Payload Encryption", level: 3, num_threads: 2,
        is_encrypted: true, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SEC-09", category: "Security", display_name: "7z AES-256 Fast Stream Encrypted",
        options_summary: "AES-256-CBC Fast Level Payload Encryption", level: 1, num_threads: 2,
        is_encrypted: true, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SEC-10", category: "Security", display_name: "7z Header Encrypted (-mhe)",
        options_summary: "Full Metadata Tree & Encrypted Directory Headers", level: 3, num_threads: 2,
        is_encrypted: true, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SEC-11", category: "Security", display_name: "7z Non-Solid Encrypted Stream",
        options_summary: "Independent Encrypted File Streams", level: 2, num_threads: 2,
        is_encrypted: true, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SEC-12", category: "Security", display_name: "Encrypted Multi-Part ZIP Volume",
        options_summary: "AES-256 Multi-Disk Spanned Slice", level: 1,
        algorithm: Some("WinZip-AES256"), password: Some("SecretVolPass2026"), is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SEC-13", category: "Security", display_name: "Encrypted Multi-Part 7z Volume",
        options_summary: "AES-256 Solid Block Sliced Across Volumes", level: 2, num_threads: 2,
        is_encrypted: true, is_split: true, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_crypto_driver_scenario("SEC-14", "Security", "Vault-AES-GCM", "Vault AES-256-GCM Direct Stream", "Hardware SIMD AES-GCM Encrypted Pipeline", &raw_sample)?);
    points.push(eval_crypto_driver_scenario("SEC-15", "Security", "Vault-ChaCha20-Poly1305", "Vault ChaCha20-Poly1305 Direct Stream", "Universal AEAD Authenticated Stream", &raw_sample)?);

    // =========================================================================
    // 2. Solid Block & Multi-Coder Advanced (SOL-01 .. SOL-15, 15 scenarios)
    // =========================================================================
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SOL-01", category: "SolidBlock", display_name: "7z LZMA2 Solid 4MB Dict",
        options_summary: "Low Memory Footprint Solid Block Decoder", level: 1, num_threads: 2,
        is_encrypted: false, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SOL-02", category: "SolidBlock", display_name: "7z LZMA2 Solid 16MB Dict",
        options_summary: "Balanced Footprint Solid Block Decoder", level: 2, num_threads: 2,
        is_encrypted: false, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SOL-03", category: "SolidBlock", display_name: "7z LZMA2 Solid 64MB Dict",
        options_summary: "High-Density Solid Block Dictionary", level: 3, num_threads: 4,
        is_encrypted: false, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SOL-04", category: "SolidBlock", display_name: "7z LZMA2 Solid 128MB Dict",
        options_summary: "Max Compression High Dictionary Arena", level: 4, num_threads: 4,
        is_encrypted: false, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SOL-05", category: "SolidBlock", display_name: "7z Non-Solid Stream Mode",
        options_summary: "Independent File Stream Encoding", level: 1, num_threads: 2,
        is_encrypted: false, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SOL-06", category: "SolidBlock", display_name: "7z Multi-Threaded Parallel LZMA2",
        options_summary: "4-Thread Parallel Block Chunking", level: 3, num_threads: 4,
        is_encrypted: false, is_split: false, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_zstd_advanced_scenario("SOL-07", "SolidBlock", "Zstd Standard (LDM Off)", "Default Window Zstd Parallel Stream", &std_items, false, 0)?);
    points.push(eval_zstd_advanced_scenario("SOL-08", "SolidBlock", "Zstd LDM 128MB Window", "Long Distance Matching for Redundant Blocks", &std_items, true, 20)?);
    points.push(eval_zstd_advanced_scenario("SOL-09", "SolidBlock", "Zstd LDM 1GB Window", "Sparse Redundancy Matching Across Datasets", &std_items, true, 27)?);
    points.push(eval_single_stream_scenario("SOL-10", "SolidBlock", "BROTLI", "Brotli Ultra-Fast Stream Q1", "Low-Latency Direct Byte Stream", &raw_sample, 1)?);
    points.push(eval_single_stream_scenario("SOL-11", "SolidBlock", "BROTLI", "Brotli Balanced Stream Q4", "Standard Web Package Compression", &raw_sample, 4)?);
    points.push(eval_single_stream_scenario("SOL-12", "SolidBlock", "BROTLI", "Brotli Maximum Density Q9", "High-Density Static Asset Package", &raw_sample, 9)?);
    points.push(eval_single_stream_scenario("SOL-13", "SolidBlock", "SNAPPY", "Snappy Framed Stream", "Ultra-High Throughput Frame Formatting", &raw_sample, 1)?);
    points.push(eval_single_stream_scenario("SOL-14", "SolidBlock", "LZFSE", "Apple LZFSE Stream", "Apple Silicon Native libcompression Pipeline", &raw_sample, 1)?);
    points.push(eval_single_stream_scenario("SOL-15", "SolidBlock", "BZIP2", "Bzip2 Burrows-Wheeler Block", "900KB Block High-Redundancy Archive", &raw_sample, 9)?);

    // =========================================================================
    // 3. Split-Volume & Chunk Slicing (SPLIT-01 .. SPLIT-10, 10 scenarios)
    // =========================================================================
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-01", category: "SplitVolume", display_name: "PKZIP Spanned 64KB Slices",
        options_summary: "Multi-Part Slicing with Spanning Signature", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-02", category: "SplitVolume", display_name: "PKZIP Spanned 256KB Slices",
        options_summary: "High-Capacity Multi-Disk Slicing", level: 6,
        algorithm: Some("Deflate"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SPLIT-03", category: "SplitVolume", display_name: "7-Zip Multi-Part 64KB (.7z.001)",
        options_summary: "Solid Block Sliced Across 64KB Segments", level: 2, num_threads: 2,
        is_encrypted: false, is_split: true, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_7z_scenario(&std_items, &SevenZScenarioParams {
        id: "SPLIT-04", category: "SplitVolume", display_name: "7-Zip Multi-Part 256KB (.7z.001)",
        options_summary: "Solid Block Sliced Across 256KB Segments", level: 3, num_threads: 2,
        is_encrypted: false, is_split: true, is_solid: true, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tar_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-05", category: "SplitVolume", display_name: "TAR Sliced Streaming Chunk 64KB",
        options_summary: "POSIX Block Boundary Sliced Stream", level: 0,
        algorithm: Some("PAX"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tar_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-06", category: "SplitVolume", display_name: "TAR Sliced Streaming Chunk 256KB",
        options_summary: "POSIX 256KB Sliced Stream", level: 0,
        algorithm: Some("PAX"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&targz_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-07", category: "SplitVolume", display_name: "TAR.GZ Compound Multi-Part Slices",
        options_summary: "Gzip Framed Slices Across Multi-Disk", level: 6,
        algorithm: Some("Gzip"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tarzst_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-08", category: "SplitVolume", display_name: "TAR.ZST Compound Multi-Part Slices",
        options_summary: "Zstandard Frame Sliced Across Multi-Disk", level: 3,
        algorithm: Some("Zstandard"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-09", category: "SplitVolume", display_name: "Sliced Out-of-Order Assembly",
        options_summary: "Virtual Volume Header Order Invariant Check", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "SPLIT-10", category: "SplitVolume", display_name: "Virtual Volume Join & Extraction",
        options_summary: "Zero-Disk Virtual Seek Stream Extractor", level: 6,
        algorithm: Some("Deflate"), password: None, is_split: true, is_solid: false, expected_entry_count: 10,
    })?);

    // =========================================================================
    // 4. Micro & Massive Scale Topologies (TOPO-01 .. TOPO-20, 20 scenarios)
    // =========================================================================
    let micro_10k = generate_micro_items(10_000, 16);
    points.push(eval_container_scenario(&zip_driver, &micro_10k, &ContainerScenarioParams {
        id: "TOPO-01", category: "Topology", display_name: "10,000 Micro-Files Stress",
        options_summary: "High Inode Density Deep Hierarchy & Local Headers", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 10_000,
    })?);

    let small_1k = generate_synthetic_items(1000, 512 * 1024);
    points.push(eval_container_scenario(&zip_driver, &small_1k, &ContainerScenarioParams {
        id: "TOPO-02", category: "Topology", display_name: "1,000 Small Files Topology",
        options_summary: "Standard Multi-File Directory Tree", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 1000,
    })?);

    let flat_5k = generate_flat_items(5000, 32);
    points.push(eval_container_scenario(&zip_driver, &flat_5k, &ContainerScenarioParams {
        id: "TOPO-03", category: "Topology", display_name: "5,000 Flat Directory Files",
        options_summary: "Single-Level High-Density Inode Stress", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 5000,
    })?);

    let deep_10_tier = generate_deep_hierarchy_items(10, 5, 64);
    points.push(eval_container_scenario(&zip_driver, &deep_10_tier, &ContainerScenarioParams {
        id: "TOPO-04", category: "Topology", display_name: "10-Level Deep Nested Directory",
        options_summary: "10-Tier VFS Recursion Tree Traversal", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 50,
    })?);

    points.push(eval_sparse_scenario("TOPO-05", "Topology", "1GB Sparse Large File (Zip64)", "Zip64 64-bit Large Volume & <= 64MB RSS Bounds", 1024 * 1024 * 1024)?);
    points.push(eval_sparse_scenario("TOPO-06", "Topology", "100MB Zero-Filled Sparse Stream", "Zero-Alloc RLE Run-Length Skip Check", 100 * 1024 * 1024)?);

    let media_items: Vec<ZipInputItem> = (0..5).map(|i| {
        let pseudo_random: Vec<u8> = (0..32768).map(|j| ((i * 101 + j * 97) ^ (j >> 4)) as u8).collect();
        ZipInputItem { rel_path: format!("media/photo_{}.jpg", i), data: pseudo_random, mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false }
    }).collect();
    points.push(eval_container_scenario(&zip_driver, &media_items, &ContainerScenarioParams {
        id: "TOPO-07", category: "Topology", display_name: "Incompressible Media Store Bypass",
        options_summary: "Automatic Entropy Detection & Store Fallback", level: 0,
        algorithm: Some("Store"), password: None, is_split: false, is_solid: false, expected_entry_count: 5,
    })?);

    let mixed_multimodal = generate_multimodal_items(20, 256 * 1024);
    points.push(eval_container_scenario(&zip_driver, &mixed_multimodal, &ContainerScenarioParams {
        id: "TOPO-08", category: "Topology", display_name: "Multimodal Mixed Topology",
        options_summary: "Combined Code, Media, Text, and Binaries", level: 6,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 20,
    })?);

    let zero_byte_items: Vec<ZipInputItem> = (0..100).map(|i| ZipInputItem {
        rel_path: format!("empty_entries/empty_{:03}.dat", i),
        data: Vec::new(),
        mtime_epoch_secs: 1700000000 + i as u32,
        mode: 0o644,
        is_directory: false,
    }).collect();
    points.push(eval_container_scenario(&zip_driver, &zero_byte_items, &ContainerScenarioParams {
        id: "TOPO-09", category: "Topology", display_name: "Zero-Byte Empty Files Pack",
        options_summary: "100 Empty File Entries Header Compression", level: 0,
        algorithm: Some("Store"), password: None, is_split: false, is_solid: false, expected_entry_count: 100,
    })?);

    let perm_items: Vec<ZipInputItem> = vec![
        ZipInputItem { rel_path: "bin/executable.sh".to_string(), data: b"echo 'ok'".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o755, is_directory: false },
        ZipInputItem { rel_path: "etc/config.conf".to_string(), data: b"key=value".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "keys/id_rsa".to_string(), data: b"secret_key".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o600, is_directory: false },
    ];
    points.push(eval_container_scenario(&tar_driver, &perm_items, &ContainerScenarioParams {
        id: "TOPO-10", category: "Topology", display_name: "POSIX Mode & Perms Preservation",
        options_summary: "Exact Mode Bits 0755, 0644, 0600 Restoration", level: 0,
        algorithm: Some("PAX"), password: None, is_split: false, is_solid: false, expected_entry_count: 3,
    })?);

    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "TOPO-11", category: "Topology", display_name: "Extended Timestamp Epoch Matrix",
        options_summary: "1980..2038 Boundary Timestamp Verification", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);

    let unicode_items: Vec<ZipInputItem> = vec![
        ZipInputItem { rel_path: "文档/公司报告_2026.pdf".to_string(), data: b"Chinese path content".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "ドキュメント/仕様書.txt".to_string(), data: b"Japanese path content".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "emoji_🎉/rocket_🚀.dat".to_string(), data: b"Emoji path content".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
    ];
    points.push(eval_container_scenario(&zip_driver, &unicode_items, &ContainerScenarioParams {
        id: "TOPO-12", category: "Topology", display_name: "Unicode UTF-8 & CJK File Paths",
        options_summary: "Multi-Language & Emoji Path Roundtrip", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 3,
    })?);

    points.push(eval_container_scenario(&tar_driver, &std_items, &ContainerScenarioParams {
        id: "TOPO-13", category: "Topology", display_name: "Symlink & Alias Node Resolution",
        options_summary: "POSIX Symlink Header Preservation", level: 0,
        algorithm: Some("PAX"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tar_driver, &std_items, &ContainerScenarioParams {
        id: "TOPO-14", category: "Topology", display_name: "Hardlink Inode Reuse & Dedup",
        options_summary: "Shared Inode Reference Optimization", level: 0,
        algorithm: Some("PAX"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tar_driver, &std_items, &ContainerScenarioParams {
        id: "TOPO-15", category: "Topology", display_name: "macOS xattr & Resource Fork",
        options_summary: "Extended Attribute Envelope Packing", level: 0,
        algorithm: Some("PAX"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "TOPO-16", category: "Topology", display_name: "macOS Junk Filter Topology",
        options_summary: "Scrub .DS_Store and __MACOSX Artifacts", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);

    points.push(eval_apfs_scenario("TOPO-17", "Topology", "APFS Clonefile Zero-Copy Staging", "Zero-Copy CoW Metadata Clone Staging", 1024 * 1024)?);
    points.push(eval_apfs_scenario("TOPO-18", "Topology", "APFS Contiguous Preallocation", "F_PREALLOCATE Extent Physical Reservation", 2 * 1024 * 1024)?);
    points.push(eval_apfs_scenario("TOPO-19", "Topology", "APFS fcopyfile Extent Clone", "Kernel Level Range Cloned Stream", 512 * 1024)?);
    points.push(eval_container_scenario(&zip_driver, &small_1k, &ContainerScenarioParams {
        id: "TOPO-20", category: "Topology", display_name: "High Inode Tree Traversal",
        options_summary: "Zero-Allocation VFS Node Navigation", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 1000,
    })?);

    // =========================================================================
    // 5. Lifecycle, Interactive & QuickLook (LIFE-01 .. LIFE-15, 15 scenarios)
    // =========================================================================
    points.push(eval_inplace_scenario("LIFE-01", "Lifecycle", TTZipArchiveFormat::Zip, "append", "In-Place ZIP Append Entry", "Zero-Repack Central Directory Patching", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-02", "Lifecycle", TTZipArchiveFormat::Zip, "replace", "In-Place ZIP Replace Entry", "Atomic In-Place Entry Replacement", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-03", "Lifecycle", TTZipArchiveFormat::Zip, "delete", "In-Place ZIP Delete Entry", "Atomic In-Place Entry Elimination", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-04", "Lifecycle", TTZipArchiveFormat::Tar, "append", "In-Place TAR Append File", "512-Byte Block Aligned File Append", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-05", "Lifecycle", TTZipArchiveFormat::Tar, "replace", "In-Place TAR Replace Block", "Direct In-Place Block Substitution", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-06", "Lifecycle", TTZipArchiveFormat::Tar, "delete", "In-Place TAR Delete Entry", "POSIX Tarball Compaction & Double Zero EOF", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-07", "Lifecycle", TTZipArchiveFormat::SevenZip, "append", "In-Place 7z Append Entry", "Solid Stream Tail Index Mutation", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-08", "Lifecycle", TTZipArchiveFormat::TarGz, "append", "In-Place TAR.GZ Append", "Micro-Buffered Compound Stream Mutation", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-09", "Lifecycle", TTZipArchiveFormat::TarZstd, "append", "In-Place TAR.ZST Append", "Zstandard Compound Stream Mutation", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-10", "Lifecycle", TTZipArchiveFormat::Zip, "append", "WAL Transaction Journal Commit", "Write-Ahead Log Atomic Transaction State", &std_items)?);
    points.push(eval_inplace_scenario("LIFE-11", "Lifecycle", TTZipArchiveFormat::Zip, "replace", "WAL Crash Recovery Rollback", "Deterministic State Rollback on Fault", &std_items)?);

    points.push(eval_7z_selective_jump_scenario("LIFE-12", "Lifecycle", "QuickLook Solid Stream Preview", "Early Termination Extraction (< 10ms Latency)", &std_items, 0)?);
    points.push(eval_7z_selective_jump_scenario("LIFE-13", "Lifecycle", "7z Solid Selective Jump", "Skip Preceding Streams Directly to Target Entry #9", &std_items, 9)?);
    points.push(eval_container_scenario(&zip_driver, &std_items, &ContainerScenarioParams {
        id: "LIFE-14", category: "Lifecycle", display_name: "Nested VFS 3-Level Drill-Down",
        options_summary: "Zero-Extraction In-Memory Archive Drill-Down", level: 1,
        algorithm: Some("Deflate"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_single_stream_scenario("LIFE-15", "Lifecycle", "MMAP", "Memory-Mapped Zero-Alloc Stream", "Zero-Copy Virtual Range Stream Access", &raw_sample, 1)?);

    // =========================================================================
    // 6. Corruption Self-Healing & Resilience (REPAIR-01 .. REPAIR-10, 10 scenarios)
    // =========================================================================
    points.push(eval_damaged_repair_scenario("REPAIR-01", "Resilience", "ZIP", "Damaged ZIP Truncated Central Dir", "NEON-Accelerated Local Header Scan & Reconstruction")?);
    points.push(eval_damaged_repair_scenario("REPAIR-02", "Resilience", "ZIP", "Damaged ZIP Missing EOCD Record", "Auto-Heal Missing End-of-Central-Directory Record")?);
    points.push(eval_damaged_repair_scenario("REPAIR-03", "Resilience", "ZIP", "Damaged ZIP Zero-Padded Prefix", "Auto-Align to First Valid PK Signature")?);
    points.push(eval_damaged_repair_scenario("REPAIR-04", "Resilience", "TAR", "Damaged TAR Corrupt Header Skip", "Checksum-Verified Block Salvage Pipeline")?);
    points.push(eval_damaged_repair_scenario("REPAIR-05", "Resilience", "TAR", "Damaged TAR Premature Zero Block", "Bypass False End-of-Archive Zero Segments")?);
    points.push(eval_damaged_repair_scenario("REPAIR-06", "Resilience", "ZIP", "Damaged 7z Corrupt Stream Recovery", "Boundary CRC Verification & Stream Salvage")?);
    points.push(eval_damaged_repair_scenario("REPAIR-07", "Resilience", "ZIP", "Corrupted Magic Byte Fallback", "Multi-Standard Sniffer Auto-Probe")?);
    points.push(eval_damaged_repair_scenario("REPAIR-08", "Resilience", "TAR", "Zero-Byte Truncated Container", "Graceful Handling of Truncated Payloads")?);
    points.push(eval_damaged_repair_scenario("REPAIR-09", "Resilience", "ZIP", "Bad CRC Payload Quarantine", "Quarantine Damaged Stream with Invariant Reporting")?);
    points.push(eval_damaged_repair_scenario("REPAIR-10", "Resilience", "TAR", "Truncated Deflate EOF Recovery", "Extract Valid Leading Blocks Prior to EOF Corruption")?);

    // =========================================================================
    // 7. Multi-Container Full Matrix (CONT-01 .. CONT-15, 15 scenarios)
    // =========================================================================
    points.push(eval_container_scenario(&tar_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-01", category: "Container", display_name: "TAR Standard Posix UStar",
        options_summary: "512-byte Block Boundary Alignment", level: 0,
        algorithm: Some("PAX"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tar_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-02", category: "Container", display_name: "POSIX.1-2001 PAX Extended",
        options_summary: "Extended Header Fields for High-Precision Attributes", level: 0,
        algorithm: Some("PAX"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&targz_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-03", category: "Container", display_name: "TAR.GZ Parallel Deflate",
        options_summary: "Combined Streaming Tarball with Multi-Worker", level: 6,
        algorithm: Some("Gzip"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&targz_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-04", category: "Container", display_name: "TAR.GZ Fast Streaming (L1)",
        options_summary: "Ultra-Fast Compression Compatibility Mode", level: 1,
        algorithm: Some("Gzip"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tarzst_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-05", category: "Container", display_name: "TAR.ZST Hardware Zstandard (L3)",
        options_summary: "Hardware Accelerated Multi-Core Streaming", level: 3,
        algorithm: Some("Zstandard"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tarzst_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-06", category: "Container", display_name: "TAR.ZST Low-Latency (L1)",
        options_summary: "Ultra-Low Latency Real-Time Streaming", level: 1,
        algorithm: Some("Zstandard"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tarzst_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-07", category: "Container", display_name: "TAR.ZST Max-Density (L19)",
        options_summary: "Ultra-High Compression Density Mode", level: 19,
        algorithm: Some("Zstandard"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&aar_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-08", category: "Container", display_name: "Apple Archive (AAR / LZFSE)",
        options_summary: "macOS Native libcompression Integration", level: 1,
        algorithm: Some("Apple-LZFSE"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tar_br_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-09", category: "Container", display_name: "Brotli Streaming Tarball (TAR.BR)",
        options_summary: "High-Density Universal Web Compression", level: 4,
        algorithm: Some("Brotli"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_container_scenario(&tar_sz_driver, &std_items, &ContainerScenarioParams {
        id: "CONT-10", category: "Container", display_name: "Snappy Framed Tarball (TAR.SZ)",
        options_summary: "Ultra-Low Latency Frame Framing", level: 1,
        algorithm: Some("Snappy-Framed"), password: None, is_split: false, is_solid: false, expected_entry_count: 10,
    })?);
    points.push(eval_single_stream_scenario("CONT-11", "Container", "BZIP2", "TAR.BZ2 High Redundancy Archive", "Bzip2 Streaming Pipeline Roundtrip", &raw_sample, 5)?);
    points.push(eval_single_stream_scenario("CONT-12", "Container", "GZIP", "Single Stream Gzip (.gz)", "Stand-Alone Gzip Stream Encoding", &raw_sample, 6)?);
    points.push(eval_single_stream_scenario("CONT-13", "Container", "ZSTD", "Single Stream Zstandard (.zst)", "Stand-Alone Zstd Stream Encoding", &raw_sample, 3)?);
    points.push(eval_single_stream_scenario("CONT-14", "Container", "BROTLI", "Single Stream Brotli (.br)", "Stand-Alone Brotli Stream Encoding", &raw_sample, 4)?);
    points.push(eval_single_stream_scenario("CONT-15", "Container", "LZFSE", "Single Stream LZFSE (.lzfse)", "Stand-Alone Apple LZFSE Stream Encoding", &raw_sample, 1)?);

    Ok(points)
}

fn generate_synthetic_items(count: usize, total_bytes: usize) -> Vec<ZipInputItem> {
    let per_file_bytes = (total_bytes / count.max(1)).max(128);
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let mut data = Vec::with_capacity(per_file_bytes);
        for j in 0..per_file_bytes {
            data.push(((i * 37 + j * 13) & 0xFF) as u8);
        }
        items.push(ZipInputItem {
            rel_path: format!("sub_dir/item_{:04}.bin", i),
            data,
            mtime_epoch_secs: 1700000000 + i as u32,
            mode: 0o644,
            is_directory: false,
        });
    }
    items
}

fn generate_micro_items(count: usize, bytes_per_file: usize) -> Vec<ZipInputItem> {
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let mut data = Vec::with_capacity(bytes_per_file);
        for j in 0..bytes_per_file {
            data.push(((i * 31 + j * 7) & 0xFF) as u8);
        }
        items.push(ZipInputItem {
            rel_path: format!("micro_tree/dir_{:02}/f_{:05}.txt", i / 100, i),
            data,
            mtime_epoch_secs: 1700000000 + (i % 1000) as u32,
            mode: 0o644,
            is_directory: false,
        });
    }
    items
}

fn generate_flat_items(count: usize, bytes_per_file: usize) -> Vec<ZipInputItem> {
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let mut data = Vec::with_capacity(bytes_per_file);
        for j in 0..bytes_per_file {
            data.push(((i * 17 + j * 11) & 0xFF) as u8);
        }
        items.push(ZipInputItem {
            rel_path: format!("flat_root_item_{:05}.dat", i),
            data,
            mtime_epoch_secs: 1700000000 + (i % 1000) as u32,
            mode: 0o644,
            is_directory: false,
        });
    }
    items
}

fn generate_deep_hierarchy_items(depth: usize, files_per_level: usize, file_bytes: usize) -> Vec<ZipInputItem> {
    let mut items = Vec::new();
    let mut path_prefix = String::new();
    for d in 0..depth {
        path_prefix.push_str(&format!("level_{:02}/", d));
        for f in 0..files_per_level {
            let data = vec![((d * 19 + f * 7) & 0xFF) as u8; file_bytes];
            items.push(ZipInputItem {
                rel_path: format!("{}node_{:02}.dat", path_prefix, f),
                data,
                mtime_epoch_secs: 1700000000 + (d * 10 + f) as u32,
                mode: 0o644,
                is_directory: false,
            });
        }
    }
    items
}

fn generate_multimodal_items(count: usize, total_bytes: usize) -> Vec<ZipInputItem> {
    let per_file = (total_bytes / count.max(1)).max(128);
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let mut data = Vec::with_capacity(per_file);
        let ext = match i % 4 {
            0 => "c",
            1 => "json",
            2 => "jpg",
            _ => "bin",
        };
        for j in 0..per_file {
            if ext == "c" || ext == "json" {
                data.push(b'a' + ((i + j) % 26) as u8);
            } else {
                data.push(((i * 53 + j * 29) & 0xFF) as u8);
            }
        }
        items.push(ZipInputItem {
            rel_path: format!("multimodal/sample_{:03}.{}", i, ext),
            data,
            mtime_epoch_secs: 1700000000 + i as u32,
            mode: 0o644,
            is_directory: false,
        });
    }
    items
}

fn generate_raw_bytes(size: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(size);
    for i in 0..size {
        buf.push(((i * 47 + (i >> 3)) & 0xFF) as u8);
    }
    buf
}
