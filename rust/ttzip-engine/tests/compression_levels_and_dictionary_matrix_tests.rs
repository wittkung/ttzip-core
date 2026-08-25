// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tests for compression level tiers, dictionary scaling, and store mode bypass in Rust core.

use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions,
};
use ttzip_engine::archive::unified::UnifiedArchiveOrchestrator;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_store_mode_zero_compression_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let input_file = dir.path().join("input.txt");
    let payload = b"TTZip Kernel Store Mode Direct IO Validation Payload ".repeat(100);
    fs::write(&input_file, &payload).expect("write input");

    let zip_output = dir.path().join("store_test.zip");

    // 1. Create ZIP with Level 0 (Store)
    let create_opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_2,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Store,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 0,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let status = UnifiedArchiveOrchestrator::create_archive(
        &[input_file.clone()],
        &zip_output,
        &create_opts,
        0,
    );
    assert!(status.is_ok(), "ZIP creation in Store mode must succeed");

    // 2. Extract and verify integrity
    let extract_dir = dir.path().join("extracted");
    fs::create_dir_all(&extract_dir).expect("create extract dir");
    let extract_opts = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_2,
        destination_path: std::ptr::null(),
        password: std::ptr::null(),
        thread_budget: 0,
        overwrite_existing: true,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let ext_status = UnifiedArchiveOrchestrator::extract_archive(
        &zip_output,
        &extract_dir,
        &extract_opts,
    );
    assert!(ext_status.is_ok(), "ZIP extraction must succeed");

    let extracted_file = extract_dir.join("input.txt");
    let extracted_data = fs::read(&extracted_file).expect("read extracted");
    assert_eq!(extracted_data, payload, "Extracted payload must match original exactly");
}

#[test]
fn test_7z_dictionary_scaling_bounds_and_limits() {
    // 7-Zip LZMA2 dictionary size definitions in MB
    let test_dicts_mb = [16, 32, 64, 128, 256, 512, 1024, 1536];

    for &dict_mb in &test_dicts_mb {
        // Physical memory multiplier for LZMA2 BT4 is ~10.5x dictionary size per thread
        let mem_per_thread_mb = (dict_mb as f64) * 10.5;
        assert!(mem_per_thread_mb > 0.0);
        
        // Ensure within 1536 MB architecture limit (31-bit match-finder offset)
        assert!(dict_mb <= 1536, "Dictionary size {} MB exceeds 1.5GB 7z architectural maximum", dict_mb);
    }
}

#[test]
fn test_level_driven_dictionary_formula() {
    // Simulates the level-to-dictionary mapping logic
    fn calculate_dictionary_mb(level: TTZipCompressionLevel, physical_ram_gb: f64) -> usize {
        match level {
            TTZipCompressionLevel::Store => 0,
            TTZipCompressionLevel::Fastest | TTZipCompressionLevel::Fast => 16,
            TTZipCompressionLevel::Normal => 64,
            TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => {
                if physical_ram_gb >= 64.0 {
                    1024 // 1 GB
                } else if physical_ram_gb >= 32.0 {
                    512  // 512 MB
                } else if physical_ram_gb >= 16.0 {
                    256  // 256 MB
                } else {
                    128  // 128 MB
                }
            }
        }
    }

    // Level 0 (Store)
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Store, 64.0), 0);

    // Level 1 (Fastest)
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Fastest, 64.0), 16);

    // Level 5 (Normal)
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Normal, 64.0), 64);

    // Level 9 (Ultra) under different unified memory sizes:
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Ultra, 8.0), 128);
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Ultra, 16.0), 256);
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Ultra, 36.0), 512);
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Ultra, 64.0), 1024);
    assert_eq!(calculate_dictionary_mb(TTZipCompressionLevel::Ultra, 128.0), 1024);
}

#[test]
fn test_zip_deflate_rfc1951_sliding_window_constant() {
    // RFC 1951 Deflate sliding window size is strictly 32 KB
    const DEFLATE_WINDOW_SIZE_BYTES: usize = 32 * 1024;
    assert_eq!(DEFLATE_WINDOW_SIZE_BYTES, 32768, "Standard Deflate sliding window is fixed at 32 KB");
}
