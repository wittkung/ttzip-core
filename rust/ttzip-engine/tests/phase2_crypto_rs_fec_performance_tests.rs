// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::time::Instant;
use ttzip_engine::crypto::rs_fec::gf8::gf8_mul_add_slice;

#[test]
fn test_gf8_neon_simd_throughput_benchmark() {
    let size = 8 * 1024 * 1024; // 8 MB
    let src = vec![0xABu8; size];
    let mut dst = vec![0u8; size];
    let coeff = 42u8;

    let iters = 50;
    let start = Instant::now();
    for _ in 0..iters {
        gf8_mul_add_slice(coeff, &src, &mut dst);
    }
    let dur = start.elapsed();
    let gb = (size as f64 * iters as f64) / (1024.0 * 1024.0 * 1024.0);
    let speed = gb / dur.as_secs_f64();
    println!("GF(2^8) NEON SIMD Throughput: {:.2} GB/s", speed);
    assert!(speed > 1.0, "Throughput must be at least 1 GB/s");
}
