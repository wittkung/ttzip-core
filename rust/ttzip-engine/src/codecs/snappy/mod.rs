// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Pure-Rust Google Snappy block & streaming framing codecs.

pub mod block;
pub mod frame;
pub mod pipe;

pub use block::*;
pub use frame::*;
pub use pipe::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_snappy_raw_block_roundtrip() {
        let data = b"Pure Rust Snappy zero-copy block compression in TTZip glue layer 2026.";
        let bound = snappy_compress_bound(data.len());
        let mut comp = vec![0u8; bound];
        let c_len = snappy_compress(data, &mut comp).expect("snappy compress");
        assert!(c_len > 0);

        assert!(snappy_validate(&comp[..c_len]));
        assert!(snappy_validate_bounded(&comp[..c_len], 1024 * 1024));
        assert!(!snappy_validate_bounded(&comp[..c_len], 10)); // bounded reject
        let d_len_expected = snappy_uncompressed_length(&comp[..c_len]).expect("uncompressed len");
        assert_eq!(d_len_expected, data.len());

        let mut decomp = vec![0u8; data.len()];
        let d_len = snappy_decompress(&comp[..c_len], &mut decomp).expect("snappy decompress");
        assert_eq!(d_len, data.len());
        assert_eq!(&decomp[..d_len], data);
    }

    #[test]
    fn test_snappy_framing_roundtrip() {
        let data = b"Framing format test payload with standard Castagnoli CRC32-C checksum validation.";
        let encoded = snappy_frame_encode_to_vec(data).expect("frame encode");
        assert!(is_framed_snappy(&encoded));
        assert!(snappy_frame_validate(&encoded));

        let decoded = snappy_frame_decode_to_vec(&encoded, 1024 * 1024).expect("frame decode");
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_snappy_frame_validation_and_corruption() {
        let data = b"Testing framed snappy corruption detection with CRC32-C mismatch verification.";
        let mut encoded = snappy_frame_encode_to_vec(data).expect("frame encode");
        assert!(snappy_frame_validate(&encoded));

        // Corrupt stream identifier
        encoded[0] = 0x00;
        assert!(!snappy_frame_validate(&encoded));
        encoded[0] = 0xFF; // Restore

        // Corrupt payload byte
        let last_idx = encoded.len() - 1;
        encoded[last_idx] ^= 0xFF;
        assert!(!snappy_frame_validate(&encoded));
    }

    #[test]
    fn test_snappy_anti_oom_malicious_varint_header() {
        // 1. 4GB malicious varint header (0xFF, 0xFF, 0xFF, 0xFF, 0x0F = 4294967295 bytes uncompressed claim)
        let malicious_4gb = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F, 0x00, 0x01, 0x02];
        let uncomp = snappy_uncompressed_length(&malicious_4gb).expect("parse varint");
        assert_eq!(uncomp, 4294967295);

        // Zero-heap validation MUST reject immediately without OOM
        assert!(!snappy_validate(&malicious_4gb));
        assert!(!snappy_validate_bounded(&malicious_4gb, 10 * 1024 * 1024));

        // 2. Malicious 64-bit varint overflow (more than 5 bytes / > 32-bit LEB128)
        let malicious_overflow = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01];
        assert!(parse_varint(&malicious_overflow).is_none());
        assert!(!snappy_validate(&malicious_overflow));

        // 3. Truncated varint header
        let truncated = [0x80, 0x80, 0x80];
        assert!(parse_varint(&truncated).is_none());
        assert!(!snappy_validate(&truncated));
    }

    #[test]
    fn test_snappy_validate_backtrack_out_of_bounds() {
        // Tag 0b01 (Copy 1-byte) before any literals emitted (offset > uncompressed_pos)
        // Header: length 10 (0x0A), Tag: 0b00000101 (Copy len 4, offset high 0), Offset low: 0x05
        let invalid_copy = [0x0A, 0x05, 0x05];
        assert!(!snappy_validate(&invalid_copy));

        // Zero offset
        let zero_offset = [0x0A, 0x01, 0x00];
        assert!(!snappy_validate(&zero_offset));
    }

    #[test]
    fn test_snappy_crc32c_mask_unmask() {
        let original_crc: u32 = 0x12345678;
        let masked = mask_crc32c(original_crc);
        let unmasked = unmask_crc32c(masked);
        assert_eq!(unmasked, original_crc);
    }

    #[test]
    fn test_snappy_pipe_exact_byte_counts_and_compression_ratio() {
        let payload = vec![0x3Cu8; 5 * 1024 * 1024]; // 5MB highly compressible payload
        let mut reader = Cursor::new(&payload);
        let mut compressed = Vec::new();

        let (read_bytes, written_bytes) = snappy_compress_stream_pipe(&mut reader, &mut compressed, None)
            .expect("compress pipe failed");
        assert_eq!(read_bytes, payload.len() as u64);
        assert_eq!(written_bytes, compressed.len() as u64);
        assert!(written_bytes < read_bytes, "Compressed size must be less than raw size");
        assert!(is_framed_snappy(&compressed));
        assert!(snappy_frame_validate(&compressed));

        let ratio = (written_bytes as f64) / (read_bytes as f64);
        assert!(ratio < 0.05, "Highly repetitive data ratio must be < 5%, got {}", ratio);

        let mut comp_reader = Cursor::new(&compressed);
        let mut decompressed = Vec::new();
        let (dec_read, dec_written) = snappy_decompress_stream_pipe(&mut comp_reader, &mut decompressed, None)
            .expect("decompress pipe failed");
        assert_eq!(dec_read, compressed.len() as u64);
        assert_eq!(dec_written, payload.len() as u64);
        assert_eq!(decompressed, payload);
    }
}

