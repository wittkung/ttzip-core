// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and integration tests for LZFSE 4-state interleaved FSE decoder.
//!
//! Validates:
//! 1. 64-bit reverse bitstream accumulator and load/flush boundaries.
//! 2. 4-way interleaved literal decoding pipeline.
//! 3. Fused value decoding for L, M, and D tables.
//! 4. LMD command stream execution with overlapping match semantics.
//! 5. Error handling and bounds-checking defense against corruption and panics.

use ttzip_engine::codecs::lzfse::fse::*;
use ttzip_engine::codecs::lzfse::fse_decoder::*;
use ttzip_engine::codecs::lzfse::tables::*;
use ttzip_engine::types::TTZipStatus;

// MARK: - Test Helper: Reference FSE Encoder for Testing

struct TestFseEncoder {
    accum: u64,
    accum_nbits: i32,
    buf: Vec<u8>,
}

impl TestFseEncoder {
    fn new() -> Self {
        Self {
            accum: 0,
            accum_nbits: 0,
            buf: Vec::new(),
        }
    }

    fn push(&mut self, n: u8, val: u64) {
        if n == 0 {
            return;
        }
        let mask = if n >= 64 { !0u64 } else { (1u64 << n) - 1 };
        self.accum |= (val & mask) << self.accum_nbits;
        self.accum_nbits += n as i32;
    }

    fn flush(&mut self) {
        let nbits = self.accum_nbits & !7;
        if nbits > 0 {
            let nbytes = (nbits >> 3) as usize;
            for i in 0..nbytes {
                self.buf.push(((self.accum >> (i * 8)) & 0xff) as u8);
            }
            self.accum >>= nbits;
            self.accum_nbits -= nbits;
        }
    }

    fn finish(mut self) -> (Vec<u8>, i32) {
        // Write the remaining bits padded to full bytes
        let nbits = (self.accum_nbits + 7) & !7;
        let nbytes = (nbits >> 3) as usize;
        for i in 0..nbytes {
            self.buf.push(((self.accum >> (i * 8)) & 0xff) as u8);
        }
        let final_bits = self.accum_nbits - nbits; // in [-7, 0]
        (self.buf, final_bits)
    }
}

// MARK: - Reverse Bitstream Tests

#[test]
fn test_fse_in_stream_init_and_pull() {
    // 8 bytes of known data: [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    let payload = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];

    // Case 1: nbits = -1 (accum_nbits = 63)
    // High byte is 0x08, top bit of 0x08 is 0, so (val >> 63) == 0.
    let mut stream = FseInStream::init(-1, &payload).expect("init stream nbits=-1");
    assert_eq!(stream.accum_nbits, 63);
    assert!(stream.check());

    // Pull 7 bits from top
    let val7 = stream.pull(7);
    assert_eq!(stream.accum_nbits, 56);
    assert!(stream.check());
    assert_eq!(val7, 0x08); // Top 7 bits of 63 bits = bits 56..62

    // Pull 0 bits
    assert_eq!(stream.pull(0), 0);
    assert_eq!(stream.accum_nbits, 56);
}

#[test]
fn test_fse_in_stream_flush_refill() {
    // 16 bytes buffer: test reverse consumption and accumulator refill
    let mut payload = Vec::new();
    for i in 0..16 {
        payload.push((i * 17 + 5) as u8);
    }

    let mut stream = FseInStream::init(-1, &payload).expect("init 16-byte stream");
    assert_eq!(stream.cursor, 8); // Read last 8 bytes first

    // Pull 16 bits
    let _ = stream.pull(16);
    assert_eq!(stream.accum_nbits, 47);

    // Flush should pull 2 bytes backwards (nbits = (63 - 47) & !7 = 16)
    stream.flush();
    assert_eq!(stream.accum_nbits, 63);
    assert_eq!(stream.cursor, 6);
    assert!(stream.check());

    // Pull all bits until underflow
    let _ = stream.pull(63);
    assert_eq!(stream.accum_nbits, 0);

    // Flush should pull (63 & !7) = 56 bits (7 bytes)
    // But only 6 bytes left in buffer (cursor == 6 < 7), so stream should mark ok = false
    stream.flush();
    assert!(!stream.check());
}

#[test]
fn test_fse_in_stream_edge_cases() {
    // Empty payload
    let empty = [];
    assert!(FseInStream::init(0, &empty).is_err());

    // Short payload (< 7 bytes for nbits=0)
    let short = [1u8, 2, 3, 4, 5, 6];
    assert!(FseInStream::init(0, &short).is_err());

    // Short payload (< 8 bytes for nbits=-1)
    assert!(FseInStream::init(-1, &short).is_err());

    // Invalid nbits (+1 is outside allowed [-7, 0])
    let valid_len = [0u8; 16];
    assert!(FseInStream::init(1, &valid_len).is_err());
}

// MARK: - 4-Way Interleaved Literal Decoding Tests

#[test]
fn test_4way_interleaved_literal_roundtrip() {
    // Create an FSE literal frequency distribution: 4 symbols 'A', 'B', 'C', 'D' with 256 count each = 1024 states
    let mut freq = [0u16; 256];
    freq[b'A' as usize] = 256;
    freq[b'B' as usize] = 256;
    freq[b'C' as usize] = 256;
    freq[b'D' as usize] = 256;

    let mut decoder_table = [0i32; 1024];
    fse_init_decoder_table_packed(1024, 256, &freq, &mut decoder_table).expect("init decoder table");

    let mut encoder_table = [FseEncoderEntry::default(); 256];
    let _ = fse_init_encoder_table(1024, 256, &freq, &mut encoder_table);
    let original = b"ABCDABCDABCDABCDABCDABCDABCDABCD";
    let n = original.len();

    // In LZFSE, 4-way decoding decodes 4 streams interleaved:
    // dst[0], dst[1], dst[2], dst[3], dst[4], ...
    // Since FSE is a LIFO/reverse entropy code, we encode backward in each stream!
    let mut encoder = TestFseEncoder::new();
    encoder.buf.extend_from_slice(&[0u8; 8]); // 8-byte prefix padding for reverse bitstream

    // Initial states for the 4 streams when starting backward encoding
    let mut states = [0u16; 4];

    // Helper encode step
    let encode_step = |state: &mut u16, sym: u8, enc: &mut TestFseEncoder| {
        let e = encoder_table[sym as usize];
        let s = *state as i32;
        let hi = s >= (e.s0 as i32);
        let nbits = if hi { e.k } else { e.k - 1 } as u8;
        let delta = if hi { e.delta0 } else { e.delta1 } as u16;
        let mask = if nbits >= 16 { 0xffff } else { (1u16 << nbits) - 1 };
        let b = *state & mask;
        enc.push(nbits, b as u64);
        *state = delta.wrapping_add(*state >> nbits);
    };

    // Encode backwards: from last block to first (lanes 3, 2, 1, 0)
    for block_idx in (0..n / 4).rev() {
        encode_step(&mut states[3], original[block_idx * 4 + 3], &mut encoder);
        encode_step(&mut states[2], original[block_idx * 4 + 2], &mut encoder);
        encode_step(&mut states[1], original[block_idx * 4 + 1], &mut encoder);
        encode_step(&mut states[0], original[block_idx * 4], &mut encoder);
        encoder.flush();
    }

    let (payload_raw, nbits) = encoder.finish();
    let mut payload = vec![0u8; 8];
    payload.extend_from_slice(&payload_raw);

    let mut in_stream = FseInStream::init(nbits, &payload).expect("init in_stream");
    let mut decoded = vec![0u8; n];
    let mut decode_states = states;
    let res = decode_literals_4way(&mut in_stream, &decoder_table, &mut decode_states, &mut decoded);
    assert!(res.is_ok());
    assert_eq!(&decoded[..], original);
}

#[test]
fn test_4way_interleaved_invalid_buffer_size() {
    let table = [0i32; 1024];
    let mut states = [0u16; 4];
    let payload = [0u8; 16];
    let mut stream = FseInStream::init(0, &payload).expect("init");

    // Buffer not multiple of 4
    let mut dst = [0u8; 15];
    let err = decode_literals_4way(&mut stream, &table, &mut states, &mut dst);
    assert_eq!(err, Err(TTZipStatus::ErrInvalidParam));
}

// MARK: - Value Decoder & LMD Stream Tests

#[test]
fn test_fse_value_decode_tables() {
    // Setup 64-state L table with 20 symbols
    let mut l_freq = [0u16; 20];
    l_freq[0] = 32;
    l_freq[16] = 32; // symbol 16 has 2 extra bits and base 16

    let mut l_table = [FseValueDecoderEntry::default(); 64];
    let _ = fse_init_value_decoder_table(64, 20, &l_freq, &L_BASE_VALUE, &L_EXTRA_BITS, &mut l_table);

    assert_eq!(l_table[0].value_bits, 0);
    assert_eq!(l_table[0].vbase, 0);
    assert_eq!(l_table[32].value_bits, 2);
    assert_eq!(l_table[32].vbase, 16);
}

#[test]
fn test_decode_lmd_stream_execution() {
    // Construct dummy L, M, D tables
    let mut l_freq = [0u16; 20];
    l_freq[0] = 64; // All states map to symbol 0 (L=0)
    let mut l_table = [FseValueDecoderEntry::default(); 64];
    let _ = fse_init_value_decoder_table(64, 20, &l_freq, &L_BASE_VALUE, &L_EXTRA_BITS, &mut l_table);

    let mut m_freq = [0u16; 20];
    m_freq[0] = 64; // All states map to symbol 0 (M=0)
    let mut m_table = [FseValueDecoderEntry::default(); 64];
    let _ = fse_init_value_decoder_table(64, 20, &m_freq, &M_BASE_VALUE, &M_EXTRA_BITS, &mut m_table);

    let mut d_freq = [0u16; 64];
    d_freq[0] = 256; // All states map to symbol 0 (D=0)
    let mut d_table = [FseValueDecoderEntry::default(); 256];
    let _ = fse_init_value_decoder_table(256, 64, &d_freq, &D_BASE_VALUE, &D_EXTRA_BITS, &mut d_table);

    // Manually set entry 0 to test specific L, M, D values
    // Match 1: L=5, M=0, D=0 (copies 5 literal bytes)
    l_table[0] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 1, // transitions to state 1
        vbase: 5,
    };
    m_table[0] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 1,
        vbase: 0,
    };
    d_table[0] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 1,
        vbase: 0,
    };

    // Match 2: L=0, M=10, D=5 (copies 10 match bytes with overlapping repeat from dist 5)
    l_table[1] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 2,
        vbase: 0,
    };
    m_table[1] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 2,
        vbase: 10,
    };
    d_table[1] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 2,
        vbase: 5,
    };

    let literals = b"HELLO_EXTRA_DATA";
    let payload = [0u8; 16];
    let mut stream = FseInStream::init(0, &payload).expect("init stream");

    let mut state = FseLmdState {
        l_state: 0,
        m_state: 0,
        d_state: 0,
    };
    let tables = FseLmdTables {
        l_table: &l_table,
        m_table: &m_table,
        d_table: &d_table,
    };

    let mut dst = Vec::new();
    let written = decode_lmd_stream(
        &mut stream,
        &tables,
        &mut state,
        2,
        literals,
        &mut dst,
        15,
    )
    .expect("decode lmd");

    assert_eq!(written, 15); // 5 literals + 10 matches
    assert_eq!(&dst[..5], b"HELLO");
    // Repeating 10 bytes from dist 5 (b"HELLO"): "HELLOHELLO"
    assert_eq!(&dst[5..15], b"HELLOHELLO");
}

// MARK: - Bounds Checking & Defensive Safety Tests

#[test]
fn test_lmd_out_of_bounds_match_distance_defense() {
    let mut l_table = [FseValueDecoderEntry::default(); 64];
    let mut m_table = [FseValueDecoderEntry::default(); 64];
    let mut d_table = [FseValueDecoderEntry::default(); 256];

    // L=2, M=4, D=10 (D exceeds current output offset 2)
    l_table[0] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 0,
        vbase: 2,
    };
    m_table[0] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 0,
        vbase: 4,
    };
    d_table[0] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 0,
        vbase: 10,
    };

    let literals = b"AB";
    let payload = [0u8; 16];
    let mut stream = FseInStream::init(0, &payload).expect("init stream");
    let mut dst = Vec::new();
    let mut state = FseLmdState::default();
    let tables = FseLmdTables {
        l_table: &l_table,
        m_table: &m_table,
        d_table: &d_table,
    };

    let res = decode_lmd_stream(
        &mut stream,
        &tables,
        &mut state,
        1,
        literals,
        &mut dst,
        32,
    );
    assert_eq!(res, Err(TTZipStatus::ErrExtractionFailed));
}

#[test]
fn test_lmd_dst_overflow_defense() {
    let mut l_table = [FseValueDecoderEntry::default(); 64];
    let m_table = [FseValueDecoderEntry::default(); 64];
    let d_table = [FseValueDecoderEntry::default(); 256];

    l_table[0] = FseValueDecoderEntry {
        total_bits: 0,
        value_bits: 0,
        delta: 0,
        vbase: 100, // L exceeds expected length
    };

    let literals = vec![b'X'; 200];
    let payload = [0u8; 16];
    let mut stream = FseInStream::init(0, &payload).expect("init stream");
    let mut dst = Vec::new();
    let mut state = FseLmdState::default();
    let tables = FseLmdTables {
        l_table: &l_table,
        m_table: &m_table,
        d_table: &d_table,
    };

    let res = decode_lmd_stream(
        &mut stream,
        &tables,
        &mut state,
        1,
        &literals,
        &mut dst,
        32, // expected is only 32 bytes, but L=100
    );
    assert_eq!(res, Err(TTZipStatus::ErrExtractionFailed));
}


#[test]
fn test_fuzz_random_bitstreams_zero_panic() {
    let table = [0i32; 1024];
    let mut states = [0u16; 4];

    // Seeded deterministic pseudo-random sequences
    for seed in 0..50 {
        let mut pseudo_random = Vec::with_capacity(32);
        let mut val = seed as u32;
        for _ in 0..32 {
            val = val.wrapping_mul(1103515245).wrapping_add(12345);
            pseudo_random.push((val >> 16) as u8);
        }

        for nbits in -7..=0 {
            if let Ok(mut stream) = FseInStream::init(nbits, &pseudo_random) {
                let mut dst = [0u8; 16];
                // Must not panic under any circumstance
                let _ = decode_literals_4way(&mut stream, &table, &mut states, &mut dst);
            }
        }
    }
}
