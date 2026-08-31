// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and invariant tests for Apple LZFSE Block container parsing,
//! bitfield packing/unpacking, Huffman frequency tables codec, and streaming FSM.

use ttzip_engine::codecs::lzfse::block::{
    decode_v1_freq_value, decode_v2_freq_tables, emit_block_header_v2, encode_v1_freq_value,
    encode_v2_freq_tables, parse_block_header, BvxMagic, LzfseBlockHeader, LzfseFreqTables,
    LZFSE_V2_HEADER_FIXED_SIZE,
};
use ttzip_engine::codecs::lzfse::fsm::{LzfseBlockFsm, LzfseFsmStep};
use ttzip_engine::codecs::lzfse::tables::{
    LZFSE_ENCODE_D_STATES, LZFSE_ENCODE_D_SYMBOLS, LZFSE_ENCODE_LITERAL_SYMBOLS,
    LZFSE_ENCODE_L_STATES, LZFSE_ENCODE_L_SYMBOLS, LZFSE_ENCODE_M_STATES, LZFSE_ENCODE_M_SYMBOLS,
    LZFSE_LITERALS_PER_BLOCK, LZFSE_MATCHES_PER_BLOCK,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - 1. BVX Magic Parsing and Emission Tests

#[test]
fn test_bvx_magic_values_and_representations() {
    let magics = [
        (BvxMagic::RawUncompressed, 0x2d78_7662u32, *b"bvx-", "bvx-"),
        (BvxMagic::CompressedV1, 0x3178_7662u32, *b"bvx1", "bvx1"),
        (BvxMagic::CompressedV2, 0x3278_7662u32, *b"bvx2", "bvx2"),
        (BvxMagic::CompressedLZVN, 0x6e78_7662u32, *b"bvxn", "bvxn"),
        (BvxMagic::EndOfStream, 0x2478_7662u32, *b"bvx$", "bvx$"),
    ];

    for (magic, expected_u32, expected_bytes, expected_str) in magics {
        assert_eq!(magic.as_u32(), expected_u32);
        assert_eq!(magic.as_bytes(), expected_bytes);
        assert_eq!(magic.as_str(), expected_str);
        assert_eq!(BvxMagic::from_u32(expected_u32), Some(magic));
        assert_eq!(BvxMagic::from_bytes(expected_bytes), Some(magic));
        assert_eq!(BvxMagic::try_from(expected_u32).unwrap(), magic);
        assert_eq!(u32::from(magic), expected_u32);
    }

    // Invalid magics
    assert_eq!(BvxMagic::from_u32(0x0000_0000), None);
    assert_eq!(BvxMagic::from_u32(0xDEAD_BEEF), None);
    assert!(BvxMagic::try_from(0x1234_5678).is_err());
}

#[test]
fn test_parse_block_header_all_magics() {
    // 1. End of stream block (4 bytes)
    let eos_bytes = BvxMagic::EndOfStream.as_bytes();
    let (eos_hdr, eos_len) = parse_block_header(&eos_bytes).expect("parse eos");
    assert_eq!(eos_hdr.magic, BvxMagic::EndOfStream);
    assert_eq!(eos_len, 4);

    // 2. Raw uncompressed block (8 bytes)
    let mut raw_bytes = Vec::new();
    raw_bytes.extend_from_slice(&BvxMagic::RawUncompressed.as_bytes());
    raw_bytes.extend_from_slice(&123456u32.to_le_bytes());
    let (raw_hdr, raw_len) = parse_block_header(&raw_bytes).expect("parse raw");
    assert_eq!(raw_hdr.magic, BvxMagic::RawUncompressed);
    assert_eq!(raw_hdr.n_raw_bytes, 123456);
    assert_eq!(raw_len, 8);

    // 3. Compressed LZVN block (12 bytes)
    let mut lzvn_bytes = Vec::new();
    lzvn_bytes.extend_from_slice(&BvxMagic::CompressedLZVN.as_bytes());
    lzvn_bytes.extend_from_slice(&65536u32.to_le_bytes()); // n_raw_bytes
    lzvn_bytes.extend_from_slice(&24500u32.to_le_bytes()); // n_payload_bytes
    let (lzvn_hdr, lzvn_len) = parse_block_header(&lzvn_bytes).expect("parse lzvn");
    assert_eq!(lzvn_hdr.magic, BvxMagic::CompressedLZVN);
    assert_eq!(lzvn_hdr.n_raw_bytes, 65536);
    assert_eq!(lzvn_hdr.n_payload_bytes, 24500);
    assert_eq!(lzvn_len, 12);

    // 4. Compressed V1 block (770 bytes)
    let mut v1_bytes = Vec::new();
    v1_bytes.extend_from_slice(&BvxMagic::CompressedV1.as_bytes());
    v1_bytes.extend_from_slice(&10000u32.to_le_bytes()); // n_raw_bytes
    v1_bytes.extend_from_slice(&4000u32.to_le_bytes());  // n_payload_bytes
    v1_bytes.extend_from_slice(&1000u32.to_le_bytes());  // n_literals
    v1_bytes.extend_from_slice(&250u32.to_le_bytes());   // n_matches
    v1_bytes.extend_from_slice(&2000u32.to_le_bytes());  // n_literal_payload_bytes
    v1_bytes.extend_from_slice(&2000u32.to_le_bytes());  // n_lmd_payload_bytes
    v1_bytes.extend_from_slice(&(-3i32).to_le_bytes());  // literal_bits
    v1_bytes.extend_from_slice(&10u16.to_le_bytes());    // literal_state[0]
    v1_bytes.extend_from_slice(&20u16.to_le_bytes());    // literal_state[1]
    v1_bytes.extend_from_slice(&30u16.to_le_bytes());    // literal_state[2]
    v1_bytes.extend_from_slice(&40u16.to_le_bytes());    // literal_state[3]
    v1_bytes.extend_from_slice(&(-5i32).to_le_bytes());  // lmd_bits
    v1_bytes.extend_from_slice(&5u16.to_le_bytes());     // l_state
    v1_bytes.extend_from_slice(&8u16.to_le_bytes());     // m_state
    v1_bytes.extend_from_slice(&15u16.to_le_bytes());    // d_state
    // 360 frequency values (all 0)
    v1_bytes.extend_from_slice(&[0u8; 720]);

    let (v1_hdr, v1_len) = parse_block_header(&v1_bytes).expect("parse v1");
    assert_eq!(v1_hdr.magic, BvxMagic::CompressedV1);
    assert_eq!(v1_hdr.n_raw_bytes, 10000);
    assert_eq!(v1_hdr.n_literals, 1000);
    assert_eq!(v1_hdr.literal_bits, -3);
    assert_eq!(v1_hdr.l_state, 5);
    assert_eq!(v1_len, 770);
}

// MARK: - 2. V2 Header 3x 64-bit Bitfields Roundtrip Tests

#[test]
fn test_v2_bitfield_packing_unpacking_roundtrip() {
    let test_cases = [
        // Case 1: Minimum / zero values
        LzfseBlockHeader {
            magic: BvxMagic::CompressedV2,
            n_raw_bytes: 0,
            n_payload_bytes: 0,
            n_literals: 0,
            n_matches: 0,
            n_literal_payload_bytes: 0,
            n_lmd_payload_bytes: 0,
            literal_bits: -7,
            literal_state: [0, 0, 0, 0],
            lmd_bits: -7,
            l_state: 0,
            m_state: 0,
            d_state: 0,
            freq_tables: None,
            header_size: 32,
        },
        // Case 2: Realistic medium values
        LzfseBlockHeader {
            magic: BvxMagic::CompressedV2,
            n_raw_bytes: 65536,
            n_payload_bytes: 25000,
            n_literals: 15000,
            n_matches: 4000,
            n_literal_payload_bytes: 12000,
            n_lmd_payload_bytes: 13000,
            literal_bits: -3,
            literal_state: [128, 256, 512, 768],
            lmd_bits: 0,
            l_state: 32,
            m_state: 45,
            d_state: 180,
            freq_tables: None,
            header_size: 160,
        },
        // Case 3: Boundary maximum values
        LzfseBlockHeader {
            magic: BvxMagic::CompressedV2,
            n_raw_bytes: 1048575,
            n_payload_bytes: 2097150,
            n_literals: LZFSE_LITERALS_PER_BLOCK as u32,
            n_matches: LZFSE_MATCHES_PER_BLOCK as u32,
            n_literal_payload_bytes: 0xF_FFFF,
            n_lmd_payload_bytes: 0xF_FFFF,
            literal_bits: 0,
            literal_state: [1023, 1023, 1023, 1023],
            lmd_bits: 0,
            l_state: (LZFSE_ENCODE_L_STATES - 1) as u16,
            m_state: (LZFSE_ENCODE_M_STATES - 1) as u16,
            d_state: (LZFSE_ENCODE_D_STATES - 1) as u16,
            freq_tables: None,
            header_size: 4096,
        },
    ];

    for (idx, original) in test_cases.iter().enumerate() {
        let packed = original.pack_v2_fields();
        let unpacked = LzfseBlockHeader::unpack_v2_fields(
            original.n_raw_bytes,
            packed,
            original.freq_tables.clone(),
        )
        .unwrap_or_else(|e| panic!("Case {idx} failed unpacking: {:?}", e));

        assert_eq!(unpacked.n_raw_bytes, original.n_raw_bytes, "Case {idx} n_raw_bytes mismatch");
        assert_eq!(unpacked.n_literals, original.n_literals, "Case {idx} n_literals mismatch");
        assert_eq!(
            unpacked.n_literal_payload_bytes, original.n_literal_payload_bytes,
            "Case {idx} n_literal_payload_bytes mismatch"
        );
        assert_eq!(unpacked.n_matches, original.n_matches, "Case {idx} n_matches mismatch");
        assert_eq!(unpacked.literal_bits, original.literal_bits, "Case {idx} literal_bits mismatch");
        assert_eq!(unpacked.literal_state, original.literal_state, "Case {idx} literal_state mismatch");
        assert_eq!(
            unpacked.n_lmd_payload_bytes, original.n_lmd_payload_bytes,
            "Case {idx} n_lmd_payload_bytes mismatch"
        );
        assert_eq!(unpacked.lmd_bits, original.lmd_bits, "Case {idx} lmd_bits mismatch");
        assert_eq!(unpacked.header_size, original.header_size, "Case {idx} header_size mismatch");
        assert_eq!(unpacked.l_state, original.l_state, "Case {idx} l_state mismatch");
        assert_eq!(unpacked.m_state, original.m_state, "Case {idx} m_state mismatch");
        assert_eq!(unpacked.d_state, original.d_state, "Case {idx} d_state mismatch");
    }
}

#[test]
fn test_emit_and_parse_v2_header_roundtrip() {
    let mut tables = LzfseFreqTables::default();
    tables.l_freq[0] = 32;
    tables.l_freq[1] = 32;
    tables.m_freq[0] = 64;
    tables.d_freq[0] = 256;
    tables.literal_freq[0] = 512;
    tables.literal_freq[1] = 512;
    tables.validate().expect("valid tables");

    let header = LzfseBlockHeader {
        magic: BvxMagic::CompressedV2,
        n_raw_bytes: 32768,
        n_payload_bytes: 15000,
        n_literals: 8000,
        n_matches: 2000,
        n_literal_payload_bytes: 7000,
        n_lmd_payload_bytes: 8000,
        literal_bits: -2,
        literal_state: [100, 200, 300, 400],
        lmd_bits: -4,
        l_state: 12,
        m_state: 24,
        d_state: 120,
        freq_tables: Some(tables.clone()),
        header_size: 0, // will be computed on emit
    };

    let mut encoded = Vec::new();
    emit_block_header_v2(&header, &mut encoded);
    assert!(encoded.len() > LZFSE_V2_HEADER_FIXED_SIZE);

    let (parsed, consumed) = parse_block_header(&encoded).expect("parse encoded v2");
    assert_eq!(consumed, encoded.len());
    assert_eq!(parsed.magic, BvxMagic::CompressedV2);
    assert_eq!(parsed.n_raw_bytes, 32768);
    assert_eq!(parsed.n_literals, 8000);
    assert_eq!(parsed.n_matches, 2000);
    assert_eq!(parsed.literal_bits, -2);
    assert_eq!(parsed.lmd_bits, -4);
    assert_eq!(parsed.l_state, 12);
    assert_eq!(parsed.m_state, 24);
    assert_eq!(parsed.d_state, 120);

    let parsed_tables = parsed.freq_tables.expect("freq tables present");
    assert_eq!(parsed_tables.l_freq, tables.l_freq);
    assert_eq!(parsed_tables.m_freq, tables.m_freq);
    assert_eq!(parsed_tables.d_freq, tables.d_freq);
    assert_eq!(parsed_tables.literal_freq, tables.literal_freq);
}

// MARK: - 3. Huffman Frequency Table Codec Fidelity Tests

#[test]
fn test_huffman_single_value_encoding_decoding() {
    for val in 0..=1024u16 {
        let (bits, nbits) = encode_v1_freq_value(val);
        assert!(nbits > 0, "val={val} returned nbits=0");
        assert!(nbits <= 14, "val={val} returned nbits > 14");
        assert!(bits < (1 << nbits), "bits out of range for val={val}");

        let (decoded_val, decoded_nbits) =
            decode_v1_freq_value(bits).unwrap_or_else(|e| panic!("Decode failed for val {val}: {:?}", e));

        assert_eq!(decoded_val, val, "Decoded value mismatch for val={val}");
        assert_eq!(decoded_nbits, nbits, "Decoded nbits mismatch for val={val}");
    }
}

#[test]
fn test_freq_tables_full_serialization_roundtrip() {
    let mut tables = LzfseFreqTables::default();
    // Fill with diverse valid frequencies
    for i in 0..LZFSE_ENCODE_L_SYMBOLS {
        tables.l_freq[i] = if i < 4 { 16 } else { 0 };
    }
    for i in 0..LZFSE_ENCODE_M_SYMBOLS {
        tables.m_freq[i] = if i < 8 { 8 } else { 0 };
    }
    for i in 0..LZFSE_ENCODE_D_SYMBOLS {
        tables.d_freq[i] = if i < 16 { 16 } else { 0 };
    }
    for i in 0..LZFSE_ENCODE_LITERAL_SYMBOLS {
        tables.literal_freq[i] = if i < 32 { 32 } else { 0 };
    }

    tables.validate().expect("tables valid");

    let mut dst = Vec::new();
    encode_v2_freq_tables(&tables, &mut dst);
    assert!(!dst.is_empty());

    let (decoded_tables, consumed) = decode_v2_freq_tables(&dst).expect("decode freq tables");
    assert_eq!(consumed, dst.len());
    assert_eq!(decoded_tables.l_freq, tables.l_freq);
    assert_eq!(decoded_tables.m_freq, tables.m_freq);
    assert_eq!(decoded_tables.d_freq, tables.d_freq);
    assert_eq!(decoded_tables.literal_freq, tables.literal_freq);
}

// MARK: - 4. Malformed Block Header & Error Rejection Tests

#[test]
fn test_malformed_headers_zero_panic() {
    // 1. Buffer too short
    assert_eq!(parse_block_header(&[]), Err(TTZipStatus::ErrCorruptHeader));
    assert_eq!(parse_block_header(&[0x62, 0x76]), Err(TTZipStatus::ErrCorruptHeader));
    assert_eq!(parse_block_header(&[0x62, 0x76, 0x78]), Err(TTZipStatus::ErrCorruptHeader));

    // 2. Unknown magic
    assert_eq!(
        parse_block_header(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert_eq!(
        parse_block_header(b"PK\x03\x04\x00\x00\x00\x00"),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    // 3. Truncated raw block header
    let raw_magic = BvxMagic::RawUncompressed.as_bytes();
    assert_eq!(parse_block_header(&raw_magic[..4]), Err(TTZipStatus::ErrCorruptHeader));

    // 4. Truncated LZVN block header
    let lzvn_magic = BvxMagic::CompressedLZVN.as_bytes();
    let mut short_lzvn = Vec::new();
    short_lzvn.extend_from_slice(&lzvn_magic);
    short_lzvn.extend_from_slice(&[1, 2, 3, 4]); // only 8 bytes, need 12
    assert_eq!(parse_block_header(&short_lzvn), Err(TTZipStatus::ErrCorruptHeader));

    // 5. Truncated V2 block header
    let v2_magic = BvxMagic::CompressedV2.as_bytes();
    let mut short_v2 = Vec::new();
    short_v2.extend_from_slice(&v2_magic);
    short_v2.extend_from_slice(&[0u8; 20]); // only 24 bytes, need 32
    assert_eq!(parse_block_header(&short_v2), Err(TTZipStatus::ErrCorruptHeader));

    // 6. V2 header with declared header_size < 32
    let mut invalid_size_v2 = Vec::new();
    invalid_size_v2.extend_from_slice(&v2_magic);
    invalid_size_v2.extend_from_slice(&100u32.to_le_bytes()); // n_raw_bytes
    invalid_size_v2.extend_from_slice(&0u64.to_le_bytes());   // packed0
    invalid_size_v2.extend_from_slice(&0u64.to_le_bytes());   // packed1
    invalid_size_v2.extend_from_slice(&16u64.to_le_bytes());  // packed2: header_size = 16 (< 32!)
    assert_eq!(parse_block_header(&invalid_size_v2), Err(TTZipStatus::ErrCorruptHeader));

    // 7. V2 header with declared header_size > input buffer length
    let mut oob_size_v2 = Vec::new();
    oob_size_v2.extend_from_slice(&v2_magic);
    oob_size_v2.extend_from_slice(&100u32.to_le_bytes()); // n_raw_bytes
    oob_size_v2.extend_from_slice(&0u64.to_le_bytes());   // packed0
    oob_size_v2.extend_from_slice(&0u64.to_le_bytes());   // packed1
    oob_size_v2.extend_from_slice(&500u64.to_le_bytes()); // packed2: header_size = 500 (> 32!)
    assert_eq!(parse_block_header(&oob_size_v2), Err(TTZipStatus::ErrCorruptHeader));

    // 8. FSE state out of range validation
    let bad_header = LzfseBlockHeader {
        magic: BvxMagic::CompressedV2,
        literal_state: [1024, 0, 0, 0], // >= 1024 illegal!
        ..Default::default()
    };
    assert_eq!(bad_header.validate(), Err(TTZipStatus::ErrCorruptHeader));

    let bad_l_header = LzfseBlockHeader {
        magic: BvxMagic::CompressedV2,
        l_state: 64, // >= 64 illegal!
        ..Default::default()
    };
    assert_eq!(bad_l_header.validate(), Err(TTZipStatus::ErrCorruptHeader));
}

// MARK: - 5. Streaming Block Container FSM Tests

#[test]
fn test_streaming_block_fsm_multi_block_stream() {
    let mut stream_bytes = Vec::new();

    // Block 1: Raw uncompressed block (10 bytes payload)
    stream_bytes.extend_from_slice(&BvxMagic::RawUncompressed.as_bytes());
    stream_bytes.extend_from_slice(&10u32.to_le_bytes());
    stream_bytes.extend_from_slice(b"0123456789");

    // Block 2: Compressed LZVN block (16 bytes raw, 8 bytes payload)
    stream_bytes.extend_from_slice(&BvxMagic::CompressedLZVN.as_bytes());
    stream_bytes.extend_from_slice(&16u32.to_le_bytes());
    stream_bytes.extend_from_slice(&8u32.to_le_bytes());
    stream_bytes.extend_from_slice(b"lzvnpayl");

    // Block 3: End of stream block
    stream_bytes.extend_from_slice(&BvxMagic::EndOfStream.as_bytes());

    let mut fsm = LzfseBlockFsm::new();

    // Step 1: Parse block 1 header
    let (step1, c1) = fsm.feed(&stream_bytes[..8]).expect("fsm step 1");
    assert_eq!(c1, 8);
    match step1 {
        LzfseFsmStep::HeaderParsed(hdr) => {
            assert_eq!(hdr.magic, BvxMagic::RawUncompressed);
            assert_eq!(hdr.n_raw_bytes, 10);
        }
        other => panic!("Unexpected step 1: {:?}", other),
    }

    // Step 2: Feed payload in two small chunks (4 bytes + 6 bytes)
    let (step2a, c2a) = fsm.feed(&stream_bytes[8..12]).expect("fsm step 2a");
    assert_eq!(c2a, 4);
    match step2a {
        LzfseFsmStep::PayloadChunk { header, data } => {
            assert_eq!(header.magic, BvxMagic::RawUncompressed);
            assert_eq!(data, b"0123");
        }
        other => panic!("Unexpected step 2a: {:?}", other),
    }

    let (step2b, c2b) = fsm.feed(&stream_bytes[12..18]).expect("fsm step 2b");
    assert_eq!(c2b, 6);
    match step2b {
        LzfseFsmStep::PayloadChunk { header, data } => {
            assert_eq!(header.magic, BvxMagic::RawUncompressed);
            assert_eq!(data, b"456789");
        }
        other => panic!("Unexpected step 2b: {:?}", other),
    }

    // Step 3: Parse block 2 header
    let (step3, c3) = fsm.feed(&stream_bytes[18..30]).expect("fsm step 3");
    assert_eq!(c3, 12);
    match step3 {
        LzfseFsmStep::HeaderParsed(hdr) => {
            assert_eq!(hdr.magic, BvxMagic::CompressedLZVN);
            assert_eq!(hdr.n_raw_bytes, 16);
            assert_eq!(hdr.n_payload_bytes, 8);
        }
        other => panic!("Unexpected step 3: {:?}", other),
    }

    // Step 4: Consume block 2 payload
    let (step4, c4) = fsm.feed(&stream_bytes[30..38]).expect("fsm step 4");
    assert_eq!(c4, 8);
    match step4 {
        LzfseFsmStep::PayloadChunk { data, .. } => {
            assert_eq!(data, b"lzvnpayl");
        }
        other => panic!("Unexpected step 4: {:?}", other),
    }

    // Step 5: Parse end of stream block
    let (step5, c5) = fsm.feed(&stream_bytes[38..42]).expect("fsm step 5");
    assert_eq!(c5, 4);
    assert!(matches!(step5, LzfseFsmStep::EndOfStream));
    assert!(fsm.is_end_of_stream());
    assert_eq!(fsm.blocks_processed(), 3);
    assert_eq!(fsm.total_raw_bytes(), 26);
    assert_eq!(fsm.total_payload_bytes(), 18);
}

#[test]
fn test_parse_complete_block_utility() {
    let mut block_data = Vec::new();
    block_data.extend_from_slice(&BvxMagic::RawUncompressed.as_bytes());
    block_data.extend_from_slice(&5u32.to_le_bytes());
    block_data.extend_from_slice(b"HELLO");

    // Partial input -> None
    let partial = LzfseBlockFsm::parse_complete_block(&block_data[..6]).expect("parse partial");
    assert!(partial.is_none());

    // Full input -> Some
    let complete = LzfseBlockFsm::parse_complete_block(&block_data).expect("parse complete");
    let blk = complete.expect("complete block found");
    assert_eq!(blk.header.magic, BvxMagic::RawUncompressed);
    assert_eq!(blk.payload, b"HELLO");
    assert_eq!(blk.total_consumed, 13);
}
