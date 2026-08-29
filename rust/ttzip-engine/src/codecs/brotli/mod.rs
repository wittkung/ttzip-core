// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Pure-Rust Google Brotli block and streaming compression codecs.

pub mod block;
pub mod pipe;
pub mod stream;

pub use block::*;
pub use pipe::*;
pub use stream::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_brotli_block_roundtrip() {
        let input = b"Pure Rust Brotli block compression test in TTZip native glue layer 2026.";
        let mut compressed = vec![0u8; brotli_compress_bound(input.len())];
        let comp_len = brotli_compress(input, &mut compressed, 6, 22).expect("brotli compress");
        assert!(comp_len > 0);

        let mut decompressed = vec![0u8; input.len()];
        let decomp_len = brotli_decompress(&compressed[..comp_len], &mut decompressed).expect("brotli decompress");
        assert_eq!(decomp_len, input.len());
        assert_eq!(&decompressed[..decomp_len], input);
    }

    #[test]
    fn test_brotli_to_vec_roundtrip() {
        let input = b"Repetitive string test for Brotli to vector compression: ABCABCABCABCABCABCABCABCABCABCABCABCABCABCABCABCABCABC";
        let comp = brotli_compress_to_vec(input, 9, 22).expect("brotli to vec compress");
        assert!(comp.len() < input.len());

        let decomp = brotli_decompress_to_vec(&comp, 1024 * 1024).expect("brotli to vec decompress");
        assert_eq!(decomp, input);
    }

    #[test]
    fn test_brotli_pipe_exact_byte_counts_and_compression_ratio() {
        let payload = vec![0xFEu8; 5 * 1024 * 1024]; // 5MB payload (spans across 4MB pipe boundary)
        let mut reader = Cursor::new(&payload);
        let mut compressed = Vec::new();

        let (read_bytes, written_bytes) = brotli_compress_stream_pipe(&mut reader, &mut compressed, 4, 20, None)
            .expect("brotli compress pipe failed");
        assert_eq!(read_bytes, payload.len() as u64);
        assert_eq!(written_bytes, compressed.len() as u64);
        assert!(written_bytes < read_bytes, "Compressed size must be less than raw size");

        let ratio = (written_bytes as f64) / (read_bytes as f64);
        assert!(ratio < 0.01, "Highly repetitive data ratio must be < 1%, got {}", ratio);

        let mut comp_reader = Cursor::new(&compressed);
        let mut decompressed = Vec::new();
        let (dec_read, dec_written) = brotli_decompress_stream_pipe(&mut comp_reader, &mut decompressed, None)
            .expect("brotli decompress pipe failed");
        assert_eq!(dec_read, compressed.len() as u64);
        assert_eq!(dec_written, payload.len() as u64);
        assert_eq!(decompressed, payload);
    }

    #[test]
    fn test_brotli_corrupt_data() {
        let corrupt = [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x01];
        let mut out = [0u8; 128];
        let res = brotli_decompress(&corrupt, &mut out);
        assert!(res.is_err());
    }

    #[test]
    fn test_brotli_quality_and_window_range() {
        let text = b"Testing Brotli across various quality settings and sliding window sizes in TTZip.";
        for q in [0, 1, 4, 6, 9, 11] {
            for lgwin in [10, 16, 22, 24] {
                let comp = brotli_compress_to_vec(text, q, lgwin).expect("compress");
                assert!(!comp.is_empty());
                let decomp = brotli_decompress_to_vec(&comp, 4096).expect("decompress");
                assert_eq!(decomp.as_slice(), text);
            }
        }
    }

    #[test]
    fn test_brotli_rfc7932_static_dictionary_efficiency() {
        // RFC 7932 static dictionary contains standard HTML/HTTP schema and keywords.
        let web_snippet = b"<html><head><meta charset=\"utf-8\"><title>TTZip RFC 7932</title></head><body><div class=\"container\"><p>http://www.w3.org/1999/xhtml</p></div></body></html>";
        let comp = brotli_compress_to_vec(web_snippet, 11, 22).expect("brotli rfc7932 compress");
        assert!(comp.len() < web_snippet.len(), "Static dictionary should compress web snippet efficiently");

        let decomp = brotli_decompress_to_vec(&comp, 4096).expect("brotli rfc7932 decompress");
        assert_eq!(decomp.as_slice(), web_snippet);
    }

    #[test]
    fn test_brotli_empty_slice() {
        let empty = b"";
        let mut dst = [0u8; 64];
        let c_len = brotli_compress(empty, &mut dst, 6, 22).expect("empty compress");
        assert_eq!(c_len, 0);

        let d_len = brotli_decompress(empty, &mut dst).expect("empty decompress");
        assert_eq!(d_len, 0);
    }
}

