// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! End-to-end fidelity and compression gain integration tests for BCJ and BCJ2 4-Stream filters.

use ttzip_engine::codecs::branch::bcj2::{decode_bcj2, encode_bcj2, Bcj2Streams};
use ttzip_engine::codecs::branch::{arm64_decode, arm64_encode, x86_decode, x86_encode};
use ttzip_engine::codecs::lzma2::fl2_compress;

fn generate_synthetic_arm64_code(len: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(len);
    let mut pc = 0u32;
    while code.len() < len {
        // Generate BL to a fixed target address (0x10004000)
        let target = 0x10004000u32;
        let rel = target.wrapping_sub(pc) >> 2;
        let bl_instr = 0x94000000 | (rel & 0x03FFFFFF);
        code.extend_from_slice(&bl_instr.to_le_bytes());
        pc = pc.wrapping_add(4);

        if code.len() < len {
            // NOP or ADD
            let add_instr = 0x910003E0u32; // mov x0, x0
            code.extend_from_slice(&add_instr.to_le_bytes());
            pc = pc.wrapping_add(4);
        }
    }
    code.truncate(len);
    code
}

fn generate_synthetic_x86_code(len: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(len);
    let mut pc = 0u32;
    while code.len() < len {
        // Generate CALL rel32 to target 0x00401000
        let target = 0x00401000u32;
        let rel = target.wrapping_sub(pc.wrapping_add(5));
        code.push(0xE8);
        code.extend_from_slice(&rel.to_le_bytes());
        pc = pc.wrapping_add(5);

        if code.len() < len {
            code.push(0x90); // NOP
            pc = pc.wrapping_add(1);
        }
    }
    code.truncate(len);
    code
}

#[test]
fn test_arm64_bcj_bit_exact_fidelity_and_compression_gain() {
    let original = generate_synthetic_arm64_code(64 * 1024);
    let mut filtered = original.clone();

    // 1. Encode
    let processed_enc = arm64_encode(&mut filtered, 0x1000);
    assert_eq!(processed_enc, original.len());
    assert_ne!(filtered, original, "ARM64 filter should normalize relative addresses");

    // 2. Decode & verify 100% bit-exact fidelity
    let mut restored = filtered.clone();
    let processed_dec = arm64_decode(&mut restored, 0x1000);
    assert_eq!(processed_dec, original.len());
    assert_eq!(restored, original, "ARM64 BCJ roundtrip must be 100% bit-exact");

    // 3. Verify LZMA2 compression gain
    let mut comp_raw = vec![0u8; original.len() * 2];
    let raw_size = fl2_compress(&original, &mut comp_raw, 3, 1).expect("compress raw");

    let mut comp_filtered = vec![0u8; filtered.len() * 2];
    let filtered_size = fl2_compress(&filtered, &mut comp_filtered, 3, 1).expect("compress filtered");

    println!("ARM64 Raw Comp Size: {} bytes, Filtered Comp Size: {} bytes", raw_size, filtered_size);
    assert!(filtered_size < raw_size, "BCJ filtering should improve compression ratio on machine code");
}

#[test]
fn test_x86_bcj_bit_exact_fidelity_and_compression_gain() {
    let original = generate_synthetic_x86_code(64 * 1024);
    let mut filtered = original.clone();

    // 1. Encode
    let processed_enc = x86_encode(&mut filtered, 0x1000);
    assert!(processed_enc >= original.len() - 4);
    assert_ne!(filtered, original, "x86 filter should normalize CALL/JMP targets");

    // 2. Decode & verify 100% bit-exact fidelity
    let mut restored = filtered.clone();
    let processed_dec = x86_decode(&mut restored, 0x1000);
    assert_eq!(processed_dec, processed_enc);
    assert_eq!(restored, original, "x86 BCJ roundtrip must be 100% bit-exact");

    // 3. Verify LZMA2 compression gain
    let mut comp_raw = vec![0u8; original.len() * 2];
    let raw_size = fl2_compress(&original, &mut comp_raw, 3, 1).expect("compress raw");

    let mut comp_filtered = vec![0u8; filtered.len() * 2];
    let filtered_size = fl2_compress(&filtered, &mut comp_filtered, 3, 1).expect("compress filtered");

    println!("x86 Raw Comp Size: {} bytes, Filtered Comp Size: {} bytes", raw_size, filtered_size);
    assert!(filtered_size < raw_size, "BCJ filtering should improve compression ratio on machine code");
}

#[test]
fn test_bcj2_4stream_bit_exact_fidelity_and_decompression() {
    let original = generate_synthetic_x86_code(128 * 1024);

    // 1. 1-In-4-Out Encode
    let streams: Bcj2Streams = encode_bcj2(&original, 0);
    assert!(!streams.main.is_empty());
    assert!(!streams.call.is_empty());

    // 2. 4-In-1-Out Decode
    let restored = decode_bcj2(
        &streams.main,
        &streams.call,
        &streams.jump,
        &streams.rc,
        0,
    )
    .expect("bcj2 decode");

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored, original, "BCJ2 4-Stream DAG roundtrip must be bit-exact");
}
