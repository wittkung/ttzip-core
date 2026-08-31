// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration and property tests for BCJ2 4-Stream Zero-Deadlock Stream Arbitrator.

use std::io::{self, Read};
use ttzip_engine::codecs::branch::bcj2::{encode_bcj2, Bcj2Streams};
use ttzip_engine::codecs::branch::bcj2_stream::{
    decode_bcj2_stream, Bcj2ArbitratorStatus, Bcj2StreamArbitrator, Bcj2StreamReader,
    BCJ2_STREAM_BUFFER_SIZE,
};

/// Generates a realistic synthetic x86/x64 executable bytecode stream with mixed CALLs, JMPs,
/// conditional branches, NOP padding, and non-branch opcode literals.
fn generate_rich_x86_bytecode(len: usize, base_ip: u64) -> Vec<u8> {
    let mut code = Vec::with_capacity(len);
    let mut pc = base_ip;
    let mut step = 0usize;

    while code.len() < len {
        step += 1;
        match step % 7 {
            0 => {
                // Standard function prologue: PUSH RBP; MOV RBP, RSP; SUB RSP, 0x20
                let prologue = [0x55, 0x48, 0x89, 0xE5, 0x48, 0x83, 0xEC, 0x20];
                code.extend_from_slice(&prologue);
                pc = pc.wrapping_add(prologue.len() as u64);
            }
            1 => {
                // Forward CALL rel32
                let target = base_ip.wrapping_add((step * 1024) as u64);
                let rel = target.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE8);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            2 => {
                // ALU instructions & NOPs
                let instrs = [0x48, 0x8B, 0x05, 0x10, 0x00, 0x00, 0x00, 0x90, 0x90, 0x90];
                code.extend_from_slice(&instrs);
                pc = pc.wrapping_add(instrs.len() as u64);
            }
            3 => {
                // Backward JMP rel32
                let target = base_ip.wrapping_add(0x100);
                let rel = target.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE9);
                code.extend_from_slice(&rel.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            4 => {
                // 0x0F prefix followed by opcode or literal
                let instrs = [0x0F, 0x1F, 0x44, 0x00, 0x00]; // 5-byte NOP
                code.extend_from_slice(&instrs);
                pc = pc.wrapping_add(instrs.len() as u64);
            }
            5 => {
                // Immediate consecutive CALL instructions
                let target1 = base_ip.wrapping_add(0x2000);
                let rel1 = target1.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE8);
                code.extend_from_slice(&rel1.to_le_bytes());
                pc = pc.wrapping_add(5);

                let target2 = base_ip.wrapping_add(0x3000);
                let rel2 = target2.wrapping_sub(pc.wrapping_add(5)) as u32;
                code.push(0xE8);
                code.extend_from_slice(&rel2.to_le_bytes());
                pc = pc.wrapping_add(5);
            }
            _ => {
                // Function epilogue: LEAVE; RET
                let epilogue = [0xC9, 0xC3];
                code.extend_from_slice(&epilogue);
                pc = pc.wrapping_add(epilogue.len() as u64);
            }
        }
    }
    code.truncate(len);
    code
}

/// A reader that yields bytes in strictly throttled, small, non-uniform chunks
/// to simulate asynchronous stream stalls and stress-test demand-driven pull arbitration.
struct StaggeredChunkReader<'a> {
    data: &'a [u8],
    pos: usize,
    chunk_sizes: Vec<usize>,
    chunk_idx: usize,
}

impl<'a> StaggeredChunkReader<'a> {
    fn new(data: &'a [u8], chunk_sizes: Vec<usize>) -> Self {
        Self {
            data,
            pos: 0,
            chunk_sizes,
            chunk_idx: 0,
        }
    }
}

impl<'a> Read for StaggeredChunkReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.data.len() || buf.is_empty() {
            return Ok(0);
        }

        let max_chunk = if self.chunk_sizes.is_empty() {
            1
        } else {
            let sz = self.chunk_sizes[self.chunk_idx % self.chunk_sizes.len()];
            self.chunk_idx += 1;
            sz
        };

        let available = self.data.len() - self.pos;
        let to_read = available.min(buf.len()).min(max_chunk);

        buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
}

// ---------------------------------------------------------------------------
// 1. Bit-Exact Fidelity & Roundtrip Integration Tests
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_bit_exact_fidelity_various_ips() {
    let base_ips: &[u64] = &[
        0x0,
        0x1000,
        0x0040_0000,
        0x0001_8000_0000,
        0x7FFF_FFFF_0000_0000,
    ];

    for &base_ip in base_ips {
        let original = generate_rich_x86_bytecode(128 * 1024, base_ip);
        let streams: Bcj2Streams = encode_bcj2(&original, base_ip as u32);

        // Decompress via Bcj2StreamReader
        let mut restored = Vec::new();
        let bytes_decompressed = decode_bcj2_stream(
            &streams.main[..],
            &streams.call[..],
            &streams.jump[..],
            &streams.rc[..],
            &mut restored,
            base_ip,
        )
        .expect("Streaming BCJ2 decode failed");

        assert_eq!(bytes_decompressed, original.len() as u64);
        assert_eq!(restored.len(), original.len());
        assert_eq!(
            restored, original,
            "Bit-exact mismatch at base IP {:#X}",
            base_ip
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Arbitrary Chunk Slice Consistency (1B, 7B, 13B, 64B, 64KB)
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_stream_arbitrator_arbitrary_chunk_slices() {
    let original = generate_rich_x86_bytecode(32 * 1024, 0x4000);
    let streams = encode_bcj2(&original, 0x4000);

    let test_slice_sizes = [1usize, 3, 7, 13, 31, 64, 256, 1024, BCJ2_STREAM_BUFFER_SIZE];

    for &slice_sz in &test_slice_sizes {
        let mut arb = Bcj2StreamArbitrator::new(0x4000);

        let mut main_remaining = &streams.main[..];
        let mut call_remaining = &streams.call[..];
        let mut jump_remaining = &streams.jump[..];
        let mut rc_remaining = &streams.rc[..];

        let mut restored = Vec::with_capacity(original.len());
        let mut out_buf = vec![0u8; slice_sz];

        loop {
            // Provide slices limited to `slice_sz`
            let main_chunk_len = main_remaining.len().min(slice_sz);
            let call_chunk_len = call_remaining.len().min(slice_sz);
            let jump_chunk_len = jump_remaining.len().min(slice_sz);
            let rc_chunk_len = rc_remaining.len().min(slice_sz);

            let mut cur_main = &main_remaining[..main_chunk_len];
            let mut cur_call = &call_remaining[..call_chunk_len];
            let mut cur_jump = &jump_remaining[..jump_chunk_len];
            let mut cur_rc = &rc_remaining[..rc_chunk_len];

            let is_main_eof = main_remaining.is_empty();
            let mut out_slice = &mut out_buf[..];

            let status = arb
                .process(
                    &mut cur_main,
                    &mut cur_call,
                    &mut cur_jump,
                    &mut cur_rc,
                    &mut out_slice,
                    is_main_eof,
                )
                .expect("Arbitrator step error");

            let main_used = main_chunk_len - cur_main.len();
            let call_used = call_chunk_len - cur_call.len();
            let jump_used = jump_chunk_len - cur_jump.len();
            let rc_used = rc_chunk_len - cur_rc.len();
            let out_produced = slice_sz - out_slice.len();

            main_remaining = &main_remaining[main_used..];
            call_remaining = &call_remaining[call_used..];
            jump_remaining = &jump_remaining[jump_used..];
            rc_remaining = &rc_remaining[rc_used..];

            restored.extend_from_slice(&out_buf[..out_produced]);

            match status {
                Bcj2ArbitratorStatus::Finished => break,
                Bcj2ArbitratorStatus::NeedsMoreInput(_) | Bcj2ArbitratorStatus::NeedsMoreOutput => {
                    continue;
                }
            }
        }

        assert_eq!(
            restored.len(),
            original.len(),
            "Length mismatch at slice size {}",
            slice_sz
        );
        assert_eq!(
            restored, original,
            "Content mismatch at slice size {}",
            slice_sz
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Asynchronous Staggered Depletion & Zero-Deadlock Verification
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_asynchronous_staggered_zero_deadlock() {
    let original = generate_rich_x86_bytecode(64 * 1024, 0x1000);
    let streams = encode_bcj2(&original, 0x1000);

    // Each stream delivers data with wildly different, prime-patterned chunk sizes
    let main_reader = StaggeredChunkReader::new(&streams.main, vec![1, 2, 7, 3, 11]);
    let call_reader = StaggeredChunkReader::new(&streams.call, vec![4, 1, 3]);
    let jump_reader = StaggeredChunkReader::new(&streams.jump, vec![1, 4, 2, 5]);
    let rc_reader = StaggeredChunkReader::new(&streams.rc, vec![5, 1, 1, 2, 3]);

    let mut stream_reader =
        Bcj2StreamReader::new(main_reader, call_reader, jump_reader, rc_reader, 0x1000);

    let mut decompressed = Vec::with_capacity(original.len());
    let mut small_read_buf = [0u8; 17]; // Small 17-byte read buffer to force maximum IO pump cycles

    loop {
        let n = stream_reader
            .read(&mut small_read_buf)
            .expect("Staggered read should not fail");
        if n == 0 {
            break;
        }
        decompressed.extend_from_slice(&small_read_buf[..n]);
    }

    assert_eq!(decompressed.len(), original.len());
    assert_eq!(
        decompressed, original,
        "Asynchronous staggered supply must produce bit-exact output"
    );
}

// ---------------------------------------------------------------------------
// 4. Empty Streams & Edge Case Handlings
// ---------------------------------------------------------------------------

#[test]
fn test_bcj2_empty_code_stream() {
    let empty_code = Vec::<u8>::new();
    let streams = encode_bcj2(&empty_code, 0);

    let mut out = Vec::new();
    let bytes = decode_bcj2_stream(
        &streams.main[..],
        &streams.call[..],
        &streams.jump[..],
        &streams.rc[..],
        &mut out,
        0,
    )
    .expect("decode empty stream");

    assert_eq!(bytes, 0);
    assert!(out.is_empty());
}

#[test]
fn test_bcj2_code_without_any_branches() {
    let pure_data = vec![0x90u8; 10000]; // 10000 NOPs
    let streams = encode_bcj2(&pure_data, 0x5000);

    assert!(streams.call.is_empty());
    assert!(streams.jump.is_empty());

    let mut out = Vec::new();
    let bytes = decode_bcj2_stream(
        &streams.main[..],
        &streams.call[..],
        &streams.jump[..],
        &streams.rc[..],
        &mut out,
        0x5000,
    )
    .expect("decode pure data stream");

    assert_eq!(bytes, 10000);
    assert_eq!(out, pure_data);
}
