// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed LZFSE & LZVN Fault-Injection Fuzzing Harness & Jitter Streaming Suite.
//!
//! Implements a 16-dimensional fault injection test matrix, micro-step jitter streaming
//! perturbation suite (1..7 bytes), and 500+ iteration automated mutation fuzzing loop
//! aligned with Apple LZFSE / LZVN container specifications and `research_105`:
//! 1. Bad Magic Number Corruption (Replacing `bvx-` with invalid magic).
//! 2. Corrupted 3x 64-bit V2 Header Bitfields (Invalid bit lengths, states, payload sizes).
//! 3. Illegal LZVN Opcode Injections (Undefined opcodes `0x1E`, `0x26`, `0x2E`, `0x36`, `0x3E`).
//! 4. Truncated Block Headers (Sweep across 1..N header byte cuts).
//! 5. Truncated Payload Bodies (Truncated raw, LZVN, and V2 payload blocks).
//! 6. Invalid FSE Frequency Table Checksum & Excessive Frequency Sums.
//! 7. Out-of-Bounds FSE Initial States (States exceeding symbol/state capacities).
//! 8. Out-of-Bounds LZVN Backward Match Distance ($D > dst\_pos$).
//! 9. LZVN $D = 0$ Distance Injection (Illegal zero-distance match).
//! 10. Malformed Huffman Frequency Table Bitstream (V2 header Huffman decoder stress).
//! 11. Missing `bvx$` End-of-Stream Terminal Marker.
//! 12. Extreme Overlapping Match Injections ($D=1, M=1000$ RLE Splat and Wild Copy).
//! 13. Reverse Bitstream Premature EOF & Underflow.
//! 14. Random Single-Bit Flip Fuzzing (500+ Iterations).
//! 15. Random Multi-Byte Erasure & Chunk Splice Attacks (500+ Iterations).
//! 16. Multi-Seed High-Entropy Pseudo-Stream Injection (1,000+ Random Streams).

use std::io::{Cursor, Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use ttzip_engine::codecs::lzfse::block::{
    decode_v2_freq_tables, parse_block_header, LZFSE_ENCODE_D_STATES, LZFSE_ENCODE_D_SYMBOLS,
    LZFSE_ENCODE_LITERAL_STATES, LZFSE_ENCODE_LITERAL_SYMBOLS, LZFSE_ENCODE_L_STATES,
    LZFSE_ENCODE_L_SYMBOLS, LZFSE_ENCODE_M_STATES, LZFSE_V2_HEADER_FIXED_SIZE,
};
use ttzip_engine::codecs::lzfse::fse::{
    fse_check_freq, fse_init_decoder_table_packed, fse_init_value_decoder_table,
    FseValueDecoderEntry,
};
use ttzip_engine::codecs::lzfse::fse_decoder::{
    decode_literals_4way, decode_lmd_stream, FseInStream, FseLmdState, FseLmdTables,
};
use ttzip_engine::codecs::lzfse::lzvn_decoder::{
    lzvn_decompress_pure_rust, lzvn_validate, LzvnDecoder,
};
use ttzip_engine::codecs::lzfse::reader::{
    lzfse_decompress_stream, lzfse_validate, LzfseReader,
};
use ttzip_engine::codecs::lzfse::tables::{
    D_BASE_VALUE, D_EXTRA_BITS, L_BASE_VALUE, L_EXTRA_BITS,
};
use ttzip_engine::codecs::lzfse::writer::{
    lzfse_compress_stream, LzfseWriter,
};
use ttzip_engine::codecs::lzfse::lzfse_decompress;
use ttzip_engine::types::TTZipStatus;

// MARK: - Deterministic Knuth Multiplicative PRNG (FUZ_rand)

/// Deterministic Knuth multiplicative hash PRNG for reproducible fuzzing seeds.
#[derive(Debug, Clone)]
pub struct FuzRand {
    seed: u32,
}

impl FuzRand {
    /// Constructs a new PRNG with the specified 32-bit seed.
    #[inline]
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    /// Generates the next pseudo-random 32-bit integer.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.seed = self
            .seed
            .wrapping_mul(2_654_435_761)
            .wrapping_add(2_246_822_519);
        self.seed >> 13
    }

    /// Generates a pseudo-random integer in the closed interval `[min, max]`.
    #[inline]
    pub fn rand_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max {
            return min;
        }
        let span = max - min + 1;
        min + (self.next_u32() % span)
    }

    /// Generates a pseudo-random `usize` in half-open interval `[0, bound)`.
    #[inline]
    pub fn rand_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u32() as usize) % bound
        }
    }

    /// Generates a pseudo-random byte.
    #[inline]
    pub fn rand_u8(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }

    /// Generates a pseudo-random payload buffer with target compressibility.
    pub fn gen_buffer(&mut self, size: usize, compressibility_pct: u32) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        if compressibility_pct >= 100 {
            return buf;
        }
        let alphabet_len = match compressibility_pct {
            0..=10 => 256,
            11..=30 => 128,
            31..=60 => 32,
            61..=80 => 8,
            81..=95 => 3,
            _ => 1,
        };
        for b in buf.iter_mut() {
            *b = (self.next_u32() % alphabet_len) as u8;
        }
        buf
    }
}

/// Helper function to generate structured repetitive text for compression test fixtures.
fn generate_structured_text(repetitions: usize) -> Vec<u8> {
    let paragraph = b"Apple LZFSE / LZVN high-performance compression format. \
Pure Safe Rust implementation with 4-way interleaved FSE entropy coding and LZ77 byte matching.\n";
    let mut out = Vec::with_capacity(paragraph.len() * repetitions);
    for _ in 0..repetitions {
        out.extend_from_slice(paragraph);
    }
    out
}

// MARK: - Target 1: Bad Magic Number Corruption

#[test]
fn test_lzfse_target01_bad_magic_corruption() {
    let invalid_magics: [u32; 8] = [
        0x0000_0000,
        0xFFFF_FFFF,
        0xDEAD_BEEF,
        0x1234_5678,
        0x3378_7662, // "bvx3"
        0x3078_7662, // "bvx0"
        0x2D78_7663, // "cvx-" (off-by-one)
        0x5858_5858, // "XXXX"
    ];

    for &magic in &invalid_magics {
        let mut corrupted_header = magic.to_le_bytes().to_vec();
        corrupted_header.extend_from_slice(&[0u8; 28]); // Padding to 32 bytes

        let parse_res = parse_block_header(&corrupted_header);
        assert!(
            parse_res.is_err(),
            "Invalid magic {magic:#010X} must be rejected by parse_block_header"
        );

        let validate_res = lzfse_validate(&corrupted_header);
        assert!(!validate_res, "lzfse_validate must return false for bad magic");

        let mut dst = vec![0u8; 128];
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            lzfse_decompress(&corrupted_header, &mut dst)
        }));
        assert!(unwind_res.is_ok(), "lzfse_decompress panicked on bad magic");
        assert!(unwind_res.unwrap().is_err());

        let unwind_stream = catch_unwind(AssertUnwindSafe(|| {
            lzfse_decompress_stream(&corrupted_header)
        }));
        assert!(unwind_stream.is_ok(), "lzfse_decompress_stream panicked on bad magic");
        assert!(unwind_stream.unwrap().is_err());
    }
}

// MARK: - Target 2: Corrupted 3x 64-bit V2 Header Bitfields

#[test]
fn test_lzfse_target02_corrupted_v2_header_bitfields() {
    let valid_data = generate_structured_text(10);
    let compressed = lzfse_compress_stream(&valid_data).expect("valid compress");
    assert!(lzfse_validate(&compressed));

    // Verify V2 block bitfield corruption resistance
    if compressed.len() >= LZFSE_V2_HEADER_FIXED_SIZE && &compressed[0..4] == b"bvx2" {
        // Corrupt v0, v1, v2 words in the 32-byte header
        for word_offset in [8, 16, 24] {
            let mut mutated = compressed.clone();
            // Flip highest bits in bitfield words
            mutated[word_offset] ^= 0xFF;
            mutated[word_offset + 1] ^= 0xFF;

            let unwind_res = catch_unwind(AssertUnwindSafe(|| {
                lzfse_decompress_stream(&mutated)
            }));
            assert!(
                unwind_res.is_ok(),
                "Decompressor panicked on corrupted V2 header word at offset {word_offset}"
            );
            assert!(unwind_res.unwrap().is_err());
        }
    }
}

// MARK: - Target 3: Illegal LZVN Opcode Injection

#[test]
fn test_lzfse_target03_illegal_lzvn_opcode_injection() {
    let undefined_opcodes: [u8; 5] = [0x1E, 0x26, 0x2E, 0x36, 0x3E];
    let mut dst = vec![0u8; 256];

    for &opcode in &undefined_opcodes {
        // 1. Illegal opcode at stream start
        let stream = vec![opcode, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut decoder = LzvnDecoder::new();
        let decode_res = decoder.decode(&stream, &mut dst);
        assert_eq!(
            decode_res,
            Err(TTZipStatus::ErrCorruptHeader),
            "Undefined LZVN opcode {opcode:#04X} must return ErrCorruptHeader"
        );

        // 2. Illegal opcode mid-stream after valid literal
        let mid_stream = vec![0xE4, b'T', b'T', b'Z', b'P', opcode, 0x06, 0, 0, 0, 0, 0, 0, 0];
        let mut decoder_mid = LzvnDecoder::new();
        let decode_mid_res = decoder_mid.decode(&mid_stream, &mut dst);
        assert_eq!(decode_mid_res, Err(TTZipStatus::ErrCorruptHeader));

        // 3. Pure Rust facade check
        let pure_res = lzvn_decompress_pure_rust(&stream, &mut dst);
        assert_eq!(pure_res, Err(TTZipStatus::ErrCorruptHeader));

        // 4. Validation facade check
        assert!(!lzvn_validate(&stream));
    }
}

// MARK: - Target 4: Truncated Block Headers

#[test]
fn test_lzfse_target04_truncated_block_headers() {
    let mut v2_header = Vec::new();
    v2_header.extend_from_slice(b"bvx2");
    v2_header.extend_from_slice(&1000u32.to_le_bytes()); // n_raw_bytes
    v2_header.extend_from_slice(&[0u8; 24]); // 3x 64-bit bitfields (v0, v1, v2)

    // Sweep all prefix cut lengths from 1 to 31 bytes
    for cut_len in 1..v2_header.len() {
        let prefix = &v2_header[..cut_len];
        let parse_res = parse_block_header(prefix);
        assert!(
            parse_res.is_err(),
            "Truncated header at length {cut_len} must return Err"
        );

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            lzfse_decompress_stream(prefix)
        }));
        assert!(unwind_res.is_ok(), "Panic on truncated header at length {cut_len}");
        assert!(unwind_res.unwrap().is_err());
    }

    // Truncated Raw header (1..7 bytes) and LZVN header (1..11 bytes)
    let raw_hdr = [b'b', b'v', b'x', b'-', 10, 0, 0, 0];
    for cut in 1..raw_hdr.len() {
        assert!(parse_block_header(&raw_hdr[..cut]).is_err());
    }

    let lzvn_hdr = [b'b', b'v', b'x', b'n', 10, 0, 0, 0, 5, 0, 0, 0];
    for cut in 1..lzvn_hdr.len() {
        assert!(parse_block_header(&lzvn_hdr[..cut]).is_err());
    }
}

// MARK: - Target 5: Truncated Payload Bodies

#[test]
fn test_lzfse_target05_truncated_payload_bodies() {
    let original = generate_structured_text(15);
    let compressed = lzfse_compress_stream(&original).expect("compress");

    // Cut compressed stream at various points after the header
    for cut_len in [4, 8, 12, 16, 24, 32, 40, compressed.len() / 2, compressed.len() - 1] {
        if cut_len >= compressed.len() {
            continue;
        }
        let truncated = &compressed[..cut_len];

        assert!(!lzfse_validate(truncated));

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            lzfse_decompress_stream(truncated)
        }));
        assert!(unwind_res.is_ok(), "Panic on truncated payload at {cut_len}");
        assert!(unwind_res.unwrap().is_err());

        // Test with LzfseReader streaming interface
        let mut reader = LzfseReader::new(Cursor::new(truncated));
        let mut out = Vec::new();
        let read_res = reader.read_to_end(&mut out);
        assert!(
            read_res.is_err(),
            "LzfseReader must return error on truncated payload cut at {cut_len}"
        );
    }
}

// MARK: - Target 6: Invalid FSE Frequency Table Checksum & Excessive Frequency Sums

#[test]
fn test_lzfse_target06_invalid_fse_freq_table_checksum_and_sum() {
    // 1. Check excessive literal frequencies (sum > 1024)
    let mut over_literal_freq = [0u16; LZFSE_ENCODE_LITERAL_SYMBOLS];
    over_literal_freq[0] = 512;
    over_literal_freq[1] = 513; // sum = 1025 > 1024
    let check_res = fse_check_freq(&over_literal_freq, LZFSE_ENCODE_LITERAL_STATES);
    assert_eq!(check_res, Err(TTZipStatus::ErrCorruptHeader));

    let mut lit_table = [0i32; LZFSE_ENCODE_LITERAL_STATES];
    let init_res = fse_init_decoder_table_packed(
        LZFSE_ENCODE_LITERAL_STATES,
        LZFSE_ENCODE_LITERAL_SYMBOLS,
        &over_literal_freq,
        &mut lit_table,
    );
    assert_eq!(init_res, Err(TTZipStatus::ErrCorruptHeader));

    // 2. Check excessive L/M frequencies (sum > 64)
    let mut over_l_freq = [0u16; LZFSE_ENCODE_L_SYMBOLS];
    over_l_freq[0] = 40;
    over_l_freq[1] = 25; // sum = 65 > 64
    let mut l_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_L_STATES];
    let init_l_res = fse_init_value_decoder_table(
        LZFSE_ENCODE_L_STATES,
        LZFSE_ENCODE_L_SYMBOLS,
        &over_l_freq,
        &L_BASE_VALUE,
        &L_EXTRA_BITS,
        &mut l_table,
    );
    assert_eq!(init_l_res, Err(TTZipStatus::ErrCorruptHeader));

    // 3. Check excessive D frequencies (sum > 256)
    let mut over_d_freq = [0u16; LZFSE_ENCODE_D_SYMBOLS];
    over_d_freq[0] = 200;
    over_d_freq[1] = 57; // sum = 257 > 256
    let mut d_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_D_STATES];
    let init_d_res = fse_init_value_decoder_table(
        LZFSE_ENCODE_D_STATES,
        LZFSE_ENCODE_D_SYMBOLS,
        &over_d_freq,
        &D_BASE_VALUE,
        &D_EXTRA_BITS,
        &mut d_table,
    );
    assert_eq!(init_d_res, Err(TTZipStatus::ErrCorruptHeader));
}

// MARK: - Target 7: Out-of-Bounds FSE Initial States

#[test]
fn test_lzfse_target07_out_of_bounds_fse_initial_state() {
    let dummy_payload = vec![0u8; 16];
    let lit_table = [0i32; LZFSE_ENCODE_LITERAL_STATES];
    let l_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_L_STATES];
    let m_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_M_STATES];
    let d_table = [FseValueDecoderEntry::default(); LZFSE_ENCODE_D_STATES];

    // Out-of-bounds literal states (>= 1024)
    let invalid_states = [1024u16, 2048, 65535];
    for &state in &invalid_states {
        let mut stream = FseInStream::init(0, &dummy_payload).expect("init stream");
        let mut states = [state, 0, 0, 0];
        let mut literals = [0u8; 4];
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            decode_literals_4way(&mut stream, &lit_table, &mut states, &mut literals)
        }));
        assert!(unwind_res.is_ok(), "decode_literals_4way panicked on out-of-bounds state {state}");
        assert!(unwind_res.unwrap().is_err());
    }

    // Out-of-bounds L/M/D states
    let mut stream_lmd = FseInStream::init(0, &dummy_payload).expect("init stream");
    let mut state = FseLmdState {
        l_state: 64u16, // >= 64
        m_state: 0u16,
        d_state: 0u16,
    };
    let tables = FseLmdTables {
        l_table: &l_table,
        m_table: &m_table,
        d_table: &d_table,
    };
    let mut dst = Vec::new();
    let unwind_lmd = catch_unwind(AssertUnwindSafe(|| {
        decode_lmd_stream(
            &mut stream_lmd,
            &tables,
            &mut state,
            1,
            b"A",
            &mut dst,
            16,
        )
    }));
    assert!(unwind_lmd.is_ok(), "decode_lmd_stream panicked on out-of-bounds L state");
    assert!(unwind_lmd.unwrap().is_err());
}

// MARK: - Target 8: Out-of-Bounds LZVN Backward Distance (D > dst_pos)

#[test]
fn test_lzfse_target08_out_of_bounds_lzvn_backward_distance() {
    let mut stream = Vec::new();
    // 1. Emit 4 literal bytes (dst_pos becomes 4)
    stream.push(0xE4);
    stream.extend_from_slice(b"TEST");
    // 2. Emit Large Distance Match with D = 100 > 4
    // LrgD: 0x07 (L=0, M=3), followed by 2-byte distance D = 100
    stream.push(0x07);
    stream.push(100);
    stream.push(0);
    stream.extend_from_slice(&[0x06, 0, 0, 0, 0, 0, 0, 0]); // EOS

    let mut dst = vec![0u8; 64];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Underflowing distance D > dst_pos must return ErrCorruptHeader"
    );

    let pure_res = lzvn_decompress_pure_rust(&stream, &mut dst);
    assert_eq!(pure_res, Err(TTZipStatus::ErrCorruptHeader));
}

// MARK: - Target 9: LZVN D = 0 Distance Injection

#[test]
fn test_lzfse_target09_lzvn_zero_distance_injection() {
    let mut stream = Vec::new();
    // 1. Emit 4 literal bytes
    stream.push(0xE4);
    stream.extend_from_slice(b"ABCD");
    // 2. Emit Small Distance Match with D = 0
    // SmlD: 0x00, byte 1: 0x00 => D = 0
    stream.push(0x00);
    stream.push(0x00);
    stream.extend_from_slice(&[0x06, 0, 0, 0, 0, 0, 0, 0]); // EOS

    let mut dst = vec![0u8; 64];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Distance D = 0 must be rejected with ErrCorruptHeader"
    );
}

// MARK: - Target 10: Malformed Huffman Frequency Table Bitstream

#[test]
fn test_lzfse_target10_malformed_huffman_freq_table_bitstream() {
    // 1. Truncated or incomplete bitstreams for 360 symbols
    assert!(decode_v2_freq_tables(&[0u8; 4]).is_err());
    assert!(decode_v2_freq_tables(&[0xFFu8; 8]).is_err());
    assert!(decode_v2_freq_tables(&[0x12, 0x34, 0x56]).is_err());

    // 2. High-entropy random bitstream as Huffman table
    let mut rng = FuzRand::new(0x2026_0830);
    for _ in 0..50 {
        let noise = rng.gen_buffer(64, 0);
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            decode_v2_freq_tables(&noise)
        }));
        assert!(unwind_res.is_ok(), "decode_v2_freq_tables panicked on random bitstream");
    }
}

// MARK: - Target 11: Missing bvx$ EOS Terminator

#[test]
fn test_lzfse_target11_missing_bvx_eos_terminator() {
    // Valid raw uncompressed block without bvx$ terminal block
    let mut stream_without_eos = Vec::new();
    stream_without_eos.extend_from_slice(b"bvx-");
    stream_without_eos.extend_from_slice(&5u32.to_le_bytes()); // n_raw_bytes = 5
    stream_without_eos.extend_from_slice(b"Hello");

    let validate_res = lzfse_validate(&stream_without_eos);
    assert!(
        !validate_res,
        "lzfse_validate must fail when stream does not end with bvx$"
    );

    let decompress_res = lzfse_decompress_stream(&stream_without_eos);
    assert_eq!(
        decompress_res,
        Err(TTZipStatus::ErrCorruptHeader),
        "Missing bvx$ must return ErrCorruptHeader"
    );
}

// MARK: - Target 12: Extreme Overlapping Match Injections (D=1, M=1000)

#[test]
fn test_lzfse_target12_extreme_overlapping_match_rle_splat() {
    // Construct LZVN stream with D=1, M=1000 (RLE Splat)
    // 1. Emit 1 literal byte 'X' and SmlD with D=1, M=3
    let mut stream = vec![0xE1, b'X', 0x00, 0x01];
    // 3. Emit 60x LrgM M=16 matches (M = 16 * 60 + 3 + 1 = 964 bytes)
    for _ in 0..60 {
        stream.push(0xF0);
        stream.push(0); // M = 16
    }
    stream.extend_from_slice(&[0x06, 0, 0, 0, 0, 0, 0, 0]); // EOS

    // Test with adequate destination buffer
    let mut dst = vec![0u8; 2048];
    let mut decoder = LzvnDecoder::new();
    let res = decoder.decode(&stream, &mut dst).expect("decode extreme overlap");
    assert!(res.1 > 900);
    for b in &dst[..res.1] {
        assert_eq!(*b, b'X');
    }

    // Test with undersized destination buffer (capacity overflow defense)
    let mut small_dst = vec![0u8; 100];
    let mut small_decoder = LzvnDecoder::new();
    let overflow_res = small_decoder.decode(&stream, &mut small_dst);
    assert_eq!(
        overflow_res,
        Err(TTZipStatus::ErrExtractionFailed),
        "Destination buffer overflow must be gracefully rejected"
    );
}

// MARK: - Target 13: Reverse Bitstream Premature EOF & Underflow

#[test]
fn test_lzfse_target13_reverse_bitstream_premature_eof() {
    // 1. Stream with insufficient bytes for bit count
    let short_payload = [0xAA, 0xBB];
    let err_init = FseInStream::init(-3, &short_payload);
    assert_eq!(err_init.err(), Some(TTZipStatus::ErrCorruptHeader));

    // 2. Reverse bitstream pull past EOF
    let valid_payload = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let mut stream = FseInStream::init(0, &valid_payload).expect("init stream");
    assert!(stream.check());

    // Exhaust stream by pulling excessive bits without refilling
    for _ in 0..10 {
        let _ = stream.pull(30);
    }
    assert!(!stream.check(), "Stream check must fail when exhausted");
}

// MARK: - Target 14: Random Single-Bit Flip Fuzzing (500+ Iterations)

#[test]
fn test_lzfse_target14_random_single_bit_flip_fuzzing() {
    let payload = generate_structured_text(10);
    let compressed = lzfse_compress_stream(&payload).expect("compress stream");
    assert!(!compressed.is_empty());

    let mut rng = FuzRand::new(0x2026_0830);
    let mut dst = vec![0u8; payload.len() + 512];

    for iter in 0..500 {
        let mut mutated = compressed.clone();
        let byte_idx = rng.rand_usize(mutated.len());
        let bit_idx = rng.rand_range(0, 7) as u8;
        mutated[byte_idx] ^= 1 << bit_idx;

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let _ = lzfse_decompress_stream(&mutated);
            let _ = lzfse_decompress(&mutated, &mut dst);
            let mut reader = LzfseReader::new(Cursor::new(&mutated));
            let mut out = Vec::new();
            let _ = reader.read_to_end(&mut out);
        }));

        assert!(
            unwind_res.is_ok(),
            "Single-bit flip panicked on iteration {iter} at byte {byte_idx}, bit {bit_idx}"
        );
    }
}

// MARK: - Target 15: Random Multi-Byte Erasure & Chunk Splice Attacks (500+ Iterations)

#[test]
fn test_lzfse_target15_random_multi_byte_erasure() {
    let payload = generate_structured_text(12);
    let compressed = lzfse_compress_stream(&payload).expect("compress stream");
    let mut rng = FuzRand::new(0x1337_BEEF);
    let mut dst = vec![0u8; payload.len() + 512];

    for iter in 0..500 {
        let mut mutated = compressed.clone();
        let erase_len = rng.rand_range(1, 32) as usize;
        let erase_offset = rng.rand_usize(mutated.len().saturating_sub(erase_len));

        // 50% chance zero-fill, 50% chance slice drop
        if rng.next_u32().is_multiple_of(2) {
            for b in &mut mutated[erase_offset..erase_offset + erase_len] {
                *b = 0;
            }
        } else {
            mutated.drain(erase_offset..erase_offset + erase_len);
        }

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let _ = lzfse_decompress_stream(&mutated);
            let _ = lzfse_decompress(&mutated, &mut dst);
        }));

        assert!(
            unwind_res.is_ok(),
            "Multi-byte erasure panicked on iteration {iter} (offset {erase_offset}, len {erase_len})"
        );
    }
}

// MARK: - Target 16: Multi-Seed High-Entropy Pseudo-Stream Injection (1,000+ Random Streams)

#[test]
fn test_lzfse_target16_random_high_entropy_pseudostream_injection() {
    let seeds = [
        0u32, 1, 42, 1337, 0x2026_0830, 0xDEAD_BEEF, 0xCAFE_BABE, 0x8000_0000, 0xFFFF_FFFF,
        0x1234_5678,
    ];
    let mut dst = vec![0u8; 4096];

    for &seed in &seeds {
        let mut rng = FuzRand::new(seed);
        for _ in 0..100 {
            let len = rng.rand_range(1, 2048) as usize;
            let noise = rng.gen_buffer(len, 0);

            let unwind_res = catch_unwind(AssertUnwindSafe(|| {
                let _ = lzfse_validate(&noise);
                let _ = lzvn_validate(&noise);
                let _ = lzfse_decompress_stream(&noise);
                let _ = lzfse_decompress(&noise, &mut dst);
                let _ = lzvn_decompress_pure_rust(&noise, &mut dst);
                let mut reader = LzfseReader::new(Cursor::new(&noise));
                let mut out = Vec::new();
                let _ = reader.read_to_end(&mut out);
            }));

            assert!(
                unwind_res.is_ok(),
                "High-entropy pseudo-stream panicked on seed {seed:#010X}"
            );
        }
    }
}

// MARK: - Micro-Step Jitter Streaming Push & Pull (1..7 Bytes)

#[test]
fn test_lzfse_microstep_jitter_streaming_1_to_7_bytes() {
    let payloads: Vec<Vec<u8>> = vec![
        b"Small microstep jitter test payload.".to_vec(),
        generate_structured_text(5),
        generate_structured_text(25), // Spanning across block thresholds
    ];

    for (case_idx, payload) in payloads.iter().enumerate() {
        for step in 1..=7 {
            // 1. Jitter Push into LzfseWriter
            let mut compressed = Vec::new();
            {
                let mut writer = LzfseWriter::new(&mut compressed);
                let mut cursor = 0;
                while cursor < payload.len() {
                    let end = (cursor + step).min(payload.len());
                    writer
                        .write_all(&payload[cursor..end])
                        .expect("jitter write chunk");
                    cursor = end;
                }
                writer.finish().expect("finish jitter writer");
            }

            assert!(
                lzfse_validate(&compressed),
                "Jitter compressed stream for case {case_idx}, step {step} must be valid"
            );

            // 2. Jitter Pull from LzfseReader
            let mut reader = LzfseReader::new(Cursor::new(&compressed));
            let mut decompressed = Vec::new();
            let mut chunk = vec![0u8; step];

            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => decompressed.extend_from_slice(&chunk[..n]),
                    Err(e) => panic!("Jitter read failed for case {case_idx}, step {step}: {e:?}"),
                }
            }

            assert_eq!(
                decompressed.as_slice(),
                payload.as_slice(),
                "Decompressed data mismatch for case {case_idx}, step {step}"
            );
        }
    }
}

// MARK: - 500+ Round Automated Mutation Fuzzing Loop

#[test]
fn test_lzfse_500_round_automated_mutation_fuzzing_loop() {
    let payload = generate_structured_text(8);
    let compressed = lzfse_compress_stream(&payload).expect("compress stream");
    let mut rng = FuzRand::new(0x2026_0830);
    let mut dst = vec![0u8; payload.len() + 1024];

    for iter in 0..500 {
        let mut mutated = compressed.clone();
        let mutation_kind = rng.rand_range(0, 3);

        match mutation_kind {
            0 => {
                // Random single byte replace
                let idx = rng.rand_usize(mutated.len());
                mutated[idx] = rng.rand_u8();
            }
            1 => {
                // Random bit flip
                let idx = rng.rand_usize(mutated.len());
                mutated[idx] ^= 1 << (rng.next_u32() & 7);
            }
            2 => {
                // Random byte swap
                let idx1 = rng.rand_usize(mutated.len());
                let idx2 = rng.rand_usize(mutated.len());
                mutated.swap(idx1, idx2);
            }
            _ => {
                // Header byte corruption
                let idx = rng.rand_usize(mutated.len().min(32));
                mutated[idx] ^= 0xFF;
            }
        }

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let stream_res = lzfse_decompress_stream(&mutated);
            if let Ok(data) = stream_res {
                assert!(data.len() <= payload.len() + 1024);
            }
            let block_res = lzfse_decompress(&mutated, &mut dst);
            if let Ok(written) = block_res {
                assert!(written <= dst.len());
            }
        }));

        assert!(
            unwind_res.is_ok(),
            "Decompressor panicked on mutation fuzzing loop iteration {iter} (mutation_kind {mutation_kind})"
        );
    }
}
