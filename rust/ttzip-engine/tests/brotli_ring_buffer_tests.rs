// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit and invariant tests for Brotli ring buffer and meta-block state machine.

use ttzip_engine::codecs::brotli::{
    BrotliBitReader, BrotliDecoderFsmState, BrotliDecoderRingBuffer, BrotliError, MetaBlockHeader,
    MetaBlockType, RING_BUFFER_WRITE_AHEAD_SLACK,
};

/// Helper to pack LSB-first bit sequences into a byte vector.
fn pack_lsb_bits(chunks: &[(u32, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u64;
    let mut count = 0u32;
    for &(val, len) in chunks {
        let mask = if len >= 32 {
            0xFFFF_FFFF
        } else {
            (1u32 << len) - 1
        };
        acc |= ((val & mask) as u64) << count;
        count += len;
        while count >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            count -= 8;
        }
    }
    if count > 0 {
        out.push((acc & 0xFF) as u8);
    }
    out
}

#[test]
fn test_ring_buffer_creation_and_geometry() {
    let rb = BrotliDecoderRingBuffer::new(10).expect("window bits 10");
    assert_eq!(rb.window_bits(), 10);
    assert_eq!(rb.size(), 1024);
    assert_eq!(rb.mask(), 1023);
    assert_eq!(rb.total_capacity(), 1024 + RING_BUFFER_WRITE_AHEAD_SLACK);
    assert_eq!(rb.pos(), 0);
    assert_eq!(rb.tail(), 0);
    assert_eq!(rb.available_data(), 0);
    assert!(rb.is_empty());

    // Invalid window bits (< 10 or > 30)
    assert!(BrotliDecoderRingBuffer::new(9).is_err());
    assert!(BrotliDecoderRingBuffer::new(31).is_err());

    // With explicit size
    let rb2 = BrotliDecoderRingBuffer::with_size(4096).expect("size 4096");
    assert_eq!(rb2.window_bits(), 12);
    assert_eq!(rb2.size(), 4096);
    assert!(BrotliDecoderRingBuffer::with_size(1000).is_err());
}

#[test]
fn test_ring_buffer_write_byte_and_wrap_around() {
    let mut rb = BrotliDecoderRingBuffer::new(10).expect("window bits 10");

    // Write 2500 bytes (> 2 full window rotations)
    for i in 0..2500 {
        let byte = (i % 251) as u8;
        rb.write_byte(byte);
        assert_eq!(rb.pos(), i + 1);
        assert_eq!(rb.get_byte_at(i), byte);
        assert_eq!(rb.get_recent_byte(1), Some(byte));
    }

    assert_eq!(rb.available_data(), 2500);
    assert!(!rb.is_empty());

    // Buffer array index 0 should contain byte at index 2048 (2048 % 1024 == 0)
    assert_eq!(rb.buffer[0], (2048 % 251) as u8);
}

#[test]
fn test_ring_buffer_lz77_run_length_overlap_expansion() {
    let mut rb = BrotliDecoderRingBuffer::new(10).expect("window bits 10");

    // Write initial seed: "A"
    rb.write_byte(b'A');

    // Run-length copy with distance = 1, length = 15 -> yields 16 'A's total
    rb.copy_match(1, 15).expect("run-length expand 'A'");
    assert_eq!(rb.pos(), 16);

    let mut out = vec![0u8; 16];
    let drained = rb.drain_to(&mut out);
    assert_eq!(drained, 16);
    assert_eq!(out, vec![b'A'; 16]);

    // Seed "XYZ"
    rb.copy_slice(b"XYZ");
    // Overlapping copy distance = 3, length = 9 -> "XYZXYZXYZ"
    rb.copy_match(3, 9).expect("pattern repeat 'XYZ'");
    assert_eq!(rb.pos(), 16 + 3 + 9);

    let mut out_pattern = vec![0u8; 12];
    let drained_pat = rb.drain_to(&mut out_pattern);
    assert_eq!(drained_pat, 12);
    assert_eq!(&out_pattern, b"XYZXYZXYZXYZ");
}

#[test]
fn test_ring_buffer_lz77_copy_across_window_boundary() {
    let mut rb = BrotliDecoderRingBuffer::new(10).expect("window bits 10");

    // Fill up to position 1020 (4 bytes before boundary)
    let fill = vec![0x55u8; 1020];
    rb.copy_slice(&fill);
    assert_eq!(rb.pos(), 1020);

    // Write distinctive 8-byte pattern
    rb.copy_slice(b"ABCDEFGH");
    // pos is now 1028 (crosses 1024 boundary)
    assert_eq!(rb.pos(), 1028);

    // Copy match from distance 8, length 8 (repeats "ABCDEFGH" spanning across boundary)
    rb.copy_match(8, 8).expect("cross boundary copy match");
    assert_eq!(rb.pos(), 1036);

    // Verify recent bytes
    let mut drained = vec![0u8; 1036];
    let n = rb.drain_to(&mut drained);
    assert_eq!(n, 1036);
    assert_eq!(&drained[1020..1028], b"ABCDEFGH");
    assert_eq!(&drained[1028..1036], b"ABCDEFGH");
}

#[test]
fn test_ring_buffer_drain_wrapping_and_chunks() {
    let mut rb = BrotliDecoderRingBuffer::new(10).expect("window bits 10");

    // Advance pos to 1000 and drain 1000
    let dummy = vec![0x11u8; 1000];
    rb.copy_slice(&dummy);
    let mut sink = vec![0u8; 1000];
    assert_eq!(rb.drain_to(&mut sink), 1000);
    assert_eq!(rb.tail(), 1000);
    assert!(rb.is_empty());

    // Write 50 bytes: spans from tail 1000 to pos 1050 (wrapping across 1024)
    let test_data: Vec<u8> = (0..50).map(|i| i as u8).collect();
    rb.copy_slice(&test_data);
    assert_eq!(rb.available_data(), 50);

    // Drain into small buffer (first chunk 24 bytes, second chunk 26 bytes)
    let mut out = vec![0u8; 50];
    let drained = rb.drain_to(&mut out);
    assert_eq!(drained, 50);
    assert_eq!(out, test_data);
    assert_eq!(rb.tail(), 1050);
    assert!(rb.is_empty());
}

#[test]
fn test_ring_buffer_invalid_backward_distance_rejection() {
    let mut rb = BrotliDecoderRingBuffer::new(10).expect("window bits 10");
    rb.copy_slice(b"HELLO");

    // distance == 0
    let err0 = rb.copy_match(0, 5).expect_err("distance 0 must fail");
    assert!(matches!(err0, BrotliError::CorruptHeader(_)));

    // distance > pos (pos is 5, distance is 6)
    let err_pos = rb.copy_match(6, 2).expect_err("distance > pos must fail");
    assert!(matches!(err_pos, BrotliError::CorruptHeader(_)));

    // distance > window size (1025 > 1024)
    let large = vec![0u8; 2000];
    rb.copy_slice(&large);
    let err_win = rb
        .copy_match(1025, 2)
        .expect_err("distance > window_size must fail");
    assert!(matches!(err_win, BrotliError::CorruptHeader(_)));
}

#[test]
fn test_ring_buffer_reset() {
    let mut rb = BrotliDecoderRingBuffer::new(10).expect("window bits 10");
    rb.copy_slice(b"SOME_DATA");
    assert_eq!(rb.available_data(), 9);

    rb.reset();
    assert_eq!(rb.pos(), 0);
    assert_eq!(rb.tail(), 0);
    assert_eq!(rb.available_data(), 0);
    assert!(rb.is_empty());
    assert_eq!(rb.buffer[0], 0);
}

#[test]
fn test_meta_block_header_empty_last_block() {
    // ISLAST = 1 (1 bit '1'), ISLASTEMPTY = 1 (1 bit '1')
    let stream = pack_lsb_bits(&[(1, 1), (1, 1)]);
    let mut br = BrotliBitReader::new(&stream);

    let header = MetaBlockHeader::parse(&mut br).expect("parse empty metablock");
    assert!(header.is_last);
    assert!(header.is_last_empty);
    assert_eq!(header.block_type, MetaBlockType::Empty);
    assert_eq!(header.uncompressed_len, 0);
}

#[test]
fn test_meta_block_header_uncompressed() {
    // Non-last uncompressed block:
    // ISLAST = 0 (1 bit '0')
    // MNIBBLES = 0 (2 bits '00' -> size_nibbles = 4)
    // MLEN nibbles = 0x00FF (4 nibbles: 0xF, 0xF, 0x0, 0x0 -> mlen = 255 -> uncompressed_len = 256)
    // ISUNCOMPRESSED = 1 (1 bit '1')
    // Padding = 0 (7 bits '0000000' to byte boundary)
    // Followed by raw byte 0x42
    let stream = pack_lsb_bits(&[
        (0, 1),  // ISLAST = 0
        (0, 2),  // MNIBBLES = 0 (size_nibbles = 4)
        (15, 4), // nibble 0: 0xF
        (15, 4), // nibble 1: 0xF
        (0, 4),  // nibble 2: 0x0
        (0, 4),  // nibble 3: 0x0 (mlen = 255)
        (1, 1),  // ISUNCOMPRESSED = 1
        (0, 4),  // zero padding to byte boundary (20 + 4 = 24 bits = 3 bytes)
        (0x42, 8),
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let header = MetaBlockHeader::parse(&mut br).expect("parse uncompressed metablock");

    assert!(!header.is_last);
    assert!(!header.is_last_empty);
    assert_eq!(header.block_type, MetaBlockType::Uncompressed);
    assert_eq!(header.uncompressed_len, 256);

    // Bit reader should now be aligned to read 0x42
    assert_eq!(br.read_byte().expect("read raw byte"), 0x42);
}

#[test]
fn test_meta_block_header_compressed_non_last() {
    // Non-last compressed block:
    // ISLAST = 0 (1 bit '0')
    // MNIBBLES = 1 (2 bits '01' -> size_nibbles = 5)
    // MLEN nibbles = 0x12345 (5 nibbles: 5, 4, 3, 2, 1 -> mlen = 0x12345 = 74565 -> uncompressed_len = 74566)
    // ISUNCOMPRESSED = 0 (1 bit '0')
    let stream = pack_lsb_bits(&[
        (0, 1), // ISLAST = 0
        (1, 2), // MNIBBLES = 1 (size_nibbles = 5)
        (5, 4), // nibble 0
        (4, 4), // nibble 1
        (3, 4), // nibble 2
        (2, 4), // nibble 3
        (1, 4), // nibble 4 (highest non-zero nibble)
        (0, 1), // ISUNCOMPRESSED = 0
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let header = MetaBlockHeader::parse(&mut br).expect("parse compressed non-last");

    assert!(!header.is_last);
    assert!(!header.is_last_empty);
    assert_eq!(header.block_type, MetaBlockType::Compressed);
    assert_eq!(header.uncompressed_len, 74566);
}

#[test]
fn test_meta_block_header_compressed_last_non_empty() {
    // Last non-empty compressed block:
    // ISLAST = 1 (1 bit '1')
    // ISLASTEMPTY = 0 (1 bit '0')
    // MNIBBLES = 0 (2 bits '00' -> size_nibbles = 4)
    // MLEN nibbles = 0x000F (4 nibbles: 15, 0, 0, 0 -> mlen = 15 -> uncompressed_len = 16)
    // No ISUNCOMPRESSED bit present
    let stream = pack_lsb_bits(&[
        (1, 1),  // ISLAST = 1
        (0, 1),  // ISLASTEMPTY = 0
        (0, 2),  // MNIBBLES = 0
        (15, 4), // nibble 0
        (0, 4),  // nibble 1
        (0, 4),  // nibble 2
        (0, 4),  // nibble 3
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let header = MetaBlockHeader::parse(&mut br).expect("parse compressed last");

    assert!(header.is_last);
    assert!(!header.is_last_empty);
    assert_eq!(header.block_type, MetaBlockType::Compressed);
    assert_eq!(header.uncompressed_len, 16);
}

#[test]
fn test_meta_block_header_metadata() {
    // Metadata block:
    // ISLAST = 0 (1 bit '0')
    // MNIBBLES = 3 (2 bits '11')
    // RESERVED = 0 (1 bit '0')
    // MSKIPBYTES = 2 (2 bits '10' -> 2 length bytes)
    // Length bytes: 0x34, 0x12 -> mlen = 0x1234 = 4660 -> uncompressed_len = 4661
    // Padding = 0 (2 bits '00' to byte boundary)
    // Followed by raw byte 0x99
    let stream = pack_lsb_bits(&[
        (0, 1),    // ISLAST = 0
        (3, 2),    // MNIBBLES = 3 (Metadata)
        (0, 1),    // RESERVED = 0
        (2, 2),    // MSKIPBYTES = 2
        (0x34, 8), // byte 0
        (0x12, 8), // byte 1 (highest non-zero)
        (0, 2),    // padding to byte boundary
        (0x99, 8),
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let header = MetaBlockHeader::parse(&mut br).expect("parse metadata");

    assert!(!header.is_last);
    assert!(!header.is_last_empty);
    assert_eq!(header.block_type, MetaBlockType::Metadata);
    assert_eq!(header.uncompressed_len, 4661);
    assert_eq!(br.read_byte().expect("read byte"), 0x99);
}

#[test]
fn test_meta_block_exuberant_nibble_rejection() {
    // Exuberant nibble: size_nibbles = 5 (MNIBBLES = 1), but highest nibble is 0
    let stream = pack_lsb_bits(&[
        (0, 1), // ISLAST = 0
        (1, 2), // MNIBBLES = 1 (size_nibbles = 5)
        (1, 4), // nibble 0
        (2, 4), // nibble 1
        (3, 4), // nibble 2
        (4, 4), // nibble 3
        (0, 4), // nibble 4 = 0 (EXUBERANT NIBBLE VIOLATION)
        (0, 1), // ISUNCOMPRESSED
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let err = MetaBlockHeader::parse(&mut br).expect_err("must reject exuberant nibble");
    assert!(matches!(err, BrotliError::CorruptHeader(_)));
}

#[test]
fn test_meta_block_exuberant_meta_byte_rejection() {
    // Exuberant meta byte: MSKIPBYTES = 2, but highest byte is 0
    let stream = pack_lsb_bits(&[
        (0, 1), // ISLAST = 0
        (3, 2), // MNIBBLES = 3
        (0, 1), // RESERVED = 0
        (2, 2), // MSKIPBYTES = 2
        (0x55, 8),
        (0x00, 8), // Highest byte is 0 (EXUBERANT META BYTE VIOLATION)
        (0, 2),    // Padding
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let err = MetaBlockHeader::parse(&mut br).expect_err("must reject exuberant meta byte");
    assert!(matches!(err, BrotliError::CorruptHeader(_)));
}

#[test]
fn test_meta_block_invalid_reserved_bit_rejection() {
    // Non-zero reserved bit in metadata block header (RESERVED = 1)
    let stream = pack_lsb_bits(&[
        (0, 1), // ISLAST = 0
        (3, 2), // MNIBBLES = 3
        (1, 1), // RESERVED = 1 (INVALID RESERVED BIT)
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let err = MetaBlockHeader::parse(&mut br).expect_err("must reject reserved bit = 1");
    assert!(matches!(err, BrotliError::CorruptHeader(_)));
}

#[test]
fn test_meta_block_invalid_padding_rejection() {
    // Uncompressed block with non-zero padding bit before byte boundary
    let stream = pack_lsb_bits(&[
        (0, 1), // ISLAST = 0
        (0, 2), // MNIBBLES = 0 (size_nibbles = 4)
        (1, 4), // nibble 0
        (0, 4), // nibble 1
        (0, 4), // nibble 2
        (0, 4), // nibble 3
        (1, 1), // ISUNCOMPRESSED = 1
        // Total bits before jump: 1 + 2 + 16 + 1 = 20 bits -> 4 padding bits needed to reach 24
        (0b1000, 4), // NON-ZERO PADDING BITS (bit 3 is 1)
    ]);

    let mut br = BrotliBitReader::new(&stream);
    let err = MetaBlockHeader::parse(&mut br).expect_err("must reject non-zero padding");
    assert_eq!(err, BrotliError::InvalidPadding);
}

#[test]
fn test_brotli_fsm_state_predicates() {
    assert!(!BrotliDecoderFsmState::Init.is_done());
    assert!(!BrotliDecoderFsmState::ReadWindowBits.is_done());
    assert!(!BrotliDecoderFsmState::ReadMetaBlockHeader.is_done());
    assert!(BrotliDecoderFsmState::Done.is_done());

    assert!(BrotliDecoderFsmState::UncompressedData.is_processing_payload());
    assert!(BrotliDecoderFsmState::CompressedCommands.is_processing_payload());
    assert!(BrotliDecoderFsmState::MetadataSkip.is_processing_payload());
    assert!(!BrotliDecoderFsmState::Init.is_processing_payload());
    assert!(!BrotliDecoderFsmState::Done.is_processing_payload());
}
