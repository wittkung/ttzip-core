// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for Snappy framing format chunk header parsing,
//! chunk types, bounds verification, and FSM state machine transitions.

use ttzip_engine::codecs::snappy::error::SnappyError;
use ttzip_engine::codecs::snappy::frame::{
    is_framed_snappy, validate_stream_identifier, SnappyChunkHeader, SnappyChunkType,
    SnappyFrameFsm, SnappyFrameFsmState, CHUNK_HEADER_SIZE, CRC_SIZE,
    MAX_CHUNK_PAYLOAD_SIZE, MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE, MAX_UNCOMPRESSED_CHUNK_SIZE,
    SNAPPY_MAX_CHUNK_SIZE, SNAPPY_STREAM_IDENTIFIER, STREAM_IDENTIFIER_CHUNK,
    STREAM_IDENTIFIER_MAGIC,
};

#[test]
fn test_snappy_stream_identifier_constants_and_validation() {
    // 1. Exact wire representation of 10-byte stream identifier chunk
    assert_eq!(
        STREAM_IDENTIFIER_CHUNK,
        [0xff, 0x06, 0x00, 0x00, 0x73, 0x4e, 0x61, 0x50, 0x70, 0x59],
        "Stream identifier chunk wire format must match framing spec"
    );
    assert_eq!(
        STREAM_IDENTIFIER_MAGIC,
        [0x73, 0x4e, 0x61, 0x50, 0x70, 0x59],
        "Magic payload must be 'sNaPpY'"
    );
    assert_eq!(STREAM_IDENTIFIER_CHUNK[0], 0xff); // Chunk type 0xff
    assert_eq!(STREAM_IDENTIFIER_CHUNK[1..4], [0x06, 0x00, 0x00]); // 3-byte LE length = 6
    assert_eq!(&STREAM_IDENTIFIER_CHUNK[4..], &STREAM_IDENTIFIER_MAGIC);
    assert_eq!(SNAPPY_STREAM_IDENTIFIER, STREAM_IDENTIFIER_CHUNK);

    // 2. Validate valid stream identifier
    assert_eq!(
        validate_stream_identifier(&STREAM_IDENTIFIER_CHUNK),
        Ok(10)
    );
    assert!(is_framed_snappy(&STREAM_IDENTIFIER_CHUNK));

    // 3. Validate stream identifier with trailing chunk bytes
    let stream_with_tail = [
        STREAM_IDENTIFIER_CHUNK.as_slice(),
        &[0x01, 0x05, 0x00, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE],
    ]
    .concat();
    assert_eq!(validate_stream_identifier(&stream_with_tail), Ok(10));
    assert!(is_framed_snappy(&stream_with_tail));

    // 4. Truncated input slices (< 10 bytes) MUST return UnexpectedEof
    for len in 0..10 {
        let truncated = &STREAM_IDENTIFIER_CHUNK[..len];
        assert_eq!(
            validate_stream_identifier(truncated),
            Err(SnappyError::UnexpectedEof),
            "Truncated slice of length {} must return UnexpectedEof",
            len
        );
        assert!(!is_framed_snappy(truncated));
    }

    // 5. Corrupted magic header MUST return InvalidMagicHeader
    let mut corrupted = STREAM_IDENTIFIER_CHUNK;
    corrupted[0] = 0xfe; // Wrong chunk type
    assert_eq!(
        validate_stream_identifier(&corrupted),
        Err(SnappyError::InvalidMagicHeader)
    );
    assert!(!is_framed_snappy(&corrupted));

    corrupted = STREAM_IDENTIFIER_CHUNK;
    corrupted[4] = b'S'; // Case mismatch in magic
    assert_eq!(
        validate_stream_identifier(&corrupted),
        Err(SnappyError::InvalidMagicHeader)
    );
    assert!(!is_framed_snappy(&corrupted));
}

#[test]
fn test_snappy_chunk_type_all_categories() {
    // 1. Compressed chunk (0x00)
    let comp = SnappyChunkType::from_u8(0x00);
    assert_eq!(comp, SnappyChunkType::Compressed);
    assert_eq!(comp.as_u8(), 0x00);
    assert!(!comp.is_skippable());
    assert!(!comp.is_unskippable());

    // 2. Uncompressed chunk (0x01)
    let uncomp = SnappyChunkType::from_u8(0x01);
    assert_eq!(uncomp, SnappyChunkType::Uncompressed);
    assert_eq!(uncomp.as_u8(), 0x01);
    assert!(!uncomp.is_skippable());
    assert!(!uncomp.is_unskippable());

    // 3. Padding chunk (0xfe)
    let padding = SnappyChunkType::from_u8(0xfe);
    assert_eq!(padding, SnappyChunkType::Padding);
    assert_eq!(padding.as_u8(), 0xfe);
    assert!(padding.is_skippable());
    assert!(!padding.is_unskippable());

    // 4. Stream identifier chunk (0xff)
    let ident = SnappyChunkType::from_u8(0xff);
    assert_eq!(ident, SnappyChunkType::StreamIdentifier);
    assert_eq!(ident.as_u8(), 0xff);
    assert!(ident.is_skippable());
    assert!(!ident.is_unskippable());

    // 5. Reserved unskippable chunks (0x02..=0x7f)
    for tag in 0x02..=0x7f {
        let chunk = SnappyChunkType::from_u8(tag);
        assert_eq!(chunk, SnappyChunkType::ReservedUnskippable(tag));
        assert_eq!(chunk.as_u8(), tag);
        assert!(!chunk.is_skippable());
        assert!(chunk.is_unskippable());
    }

    // 6. Reserved skippable chunks (0x80..=0xfd)
    for tag in 0x80..=0xfd {
        let chunk = SnappyChunkType::from_u8(tag);
        assert_eq!(chunk, SnappyChunkType::ReservedSkippable(tag));
        assert_eq!(chunk.as_u8(), tag);
        assert!(chunk.is_skippable());
        assert!(!chunk.is_unskippable());
    }

    // 7. Reversibility across entire 0x00..=0xFF space
    for byte in 0u8..=255 {
        let chunk_type = SnappyChunkType::from_u8(byte);
        assert_eq!(
            chunk_type.as_u8(),
            byte,
            "Reversibility failed for byte 0x{:02X}",
            byte
        );
    }
}

#[test]
fn test_snappy_chunk_header_parse_and_emit_roundtrip() {
    assert_eq!(CHUNK_HEADER_SIZE, 4);
    assert_eq!(CRC_SIZE, 4);

    // 1. Compressed chunk header roundtrip
    let header_comp = SnappyChunkHeader::new(SnappyChunkType::Compressed, 1024);
    let bytes_comp = header_comp.to_bytes();
    assert_eq!(bytes_comp, [0x00, 0x00, 0x04, 0x00]); // len = 1024 LE
    let parsed_comp = SnappyChunkHeader::parse(&bytes_comp).expect("parse compressed header");
    assert_eq!(parsed_comp, header_comp);

    // 2. Uncompressed chunk header roundtrip (64KB payload + 4 bytes CRC = 65540)
    let header_uncomp =
        SnappyChunkHeader::new(SnappyChunkType::Uncompressed, MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE);
    let bytes_uncomp = header_uncomp.to_bytes();
    assert_eq!(bytes_uncomp, [0x01, 0x04, 0x00, 0x01]); // 65540 = 0x00010004 LE
    let parsed_uncomp =
        SnappyChunkHeader::parse(&bytes_uncomp).expect("parse uncompressed header");
    assert_eq!(parsed_uncomp, header_uncomp);

    // 3. Padding chunk header
    let header_pad = SnappyChunkHeader::new(SnappyChunkType::Padding, 512);
    let mut dst = [0u8; 4];
    header_pad.emit(&mut dst);
    assert_eq!(dst, [0xfe, 0x00, 0x02, 0x00]);
    let parsed_pad = SnappyChunkHeader::parse(&dst).expect("parse padding header");
    assert_eq!(parsed_pad, header_pad);

    // 4. Stream identifier chunk header (must be 6 bytes payload)
    let header_ident = SnappyChunkHeader::new(SnappyChunkType::StreamIdentifier, 6);
    let bytes_ident = header_ident.to_bytes();
    assert_eq!(bytes_ident, [0xff, 0x06, 0x00, 0x00]);
    let parsed_ident = SnappyChunkHeader::parse(&bytes_ident).expect("parse stream identifier header");
    assert_eq!(parsed_ident, header_ident);
}

#[test]
fn test_snappy_chunk_payload_bounds_and_overflow_guards() {
    assert_eq!(MAX_UNCOMPRESSED_CHUNK_SIZE, 65536);
    assert_eq!(SNAPPY_MAX_CHUNK_SIZE, 65536);
    assert_eq!(MAX_UNCOMPRESSED_CHUNK_PAYLOAD_SIZE, 65540);
    assert_eq!(MAX_CHUNK_PAYLOAD_SIZE, 16_777_215);

    // 1. Legal boundary: exactly 65540 bytes for uncompressed chunk
    let legal_max_uncomp = [0x01, 0x04, 0x00, 0x01]; // 65540
    assert!(SnappyChunkHeader::parse(&legal_max_uncomp).is_ok());

    // 2. Illegal boundary: 65541 bytes for uncompressed chunk (exceeds 64KB + 4)
    let illegal_uncomp = [0x01, 0x05, 0x00, 0x01]; // 65541
    assert_eq!(
        SnappyChunkHeader::parse(&illegal_uncomp),
        Err(SnappyError::BlockTooLarge {
            size: 65541,
            max: 65540,
        })
    );

    // 3. Uncompressed payload smaller than 4-byte CRC-32C
    let too_short_uncomp = [0x01, 0x03, 0x00, 0x00]; // 3 bytes
    assert!(matches!(
        SnappyChunkHeader::parse(&too_short_uncomp),
        Err(SnappyError::CorruptHeader(_))
    ));

    // 4. Compressed payload smaller than 4-byte CRC-32C
    let too_short_comp = [0x00, 0x02, 0x00, 0x00]; // 2 bytes
    assert!(matches!(
        SnappyChunkHeader::parse(&too_short_comp),
        Err(SnappyError::CorruptHeader(_))
    ));

    // 5. Stream identifier payload length != 6
    let invalid_ident_len = [0xff, 0x05, 0x00, 0x00]; // 5 bytes instead of 6
    assert!(matches!(
        SnappyChunkHeader::parse(&invalid_ident_len),
        Err(SnappyError::CorruptHeader(_))
    ));

    let zero_ident_len = [0xff, 0x00, 0x00, 0x00]; // 0 bytes
    assert!(matches!(
        SnappyChunkHeader::parse(&zero_ident_len),
        Err(SnappyError::CorruptHeader(_))
    ));

    // 6. Max 24-bit payload limit (16,777,215) for skippable / padding
    let max_pad = [0xfe, 0xff, 0xff, 0xff]; // 16,777,215
    let parsed_pad = SnappyChunkHeader::parse(&max_pad).expect("parse max padding");
    assert_eq!(parsed_pad.payload_len, 16_777_215);
}

#[test]
fn test_snappy_reserved_unskippable_vs_skippable_handling() {
    // 1. Reserved Unskippable chunks MUST produce SnappyError::UnsupportedChunkType
    for &tag in &[0x02, 0x03, 0x10, 0x3F, 0x7E, 0x7F] {
        let raw = [tag, 0x10, 0x00, 0x00]; // len = 16
        assert_eq!(
            SnappyChunkHeader::parse(&raw),
            Err(SnappyError::UnsupportedChunkType(tag)),
            "Reserved unskippable chunk 0x{:02X} must return UnsupportedChunkType",
            tag
        );
    }

    // 2. Reserved Skippable chunks MUST be successfully parsed and marked skippable
    for &tag in &[0x80, 0x81, 0xA0, 0xCC, 0xFC, 0xFD] {
        let raw = [tag, 0x20, 0x00, 0x00]; // len = 32
        let header = SnappyChunkHeader::parse(&raw).expect("skippable chunk should parse");
        assert_eq!(header.chunk_type, SnappyChunkType::ReservedSkippable(tag));
        assert_eq!(header.payload_len, 32);
        assert!(header.chunk_type.is_skippable());
    }
}

#[test]
fn test_snappy_chunk_header_zero_panic_exhaustive_fuzz() {
    // Test all 256 tag values against various length configurations
    let lengths: [usize; 9] = [0, 1, 3, 4, 6, 1024, 65536, 65540, 65541];

    for tag in 0u8..=255 {
        for &len in &lengths {
            let mut wire = [0u8; 4];
            wire[0] = tag;
            wire[1] = (len & 0xFF) as u8;
            wire[2] = ((len >> 8) & 0xFF) as u8;
            wire[3] = ((len >> 16) & 0xFF) as u8;

            // Must never panic regardless of byte combinations
            let _ = SnappyChunkHeader::parse(&wire);
        }
    }
}

#[test]
fn test_snappy_frame_fsm_lifecycle_transitions() {
    let mut fsm = SnappyFrameFsm::new();
    assert_eq!(fsm.state(), SnappyFrameFsmState::ExpectIdentifier);
    assert!(!fsm.has_seen_identifier());
    assert_eq!(fsm.current_header(), None);

    // 1. Feeding chunk header when expecting stream identifier MUST fail
    let header_bytes = [0x01, 0x04, 0x00, 0x00];
    assert_eq!(
        fsm.feed_header(&header_bytes),
        Err(SnappyError::InvalidMagicHeader)
    );

    // 2. Feed valid stream identifier -> transitions to ReadChunkHeader
    let consumed = fsm
        .feed_identifier(&STREAM_IDENTIFIER_CHUNK)
        .expect("feed valid stream identifier");
    assert_eq!(consumed, 10);
    assert_eq!(fsm.state(), SnappyFrameFsmState::ReadChunkHeader);
    assert!(fsm.has_seen_identifier());

    // 3. Feed chunk header (Compressed, payload 100 bytes) -> transitions to ProcessPayload
    let comp_header_bytes = [0x00, 0x64, 0x00, 0x00];
    let header = fsm
        .feed_header(&comp_header_bytes)
        .expect("feed valid chunk header");
    assert_eq!(header.chunk_type, SnappyChunkType::Compressed);
    assert_eq!(header.payload_len, 100);
    assert_eq!(fsm.state(), SnappyFrameFsmState::ProcessPayload);
    assert_eq!(fsm.current_header(), Some(header));

    // 4. Cannot feed another header while in ProcessPayload
    assert!(matches!(
        fsm.feed_header(&comp_header_bytes),
        Err(SnappyError::InvalidParam(_))
    ));

    // 5. Complete payload processing -> transitions back to ReadChunkHeader
    fsm.finish_payload().expect("finish payload");
    assert_eq!(fsm.state(), SnappyFrameFsmState::ReadChunkHeader);
    assert_eq!(fsm.current_header(), None);

    // 6. Concatenated stream: secondary stream identifier is accepted cleanly
    let sec_consumed = fsm
        .feed_identifier(&STREAM_IDENTIFIER_CHUNK)
        .expect("secondary stream identifier");
    assert_eq!(sec_consumed, 10);
    assert_eq!(fsm.state(), SnappyFrameFsmState::ReadChunkHeader);

    // 7. Clean end-of-stream
    fsm.finish_stream().expect("clean stream finish");
    assert_eq!(fsm.state(), SnappyFrameFsmState::Done);

    // 8. Cannot feed header into Done FSM
    assert!(matches!(
        fsm.feed_header(&comp_header_bytes),
        Err(SnappyError::InvalidParam(_))
    ));

    // 9. Reset FSM
    fsm.reset();
    assert_eq!(fsm.state(), SnappyFrameFsmState::ExpectIdentifier);
    assert!(!fsm.has_seen_identifier());
}
