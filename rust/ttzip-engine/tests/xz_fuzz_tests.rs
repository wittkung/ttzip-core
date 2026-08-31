// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed XZ Fault-Injection Fuzzing Harness & VLI Overflow Truncation Test Suite.
//!
//! Enforces rock-solid resilience and zero-panic guarantees across 7 critical failure dimensions:
//! 1. Residual Stream Header/Footer Truncation Injection (1..11B Header, Footer/Index truncations).
//! 2. Bad Stream Magic Injection (Header `\xFD7zXZ\x00` and Footer `YZ` bit-flips).
//! 3. Bad Header/Footer/Index/Block CRC32/CRC64/SHA-256 Checksum Injection (Single bit-flip defense).
//! 4. Corrupted Backward Size Injection (Reconciliation mismatches and out-of-bound alignments).
//! 5. Illegal Non-Zero Padding Injection (Block header, block data, index, and stream padding).
//! 6. Malformed VLI Overflow & Overlong Sequence Injection (>9B, 9th byte MSB, non-canonical encodings).
//! 7. RandomReader Quadratic Biased Micro-Slicing Chaos Jitter Reading (1..=7B micro-chunks streaming).

use std::io::{Cursor, Read};
use std::panic::{catch_unwind, AssertUnwindSafe};

use proptest::prelude::*;
use sha2::{Digest, Sha256};
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::xz::bcj::{BcjArm64, BcjX86, BranchFilter};
use ttzip_engine::xz::block::{
    XzBlockError, XzBlockHeader, XzFilterConfig, FILTER_ID_LZMA2,
};
use ttzip_engine::xz::checksum::{XzChecksumEngine, XzChecksumError, XzChecksumType};
use ttzip_engine::xz::decoder::{xz_decompress, XzStreamDecoder};
use ttzip_engine::xz::header::{
    XzStreamFlags, XzStreamFooter, XzStreamHeader,
};
use ttzip_engine::xz::index::XzStreamIndex;
use ttzip_engine::xz::seekable::XzSeekableReader;
use ttzip_engine::xz::types::{
    XzCheckType, XzError, XZ_FOOTER_MAGIC, XZ_HEADER_MAGIC, XZ_MAX_BACKWARD_SIZE,
    XZ_MIN_BACKWARD_SIZE,
};
use ttzip_engine::xz::vli::{
    decode_vli, decode_vli_stream, encode_vli, encode_vli_vec, vli_size, XzVliError,
    XZ_VLI_MAX,
};
use ttzip_engine::xz::writer::{
    xz_compress, XzBcjType, XzEncoderOptions, XzParallelStreamWriter,
};

// ============================================================================
// Test Utilities & Deterministic Generators
// ============================================================================

/// Computes lowercase hex SHA-256 digest string of data slice.
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Generates deterministic synthetic payload of specified size.
fn generate_deterministic_payload(size: usize, seed: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut state = seed;
    for i in 0..size {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let byte = ((state >> 16) ^ (i as u32)) as u8;
        out.push(byte);
    }
    out
}

/// Helper to compress data using single-block XZ encoder with specified check type.
fn create_single_block_xz(data: &[u8], check_type: XzCheckType) -> Vec<u8> {
    let options = XzEncoderOptions::new()
        .with_check_type(check_type)
        .with_dict_size(1024 * 1024)
        .with_preset_level(2);
    xz_compress(data, &options).expect("xz_compress failed")
}

/// Helper to compress data using multi-block parallel XZ writer.
fn create_multi_block_xz(
    data: &[u8],
    block_size: usize,
    check_type: XzCheckType,
    bcj: Option<XzBcjType>,
) -> Vec<u8> {
    let mut options = XzEncoderOptions::new()
        .with_check_type(check_type)
        .with_dict_size(1024 * 1024)
        .with_block_size(block_size)
        .with_preset_level(2);
    if let Some(b) = bcj {
        options = options.with_bcj(b);
    }
    let mut sink = Vec::new();
    let mut writer =
        XzParallelStreamWriter::new(&mut sink, options).expect("create parallel writer");
    writer.write_parallel(data).expect("write parallel");
    writer.finish().expect("finish parallel writer");
    sink
}

// ============================================================================
// Dimension 1: Residual Stream Header/Footer Truncation Injection
// ============================================================================

#[test]
fn test_dimension_1_stream_header_footer_truncation_injection() {
    let payload = generate_deterministic_payload(32 * 1024, 0x1A2B3C4D);
    let valid_xz = create_single_block_xz(&payload, XzCheckType::Crc32);
    assert!(valid_xz.len() >= 24);

    // 1.1 Truncate Stream Header at lengths 1..=11 bytes
    for trunc_len in 1..=11 {
        let truncated = &valid_xz[..trunc_len];

        // Streaming decoder must reject truncation without panic
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let mut decoder = XzStreamDecoder::new(Cursor::new(truncated));
            let mut out = Vec::new();
            decoder.read_to_end(&mut out)
        }));
        assert!(unwind_res.is_ok(), "Decoder panicked on header truncation {trunc_len}");
        let decode_res = unwind_res.unwrap();
        assert!(
            decode_res.is_err(),
            "Decoder must return Err on truncated header of length {trunc_len}"
        );

        // Raw header parser must fail safely
        let mut hdr_slice = [0u8; 12];
        hdr_slice[..trunc_len].copy_from_slice(truncated);
        if trunc_len < 6 {
            assert!(XzStreamHeader::parse(&hdr_slice).is_err());
        }
    }

    // 1.2 Truncate tail across Stream Footer, Index, and trailing Block
    for drop_bytes in 1..=24 {
        if valid_xz.len() <= drop_bytes {
            continue;
        }
        let truncated = &valid_xz[..valid_xz.len() - drop_bytes];

        // Streaming decoder must reject without panic
        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let mut decoder = XzStreamDecoder::new(Cursor::new(truncated));
            let mut out = Vec::new();
            decoder.read_to_end(&mut out)
        }));
        assert!(unwind_res.is_ok(), "Decoder panicked on tail drop {drop_bytes}");
        assert!(
            unwind_res.unwrap().is_err(),
            "Decoder must reject tail drop of {drop_bytes} bytes"
        );

        // Seekable reader must also reject without panic
        let seek_res = catch_unwind(AssertUnwindSafe(|| {
            XzSeekableReader::new(Cursor::new(truncated.to_vec()))
        }));
        assert!(seek_res.is_ok(), "Seekable reader panicked on tail drop {drop_bytes}");
        assert!(seek_res.unwrap().is_err());
    }

    // 1.3 Direct Index and Footer short buffer tests
    assert!(XzStreamIndex::parse(&[]).is_err());
    assert!(XzStreamIndex::parse(&[0x00, 0x01, 0x02]).is_err());
}

// ============================================================================
// Dimension 2: Bad Stream Magic Injection
// ============================================================================

#[test]
fn test_dimension_2_bad_stream_magic_injection() {
    let payload = generate_deterministic_payload(16 * 1024, 0x5EED0001);
    let valid_xz = create_single_block_xz(&payload, XzCheckType::Crc64);

    // 2.1 Mutate each of the 6 bytes of Stream Header Magic (\xFD7zXZ\x00)
    for magic_idx in 0..6 {
        let mut corrupted = valid_xz.clone();
        corrupted[magic_idx] ^= 0xFF;

        let unwind_res = catch_unwind(AssertUnwindSafe(|| {
            let mut decoder = XzStreamDecoder::new(Cursor::new(&corrupted));
            let mut out = Vec::new();
            decoder.read_to_end(&mut out)
        }));
        assert!(unwind_res.is_ok(), "Decoder panicked on header magic corruption at {magic_idx}");
        assert!(unwind_res.unwrap().is_err());

        let mut hdr_buf = [0u8; 12];
        hdr_buf.copy_from_slice(&corrupted[..12]);
        let parse_err = XzStreamHeader::parse(&hdr_buf).unwrap_err();
        match parse_err {
            XzError::InvalidHeaderMagic { expected, .. } => {
                assert_eq!(expected, XZ_HEADER_MAGIC);
            }
            other => panic!("Expected InvalidHeaderMagic, got {other:?}"),
        }
    }

    // 2.2 Mutate the 2 bytes of Stream Footer Magic ('YZ' = 0x59, 0x5A)
    let footer_offset = valid_xz.len() - 12;
    for &magic_idx in &[footer_offset + 10, footer_offset + 11] {
        let mut corrupted = valid_xz.clone();
        corrupted[magic_idx] ^= 0x55;

        // Streaming decoder reaches footer and rejects
        let mut decoder = XzStreamDecoder::new(Cursor::new(&corrupted));
        let mut out = Vec::new();
        let decode_err = decoder.read_to_end(&mut out);
        assert!(decode_err.is_err(), "Decoder must reject bad footer magic at {magic_idx}");

        // Seekable reader parses footer immediately and rejects
        let seek_err = XzSeekableReader::new(Cursor::new(corrupted.clone()));
        assert!(seek_err.is_err(), "Seekable reader must reject bad footer magic");

        // Direct footer parse
        let mut footer_buf = [0u8; 12];
        footer_buf.copy_from_slice(&corrupted[footer_offset..]);
        let footer_parse_err = XzStreamFooter::parse(&footer_buf).unwrap_err();
        match footer_parse_err {
            XzError::InvalidFooterMagic { expected, .. } => {
                assert_eq!(expected, XZ_FOOTER_MAGIC);
            }
            other => panic!("Expected InvalidFooterMagic, got {other:?}"),
        }
    }
}

// ============================================================================
// Dimension 3: Bad Checksum and CRC Integrity Defense Injection
// ============================================================================

#[test]
fn test_dimension_3_bad_crc_and_checksum_injection() {
    let payload = generate_deterministic_payload(24 * 1024, 0x98765432);

    // 3.1 Header CRC32 bit flip (bytes 8..12)
    let valid_xz = create_single_block_xz(&payload, XzCheckType::Crc32);
    let mut bad_header_crc = valid_xz.clone();
    bad_header_crc[8] ^= 0x01;

    let mut hdr_buf = [0u8; 12];
    hdr_buf.copy_from_slice(&bad_header_crc[..12]);
    let hdr_err = XzStreamHeader::parse(&hdr_buf).unwrap_err();
    assert!(
        matches!(hdr_err, XzError::HeaderCrcMismatch { .. }),
        "Expected HeaderCrcMismatch, got {hdr_err:?}"
    );
    assert!(xz_decompress(&bad_header_crc).is_err());

    // 3.2 Footer CRC32 bit flip (bytes 0..4 of 12-byte footer)
    let footer_start = valid_xz.len() - 12;
    let mut bad_footer_crc = valid_xz.clone();
    bad_footer_crc[footer_start] ^= 0x80;

    let mut footer_buf = [0u8; 12];
    footer_buf.copy_from_slice(&bad_footer_crc[footer_start..]);
    let footer_err = XzStreamFooter::parse(&footer_buf).unwrap_err();
    assert!(
        matches!(footer_err, XzError::FooterCrcMismatch { .. }),
        "Expected FooterCrcMismatch, got {footer_err:?}"
    );
    assert!(xz_decompress(&bad_footer_crc).is_err());

    // 3.3 Index CRC32 bit flip (last 4 bytes of Index before Footer)
    let index_crc_start = footer_start - 4;
    let mut bad_index_crc = valid_xz.clone();
    bad_index_crc[index_crc_start] ^= 0x02;
    assert!(xz_decompress(&bad_index_crc).is_err());

    // 3.4 Block Header CRC32 corruption
    let block_hdr = XzBlockHeader::new(
        vec![XzFilterConfig::lzma2(1024 * 1024)],
        XzCheckType::Crc32,
    )
    .expect("create block header");
    let mut enc_block_hdr = block_hdr.encode().expect("encode block header");
    let crc_offset = enc_block_hdr.len() - 4;
    enc_block_hdr[crc_offset] ^= 0x40;
    let block_err = XzBlockHeader::parse(&enc_block_hdr, XzCheckType::Crc32).unwrap_err();
    assert!(
        matches!(block_err, XzBlockError::Crc32Mismatch { .. }),
        "Expected BlockHeader Crc32Mismatch, got {block_err:?}"
    );

    // 3.5 Block Data Payload & Integrity Check Mismatch across all check types
    for check_type in [XzCheckType::Crc32, XzCheckType::Crc64, XzCheckType::Sha256] {
        let stream = create_single_block_xz(&payload, check_type);
        // Mutate byte in compressed block payload (offset 20)
        let mut tampered = stream.clone();
        tampered[20] ^= 0x01;

        let res = xz_decompress(&tampered);
        assert!(
            res.is_err(),
            "Decompression must reject corrupted payload for {check_type:?}"
        );

        // Check XzChecksumEngine direct verification failure
        let checksum_type = XzChecksumType::from_id(check_type.id())
            .expect("convert check type");
        let mut engine = XzChecksumEngine::new(checksum_type);
        engine.update(b"authentic payload stream");
        let valid_digest = engine.digest();

        let mut tampered_engine = XzChecksumEngine::new(checksum_type);
        tampered_engine.update(b"tampered payload stream");
        let tampered_digest = tampered_engine.digest();

        assert_ne!(valid_digest, tampered_digest);
        let verify_err = engine.verify(&tampered_digest).unwrap_err();
        assert!(
            matches!(verify_err, XzChecksumError::ChecksumMismatch { .. }),
            "Expected ChecksumMismatch, got {verify_err:?}"
        );
    }
}

// ============================================================================
// Dimension 4: Corrupted Backward Size Injection
// ============================================================================

#[test]
fn test_dimension_4_corrupted_backward_size_injection() {
    let payload = generate_deterministic_payload(64 * 1024, 0xABCDEF01);
    let valid_xz = create_single_block_xz(&payload, XzCheckType::Crc32);
    let footer_start = valid_xz.len() - 12;

    // 4.1 Modify Backward Size to a mismatching valid size and recompute Footer CRC
    let mut footer_buf = [0u8; 12];
    footer_buf.copy_from_slice(&valid_xz[footer_start..]);
    let parsed_footer = XzStreamFooter::parse(&footer_buf).expect("parse valid footer");

    let bad_backward_size = parsed_footer.backward_size + 4; // Add 4 bytes
    let bad_footer_bytes = parsed_footer
        .encode(bad_backward_size)
        .expect("encode bad backward size");

    let mut tampered_stream = valid_xz.clone();
    tampered_stream[footer_start..].copy_from_slice(&bad_footer_bytes);

    // Streaming decoder must reject due to index reconciliation failure
    let mut decoder = XzStreamDecoder::new(Cursor::new(&tampered_stream));
    let mut out = Vec::new();
    let stream_err = decoder.read_to_end(&mut out);
    assert!(
        stream_err.is_err(),
        "Decoder must reject backward size reconciliation mismatch"
    );

    // Seekable reader must reject due to incorrect backward seek index offset
    let seek_err = XzSeekableReader::new(Cursor::new(tampered_stream));
    assert!(
        seek_err.is_err(),
        "Seekable reader must fail when backward size points to corrupted index offset"
    );

    // 4.2 Out-of-bounds or non-multiple-of-4 Backward Size validation
    let flags = XzStreamFlags::new(XzCheckType::Crc32);
    let footer = XzStreamFooter::new(flags, 0);

    assert_eq!(
        footer.encode(0),
        Err(XzError::InvalidBackwardSize(0))
    );
    assert_eq!(
        footer.encode(2),
        Err(XzError::InvalidBackwardSize(2))
    );
    assert_eq!(
        footer.encode(XZ_MAX_BACKWARD_SIZE + 4),
        Err(XzError::InvalidBackwardSize(XZ_MAX_BACKWARD_SIZE + 4))
    );
    assert!(footer.encode(XZ_MIN_BACKWARD_SIZE).is_ok());
    assert!(footer.encode(XZ_MAX_BACKWARD_SIZE).is_ok());
}

// ============================================================================
// Dimension 5: Illegal Non-Zero Padding Injection
// ============================================================================

#[test]
fn test_dimension_5_illegal_non_zero_padding_injection() {
    // 5.1 Block Header Padding Non-Zero Byte Defense
    let custom_filter = XzFilterConfig::new(FILTER_ID_LZMA2, vec![0x14]);
    let padded_hdr = XzBlockHeader::new(vec![custom_filter], XzCheckType::Crc32).expect("hdr");
    let mut padded_enc = padded_hdr.encode().expect("padded encode");
    let pad_pos = padded_enc.len() - 5; // right before CRC32
    if pad_pos > 2 {
        padded_enc[pad_pos] = 0xFF; // Inject non-zero padding
        let new_crc = crc32_fast(0, &padded_enc[..padded_enc.len() - 4]);
        let crc_pos = padded_enc.len() - 4;
        padded_enc[crc_pos..].copy_from_slice(&new_crc.to_le_bytes());

        let res = XzBlockHeader::parse(&padded_enc, XzCheckType::Crc32);
        assert_eq!(
            res,
            Err(XzBlockError::NonZeroHeaderPadding),
            "Expected NonZeroHeaderPadding"
        );
    }

    // 5.2 Index Padding Non-Zero Byte Defense
    let mut index = XzStreamIndex::new();
    index.append(101, 200).expect("append index record"); // 101 unpadded -> odd length
    let mut enc_index = index.encode().expect("encode index");
    let idx_crc_pos = enc_index.len() - 4;
    let idx_pad_pos = idx_crc_pos - 1;
    enc_index[idx_pad_pos] = 0xAA; // Inject dirty padding byte
    let new_idx_crc = crc32_fast(0, &enc_index[..idx_crc_pos]);
    enc_index[idx_crc_pos..].copy_from_slice(&new_idx_crc.to_le_bytes());

    let idx_res = XzStreamIndex::parse(&enc_index);
    assert_eq!(
        idx_res,
        Err(XzError::NonZeroIndexPadding),
        "Expected NonZeroIndexPadding"
    );

    // 5.3 Stream Padding Non-Zero Byte Defense
    let payload = generate_deterministic_payload(8 * 1024, 0x33445566);
    let valid_xz = create_single_block_xz(&payload, XzCheckType::Crc32);

    let mut dirty_stream_padding = valid_xz.clone();
    dirty_stream_padding.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]); // Non-zero 4-byte padding

    let mut decoder = XzStreamDecoder::new(Cursor::new(&dirty_stream_padding));
    let mut out = Vec::new();
    let stream_res = decoder.read_to_end(&mut out);
    assert!(
        stream_res.is_err(),
        "Decoder must reject non-zero Stream Padding"
    );

    let mut arbitrary_trailing = valid_xz.clone();
    arbitrary_trailing.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let mut dec2 = XzStreamDecoder::new(Cursor::new(&arbitrary_trailing));
    assert!(dec2.read_to_end(&mut Vec::new()).is_err());
}

// ============================================================================
// Dimension 6: Malformed VLI Overflow and Overlong Sequence Injection
// ============================================================================

#[test]
fn test_dimension_6_malformed_vli_overflow_and_overlong_injection() {
    // 6.1 Overlong Sequence (> 9 bytes with continuation bits)
    let overlong_10b = [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
    let mut pos = 0;
    assert_eq!(
        decode_vli(&overlong_10b, &mut pos),
        Err(XzVliError::SequenceTooLong)
    );

    let mut stream_cursor = Cursor::new(&overlong_10b);
    assert_eq!(
        decode_vli_stream(&mut stream_cursor),
        Err(XzVliError::SequenceTooLong)
    );

    // 6.2 9th Byte MSB set (bit 7 non-zero)
    let msb_9th = [0x80; 9];
    pos = 0;
    assert_eq!(
        decode_vli(&msb_9th, &mut pos),
        Err(XzVliError::SequenceTooLong)
    );

    // 6.3 Non-canonical multi-byte encoding (trailing 0x00)
    let non_canonical_2b = [0x80, 0x00];
    pos = 0;
    assert_eq!(
        decode_vli(&non_canonical_2b, &mut pos),
        Err(XzVliError::NonCanonical { byte_index: 1 })
    );

    let non_canonical_3b = [0x81, 0x80, 0x00];
    pos = 0;
    assert_eq!(
        decode_vli(&non_canonical_3b, &mut pos),
        Err(XzVliError::NonCanonical { byte_index: 2 })
    );

    // 6.4 Value Overflow beyond XZ_VLI_MAX (2^63 - 1)
    let overflow_val = XZ_VLI_MAX.saturating_add(1);
    assert_eq!(
        vli_size(overflow_val),
        Err(XzVliError::ValueTooLarge { val: overflow_val })
    );
    assert_eq!(
        vli_size(u64::MAX),
        Err(XzVliError::ValueTooLarge { val: u64::MAX })
    );

    let mut out_buf = [0u8; 16];
    pos = 0;
    assert_eq!(
        encode_vli(overflow_val, &mut out_buf, &mut pos),
        Err(XzVliError::ValueTooLarge { val: overflow_val })
    );
    assert_eq!(
        encode_vli_vec(u64::MAX),
        Err(XzVliError::ValueTooLarge { val: u64::MAX })
    );

    // 6.5 Ladder boundaries and maximum valid VLI roundtrip
    for &boundary in &[0u64, 1, 127, 128, 16383, 16384, 2097151, 2097152, XZ_VLI_MAX] {
        let size = vli_size(boundary).expect("valid size");
        let vec = encode_vli_vec(boundary).expect("valid encode");
        assert_eq!(vec.len(), size);

        pos = 0;
        let dec = decode_vli(&vec, &mut pos).expect("valid decode");
        assert_eq!(dec, boundary);
        assert_eq!(pos, size);
    }
}

// ============================================================================
// Dimension 7: RandomReader Quadratic Biased Micro-Slicing Chaos Jitter Reading
// ============================================================================

/// Deterministic PRNG and Reader wrapper yielding 1..=7 byte micro-chunks
/// with quadratic bias to simulate hostile network/disk I/O jitter.
struct RandomReader<R: Read> {
    inner: R,
    state: u64,
}

impl<R: Read> RandomReader<R> {
    fn new(inner: R, seed: u64) -> Self {
        Self { inner, state: seed }
    }

    fn next_chunk_len(&mut self, max_cap: usize) -> usize {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let jitter = 1 + (self.state % 7) as usize;
        jitter.min(max_cap).max(1)
    }
}

impl<R: Read> Read for RandomReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let chunk_limit = self.next_chunk_len(buf.len());
        self.inner.read(&mut buf[..chunk_limit])
    }
}

#[test]
fn test_dimension_7_random_reader_micro_slicing_jitter_streaming() {
    let seeds = [1337u64, 42, 0xCAFEBABE, 0xDEADBEEF, 99999];

    // 7.1 Single-block XZ streaming under 1..=7 byte micro-slice jitter
    let payload = generate_deterministic_payload(64 * 1024, 0x778899AA);
    let expected_hash = compute_sha256(&payload);

    for check_type in [XzCheckType::Crc32, XzCheckType::Crc64, XzCheckType::Sha256] {
        let compressed = create_single_block_xz(&payload, check_type);

        for &seed in &seeds {
            let cursor = Cursor::new(&compressed);
            let jitter_reader = RandomReader::new(cursor, seed);
            let mut decoder = XzStreamDecoder::new(jitter_reader);

            let mut decompressed = Vec::new();
            // Read with small buffer to maximize step interleaving
            let mut buf = [0u8; 17];
            loop {
                let n = decoder.read(&mut buf).expect("jitter streaming read failed");
                if n == 0 {
                    break;
                }
                decompressed.extend_from_slice(&buf[..n]);
            }

            assert_eq!(decompressed.len(), payload.len());
            assert_eq!(compute_sha256(&decompressed), expected_hash);
        }
    }

    // 7.2 Multi-block parallel XZ with BCJ x86 filter under micro-slice jitter
    let multi_payload = generate_deterministic_payload(192 * 1024, 0x12345678);
    let multi_expected_hash = compute_sha256(&multi_payload);
    let multi_compressed = create_multi_block_xz(
        &multi_payload,
        64 * 1024,
        XzCheckType::Crc64,
        Some(XzBcjType::X86),
    );

    for &seed in &seeds[..3] {
        let cursor = Cursor::new(&multi_compressed);
        let jitter_reader = RandomReader::new(cursor, seed);
        let mut decoder = XzStreamDecoder::new(jitter_reader);

        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .expect("multi-block jitter streaming failed");

        assert_eq!(decompressed.len(), multi_payload.len());
        assert_eq!(compute_sha256(&decompressed), multi_expected_hash);
        assert_eq!(decoder.cumulative_records().len(), 3);
    }
}

// ============================================================================
// Proptest Invariants: Property-based Roundtrip & Bijectivity
// ============================================================================

proptest! {
    /// VLI Arbitrary Value Invariant: Any valid 64-bit integer <= XZ_VLI_MAX roundtrips losslessly.
    #[test]
    fn test_vli_arbitrary_roundtrip_property(val in 0u64..=XZ_VLI_MAX) {
        let mut buf = [0u8; 16];
        let mut enc_pos = 0;
        let written = encode_vli(val, &mut buf, &mut enc_pos).expect("vli encode");
        prop_assert!((1..=9).contains(&written));
        prop_assert_eq!(enc_pos, written);

        let mut dec_pos = 0;
        let decoded = decode_vli(&buf[..enc_pos], &mut dec_pos).expect("vli decode");
        prop_assert_eq!(decoded, val);
        prop_assert_eq!(dec_pos, enc_pos);
    }

    /// x86 BCJ Arbitrary Payload Bijectivity: Any byte sequence preserves shape under encode/decode.
    #[test]
    fn test_bcj_x86_arbitrary_bijectivity(mut bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let original = bytes.clone();
        let mut filter = BcjX86::new();
        filter.encode(&mut bytes, 0);
        let mut filter_dec = BcjX86::new();
        filter_dec.decode(&mut bytes, 0);
        prop_assert_eq!(bytes, original);
    }

    /// ARM64 BCJ Arbitrary Payload Bijectivity: Any byte sequence preserves shape under encode/decode.
    #[test]
    fn test_bcj_arm64_arbitrary_bijectivity(mut bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let original = bytes.clone();
        let mut filter = BcjArm64::new();
        filter.encode(&mut bytes, 0);
        let mut filter_dec = BcjArm64::new();
        filter_dec.decode(&mut bytes, 0);
        prop_assert_eq!(bytes, original);
    }
}
