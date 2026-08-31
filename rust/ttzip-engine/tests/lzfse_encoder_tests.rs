// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for Apple LZFSE 4-Way associative hash matcher and reverse FSE block encoder.
//!
//! Validates:
//! 1. 4-Way associative hash matcher correctness (Knuth multiplier, XOR pre-filtering, SWAR match length, reverse extension, lazy evaluation).
//! 2. 1-Deep distance caching filter (`d_prev` delta elimination).
//! 3. Pure Rust FSE block encoder compliance and roundtrip verification against both Apple C reference `lzfse_decompress` and pure Rust FSE decoder.
//! 4. Multi-block 256KB streaming chunking and terminal `bvx$` container generation.

use ttzip_engine::codecs::lzfse::block::{parse_block_header, BvxMagic};
use ttzip_engine::codecs::lzfse::encoder::{
    apply_d_prev_filter, find_matches_4way, lzfse_compress_pure_rust, lzfse_encode_block,
    split_lmd_matches, FseOutStream, LzfseMatchTable, LzfseRawMatch,
};
use ttzip_engine::codecs::lzfse::fse::{
    fse_init_decoder_table_packed, fse_init_value_decoder_table,
};
use ttzip_engine::codecs::lzfse::fse_decoder::{
    decode_literals_4way, decode_lmd_stream, FseInStream, FseLmdState, FseLmdTables,
};
use ttzip_engine::codecs::lzfse::tables::{
    D_BASE_VALUE, D_EXTRA_BITS, L_BASE_VALUE, L_EXTRA_BITS, M_BASE_VALUE, M_EXTRA_BITS,
};
use ttzip_engine::codecs::lzfse::{lzfse_decompress, lzfse_decompress_to_vec};

// MARK: - 4-Way Matcher Unit Tests

#[test]
fn test_4way_matcher_short_buffer() {
    let mut table = LzfseMatchTable::new();
    let mut literals = Vec::new();
    let mut matches = Vec::new();

    // Less than 8 bytes: matcher should emit all bytes as literals
    let short_data = b"1234567";
    find_matches_4way(short_data, &mut table, &mut literals, &mut matches);

    assert_eq!(&literals[..], short_data);
    assert!(matches.is_empty());
}

#[test]
fn test_4way_matcher_repetitive_patterns() {
    let mut table = LzfseMatchTable::new();
    let mut literals = Vec::new();
    let mut matches = Vec::new();

    // Construct repetitive sequence
    let pattern = b"The quick brown fox jumps over the lazy dog! ";
    let mut data = Vec::new();
    for _ in 0..10 {
        data.extend_from_slice(pattern);
    }

    find_matches_4way(&data, &mut table, &mut literals, &mut matches);

    // Repetitive data must produce matches
    assert!(!matches.is_empty());

    // Verify all match references are valid and identical to source slices
    for m in &matches {
        assert!(m.length >= 4);
        assert!(m.pos >= m.ref_pos + 4);
        let current_slice = &data[m.pos..m.pos + m.length];
        let ref_slice = &data[m.ref_pos..m.ref_pos + m.length];
        assert_eq!(current_slice, ref_slice);
    }
}

#[test]
fn test_4way_matcher_incompressible_data() {
    let mut table = LzfseMatchTable::new();
    let mut literals = Vec::new();
    let mut matches = Vec::new();

    // Deterministic pseudo-random bytes with high entropy (LCG)
    let mut rng_state: u64 = 0x1234_5678_9ABC_DEF0;
    let mut data = vec![0u8; 1024];
    for b in data.iter_mut() {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        *b = (rng_state >> 33) as u8;
    }

    find_matches_4way(&data, &mut table, &mut literals, &mut matches);

    // Any matches that happen to be found by chance must be strictly valid
    for m in &matches {
        assert_eq!(
            &data[m.pos..m.pos + m.length],
            &data[m.ref_pos..m.ref_pos + m.length]
        );
    }
}

#[test]
fn test_4way_matcher_reverse_match_extension() {
    let mut table = LzfseMatchTable::new();
    let mut literals = Vec::new();
    let mut matches = Vec::new();

    // Create a payload where prefix bytes before a 4-byte hash match are also identical
    let mut data = Vec::new();
    data.extend_from_slice(b"PREFIX_COMMON_SUFFIX_12345678_EXTRA_");
    data.extend_from_slice(b"PREFIX_COMMON_SUFFIX_12345678_EXTRA_");

    find_matches_4way(&data, &mut table, &mut literals, &mut matches);

    assert!(!matches.is_empty());
    // The second occurrence should absorb the common prefix via reverse match extension
    let total_matched_bytes: usize = matches.iter().map(|m| m.length).sum();
    assert!(total_matched_bytes >= 30);
}

// MARK: - LMD Triplet & d_prev Filter Tests

#[test]
fn test_split_lmd_matches_and_d_prev_filter() {
    let src = b"AAAABBBBCCCCAAAABBBBCCCCAAAABBBBCCCC";
    let raw_matches = vec![
        LzfseRawMatch {
            pos: 12,
            ref_pos: 0,
            length: 12,
        },
        LzfseRawMatch {
            pos: 24,
            ref_pos: 12,
            length: 12,
        },
    ];

    let mut triplets = Vec::new();
    split_lmd_matches(src.len(), &raw_matches, &mut triplets);

    assert_eq!(triplets.len(), 2);
    assert_eq!(triplets[0].l, 12);
    assert_eq!(triplets[0].m, 12);
    assert_eq!(triplets[0].d, 12);

    assert_eq!(triplets[1].l, 0);
    assert_eq!(triplets[1].m, 12);
    assert_eq!(triplets[1].d, 12);

    // Apply d_prev elimination filter
    apply_d_prev_filter(&mut triplets);

    // The second match has the same distance d=12 as the previous match, so it must be converted to d=0
    assert_eq!(triplets[0].d, 12);
    assert_eq!(triplets[1].d, 0);
}

// MARK: - OutStream Bitstream Tests

#[test]
fn test_fse_out_stream_push_flush_finish() {
    let mut stream = FseOutStream::new();
    let mut payload = Vec::new();

    // Push 10 bits: 0x2AA (1010101010)
    stream.push(10, 0x2AA);
    assert_eq!(stream.accum_nbits, 10);
    assert_eq!(stream.accum, 0x2AA);

    // Flush should emit 1 byte (8 bits) and leave 2 bits
    stream.flush(&mut payload);
    assert_eq!(payload.len(), 1);
    assert_eq!(payload[0], 0xAA);
    assert_eq!(stream.accum_nbits, 2);
    assert_eq!(stream.accum, 0x2);

    // Push 14 bits
    stream.push(14, 0x3FFF);
    stream.flush(&mut payload);
    assert_eq!(payload.len(), 3);

    let final_bits = stream.finish(&mut payload);
    assert!((-7..=0).contains(&final_bits));
}

// MARK: - Pure Rust Roundtrip Tests (Apple C Decoder & Pure Rust Decoder)

/// Helper function to decompress a single LZFSE V2 block using pure Rust FSE decoder.
fn decompress_pure_rust_block(block_bytes: &[u8]) -> Vec<u8> {
    let (header, header_len) = parse_block_header(block_bytes).expect("parse block header");
    assert_eq!(header.magic, BvxMagic::CompressedV2);

    let freq_tables = header.freq_tables.expect("freq tables present");

    let mut l_table = [ttzip_engine::codecs::lzfse::fse::FseValueDecoderEntry::default(); 64];
    let mut m_table = [ttzip_engine::codecs::lzfse::fse::FseValueDecoderEntry::default(); 64];
    let mut d_table = [ttzip_engine::codecs::lzfse::fse::FseValueDecoderEntry::default(); 256];
    let mut lit_table = [0i32; 1024];

    fse_init_value_decoder_table(
        64,
        20,
        &freq_tables.l_freq,
        &L_BASE_VALUE,
        &L_EXTRA_BITS,
        &mut l_table,
    )
    .expect("init l_table");
    fse_init_value_decoder_table(
        64,
        20,
        &freq_tables.m_freq,
        &M_BASE_VALUE,
        &M_EXTRA_BITS,
        &mut m_table,
    )
    .expect("init m_table");
    fse_init_value_decoder_table(
        256,
        64,
        &freq_tables.d_freq,
        &D_BASE_VALUE,
        &D_EXTRA_BITS,
        &mut d_table,
    )
    .expect("init d_table");
    fse_init_decoder_table_packed(
        1024,
        256,
        &freq_tables.literal_freq,
        &mut lit_table,
    )
    .expect("init lit_table");

    let lit_payload_len = header.n_literal_payload_bytes as usize;
    let lmd_payload_len = header.n_lmd_payload_bytes as usize;

    let lit_slice = &block_bytes[..header_len + lit_payload_len];
    let lmd_slice = &block_bytes[..header_len + lit_payload_len + lmd_payload_len];

    // Decode literals
    let mut lit_stream = FseInStream::init(header.literal_bits, lit_slice).expect("init lit stream");
    let mut literals = vec![0u8; header.n_literals as usize];
    let mut lit_states = header.literal_state;
    decode_literals_4way(&mut lit_stream, &lit_table, &mut lit_states, &mut literals)
        .expect("decode literals");

    // Decode LMD
    let mut lmd_stream = FseInStream::init(header.lmd_bits, lmd_slice).expect("init lmd stream");
    let tables = FseLmdTables {
        l_table: &l_table,
        m_table: &m_table,
        d_table: &d_table,
    };
    let mut state = FseLmdState {
        l_state: header.l_state,
        m_state: header.m_state,
        d_state: header.d_state,
    };

    let mut dst = Vec::new();
    let uncompressed_len = decode_lmd_stream(
        &mut lmd_stream,
        &tables,
        &mut state,
        header.n_matches as usize,
        &literals,
        &mut dst,
        header.n_raw_bytes as usize,
    )
    .expect("decode lmd");

    assert_eq!(uncompressed_len, header.n_raw_bytes as usize);
    dst
}


#[test]
fn test_pure_rust_encoder_single_block_roundtrip() {
    let mut table = LzfseMatchTable::new();
    let sentence = b"Welcome to Apple LZFSE (Lempel-Ziv Finite State Entropy) high performance compression! \
Apple LZFSE operates at 3x the decompression speed of zlib with comparable compression ratio. \
This test verifies pure Rust 4-Way associative hash matching, reverse entropy encoding, \
and bit-exact roundtrip compatibility across both C and Rust decoders.\n";

    let mut text = Vec::new();
    for _ in 0..16 {
        text.extend_from_slice(sentence);
    }

    let mut compressed_block = Vec::new();
    lzfse_encode_block(&text, &mut table, &mut compressed_block).expect("encode block");

    // 1. Decode with Pure Rust FSE Decoder
    let decompressed_rust = decompress_pure_rust_block(&compressed_block);
    assert_eq!(&decompressed_rust[..], &text[..]);

    // 2. Decode with C native reference decoder (append EOS to make it a full container)
    let mut full_container = compressed_block.clone();
    full_container.extend_from_slice(&BvxMagic::EndOfStream.as_bytes());

    let decompressed_c = lzfse_decompress_to_vec(&full_container, text.len())
        .expect("decompress with C reference");
    assert_eq!(&decompressed_c[..], &text[..]);
}

#[test]
fn test_pure_rust_compress_various_payloads_roundtrip() {
    let test_cases: Vec<Vec<u8>> = vec![
        // Small 32-byte string
        b"Small test payload 1234567890123".to_vec(),
        // All-identical bytes
        vec![b'Z'; 4096],
        // Alternating 2-byte pattern
        (0..2048).map(|i| if i % 2 == 0 { b'A' } else { b'B' }).collect(),
        // Ascii counting sequence
        (0..8192).map(|i| b'0' + (i % 10) as u8).collect(),
    ];

    for (idx, original) in test_cases.iter().enumerate() {
        let mut compressed = Vec::new();
        let written = lzfse_compress_pure_rust(original, &mut compressed).expect("compress");
        assert!(written > 0);

        // Verify with C reference decoder
        let decompressed_c = lzfse_decompress_to_vec(&compressed, original.len())
            .unwrap_or_else(|e| panic!("failed case {idx}: {e:?}"));
        assert_eq!(&decompressed_c[..], &original[..], "Mismatch in test case {idx}");
    }
}

// MARK: - Multi-Block (256KB+) Streaming Performance Test

#[test]
fn test_pure_rust_compress_large_multi_block_600kb() {
    // Construct 600KB mixed dataset spanning multiple 256KB blocks
    let mut large_src = Vec::with_capacity(600 * 1024);
    let sample_code = b"fn process_data(chunk: &[u8]) -> Result<usize, TTZipStatus> { \
        let hash = calculate_xxhash64(chunk); \
        let entry = cache.get(&hash); \
        Ok(entry.len()) \
    }\n";

    while large_src.len() < 600 * 1024 {
        large_src.extend_from_slice(sample_code);
    }
    large_src.truncate(600 * 1024);

    let mut compressed = Vec::new();
    let compressed_size = lzfse_compress_pure_rust(&large_src, &mut compressed)
        .expect("compress 600KB payload");

    assert!(compressed_size > 0);
    assert!(compressed_size < large_src.len() / 2, "LZFSE should compress repetitive code by >= 50%");

    // Verify terminal EOS magic block at end of container
    assert!(compressed.len() >= 4);
    let eos_magic = &compressed[compressed.len() - 4..];
    assert_eq!(eos_magic, &BvxMagic::EndOfStream.as_bytes());

    // Verify 100% Bit-Exact Roundtrip with C reference decoder
    let mut decompressed = vec![0u8; large_src.len()];
    let decompressed_bytes = lzfse_decompress(&compressed, &mut decompressed)
        .expect("decompress 600KB stream");

    assert_eq!(decompressed_bytes, large_src.len());
    assert_eq!(&decompressed[..], &large_src[..]);
}
