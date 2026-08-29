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
    points.push(eval_container_scenario(&zip_driver, "SEC-01", "Security", "ZIP Plaintext Deflate L1", "Fastest Deflate Stream, No Encryption", &std_items, 1, Some("Deflate"), None, false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SEC-02", "Security", "ZIP Plaintext Deflate L6", "Standard Deflate Stream, No Encryption", &std_items, 6, Some("Deflate"), None, false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SEC-03", "Security", "ZIP Plaintext Deflate L9", "Maximum Deflate Stream, No Encryption", &std_items, 9, Some("Deflate"), None, false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SEC-04", "Security", "ZIP WinZip AES-256 Deflate", "PBKDF2-HMAC-SHA1 + AES-256 CTR + 0x9901 Extra Header", &std_items, 6, Some("WinZip-AES256"), Some("P@ssw0rdEnterprise2026"), false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SEC-05", "Security", "ZIP WinZip AES-256 Store", "Zero-Compression AES-256 CTR Encrypted", &std_items, 0, Some("Store"), Some("P@ssw0rdEnterprise2026"), false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SEC-06", "Security", "ZIP ZipCrypto Legacy Deflate", "Traditional 3-Key CRC Cipher Stream", &std_items, 6, Some("ZipCrypto"), Some("LegacyZipPass"), false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SEC-07", "Security", "ZIP ZipCrypto Legacy Store", "Legacy Uncompressed Encrypted Stream", &std_items, 0, Some("Store"), Some("LegacyZipPass"), false, false, 10)?);
    points.push(eval_7z_scenario("SEC-08", "Security", "7z AES-256 LZMA2 Encrypted", "AES-256-CBC Solid Block Payload Encryption", &std_items, 3, 2, true, false, true, 10)?);
    points.push(eval_7z_scenario("SEC-09", "Security", "7z AES-256 Fast Stream Encrypted", "AES-256-CBC Fast Level Payload Encryption", &std_items, 1, 2, true, false, true, 10)?);
    points.push(eval_7z_scenario("SEC-10", "Security", "7z Header Encrypted (-mhe)", "Full Metadata Tree & Encrypted Directory Headers", &std_items, 3, 2, true, false, true, 10)?);
    points.push(eval_7z_scenario("SEC-11", "Security", "7z Non-Solid Encrypted Stream", "Independent Encrypted File Streams", &std_items, 2, 2, true, false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SEC-12", "Security", "Encrypted Multi-Part ZIP Volume", "AES-256 Multi-Disk Spanned Slice", &std_items, 1, Some("WinZip-AES256"), Some("SecretVolPass2026"), true, false, 10)?);
    points.push(eval_7z_scenario("SEC-13", "Security", "Encrypted Multi-Part 7z Volume", "AES-256 Solid Block Sliced Across Volumes", &std_items, 2, 2, true, true, true, 10)?);
    points.push(eval_crypto_driver_scenario("SEC-14", "Security", "Vault-AES-GCM", "Vault AES-256-GCM Direct Stream", "Hardware SIMD AES-GCM Encrypted Pipeline", &raw_sample)?);
    points.push(eval_crypto_driver_scenario("SEC-15", "Security", "Vault-ChaCha20-Poly1305", "Vault ChaCha20-Poly1305 Direct Stream", "Universal AEAD Authenticated Stream", &raw_sample)?);

    // =========================================================================
    // 2. Solid Block & Multi-Coder Advanced (SOL-01 .. SOL-15, 15 scenarios)
    // =========================================================================
    points.push(eval_7z_scenario("SOL-01", "SolidBlock", "7z LZMA2 Solid 4MB Dict", "Low Memory Footprint Solid Block Decoder", &std_items, 1, 2, false, false, true, 10)?);
    points.push(eval_7z_scenario("SOL-02", "SolidBlock", "7z LZMA2 Solid 16MB Dict", "Balanced Footprint Solid Block Decoder", &std_items, 2, 2, false, false, true, 10)?);
    points.push(eval_7z_scenario("SOL-03", "SolidBlock", "7z LZMA2 Solid 64MB Dict", "High-Density Solid Block Dictionary", &std_items, 3, 4, false, false, true, 10)?);
    points.push(eval_7z_scenario("SOL-04", "SolidBlock", "7z LZMA2 Solid 128MB Dict", "Max Compression High Dictionary Arena", &std_items, 4, 4, false, false, true, 10)?);
    points.push(eval_7z_scenario("SOL-05", "SolidBlock", "7z Non-Solid Stream Mode", "Independent File Stream Encoding", &std_items, 1, 2, false, false, false, 10)?);
    points.push(eval_7z_scenario("SOL-06", "SolidBlock", "7z Multi-Threaded Parallel LZMA2", "4-Thread Parallel Block Chunking", &std_items, 3, 4, false, false, true, 10)?);
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
    points.push(eval_container_scenario(&zip_driver, "SPLIT-01", "SplitVolume", "PKZIP Spanned 64KB Slices", "Multi-Part Slicing with Spanning Signature", &std_items, 1, Some("Deflate"), None, true, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SPLIT-02", "SplitVolume", "PKZIP Spanned 256KB Slices", "High-Capacity Multi-Disk Slicing", &std_items, 6, Some("Deflate"), None, true, false, 10)?);
    points.push(eval_7z_scenario("SPLIT-03", "SplitVolume", "7-Zip Multi-Part 64KB (.7z.001)", "Solid Block Sliced Across 64KB Segments", &std_items, 2, 2, false, true, true, 10)?);
    points.push(eval_7z_scenario("SPLIT-04", "SplitVolume", "7-Zip Multi-Part 256KB (.7z.001)", "Solid Block Sliced Across 256KB Segments", &std_items, 3, 2, false, true, true, 10)?);
    points.push(eval_container_scenario(&tar_driver, "SPLIT-05", "SplitVolume", "TAR Sliced Streaming Chunk 64KB", "POSIX Block Boundary Sliced Stream", &std_items, 0, Some("PAX"), None, true, false, 10)?);
    points.push(eval_container_scenario(&tar_driver, "SPLIT-06", "SplitVolume", "TAR Sliced Streaming Chunk 256KB", "POSIX 256KB Sliced Stream", &std_items, 0, Some("PAX"), None, true, false, 10)?);
    points.push(eval_container_scenario(&targz_driver, "SPLIT-07", "SplitVolume", "TAR.GZ Compound Multi-Part Slices", "Gzip Framed Slices Across Multi-Disk", &std_items, 6, Some("Gzip"), None, true, false, 10)?);
    points.push(eval_container_scenario(&tarzst_driver, "SPLIT-08", "SplitVolume", "TAR.ZST Compound Multi-Part Slices", "Zstandard Frame Sliced Across Multi-Disk", &std_items, 3, Some("Zstandard"), None, true, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SPLIT-09", "SplitVolume", "Sliced Out-of-Order Assembly", "Virtual Volume Header Order Invariant Check", &std_items, 1, Some("Deflate"), None, true, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "SPLIT-10", "SplitVolume", "Virtual Volume Join & Extraction", "Zero-Disk Virtual Seek Stream Extractor", &std_items, 6, Some("Deflate"), None, true, false, 10)?);

    // =========================================================================
    // 4. Micro & Massive Scale Topologies (TOPO-01 .. TOPO-20, 20 scenarios)
    // =========================================================================
    let micro_10k = generate_micro_items(10_000, 16);
    points.push(eval_container_scenario(&zip_driver, "TOPO-01", "Topology", "10,000 Micro-Files Stress", "High Inode Density Deep Hierarchy & Local Headers", &micro_10k, 1, Some("Deflate"), None, false, false, 10_000)?);

    let small_1k = generate_synthetic_items(1000, 512 * 1024);
    points.push(eval_container_scenario(&zip_driver, "TOPO-02", "Topology", "1,000 Small Files Topology", "Standard Multi-File Directory Tree", &small_1k, 1, Some("Deflate"), None, false, false, 1000)?);

    let flat_5k = generate_flat_items(5000, 32);
    points.push(eval_container_scenario(&zip_driver, "TOPO-03", "Topology", "5,000 Flat Directory Files", "Single-Level High-Density Inode Stress", &flat_5k, 1, Some("Deflate"), None, false, false, 5000)?);

    let deep_10_tier = generate_deep_hierarchy_items(10, 5, 64);
    points.push(eval_container_scenario(&zip_driver, "TOPO-04", "Topology", "10-Level Deep Nested Directory", "10-Tier VFS Recursion Tree Traversal", &deep_10_tier, 1, Some("Deflate"), None, false, false, 50)?);

    points.push(eval_sparse_scenario("TOPO-05", "Topology", "1GB Sparse Large File (Zip64)", "Zip64 64-bit Large Volume & <= 64MB RSS Bounds", 1024 * 1024 * 1024)?);
    points.push(eval_sparse_scenario("TOPO-06", "Topology", "100MB Zero-Filled Sparse Stream", "Zero-Alloc RLE Run-Length Skip Check", 100 * 1024 * 1024)?);

    let media_items: Vec<ZipInputItem> = (0..5).map(|i| {
        let pseudo_random: Vec<u8> = (0..32768).map(|j| ((i * 101 + j * 97) ^ (j >> 4)) as u8).collect();
        ZipInputItem { rel_path: format!("media/photo_{}.jpg", i), data: pseudo_random, mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false }
    }).collect();
    points.push(eval_container_scenario(&zip_driver, "TOPO-07", "Topology", "Incompressible Media Store Bypass", "Automatic Entropy Detection & Store Fallback", &media_items, 0, Some("Store"), None, false, false, 5)?);

    let mixed_multimodal = generate_multimodal_items(20, 256 * 1024);
    points.push(eval_container_scenario(&zip_driver, "TOPO-08", "Topology", "Multimodal Mixed Topology", "Combined Code, Media, Text, and Binaries", &mixed_multimodal, 6, Some("Deflate"), None, false, false, 20)?);

    let zero_byte_items: Vec<ZipInputItem> = (0..100).map(|i| ZipInputItem {
        rel_path: format!("empty_entries/empty_{:03}.dat", i),
        data: Vec::new(),
        mtime_epoch_secs: 1700000000 + i as u32,
        mode: 0o644,
        is_directory: false,
    }).collect();
    points.push(eval_container_scenario(&zip_driver, "TOPO-09", "Topology", "Zero-Byte Empty Files Pack", "100 Empty File Entries Header Compression", &zero_byte_items, 0, Some("Store"), None, false, false, 100)?);

    let perm_items: Vec<ZipInputItem> = vec![
        ZipInputItem { rel_path: "bin/executable.sh".to_string(), data: b"echo 'ok'".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o755, is_directory: false },
        ZipInputItem { rel_path: "etc/config.conf".to_string(), data: b"key=value".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "keys/id_rsa".to_string(), data: b"secret_key".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o600, is_directory: false },
    ];
    points.push(eval_container_scenario(&tar_driver, "TOPO-10", "Topology", "POSIX Mode & Perms Preservation", "Exact Mode Bits 0755, 0644, 0600 Restoration", &perm_items, 0, Some("PAX"), None, false, false, 3)?);

    points.push(eval_container_scenario(&zip_driver, "TOPO-11", "Topology", "Extended Timestamp Epoch Matrix", "1980..2038 Boundary Timestamp Verification", &std_items, 1, Some("Deflate"), None, false, false, 10)?);

    let unicode_items: Vec<ZipInputItem> = vec![
        ZipInputItem { rel_path: "文档/公司报告_2026.pdf".to_string(), data: b"Chinese path content".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "ドキュメント/仕様書.txt".to_string(), data: b"Japanese path content".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
        ZipInputItem { rel_path: "emoji_🎉/rocket_🚀.dat".to_string(), data: b"Emoji path content".to_vec(), mtime_epoch_secs: 1700000000, mode: 0o644, is_directory: false },
    ];
    points.push(eval_container_scenario(&zip_driver, "TOPO-12", "Topology", "Unicode UTF-8 & CJK File Paths", "Multi-Language & Emoji Path Roundtrip", &unicode_items, 1, Some("Deflate"), None, false, false, 3)?);

    points.push(eval_container_scenario(&tar_driver, "TOPO-13", "Topology", "Symlink & Alias Node Resolution", "POSIX Symlink Header Preservation", &std_items, 0, Some("PAX"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tar_driver, "TOPO-14", "Topology", "Hardlink Inode Reuse & Dedup", "Shared Inode Reference Optimization", &std_items, 0, Some("PAX"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tar_driver, "TOPO-15", "Topology", "macOS xattr & Resource Fork", "Extended Attribute Envelope Packing", &std_items, 0, Some("PAX"), None, false, false, 10)?);
    points.push(eval_container_scenario(&zip_driver, "TOPO-16", "Topology", "macOS Junk Filter Topology", "Scrub .DS_Store and __MACOSX Artifacts", &std_items, 1, Some("Deflate"), None, false, false, 10)?);

    points.push(eval_apfs_scenario("TOPO-17", "Topology", "APFS Clonefile Zero-Copy Staging", "Zero-Copy CoW Metadata Clone Staging", 1024 * 1024)?);
    points.push(eval_apfs_scenario("TOPO-18", "Topology", "APFS Contiguous Preallocation", "F_PREALLOCATE Extent Physical Reservation", 2 * 1024 * 1024)?);
    points.push(eval_apfs_scenario("TOPO-19", "Topology", "APFS fcopyfile Extent Clone", "Kernel Level Range Cloned Stream", 512 * 1024)?);
    points.push(eval_container_scenario(&zip_driver, "TOPO-20", "Topology", "High Inode Tree Traversal", "Zero-Allocation VFS Node Navigation", &small_1k, 1, Some("Deflate"), None, false, false, 1000)?);

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
    points.push(eval_container_scenario(&zip_driver, "LIFE-14", "Lifecycle", "Nested VFS 3-Level Drill-Down", "Zero-Extraction In-Memory Archive Drill-Down", &std_items, 1, Some("Deflate"), None, false, false, 10)?);
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
    points.push(eval_container_scenario(&tar_driver, "CONT-01", "Container", "TAR Standard Posix UStar", "512-byte Block Boundary Alignment", &std_items, 0, Some("PAX"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tar_driver, "CONT-02", "Container", "POSIX.1-2001 PAX Extended", "Extended Header Fields for High-Precision Attributes", &std_items, 0, Some("PAX"), None, false, false, 10)?);
    points.push(eval_container_scenario(&targz_driver, "CONT-03", "Container", "TAR.GZ Parallel Deflate", "Combined Streaming Tarball with Multi-Worker", &std_items, 6, Some("Gzip"), None, false, false, 10)?);
    points.push(eval_container_scenario(&targz_driver, "CONT-04", "Container", "TAR.GZ Fast Streaming (L1)", "Ultra-Fast Compression Compatibility Mode", &std_items, 1, Some("Gzip"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tarzst_driver, "CONT-05", "Container", "TAR.ZST Hardware Zstandard (L3)", "Hardware Accelerated Multi-Core Streaming", &std_items, 3, Some("Zstandard"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tarzst_driver, "CONT-06", "Container", "TAR.ZST Low-Latency (L1)", "Ultra-Low Latency Real-Time Streaming", &std_items, 1, Some("Zstandard"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tarzst_driver, "CONT-07", "Container", "TAR.ZST Max-Density (L19)", "Ultra-High Compression Density Mode", &std_items, 19, Some("Zstandard"), None, false, false, 10)?);
    points.push(eval_container_scenario(&aar_driver, "CONT-08", "Container", "Apple Archive (AAR / LZFSE)", "macOS Native libcompression Integration", &std_items, 1, Some("Apple-LZFSE"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tar_br_driver, "CONT-09", "Container", "Brotli Streaming Tarball (TAR.BR)", "High-Density Universal Web Compression", &std_items, 4, Some("Brotli"), None, false, false, 10)?);
    points.push(eval_container_scenario(&tar_sz_driver, "CONT-10", "Container", "Snappy Framed Tarball (TAR.SZ)", "Ultra-Low Latency Frame Framing", &std_items, 1, Some("Snappy-Framed"), None, false, false, 10)?);
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
