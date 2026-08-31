// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests for Libdeflate Adler-32 and CRC-32 checksum engine.
//!
//! Validates:
//! 1. Known test vectors and varying boundary sizes (0B, 1B, 15B, 16B, 5552B, 5553B, 64KB, 1MB).
//! 2. 100% bit-for-bit equivalence against IEEE 802.3 and RFC 1950 reference implementations (`crc32fast`, `adler2`).
//! 3. Multipart arbitrary split concatenation combination (`combine_adler32`, `combine_crc32`).
//! 4. Worst-case all-0xFF overflow boundary resilience at 5552 and 5553 bytes.
//! 5. Equivalence between scalar and hardware-accelerated dispatch paths.

use ttzip_engine::codecs::libdeflate::checksum::{
    adler32_compute, adler32_scalar, adler32_update, combine_adler32, crc32_compute, crc32_slice8,
    crc32_update, combine_crc32, ADLER32_DIVISOR, ADLER32_MAX_CHUNK,
};

#[test]
fn test_constants() {
    assert_eq!(ADLER32_DIVISOR, 65521);
    assert_eq!(ADLER32_MAX_CHUNK, 5552);
}

#[test]
fn test_adler32_known_vectors() {
    // Empty buffer
    assert_eq!(adler32_compute(&[]), 1);
    assert_eq!(adler32_scalar(1, &[]), 1);
    assert_eq!(adler32_update(12345, &[]), 12345);

    // Standard RFC 1950 test vectors
    assert_eq!(adler32_compute(b"a"), 0x00620062);
    assert_eq!(adler32_compute(b"abc"), 0x024D0127);
    assert_eq!(adler32_compute(b"message digest"), 0x29750586);
    assert_eq!(adler32_compute(b"123456789"), 0x091E01DE);
    assert_eq!(adler32_compute(b"Wikipedia"), 0x11E60398);
    assert_eq!(
        adler32_compute(b"The quick brown fox jumps over the lazy dog"),
        1541148634
    );

    // Verify scalar matches hardware compute for known vectors
    for vector in [
        &b""[..],
        &b"a"[..],
        &b"abc"[..],
        &b"message digest"[..],
        &b"123456789"[..],
        &b"Wikipedia"[..],
    ] {
        let hw = adler32_compute(vector);
        let sc = adler32_scalar(1, vector);
        assert_eq!(hw, sc, "Adler32 scalar/hw mismatch on vector {:?}", vector);
    }
}

#[test]
fn test_crc32_known_vectors() {
    // Empty buffer
    assert_eq!(crc32_compute(&[]), 0);
    assert_eq!(crc32_slice8(0, &[]), 0);
    assert_eq!(crc32_update(12345, &[]), 12345);

    // Standard IEEE 802.3 test vectors
    assert_eq!(crc32_compute(b"a"), 0xE8B7BE43);
    assert_eq!(crc32_compute(b"abc"), 0x352441C2);
    assert_eq!(crc32_compute(b"message digest"), 0x20159D7F);
    assert_eq!(crc32_compute(b"123456789"), 0xCBF43926);
    assert_eq!(crc32_compute(b"The quick brown fox jumps over the lazy dog"), 0x414FA339);

    // Verify slice-by-8 matches hardware compute for known vectors
    for vector in [
        &b""[..],
        &b"a"[..],
        &b"abc"[..],
        &b"message digest"[..],
        &b"123456789"[..],
    ] {
        let hw = crc32_compute(vector);
        let sc = crc32_slice8(0, vector);
        assert_eq!(hw, sc, "CRC32 slice8/hw mismatch on vector {:?}", vector);
    }
}

#[test]
fn test_boundary_sizes_oracle_fidelity() {
    // Generate deterministic pseudo-random buffer of 1MB
    let size_1mb = 1024 * 1024;
    let mut buffer = vec![0u8; size_1mb];
    for (i, byte) in buffer.iter_mut().enumerate() {
        *byte = ((i.wrapping_mul(1664525).wrapping_add(1013904223)) >> 16) as u8;
    }

    let target_sizes = [
        0,
        1,
        15,
        16,
        5552,
        5553,
        65536,
        size_1mb,
    ];

    for &size in &target_sizes {
        let slice = &buffer[..size];

        // 1. Adler-32 validation against adler2 crate oracle
        let expected_adler = adler2::adler32_slice(slice);
        let computed_adler_hw = adler32_compute(slice);
        let computed_adler_sc = adler32_scalar(1, slice);

        assert_eq!(
            computed_adler_hw, expected_adler,
            "Adler32 HW mismatch at size {}",
            size
        );
        assert_eq!(
            computed_adler_sc, expected_adler,
            "Adler32 Scalar mismatch at size {}",
            size
        );

        // 2. CRC-32 validation against crc32fast crate oracle
        let expected_crc = crc32fast::hash(slice);
        let computed_crc_hw = crc32_compute(slice);
        let computed_crc_sc = crc32_slice8(0, slice);

        assert_eq!(
            computed_crc_hw, expected_crc,
            "CRC32 HW mismatch at size {}",
            size
        );
        assert_eq!(
            computed_crc_sc, expected_crc,
            "CRC32 Scalar mismatch at size {}",
            size
        );
    }
}

#[test]
fn test_all_0xff_worst_case_overflow_boundary() {
    // 5552 is the exact threshold where s2 reaches its maximum safe 32-bit accumulation without modulo
    for size in [5551, 5552, 5553, 5554, 11104, 11105] {
        let ff_buffer = vec![0xFFu8; size];

        let oracle_adler = adler2::adler32_slice(&ff_buffer);
        let hw_adler = adler32_compute(&ff_buffer);
        let sc_adler = adler32_scalar(1, &ff_buffer);

        assert_eq!(
            hw_adler, oracle_adler,
            "Adler32 HW overflow error on all-0xFF buffer of size {}",
            size
        );
        assert_eq!(
            sc_adler, oracle_adler,
            "Adler32 scalar overflow error on all-0xFF buffer of size {}",
            size
        );

        let oracle_crc = crc32fast::hash(&ff_buffer);
        let hw_crc = crc32_compute(&ff_buffer);
        let sc_crc = crc32_slice8(0, &ff_buffer);

        assert_eq!(
            hw_crc, oracle_crc,
            "CRC32 HW error on all-0xFF buffer of size {}",
            size
        );
        assert_eq!(
            sc_crc, oracle_crc,
            "CRC32 scalar error on all-0xFF buffer of size {}",
            size
        );
    }
}

#[test]
fn test_multipart_split_combination() {
    let mut payload = vec![0u8; 131072]; // 128 KB
    for (i, b) in payload.iter_mut().enumerate() {
        *b = ((i * 37 + 19) & 0xFF) as u8;
    }

    let full_adler = adler32_compute(&payload);
    let full_crc = crc32_compute(&payload);

    // Test multiple split positions including 0, boundary chunks, prime offsets, and end
    let split_points = [
        0, 1, 7, 15, 16, 17, 5552, 5553, 10000, 65535, 65536, 100000, 131071, 131072,
    ];

    for &split in &split_points {
        let part1 = &payload[..split];
        let part2 = &payload[split..];

        // Adler-32 combine
        let a1 = adler32_compute(part1);
        let a2 = adler32_compute(part2);
        let combined_adler = combine_adler32(a1, a2, part2.len());
        assert_eq!(
            combined_adler, full_adler,
            "combine_adler32 failed at split point {}",
            split
        );

        // CRC-32 combine
        let c1 = crc32_compute(part1);
        let c2 = crc32_compute(part2);
        let combined_crc = combine_crc32(c1, c2, part2.len());
        assert_eq!(
            combined_crc, full_crc,
            "combine_crc32 failed at split point {}",
            split
        );
    }
}

#[test]
fn test_three_way_multipart_combination() {
    let payload = b"Libdeflate ultra-fast checksum combining across three independent chunks in parallel streaming pipelines.";
    let p1 = &payload[..20];
    let p2 = &payload[20..65];
    let p3 = &payload[65..];

    let full_adler = adler32_compute(payload);
    let full_crc = crc32_compute(payload);

    // Adler 3-way
    let a1 = adler32_compute(p1);
    let a2 = adler32_compute(p2);
    let a3 = adler32_compute(p3);
    let a12 = combine_adler32(a1, a2, p2.len());
    let a123 = combine_adler32(a12, a3, p3.len());
    assert_eq!(a123, full_adler);

    // CRC 3-way
    let c1 = crc32_compute(p1);
    let c2 = crc32_compute(p2);
    let c3 = crc32_compute(p3);
    let c12 = combine_crc32(c1, c2, p2.len());
    let c123 = combine_crc32(c12, c3, p3.len());
    assert_eq!(c123, full_crc);
}

#[test]
fn test_incremental_streaming_update() {
    let data = b"Testing incremental stream updating across diverse block chunk sizes.";
    
    // Adler-32 incremental
    let mut adler_running = 1u32;
    for chunk in data.chunks(7) {
        adler_running = adler32_update(adler_running, chunk);
    }
    assert_eq!(adler_running, adler32_compute(data));

    // CRC-32 incremental
    let mut crc_running = 0u32;
    for chunk in data.chunks(7) {
        crc_running = crc32_update(crc_running, chunk);
    }
    assert_eq!(crc_running, crc32_compute(data));
}
