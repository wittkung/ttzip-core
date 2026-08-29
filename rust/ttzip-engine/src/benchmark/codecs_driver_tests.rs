// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit and integration test suite for MatrixCodecDriver and all codec benchmark drivers.

use super::*;

#[test]
fn test_all_13_drivers_registered_and_identified() {
    let drivers = MatrixCodecDriver::drivers();
    assert_eq!(drivers.len(), 13);

    let ids: Vec<&str> = drivers.iter().map(|d| d.algorithm_id()).collect();
    assert_eq!(
        ids,
        vec![
            "Deflate", "Zstd", "Zstd-LDM", "Zstd-Dict", "FSE", "Huff0",
            "LZMA2", "Brotli", "Bzip2", "Snappy", "LZ4", "LZFSE", "PPMd"
        ]
    );
}

#[test]
fn test_all_13_drivers_roundtrip() {
    let mut payload = Vec::with_capacity(4096);
    for _ in 0..64 {
        payload.extend_from_slice(b"TTZip 2026 High-Performance Multi-Codec Architecture Verification Payload! ");
    }

    for driver in MatrixCodecDriver::drivers() {
        let levels = driver.available_levels();
        assert!(!levels.is_empty(), "Driver {} must have levels", driver.algorithm_id());

        let test_level = levels[levels.len() / 2];
        let compressed = driver
            .bench_compress(&payload, test_level)
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
    for cfg in configs.iter().take(15) {
        let comp = MatrixCodecDriver::compress(cfg, payload)
            .unwrap_or_else(|e| panic!("Matrix compress failed for {}: {:?}", cfg.display_name, e));
        let decomp = MatrixCodecDriver::decompress(cfg, &comp, payload.len())
            .unwrap_or_else(|e| panic!("Matrix decompress failed for {}: {:?}", cfg.display_name, e));
        assert_eq!(decomp.as_slice(), payload.as_slice());
    }
}
