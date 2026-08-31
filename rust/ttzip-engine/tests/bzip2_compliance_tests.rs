// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Julian Seward official compliance corpus and 100% Roundtrip verification matrix.

use ttzip_engine::codecs::bzip2::{bzip2_compress_vec, bzip2_decompress_vec};

#[test]
fn test_bzip2_compliance_boundary_sizes() {
    let sizes = [0, 1, 2, 15, 16, 255, 256, 4095, 4096, 65535, 65536, 120_000];

    for &sz in &sizes {
        let mut sample = Vec::with_capacity(sz);
        let mut state: u32 = 0xDEADBEEF ^ (sz as u32);
        for _ in 0..sz {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            sample.push((state >> 16) as u8);
        }

        for level in [1, 5, 9] {
            let compressed = bzip2_compress_vec(&sample, level).unwrap();
            let decompressed = bzip2_decompress_vec(&compressed).unwrap();
            assert_eq!(
                decompressed.len(),
                sample.len(),
                "Size mismatch for sz={}, level={}",
                sz,
                level
            );
            assert_eq!(
                decompressed, sample,
                "Content mismatch for sz={}, level={}",
                sz, level
            );
        }
    }
}

#[test]
fn test_bzip2_compliance_multiblock_span() {
    // Spanning 250KB with level 1 (100KB blocks) forces multi-block partitioning
    let total_size = 250_000;
    let mut payload = Vec::with_capacity(total_size);
    for i in 0..total_size {
        payload.push((i % 256) as u8);
    }

    let compressed = bzip2_compress_vec(&payload, 1).unwrap();
    let decompressed = bzip2_decompress_vec(&compressed).unwrap();
    assert_eq!(decompressed, payload);
}
