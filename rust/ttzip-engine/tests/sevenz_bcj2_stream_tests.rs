// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and stress tests for Bcj2StreamArbitrator (Task 22.6).
//!
//! Tests 4-Stream lock-free micro-buffered arbitration across various scenarios:
//! 1. Single instruction passthrough without branch opcodes.
//! 2. Pure CALL (0xE8) instruction sequences.
//! 3. Pure JUMP (0xE9) instruction sequences.
//! 4. Alternating real branches and false branches (literal 0xE8/0xE9 with RC bit = 0).
//! 5. Extreme micro-buffer jitter (1-byte stepped reading) verifying monotonic progress and zero-deadlock.
//! 6. Large executable bytecode files (1MB to 16MB) verifying 100% bit-exact deterministic reconstruction.
//! 7. Output boundary clamping with uncompressed_limit.
//! 8. Premature stream truncation error detection.

use std::io::{self, Cursor, Read};
use ttzip_engine::codecs::branch::bcj2::stream::Bcj2RangeDecoder;
use ttzip_engine::codecs::branch::bcj2::{
    encode_bcj2, Bcj2StreamArbitrator, Bcj2Streams,
};

/// Generates synthetic realistic x86/x64 machine code with mixed instructions and branches.
fn generate_synthetic_x86_code(len: usize, base_ip: u32) -> Vec<u8> {
    let mut code = Vec::with_capacity(len);
    let mut pc = base_ip;
    let mut step = 0usize;

    while code.len() < len {
        step += 1;
        match step % 7 {
            0 => {
                // Function prologue: push rbp; mov rbp, rsp; sub rsp, 0x20
                let prologue = [0x55, 0x48, 0x89, 0xE5, 0x48, 0x83, 0xEC, 0x20];
                code.extend_from_slice(&prologue);
                pc = pc.wrapping_add(prologue.len() as u32);
            }
            1 => {
                // CALL rel32
                let target = base_ip.wrapping_add((step * 512) as u32);
                let rel = target.wrapping_sub(pc.wrapping_add(5));
                code.push(0xE8);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            2 => {
                // ALU instructions and NOPs
                let alu = [0x48, 0x8B, 0x05, 0x10, 0x00, 0x00, 0x00, 0x90, 0x90, 0x90];
                code.extend_from_slice(&alu);
                pc = pc.wrapping_add(alu.len() as u32);
            }
            3 => {
                // JMP rel32
                let target = base_ip.wrapping_add(0x80);
                let rel = target.wrapping_sub(pc.wrapping_add(5));
                code.push(0xE9);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            4 => {
                // Multi-byte NOP (0x0F 0x1F ...)
                let nop5 = [0x0F, 0x1F, 0x44, 0x00, 0x00];
                code.extend_from_slice(&nop5);
                pc = pc.wrapping_add(nop5.len() as u32);
            }
            5 => {
                // Consecutive CALL instructions
                let target1 = base_ip.wrapping_add(0x1000);
                let rel1 = target1.wrapping_sub(pc.wrapping_add(5));
                code.push(0xE8);
                code.extend_from_slice(&rel1.to_le_bytes());
                pc = pc.wrapping_add(5);

                let target2 = base_ip.wrapping_add(0x2000);
                let rel2 = target2.wrapping_sub(pc.wrapping_add(5));
                code.push(0xE8);
                code.extend_from_slice(&rel2.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            _ => {
                // Function epilogue: leave; ret
                let epilogue = [0xC9, 0xC3];
                code.extend_from_slice(&epilogue);
                pc = pc.wrapping_add(epilogue.len() as u32);
            }
        }
    }

    code.truncate(len);
    code
}

/// Throttled reader that yields at most `max_chunk` bytes per `read` call.
struct ThrottledReader<R> {
    inner: R,
    max_chunk: usize,
}

impl<R> ThrottledReader<R> {
    fn new(inner: R, max_chunk: usize) -> Self {
        Self { inner, max_chunk }
    }
}

impl<R: Read> Read for ThrottledReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let to_read = buf.len().min(self.max_chunk);
        self.inner.read(&mut buf[..to_read])
    }
}

// ---------------------------------------------------------------------------
// 1. Single Instruction Passthrough (No Branch Opcodes)
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_single_instruction_passthrough() {
    let original = vec![
        0x55, 0x48, 0x89, 0xE5, 0x48, 0x83, 0xEC, 0x10,
        0x89, 0x7D, 0xFC, 0x8B, 0x45, 0xFC, 0x83, 0xC0,
        0x01, 0x5D, 0xC3, 0x90, 0x90, 0x90, 0xCC, 0xCC,
    ];

    let streams: Bcj2Streams = encode_bcj2(&original, 0x1000);
    assert!(streams.call.is_empty());
    assert!(streams.jump.is_empty());

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        0x1000,
    );

    let mut restored = Vec::new();
    arbitrator.read_to_end(&mut restored).expect("read_to_end failed");

    assert_eq!(restored, original);
    assert_eq!(arbitrator.produced_bytes(), original.len() as u64);
    assert_eq!(arbitrator.current_ip(), 0x1000 + original.len() as u32);
}

// ---------------------------------------------------------------------------
// 2. Pure CALL Instruction Sequences
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_pure_call_sequence() {
    let num_calls = 500;
    let mut original = Vec::with_capacity(num_calls * 5);
    let base_ip = 0x0040_0000u32;
    let mut pc = base_ip;

    for i in 0..num_calls {
        let target = base_ip.wrapping_add((i * 128) as u32);
        let rel = target.wrapping_sub(pc.wrapping_add(5));
        original.push(0xE8);
        original.extend_from_slice(&rel.to_le_bytes());
        pc = pc.wrapping_add(5);
    }

    let streams = encode_bcj2(&original, base_ip);
    assert_eq!(streams.call.len(), num_calls * 4);
    assert!(streams.jump.is_empty());
    assert_eq!(streams.main.len(), num_calls);

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        base_ip,
    );

    let mut restored = Vec::new();
    arbitrator.read_to_end(&mut restored).expect("decode pure CALLs");

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored, original);
    assert_eq!(arbitrator.produced_bytes(), (num_calls * 5) as u64);
}

// ---------------------------------------------------------------------------
// 3. Pure JUMP Instruction Sequences
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_pure_jump_sequence() {
    let num_jumps = 500;
    let mut original = Vec::with_capacity(num_jumps * 5);
    let base_ip = 0x0080_0000u32;
    let mut pc = base_ip;

    for i in 0..num_jumps {
        let target = base_ip.wrapping_add((i * 64) as u32);
        let rel = target.wrapping_sub(pc.wrapping_add(5));
        original.push(0xE9);
        original.extend_from_slice(&rel.to_le_bytes());
        pc = pc.wrapping_add(5);
    }

    let streams = encode_bcj2(&original, base_ip);
    assert_eq!(streams.jump.len(), num_jumps * 4);
    assert!(streams.call.is_empty());
    assert_eq!(streams.main.len(), num_jumps);

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        base_ip,
    );

    let mut restored = Vec::new();
    arbitrator.read_to_end(&mut restored).expect("decode pure JMPs");

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored, original);
    assert_eq!(arbitrator.produced_bytes(), (num_jumps * 5) as u64);
}

// ---------------------------------------------------------------------------
// 4. Alternating Real Branches and False Branches
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_alternating_real_and_false_branches() {
    // Construct bytecode with trailing bytes less than 4 after 0xE8/0xE9 (false branch candidates)
    let base_ip = 0x1000u32;
    let mut original = Vec::new();

    // 1. Real CALL
    original.push(0xE8);
    let rel_call = 0x2000u32.wrapping_sub(base_ip + 5);
    original.extend_from_slice(&rel_call.to_le_bytes());

    // 2. Real JMP
    original.push(0xE9);
    let rel_jmp = 0x3000u32.wrapping_sub(base_ip + 10);
    original.extend_from_slice(&rel_jmp.to_le_bytes());

    // 3. False CALL (0xE8 near end of stream without full 4 bytes following)
    original.push(0x90);
    original.push(0xE8);
    original.push(0x01); // Only 1 byte following

    let streams = encode_bcj2(&original, base_ip);

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        base_ip,
    );

    let mut restored = Vec::new();
    arbitrator.read_to_end(&mut restored).expect("decode mixed real/false branches");

    assert_eq!(restored, original);
}

// ---------------------------------------------------------------------------
// 5. Extreme Micro-Buffer Jitter (1-Byte Stepped Reading) & Zero Deadlock
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_single_byte_jitter_zero_deadlock() {
    let original = generate_synthetic_x86_code(16 * 1024, 0x2000);
    let streams = encode_bcj2(&original, 0x2000);

    // Wrap each stream in a 1-byte throttled reader to simulate maximum stream stall jitter
    let main_r = ThrottledReader::new(Cursor::new(streams.main), 1);
    let call_r = ThrottledReader::new(Cursor::new(streams.call), 1);
    let jump_r = ThrottledReader::new(Cursor::new(streams.jump), 1);
    let rc_r = ThrottledReader::new(Cursor::new(streams.rc), 1);

    let mut arbitrator = Bcj2StreamArbitrator::new(main_r, call_r, jump_r, rc_r, 0x2000);

    let mut restored = Vec::with_capacity(original.len());
    let mut single_byte_buf = [0u8; 1];

    loop {
        let n = arbitrator
            .read(&mut single_byte_buf)
            .expect("1-byte stepped read should succeed");
        if n == 0 {
            break;
        }
        restored.push(single_byte_buf[0]);
    }

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored, original, "1-byte stepped read must be 100% bit-exact");
    assert_eq!(arbitrator.produced_bytes(), original.len() as u64);
}

// ---------------------------------------------------------------------------
// 6. Large Executable Bytecode (1MB, 4MB, 16MB) Bit-Exact Fidelity
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_large_payload_1mb() {
    let original = generate_synthetic_x86_code(1024 * 1024, 0x0040_0000);
    let streams = encode_bcj2(&original, 0x0040_0000);

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        0x0040_0000,
    );

    let mut restored = Vec::with_capacity(original.len());
    let mut chunk = vec![0u8; 64 * 1024];

    loop {
        let n = arbitrator.read(&mut chunk).expect("decode 1MB stream");
        if n == 0 {
            break;
        }
        restored.extend_from_slice(&chunk[..n]);
    }

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored, original, "1MB payload must match bit-exactly");
}

#[test]
fn test_bcj2_stream_arbitrator_large_payload_4mb() {
    let original = generate_synthetic_x86_code(4 * 1024 * 1024, 0x0010_0000);
    let streams = encode_bcj2(&original, 0x0010_0000);

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        0x0010_0000,
    );

    let mut restored = Vec::with_capacity(original.len());
    let mut chunk = vec![0u8; 128 * 1024];

    loop {
        let n = arbitrator.read(&mut chunk).expect("decode 4MB stream");
        if n == 0 {
            break;
        }
        restored.extend_from_slice(&chunk[..n]);
    }

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored, original, "4MB payload must match bit-exactly");
}

#[test]
fn test_bcj2_stream_arbitrator_large_payload_16mb() {
    let original = generate_synthetic_x86_code(16 * 1024 * 1024, 0x0020_0000);
    let streams = encode_bcj2(&original, 0x0020_0000);

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        0x0020_0000,
    );

    let mut restored = Vec::with_capacity(original.len());
    let mut chunk = vec![0u8; 256 * 1024];

    loop {
        let n = arbitrator.read(&mut chunk).expect("decode 16MB stream");
        if n == 0 {
            break;
        }
        restored.extend_from_slice(&chunk[..n]);
    }

    assert_eq!(restored.len(), original.len());
    assert_eq!(restored, original, "16MB payload must match bit-exactly");
}

// ---------------------------------------------------------------------------
// 7. Uncompressed Limit Enforcement
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_uncompressed_limit_clamping() {
    let original = generate_synthetic_x86_code(10000, 0x1000);
    let streams = encode_bcj2(&original, 0x1000);

    let limit = 2500u64;
    let mut arbitrator = Bcj2StreamArbitrator::with_limit(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        0x1000,
        limit,
    );

    let mut restored = Vec::new();
    arbitrator.read_to_end(&mut restored).expect("read with limit");

    assert_eq!(restored.len(), limit as usize);
    assert_eq!(restored, &original[..limit as usize]);
    assert_eq!(arbitrator.produced_bytes(), limit);
}

// ---------------------------------------------------------------------------
// 8. Error Handling for Premature Stream EOF
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_truncated_call_stream_detected() {
    let original = vec![0xE8, 0x10, 0x00, 0x00, 0x00];
    let mut streams = encode_bcj2(&original, 0);
    // Truncate call stream
    streams.call.clear();

    let mut arbitrator = Bcj2StreamArbitrator::new(
        Cursor::new(streams.main),
        Cursor::new(streams.call),
        Cursor::new(streams.jump),
        Cursor::new(streams.rc),
        0,
    );

    let mut out = Vec::new();
    let res = arbitrator.read_to_end(&mut out);
    assert!(res.is_err(), "Truncated CALL stream must yield an error");
    assert_eq!(res.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
}

// ---------------------------------------------------------------------------
// 9. Isolated Bcj2RangeDecoder Streaming Roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_range_decoder_isolated_streaming_roundtrip() {
    use ttzip_engine::codecs::branch::bcj2::{RangeEncoder, NUM_BCJ2_PROBS, PROB_INIT_VAL};

    let test_bits = vec![
        (0usize, 1u32),
        (0, 0),
        (1, 1),
        (256, 1),
        (256, 0),
        (128, 1),
        (128, 1),
        (257, 0),
    ];

    let mut encoder = RangeEncoder::new();
    let mut enc_probs = [PROB_INIT_VAL; NUM_BCJ2_PROBS];
    let mut encoded_stream = Vec::new();

    for &(ctx, bit) in &test_bits {
        encoder.encode_bit(&mut enc_probs[ctx], bit, &mut encoded_stream);
    }
    encoder.finish(&mut encoded_stream);

    let mut decoder = Bcj2RangeDecoder::new(Cursor::new(encoded_stream));
    for &(ctx, expected_bit) in &test_bits {
        let decoded_bit = decoder.decode_bit(ctx).expect("decode_bit failed");
        assert_eq!(decoded_bit, expected_bit, "Bit mismatch at ctx {}", ctx);
    }
}
