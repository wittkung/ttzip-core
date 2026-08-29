// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Fast block compression facade re-exporting `LZ4`, Google `Snappy`, and Apple `LZFSE`/`LZVN`.

pub use crate::codecs::lz4::{
    lz4_compress, lz4_compress_bound, lz4_compress_fast, lz4_compress_hc,
    lz4_compress_hc_to_vec, lz4_compress_to_vec, lz4_decompress, lz4_decompress_to_vec,
};
pub use crate::codecs::lzfse::{
    lzfse_compress, lzfse_compress_bound, lzfse_compress_to_vec, lzfse_decompress,
    lzfse_decompress_to_vec, lzvn_compress, lzvn_compress_bound, lzvn_compress_to_vec,
    lzvn_decompress, lzvn_decompress_to_vec,
};
pub use crate::codecs::snappy::{
    is_framed_snappy, mask_crc32c, parse_varint, snappy_compress,
    snappy_compress_bound as snappy_max_compressed_length, snappy_compress_bound,
    snappy_compress_file, snappy_compress_stream_pipe, snappy_compress_to_vec, snappy_decompress,
    snappy_decompress_file, snappy_decompress_stream_pipe, snappy_decompress_to_vec,
    snappy_frame_decode, snappy_frame_decode_to_vec, snappy_frame_encode,
    snappy_frame_encode_to_vec, snappy_frame_max_encoded_length, snappy_frame_validate,
    snappy_uncompressed_length, snappy_validate, snappy_validate_bounded, unmask_crc32c,
    SNAPPY_MAX_CHUNK_SIZE, SNAPPY_PIPE_BUFFER_SIZE, SNAPPY_STREAM_IDENTIFIER,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz4_roundtrip() {
        let input = b"LZ4 fast compression block testing in TTZip native glue layer.";
        let mut comp = vec![0u8; lz4_compress_bound(input.len())];
        let c_len = lz4_compress(input, &mut comp).expect("lz4 compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lz4_decompress(&comp[..c_len], &mut decomp).expect("lz4 decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }

    #[test]
    fn test_lz4_hc_roundtrip() {
        let input = b"LZ4 High Compression (HC) block testing with higher compression ratio in TTZip.";
        let mut comp = vec![0u8; lz4_compress_bound(input.len())];
        let c_len = lz4_compress_hc(input, &mut comp, 9).expect("lz4 hc compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lz4_decompress(&comp[..c_len], &mut decomp).expect("lz4 decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }

    #[test]
    fn test_snappy_roundtrip_and_validation() {
        let input = b"Snappy Google fast block codec validation roundtrip TTZip 2026.";
        let mut comp = vec![0u8; snappy_max_compressed_length(input.len())];
        let c_len = snappy_compress(input, &mut comp).expect("snappy compress");
        assert!(c_len > 0);

        assert!(snappy_validate(&comp[..c_len]));
        let uncomp_len = snappy_uncompressed_length(&comp[..c_len]).expect("snappy uncompressed length");
        assert_eq!(uncomp_len, input.len());

        let mut decomp = vec![0u8; input.len()];
        let d_len = snappy_decompress(&comp[..c_len], &mut decomp).expect("snappy decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }

    #[test]
    fn test_lzfse_scratch_roundtrip() {
        let input = b"Apple LZFSE proprietary high-ratio block compression with 2MB scratch buffer.";
        let mut comp = vec![0u8; input.len() + 1024];
        let c_len = lzfse_compress(input, &mut comp).expect("lzfse compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lzfse_decompress(&comp[..c_len], &mut decomp).expect("lzfse decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }

    #[test]
    fn test_lzvn_facade_roundtrip() {
        let input = b"Apple LZVN fast hardware-oriented decompression facade test.";
        let mut comp = vec![0u8; lzvn_compress_bound(input.len())];
        let c_len = lzvn_compress(input, &mut comp).expect("lzvn compress");
        assert!(c_len > 0);

        let mut decomp = vec![0u8; input.len()];
        let d_len = lzvn_decompress(&comp[..c_len], &mut decomp).expect("lzvn decompress");
        assert_eq!(d_len, input.len());
        assert_eq!(&decomp[..d_len], input);
    }
}
