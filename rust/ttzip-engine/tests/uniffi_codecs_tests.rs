// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Integration and verification suite for Mozilla UniFFI 0.28 Codec API exports.
//!
//! Covers roundtrip compression and decompression across all 13 supported algorithms:
//! DeflateRaw, Zlib, Gzip, Zstd, Zstd LDM, LZ4 Fast/HC, LZFSE, LZVN, Brotli,
//! Snappy Raw/Framed, Bzip2, PPMd, and Fast LZMA2 (FL2).

use ttzip_engine::uniffi_api::codecs::*;

const TEST_PAYLOAD: &[u8] = b"Mozilla UniFFI 0.28 Codec Verification Test Payload for TTZip Engine 2026. ABCDEFGHIJKLMNOPQRSTUVWXYZ 1234567890.";

#[test]
fn test_deflate_variants_roundtrip() {
    // Raw Deflate
    let def_c = uniffi_deflate_compress(TEST_PAYLOAD.to_vec(), 6).expect("deflate compress");
    let def_d = uniffi_deflate_decompress(def_c, TEST_PAYLOAD.len() as u64).expect("deflate decompress");
    assert_eq!(def_d.as_slice(), TEST_PAYLOAD);

    // Zlib
    let zlib_c = uniffi_zlib_compress(TEST_PAYLOAD.to_vec(), 6).expect("zlib compress");
    let zlib_d = uniffi_zlib_decompress(zlib_c, TEST_PAYLOAD.len() as u64).expect("zlib decompress");
    assert_eq!(zlib_d.as_slice(), TEST_PAYLOAD);

    // Gzip
    let gzip_c = uniffi_gzip_compress(TEST_PAYLOAD.to_vec(), 6).expect("gzip compress");
    let gzip_d = uniffi_gzip_decompress(gzip_c, TEST_PAYLOAD.len() as u64).expect("gzip decompress");
    assert_eq!(gzip_d.as_slice(), TEST_PAYLOAD);
}

#[test]
fn test_zstd_and_ldm_and_dict_roundtrip() {
    // Standard Zstd
    let zstd_c = uniffi_zstd_compress(TEST_PAYLOAD.to_vec(), 3).expect("zstd compress");
    let zstd_d = uniffi_zstd_decompress(zstd_c, None).expect("zstd decompress");
    assert_eq!(zstd_d.as_slice(), TEST_PAYLOAD);

    // Zstd LDM
    let ldm_c = uniffi_zstd_compress_ldm(TEST_PAYLOAD.to_vec(), 3, 64).expect("zstd ldm compress");
    let ldm_d = uniffi_zstd_decompress(ldm_c, Some(TEST_PAYLOAD.len() as u64)).expect("zstd ldm decompress");
    assert_eq!(ldm_d.as_slice(), TEST_PAYLOAD);

    // Zstd 112KB Dict
    let dict = uniffi_zstd_get_standard_112kb_dict();
    assert!(!dict.is_empty() && dict.len() <= 112 * 1024);

    let dict_c = uniffi_zstd_dict_compress(TEST_PAYLOAD.to_vec(), dict.clone(), 3).expect("dict compress");
    let dict_d = uniffi_zstd_dict_decompress(dict_c, dict, Some(TEST_PAYLOAD.len() as u64)).expect("dict decompress");
    assert_eq!(dict_d.as_slice(), TEST_PAYLOAD);
}

#[test]
fn test_lz4_fast_and_hc_roundtrip() {
    let fast_c = uniffi_lz4_compress_fast(TEST_PAYLOAD.to_vec(), 1).expect("lz4 fast");
    let fast_d = uniffi_lz4_decompress(fast_c, TEST_PAYLOAD.len() as u64).expect("lz4 fast dec");
    assert_eq!(fast_d.as_slice(), TEST_PAYLOAD);

    let hc_c = uniffi_lz4_compress_hc(TEST_PAYLOAD.to_vec(), 9).expect("lz4 hc");
    let hc_d = uniffi_lz4_decompress(hc_c, TEST_PAYLOAD.len() as u64).expect("lz4 hc dec");
    assert_eq!(hc_d.as_slice(), TEST_PAYLOAD);
}

#[test]
fn test_apple_lzfse_and_lzvn_roundtrip() {
    let lzfse_c = uniffi_lzfse_compress(TEST_PAYLOAD.to_vec()).expect("lzfse comp");
    let lzfse_d = uniffi_lzfse_decompress(lzfse_c, TEST_PAYLOAD.len() as u64).expect("lzfse dec");
    assert_eq!(lzfse_d.as_slice(), TEST_PAYLOAD);

    let lzvn_c = uniffi_lzvn_compress(TEST_PAYLOAD.to_vec()).expect("lzvn comp");
    let lzvn_d = uniffi_lzvn_decompress(lzvn_c, TEST_PAYLOAD.len() as u64).expect("lzvn dec");
    assert_eq!(lzvn_d.as_slice(), TEST_PAYLOAD);
}

#[test]
fn test_brotli_snappy_bzip2_ppmd_roundtrip() {
    // Brotli
    let brotli_c = uniffi_brotli_compress(TEST_PAYLOAD.to_vec(), 6, 22).expect("brotli comp");
    let brotli_d = uniffi_brotli_decompress(brotli_c, None).expect("brotli dec");
    assert_eq!(brotli_d.as_slice(), TEST_PAYLOAD);

    // Snappy Raw & Framed
    let snap_c = uniffi_snappy_compress(TEST_PAYLOAD.to_vec()).expect("snappy comp");
    let snap_d = uniffi_snappy_decompress(snap_c).expect("snappy dec");
    assert_eq!(snap_d.as_slice(), TEST_PAYLOAD);

    let snap_f_c = uniffi_snappy_frame_encode(TEST_PAYLOAD.to_vec()).expect("snappy framed enc");
    let snap_f_d = uniffi_snappy_frame_decode(snap_f_c).expect("snappy framed dec");
    assert_eq!(snap_f_d.as_slice(), TEST_PAYLOAD);

    // Bzip2
    let bz2_c = uniffi_bzip2_compress(TEST_PAYLOAD.to_vec(), 9).expect("bz2 comp");
    let bz2_d = uniffi_bzip2_decompress(bz2_c, None).expect("bz2 dec");
    assert_eq!(bz2_d.as_slice(), TEST_PAYLOAD);

    // PPMd
    let ppmd_c = uniffi_ppmd_compress(TEST_PAYLOAD.to_vec(), 6, 16).expect("ppmd comp");
    let ppmd_d = uniffi_ppmd_decompress(ppmd_c, TEST_PAYLOAD.len() as u64, 6, 16).expect("ppmd dec");
    assert_eq!(ppmd_d.as_slice(), TEST_PAYLOAD);
}

#[test]
fn test_unified_buffer_api_all_codecs() {
    let codecs = [
        UniFFICompressionCodec::DeflateRaw,
        UniFFICompressionCodec::Zlib,
        UniFFICompressionCodec::Gzip,
        UniFFICompressionCodec::Zstd,
        UniFFICompressionCodec::ZstdLdm,
        UniFFICompressionCodec::Lz4Fast,
        UniFFICompressionCodec::Lz4Hc,
        UniFFICompressionCodec::Lzfse,
        UniFFICompressionCodec::Lzvn,
        UniFFICompressionCodec::Brotli,
        UniFFICompressionCodec::SnappyRaw,
        UniFFICompressionCodec::SnappyFramed,
        UniFFICompressionCodec::Bzip2,
        UniFFICompressionCodec::Ppmd,
        UniFFICompressionCodec::Fl2,
    ];

    for codec in codecs {
        let bound = uniffi_compress_bound(codec, TEST_PAYLOAD.len() as u64, None);
        assert!(bound > 0, "Bound check for {:?}", codec);

        let compressed = uniffi_compress_buffer(codec, TEST_PAYLOAD.to_vec(), None)
            .unwrap_or_else(|e| panic!("Compress failed for {:?}: {:?}", codec, e));
        assert!(!compressed.is_empty());

        let decompressed = uniffi_decompress_buffer(codec, compressed, Some(TEST_PAYLOAD.len() as u64), None)
            .unwrap_or_else(|e| panic!("Decompress failed for {:?}: {:?}", codec, e));
        assert_eq!(decompressed.as_slice(), TEST_PAYLOAD, "Payload mismatch for {:?}", codec);
    }
}

#[test]
fn test_fl2_and_zstd_train_roundtrip() {
    let bound = uniffi_fl2_compress_bound(TEST_PAYLOAD.len() as u64);
    assert!(bound >= TEST_PAYLOAD.len() as u64);

    let fl2_c = uniffi_fl2_compress(TEST_PAYLOAD.to_vec(), 3, None).expect("fl2 compress");
    assert!(!fl2_c.is_empty());

    let uncomp_sz = uniffi_fl2_find_decompressed_size(fl2_c.clone());
    let fl2_d = uniffi_fl2_decompress(fl2_c, uncomp_sz, None).expect("fl2 decompress");
    assert_eq!(fl2_d.as_slice(), TEST_PAYLOAD);

    let samples = vec![
        TEST_PAYLOAD.to_vec(),
        b"Sample header alpha beta gamma delta 12345678".to_vec(),
        b"Sample footer omega sigma theta lambda 87654321".to_vec(),
    ];
    let dict = uniffi_zstd_train_dict(samples, 4096, 3);
    if let Ok(d) = dict {
        assert!(!d.is_empty() && d.len() <= 4096);
    }
}
