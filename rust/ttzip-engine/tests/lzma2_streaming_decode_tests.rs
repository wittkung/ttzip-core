// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for LZMA2 Chunk Control Parsing, 12-State HMM Decoding,
//! and Streaming Dictionary Decompression.

use ttzip_engine::codecs::lzma::range_coder::RangeEncoder;
use ttzip_engine::codecs::lzma::state_machine::{
    LiteralProperties, LzmaProbTable, LzmaState,
};
use ttzip_engine::codecs::lzma2::{
    encode_lzma2_literal_chunk, Lzma2ChunkHeader, Lzma2DecodeError, Lzma2Dict,
    Lzma2StreamDecoder, LZMA2_DEFAULT_DICT_SIZE, LZMA2_MAX_PACK_CHUNK_SIZE,
    LZMA2_MAX_UNPACK_CHUNK_SIZE,
};

#[test]
fn test_lzma2_chunk_header_parsing_all_types() {
    // 1. EOS Chunk (0x00)
    let eos_bytes = [0x00u8, 0xFF, 0xAA];
    let (header, consumed) = Lzma2ChunkHeader::parse(&eos_bytes)
        .expect("parse eos")
        .expect("complete eos");
    assert_eq!(header, Lzma2ChunkHeader::Eos);
    assert_eq!(consumed, 1);
    assert!(header.is_eos());
    assert_eq!(header.unpack_size(), 0);
    assert_eq!(header.pack_size(), 0);

    // 2. Uncompressed with Reset Dict (0x01)
    // 1 byte: 0x0000 + 1 = 1
    let uncomp_reset_1 = [0x01, 0x00, 0x00];
    let (header, consumed) = Lzma2ChunkHeader::parse(&uncomp_reset_1)
        .expect("parse uncomp reset")
        .expect("complete");
    assert_eq!(
        header,
        Lzma2ChunkHeader::UncompressedResetDict { unpack_size: 1 }
    );
    assert_eq!(consumed, 3);
    assert_eq!(header.unpack_size(), 1);
    assert_eq!(header.pack_size(), 1);

    // 65536 bytes (64 KiB): 0xFFFF + 1 = 65536
    let uncomp_reset_64k = [0x01, 0xFF, 0xFF];
    let (header, consumed) = Lzma2ChunkHeader::parse(&uncomp_reset_64k)
        .expect("parse uncomp reset 64k")
        .expect("complete");
    assert_eq!(
        header,
        Lzma2ChunkHeader::UncompressedResetDict {
            unpack_size: 65536
        }
    );
    assert_eq!(consumed, 3);

    // 3. Uncompressed without Reset Dict (0x02)
    let uncomp_no_reset = [0x02, 0x03, 0xFF]; // 0x03FF = 1023 + 1 = 1024 bytes
    let (header, consumed) = Lzma2ChunkHeader::parse(&uncomp_no_reset)
        .expect("parse uncomp no reset")
        .expect("complete");
    assert_eq!(
        header,
        Lzma2ChunkHeader::UncompressedNoReset { unpack_size: 1024 }
    );
    assert_eq!(consumed, 3);

    // 4. Compressed Chunks (0x80..=0xFF)
    // Mode 0: 0b1000_0000 = 0x80 (no reset, unpack_hi=0, unpack=0x0000+1=1, pack=0x0000+1=1, no props)
    let comp_mode0 = [0x80, 0x00, 0x00, 0x00, 0x00];
    let (header, consumed) = Lzma2ChunkHeader::parse(&comp_mode0)
        .expect("parse comp mode 0")
        .expect("complete");
    assert_eq!(
        header,
        Lzma2ChunkHeader::Compressed {
            mode: 0,
            unpack_size: 1,
            pack_size: 1,
            props: None,
        }
    );
    assert_eq!(consumed, 5);

    // Mode 1: 0b1010_0000 = 0xA0 (state reset, unpack=0x0000+1=1, pack=0x0004+1=5, no props)
    let comp_mode1 = [0xA0, 0x00, 0x00, 0x00, 0x04];
    let (header, consumed) = Lzma2ChunkHeader::parse(&comp_mode1)
        .expect("parse comp mode 1")
        .expect("complete");
    assert_eq!(
        header,
        Lzma2ChunkHeader::Compressed {
            mode: 1,
            unpack_size: 1,
            pack_size: 5,
            props: None,
        }
    );
    assert_eq!(consumed, 5);

    // Mode 2: 0b1100_0000 = 0xC0 (state+props reset, unpack=1, pack=5, props=0x5D)
    let comp_mode2 = [0xC0, 0x00, 0x00, 0x00, 0x04, 0x5D];
    let (header, consumed) = Lzma2ChunkHeader::parse(&comp_mode2)
        .expect("parse comp mode 2")
        .expect("complete");
    assert_eq!(
        header,
        Lzma2ChunkHeader::Compressed {
            mode: 2,
            unpack_size: 1,
            pack_size: 5,
            props: Some(0x5D),
        }
    );
    assert_eq!(consumed, 6);

    // Mode 3: 0b1110_0000 = 0xE0 (dict+state+props reset)
    // Max unpack size: control = 0xE0 | 0x1F = 0xFF, u1=0xFF, u2=0xFF -> (0x1FFFFF + 1) = 2,097,152 (2 MiB)
    // Max pack size: p1=0xFF, p2=0xFF -> 65536 bytes
    let comp_mode3_max = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x5D];
    let (header, consumed) = Lzma2ChunkHeader::parse(&comp_mode3_max)
        .expect("parse comp mode 3 max")
        .expect("complete");
    assert_eq!(
        header,
        Lzma2ChunkHeader::Compressed {
            mode: 3,
            unpack_size: LZMA2_MAX_UNPACK_CHUNK_SIZE,
            pack_size: LZMA2_MAX_PACK_CHUNK_SIZE,
            props: Some(0x5D),
        }
    );
    assert_eq!(consumed, 6);
}

#[test]
fn test_lzma2_invalid_control_bytes_rejected() {
    // 0x03..=0x7F are reserved/invalid in LZMA2 specification
    for b in 0x03..=0x7Fu8 {
        let input = [b, 0x00, 0x00, 0x00, 0x00];
        let err = Lzma2ChunkHeader::parse(&input).unwrap_err();
        assert_eq!(err, Lzma2DecodeError::InvalidControlByte(b));
    }
}

#[test]
fn test_lzma2_truncated_header_handling() {
    // Empty
    assert_eq!(Lzma2ChunkHeader::parse(&[]).unwrap(), None);

    // Truncated uncompressed (needs 3 bytes)
    assert_eq!(Lzma2ChunkHeader::parse(&[0x01]).unwrap(), None);
    assert_eq!(Lzma2ChunkHeader::parse(&[0x01, 0x00]).unwrap(), None);
    assert_eq!(Lzma2ChunkHeader::parse(&[0x02, 0x00]).unwrap(), None);

    // Truncated compressed mode 0/1 (needs 5 bytes)
    assert_eq!(Lzma2ChunkHeader::parse(&[0x80, 0x00, 0x00]).unwrap(), None);
    assert_eq!(
        Lzma2ChunkHeader::parse(&[0x80, 0x00, 0x00, 0x00]).unwrap(),
        None
    );

    // Truncated compressed mode 2/3 (needs 6 bytes)
    assert_eq!(
        Lzma2ChunkHeader::parse(&[0xC0, 0x00, 0x00, 0x00, 0x00]).unwrap(),
        None
    );
}

#[test]
fn test_lzma2_uncompressed_roundtrip() {
    let payload = b"TTZip Safe Rust LZMA2 Uncompressed Direct Bypass Payload Validation Test.";
    let mut stream = Vec::new();

    // Chunk 1: Uncompressed with Reset Dict
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream, payload, true);
    // Chunk 2: Uncompressed without Reset Dict
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream, b" Additional uncompressed line.", false);
    // Chunk 3: EOS
    Lzma2ChunkHeader::write_eos(&mut stream);

    let mut decoder = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut decompressed = Vec::new();
    let total_bytes = decoder
        .decode_all(&stream, &mut decompressed)
        .expect("decompress all uncompressed");

    let mut expected = payload.to_vec();
    expected.extend_from_slice(b" Additional uncompressed line.");
    assert_eq!(total_bytes, expected.len());
    assert_eq!(decompressed, expected);
    assert!(decoder.is_eos());
}

#[test]
fn test_lzma2_compressed_literal_roundtrip() {
    let payload = b"TTZip Safe Rust LZMA2 Pure Literal Symbol Markov Decoding Engine Test.";
    let props = LiteralProperties::default();

    let (header, comp_payload) = encode_lzma2_literal_chunk(payload, props);
    assert_eq!(header.unpack_size(), payload.len());
    assert_eq!(header.pack_size(), comp_payload.len());
    let mut stream = Vec::new();

    // Write header bytes
    let mode = 3u8;
    let unpack_minus_1 = (payload.len() - 1) as u32;
    let pack_minus_1 = (comp_payload.len() - 1) as u16;
    let control = 0x80 | (mode << 5) | ((unpack_minus_1 >> 16) as u8 & 0x1F);
    stream.push(control);
    stream.extend_from_slice(&((unpack_minus_1 & 0xFFFF) as u16).to_be_bytes());
    stream.extend_from_slice(&pack_minus_1.to_be_bytes());
    stream.push(props.to_byte());
    stream.extend_from_slice(&comp_payload);
    Lzma2ChunkHeader::write_eos(&mut stream);

    let mut decoder = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut decompressed = Vec::new();
    let total_bytes = decoder
        .decode_all(&stream, &mut decompressed)
        .expect("decompress compressed literal");

    assert_eq!(total_bytes, payload.len());
    assert_eq!(decompressed, payload);
    assert!(decoder.is_eos());
    assert_eq!(decoder.total_uncompressed_bytes(), payload.len());
}

#[test]
fn test_lzma2_interleaved_uncompressed_and_compressed_stream() {
    let part1 = b"Part 1: Initial uncompressed configuration block.\n";
    let part2 = b"Part 2: Compressed literal data payload section.\n";
    let part3 = b"Part 3: Final trailing uncompressed data footer.\n";

    let mut stream = Vec::new();

    // 1. Uncompressed chunk (reset dict)
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream, part1, true);

    // 2. Compressed literal chunk (mode 2: state+probs reset, dict kept)
    let props = LiteralProperties::default();
    let mut enc = RangeEncoder::new();
    let mut probs = LzmaProbTable::new(props);
    let mut state = LzmaState::default();
    let mut prev_byte = *part1.last().unwrap();
    let mut comp2 = Vec::new();
    for (i, &b) in part2.iter().enumerate() {
        let pos = part1.len() + i;
        let pos_state = props.pos_state(pos);
        enc.encode_bit(&mut probs.is_match[state.as_usize()][pos_state], 0, &mut comp2);
        let sub = probs.literal_sub_table_mut(pos, prev_byte);
        enc.encode_literal_byte(sub, b, &mut comp2);
        prev_byte = b;
        state = state.update_literal();
    }
    enc.finish(&mut comp2);

    let mode = 2u8; // mode 2 keeps dict
    let unpack_minus_1 = (part2.len() - 1) as u32;
    let pack_minus_1 = (comp2.len() - 1) as u16;
    let control = 0x80 | (mode << 5) | ((unpack_minus_1 >> 16) as u8 & 0x1F);
    stream.push(control);
    stream.extend_from_slice(&((unpack_minus_1 & 0xFFFF) as u16).to_be_bytes());
    stream.extend_from_slice(&pack_minus_1.to_be_bytes());
    stream.push(props.to_byte());
    stream.extend_from_slice(&comp2);

    // 3. Uncompressed chunk (no reset dict)
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream, part3, false);

    // 4. EOS
    Lzma2ChunkHeader::write_eos(&mut stream);

    let mut decoder = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut decompressed = Vec::new();
    let total = decoder
        .decode_all(&stream, &mut decompressed)
        .expect("decode interleaved stream");

    let mut expected = Vec::new();
    expected.extend_from_slice(part1);
    expected.extend_from_slice(part2);
    expected.extend_from_slice(part3);

    assert_eq!(total, expected.len());
    assert_eq!(decompressed, expected);
    assert!(decoder.is_eos());
}

#[test]
fn test_lzma2_eos_exact_stop_and_multi_stream_concatenation() {
    let stream1_data = b"Stream segment number one.";
    let stream2_data = b"Stream segment number two.";

    let mut stream1 = Vec::new();
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream1, stream1_data, true);
    Lzma2ChunkHeader::write_eos(&mut stream1);
    // Append trailing garbage bytes after EOS
    stream1.extend_from_slice(b"GARBAGE_IGNORED_AFTER_EOS");

    let mut decoder1 = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut out1 = Vec::new();
    let n1 = decoder1.decode_all(&stream1, &mut out1).expect("decode stream 1");
    assert_eq!(n1, stream1_data.len());
    assert_eq!(out1, stream1_data);
    assert!(decoder1.is_eos());

    // Stream 2 decoding on a fresh decoder
    let mut stream2 = Vec::new();
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream2, stream2_data, true);
    Lzma2ChunkHeader::write_eos(&mut stream2);

    let mut decoder2 = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut out2 = Vec::new();
    let n2 = decoder2.decode_all(&stream2, &mut out2).expect("decode stream 2");
    assert_eq!(n2, stream2_data.len());
    assert_eq!(out2, stream2_data);
}

#[test]
fn test_lzma2_boundary_cases_empty_single_byte_64k() {
    // 1. Empty Stream (only EOS)
    let mut empty_stream = Vec::new();
    Lzma2ChunkHeader::write_eos(&mut empty_stream);

    let mut dec = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut out = Vec::new();
    let n = dec.decode_all(&empty_stream, &mut out).expect("empty stream decode");
    assert_eq!(n, 0);
    assert!(out.is_empty());
    assert!(dec.is_eos());

    // 2. Single Byte Uncompressed
    let single_byte = [0x42u8];
    let mut single_stream = Vec::new();
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut single_stream, &single_byte, true);
    Lzma2ChunkHeader::write_eos(&mut single_stream);

    let mut dec = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut out = Vec::new();
    let n = dec.decode_all(&single_stream, &mut out).expect("single byte decode");
    assert_eq!(n, 1);
    assert_eq!(out, vec![0x42]);

    // 3. 64 KiB Uncompressed Chunk (maximum size for 0x01/0x02)
    let big_data = vec![0x77u8; 65536];
    let mut big_stream = Vec::new();
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut big_stream, &big_data, true);
    Lzma2ChunkHeader::write_eos(&mut big_stream);

    let mut dec = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut out = Vec::new();
    let n = dec.decode_all(&big_stream, &mut out).expect("64k chunk decode");
    assert_eq!(n, 65536);
    assert_eq!(out, big_data);
}

#[test]
fn test_lzma2_sliding_dictionary_defensive_bounds() {
    let mut dict = Lzma2Dict::new(1024);
    assert!(dict.is_empty());
    assert_eq!(dict.len(), 0);
    assert_eq!(dict.last_byte(), 0);

    // Reading distance when empty must fail
    assert!(dict.get_byte_at_distance(0).is_err());
    assert!(dict.get_byte_at_distance(10).is_err());

    // Add some bytes
    dict.put_byte(b'A');
    dict.put_byte(b'B');
    dict.put_byte(b'C');

    assert_eq!(dict.len(), 3);
    assert_eq!(dict.last_byte(), b'C');
    assert_eq!(dict.get_byte_at_distance(0).unwrap(), b'C'); // distance 1 = 'C'
    assert_eq!(dict.get_byte_at_distance(1).unwrap(), b'B'); // distance 2 = 'B'
    assert_eq!(dict.get_byte_at_distance(2).unwrap(), b'A'); // distance 3 = 'A'
    assert!(dict.get_byte_at_distance(3).is_err()); // distance 4 > len (3)

    // Reset clears history
    dict.reset();
    assert!(dict.is_empty());
    assert_eq!(dict.len(), 0);
    assert!(dict.get_byte_at_distance(0).is_err());
}

#[test]
fn test_lzma2_truncated_payload_error_detection() {
    // Header indicates 100 bytes uncompressed payload, but only 10 bytes provided
    let mut stream = vec![0x01, 0x00, 0x63]; // 0x0063 = 99 + 1 = 100 bytes
    stream.extend_from_slice(&[0x55u8; 10]); // only 10 bytes

    let mut dec = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut out = Vec::new();
    let err = dec.decode_all(&stream, &mut out).unwrap_err();
    assert_eq!(
        err,
        Lzma2DecodeError::TruncatedPayload {
            expected: 100,
            available: 10,
        }
    );
}

#[test]
fn test_lzma2_compressed_match_rle_roundtrip() {
    // 100 consecutive 'Z' characters: 1 literal + 1 match of length 99 at distance 1
    let raw = vec![b'Z'; 100];
    let props = LiteralProperties::default();
    let mut enc = RangeEncoder::new();
    let mut probs = LzmaProbTable::new(props);
    let mut state = LzmaState::default(); // State0 (LitLit)
    let mut payload = Vec::new();

    // 1. First byte as Literal 'Z'
    let pos_state0 = props.pos_state(0);
    enc.encode_bit(&mut probs.is_match[state.as_usize()][pos_state0], 0, &mut payload);
    let sub = probs.literal_sub_table_mut(0, 0);
    enc.encode_literal_byte(sub, b'Z', &mut payload);
    state = state.update_literal(); // -> State0

    // 2. Remaining 99 bytes as Match (distance = 1, i.e. rep0 = 0, len = 99)
    let pos_state1 = props.pos_state(1);
    // is_match = 1
    enc.encode_bit(&mut probs.is_match[state.as_usize()][pos_state1], 1, &mut payload);
    // is_rep = 0 (simple match)
    enc.encode_bit(&mut probs.is_rep[state.as_usize()], 0, &mut payload);

    // Length = 99 (base 2 + 8 + 8 + 81)
    let len = 99usize;
    enc.encode_bit(&mut probs.len_coder.choice1, 1, &mut payload);
    enc.encode_bit(&mut probs.len_coder.choice2, 1, &mut payload);
    let high_sym = (len - 18) as u32; // 81
    enc.encode_bit_tree(&mut probs.len_coder.high, high_sym, 8, &mut payload);

    // Distance slot: for distance 1 (rep0 = 0), slot = 0
    let len_to_pos_state = (len.min(4) - 2).min(3); // 2
    enc.encode_bit_tree(&mut probs.pos_slot[len_to_pos_state], 0, 6, &mut payload);

    state = state.update_match(); // -> State7 (LitMatch)
    enc.finish(&mut payload);

    // Assemble chunk
    let mut stream = Vec::new();
    let mode = 3u8; // reset all
    let unpack_minus_1 = (raw.len() - 1) as u32;
    let pack_minus_1 = (payload.len() - 1) as u16;
    let control = 0x80 | (mode << 5) | ((unpack_minus_1 >> 16) as u8 & 0x1F);
    stream.push(control);
    stream.extend_from_slice(&((unpack_minus_1 & 0xFFFF) as u16).to_be_bytes());
    stream.extend_from_slice(&pack_minus_1.to_be_bytes());
    stream.push(props.to_byte());
    stream.extend_from_slice(&payload);
    Lzma2ChunkHeader::write_eos(&mut stream);

    let mut decoder = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut decompressed = Vec::new();
    let n = decoder
        .decode_all(&stream, &mut decompressed)
        .expect("decode rle match");

    assert_eq!(n, 100);
    assert_eq!(decompressed, raw);
    assert_eq!(decoder.current_state(), state); // State7 (LitMatch)
    assert_eq!(decoder.repeat_distances()[0], 0); // rep0 = 0 (distance 1)
    assert!(decoder.is_eos());
}

#[test]
fn test_lzma2_decompress_stream_slice_api() {
    let data = b"Chunk 1 payload test for incremental stream interface.";
    let mut stream = Vec::new();
    Lzma2ChunkHeader::write_uncompressed_chunk(&mut stream, data, true);
    Lzma2ChunkHeader::write_eos(&mut stream);

    let mut dec = Lzma2StreamDecoder::new(LZMA2_DEFAULT_DICT_SIZE);
    let mut out_buf = vec![0u8; 128];
    let (in_consumed, out_produced, is_eos) = dec
        .decompress_stream(&stream, &mut out_buf)
        .expect("decompress stream slice");

    assert_eq!(out_produced, data.len());
    assert_eq!(&out_buf[..out_produced], data);
    assert!(is_eos);
    assert_eq!(in_consumed, stream.len());
}
