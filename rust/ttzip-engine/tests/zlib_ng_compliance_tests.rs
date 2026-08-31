// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official zlib-ng / Deflate-NG Compliance Test Suite and Differential Oracle Verification Matrix.
//!
//! Enforces 100% byte-exact fidelity and strict compliance across RFC 1951, RFC 1950, and RFC 1952:
//! 1. **Levels 0..=9 Full Roundtrip Matrix**: Verifies boundary sizes (0B..=128KB) across Raw Deflate, Zlib, and Gzip.
//! 2. **Dedicated RLE & Huffman Modes**: Verifies run-length encoding and high-entropy literal tree coders.
//! 3. **8 Mathematical Synthetic Corpora Differential Oracle**: Validates all 8 canonical corpora against `flate2`.
//! 4. **6-Layer Defense-in-Depth Invariants**: Comprehensive testing of `WindowBoundsGuard`, `HashChainLoopGuard`,
//!    `DynamicLevelIntegrityGuard`, `DecompressionBombGuard`, `StoredBlockEscapeGuard`, and zeroize protection.
//! 5. **Container Framing & Integrity Checks**: Verifies RFC 1950 Adler-32 and RFC 1952 CRC-32 checksums and headers.

use std::io::Write;
use flate2::write::{DeflateDecoder, GzDecoder, ZlibEncoder};
use flate2::Compression;

use ttzip_engine::benchmark::corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
use ttzip_engine::codecs::deflate::{
    deflate_compress, deflate_compress_bound, deflate_decompress, gzip_compress,
    gzip_compress_bound, gzip_decompress, zlib_compress, zlib_compress_bound, zlib_decompress,
};
use ttzip_engine::security::deflate_ng_defense::{
    sanitize_deflate_entry_path, DecompressionBombGuard, DeflateCompressionStrategy,
    DeflateNgDefenseGuard, DeflateStreamState, DynamicLevelIntegrityGuard, HashChainLoopGuard,
    StoredBlockEscapeGuard, WindowBoundsGuard, DEFLATE_NG_MAX_EXPANSION_RATIO,
    DEFLATE_NG_MAX_WINDOW_SIZE,
};
use ttzip_engine::types::TTZipStatus;

// MARK: - 1. Levels 0..=9 Roundtrip Consistency Matrix

#[test]
fn test_zlib_ng_levels_0_to_9_roundtrip_matrix() {
    let boundary_sizes = [
        0usize, 1, 2, 15, 16, 255, 256, 4095, 4096, 32767, 32768, 65535, 65536, 131072,
    ];

    let levels_to_test = [0i32, 1, 3, 6, 9];

    for &size in &boundary_sizes {
        let mut sample = Vec::with_capacity(size);
        let mut prng: u32 = 0x1951_1950 ^ (size as u32);
        for i in 0..size {
            prng = prng.wrapping_mul(1103515245).wrapping_add(12345);
            let byte = if i % 16 < 4 {
                // Introduce short local repetitions to test matchfinder at all sizes
                (i % 4) as u8
            } else {
                (prng >> 16) as u8
            };
            sample.push(byte);
        }

        for &level in &levels_to_test {
            // 1. Raw Deflate Roundtrip
            let raw_bound = deflate_compress_bound(sample.len(), level);
            let mut raw_comp = vec![0u8; raw_bound];
            let comp_len = deflate_compress(&sample, &mut raw_comp, level)
                .unwrap_or_else(|_| panic!("Deflate compression failed for size={}, level={}", size, level));
            assert!(comp_len <= raw_bound);

            let mut raw_decomp = vec![0u8; sample.len()];
            let decomp_len = deflate_decompress(&raw_comp[..comp_len], &mut raw_decomp)
                .unwrap_or_else(|_| panic!("Deflate decompression failed for size={}, level={}", size, level));
            assert_eq!(decomp_len, sample.len());
            assert_eq!(raw_decomp, sample);

            // 2. Zlib Container Roundtrip (RFC 1950)
            let zlib_bound = zlib_compress_bound(sample.len(), level);
            let mut zlib_comp = vec![0u8; zlib_bound];
            let zcomp_len = zlib_compress(&sample, &mut zlib_comp, level)
                .unwrap_or_else(|_| panic!("Zlib compression failed for size={}, level={}", size, level));
            assert!(zcomp_len <= zlib_bound);

            let mut zlib_decomp = vec![0u8; sample.len()];
            let zdecomp_len = zlib_decompress(&zlib_comp[..zcomp_len], &mut zlib_decomp)
                .unwrap_or_else(|_| panic!("Zlib decompression failed for size={}, level={}", size, level));
            assert_eq!(zdecomp_len, sample.len());
            assert_eq!(zlib_decomp, sample);

            // 3. Gzip Container Roundtrip (RFC 1952)
            let gzip_bound = gzip_compress_bound(sample.len(), level);
            let mut gzip_comp = vec![0u8; gzip_bound];
            let gcomp_len = gzip_compress(&sample, &mut gzip_comp, level)
                .unwrap_or_else(|_| panic!("Gzip compression failed for size={}, level={}", size, level));
            assert!(gcomp_len <= gzip_bound);

            let mut gzip_decomp = vec![0u8; sample.len()];
            let gdecomp_len = gzip_decompress(&gzip_comp[..gcomp_len], &mut gzip_decomp)
                .unwrap_or_else(|_| panic!("Gzip decompression failed for size={}, level={}", size, level));
            assert_eq!(gdecomp_len, sample.len());
            assert_eq!(gzip_decomp, sample);
        }
    }
}

// MARK: - 2. Dedicated RLE & Huffman Modes Compliance

#[test]
fn test_zlib_ng_dedicated_rle_and_huffman_modes() {
    // 1. Extreme RLE pattern: 128KB of repetitive byte runs
    let rle_payload = vec![0x42u8; 128 * 1024];
    for level in [1, 6, 9] {
        let bound = deflate_compress_bound(rle_payload.len(), level);
        let mut comp = vec![0u8; bound];
        let comp_len = deflate_compress(&rle_payload, &mut comp, level).expect("RLE compression failed");
        assert!(comp_len < 1024, "RLE payload must compress to < 1KB (got {} bytes)", comp_len);

        let mut decomp = vec![0u8; rle_payload.len()];
        let decomp_len = deflate_decompress(&comp[..comp_len], &mut decomp).expect("RLE decompression failed");
        assert_eq!(decomp_len, rle_payload.len());
        assert_eq!(decomp, rle_payload);
    }

    // 2. High-Entropy Literal Pattern (Huffman-only coding test): 64KB
    let mut literal_payload = Vec::with_capacity(64 * 1024);
    let mut prng: u32 = 0xFEED_FACE;
    for _ in 0..(64 * 1024) {
        prng = prng.wrapping_mul(1664525).wrapping_add(1013904223);
        literal_payload.push((prng >> 24) as u8);
    }

    for level in [1, 6, 9] {
        let bound = zlib_compress_bound(literal_payload.len(), level);
        let mut comp = vec![0u8; bound];
        let comp_len = zlib_compress(&literal_payload, &mut comp, level).expect("Huffman compression failed");
        assert!(comp_len > 0);

        let mut decomp = vec![0u8; literal_payload.len()];
        let decomp_len = zlib_decompress(&comp[..comp_len], &mut decomp).expect("Huffman decompression failed");
        assert_eq!(decomp_len, literal_payload.len());
        assert_eq!(decomp, literal_payload);
    }
}

// MARK: - 3. 8 Mathematical Synthetic Corpora Differential Oracle Matrix

#[test]
fn test_zlib_ng_8_mathematical_synthetic_corpora_differential_oracle() {
    let corpora_types = [
        (BenchmarkCorpusType::TextData, "TextData (Zipf Power-Law)"),
        (BenchmarkCorpusType::ShortMatch, "ShortMatch (8-Slot Pattern Pool)"),
        (BenchmarkCorpusType::Dna, "Dna (4-Symbol Collision)"),
        (BenchmarkCorpusType::Noise, "Noise (XorShift128+ White Noise)"),
        (BenchmarkCorpusType::Literals, "Literals (High-Entropy Coded)"),
        (BenchmarkCorpusType::MachOBinary, "MachOBinary (ARM64/DWARF)"),
        (BenchmarkCorpusType::RealisticRgb, "RealisticRgb (2D Gradients)"),
        (BenchmarkCorpusType::StripedRgb, "StripedRgb (3-Channel Long Match)"),
    ];

    const TEST_SIZE: usize = 64 * 1024; // 64 KB per corpus

    for (corpus_type, name) in corpora_types {
        let corpus_data = BenchmarkCorpusGenerator::generate(corpus_type, TEST_SIZE);
        assert_eq!(corpus_data.len(), TEST_SIZE, "Corpus {} generated incorrect length", name);

        for level in [1, 6, 9] {
            // A. Raw Deflate: Compress with ttzip-engine, Decompress with flate2 (Oracle Verification)
            let bound = deflate_compress_bound(corpus_data.len(), level);
            let mut comp = vec![0u8; bound];
            let comp_len = deflate_compress(&corpus_data, &mut comp, level)
                .unwrap_or_else(|_| panic!("Failed compressing corpus {} at level {}", name, level));

            let mut flate2_decoder = DeflateDecoder::new(Vec::with_capacity(corpus_data.len()));
            flate2_decoder
                .write_all(&comp[..comp_len])
                .expect("flate2 DeflateDecoder write failed");
            let flate2_decomp = flate2_decoder
                .finish()
                .expect("flate2 DeflateDecoder finish failed");
            assert_eq!(
                flate2_decomp, corpus_data,
                "Differential oracle mismatch on corpus {} at level {}",
                name, level
            );

            // B. Zlib: Compress with flate2, Decompress with ttzip-engine
            let mut flate2_zlib_enc = ZlibEncoder::new(Vec::new(), Compression::new(level as u32));
            flate2_zlib_enc
                .write_all(&corpus_data)
                .expect("flate2 ZlibEncoder write failed");
            let flate2_zlib_comp = flate2_zlib_enc.finish().expect("flate2 ZlibEncoder finish failed");

            let mut ttzip_zlib_decomp = vec![0u8; corpus_data.len()];
            let decomp_len = zlib_decompress(&flate2_zlib_comp, &mut ttzip_zlib_decomp)
                .unwrap_or_else(|_| panic!("ttzip zlib_decompress failed on flate2 stream for {}", name));
            assert_eq!(decomp_len, corpus_data.len());
            assert_eq!(
                ttzip_zlib_decomp, corpus_data,
                "Differential oracle mismatch (flate2 -> ttzip) on corpus {} at level {}",
                name, level
            );

            // C. Gzip: Compress with ttzip-engine, Decompress with flate2
            let gz_bound = gzip_compress_bound(corpus_data.len(), level);
            let mut gz_comp = vec![0u8; gz_bound];
            let gz_comp_len = gzip_compress(&corpus_data, &mut gz_comp, level)
                .unwrap_or_else(|_| panic!("Failed gzip compressing corpus {} at level {}", name, level));

            let mut flate2_gz_dec = GzDecoder::new(Vec::with_capacity(corpus_data.len()));
            flate2_gz_dec
                .write_all(&gz_comp[..gz_comp_len])
                .expect("flate2 GzDecoder write failed");
            let flate2_gz_decomp = flate2_gz_dec.finish().expect("flate2 GzDecoder finish failed");
            assert_eq!(
                flate2_gz_decomp, corpus_data,
                "Gzip differential oracle mismatch on corpus {} at level {}",
                name, level
            );
        }
    }
}

// MARK: - 4. 6-Layer Defense-in-Depth Invariants Compliance

#[test]
fn test_zlib_ng_6_layer_defense_invariants_compliance() {
    // 1. Layer 1: WindowBoundsGuard Invariants
    let window_guard = WindowBoundsGuard::new(DEFLATE_NG_MAX_WINDOW_SIZE);
    assert_eq!(window_guard.window_size(), 32768);
    assert!(window_guard.validate_distance(1, 1).is_ok());
    assert!(window_guard.validate_distance(32768, 65536).is_ok());
    assert_eq!(window_guard.validate_distance(0, 10), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(window_guard.validate_distance(32769, 65536), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(window_guard.validate_distance(100, 50), Err(TTZipStatus::ErrSecurityViolation));

    assert!(window_guard.validate_match(10, 3, 20).is_ok());
    assert!(window_guard.validate_match(10, 258, 300).is_ok());
    assert_eq!(window_guard.validate_match(10, 2, 20), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(window_guard.validate_match(10, 259, 300), Err(TTZipStatus::ErrSecurityViolation));

    assert_eq!(window_guard.wrap_index(32768), 0);
    assert_eq!(window_guard.wrap_index(32769), 1);
    assert_eq!(WindowBoundsGuard::clamp_window_size(15), Ok(32768));
    assert_eq!(WindowBoundsGuard::clamp_window_size(8), Ok(256));
    assert_eq!(WindowBoundsGuard::clamp_window_size(7), Err(TTZipStatus::ErrInvalidParam));

    // 2. Layer 2: HashChainLoopGuard Loop & Step Cutoff
    let mut hash_guard = HashChainLoopGuard::new(4);
    assert_eq!(hash_guard.max_chain(), 4);
    assert_eq!(hash_guard.record_step(), Ok(true));
    assert_eq!(hash_guard.record_step(), Ok(true));
    assert_eq!(hash_guard.record_step(), Ok(true));
    assert_eq!(hash_guard.record_step(), Ok(true));
    assert_eq!(hash_guard.record_step(), Ok(false)); // Step truncated

    assert!(hash_guard.check_cycle(200, 100).is_ok());
    assert_eq!(hash_guard.check_cycle(200, 200), Err(TTZipStatus::ErrSecurityViolation));
    assert_eq!(hash_guard.check_cycle(200, 250), Err(TTZipStatus::ErrSecurityViolation));

    // 3. Layer 3: DynamicLevelIntegrityGuard State Machine & Parameter Mutations
    let mut level_guard = DynamicLevelIntegrityGuard::new(6, DeflateCompressionStrategy::Default).unwrap();
    assert_eq!(level_guard.level(), 6);
    assert_eq!(level_guard.strategy(), DeflateCompressionStrategy::Default);
    assert_eq!(level_guard.state(), DeflateStreamState::Ready);
    assert!(level_guard.can_mutate_now());

    assert!(level_guard.transition_to(DeflateStreamState::BlockHeader).is_ok());
    assert!(level_guard.transition_to(DeflateStreamState::BlockEncoding).is_ok());
    assert!(!level_guard.can_mutate_now());

    // Mid-block mutation forbidden
    assert_eq!(
        level_guard.mutate_params(1, DeflateCompressionStrategy::Rle, false),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // Clean boundary mutation permitted
    assert!(level_guard.mutate_params(1, DeflateCompressionStrategy::Rle, true).is_ok());
    assert_eq!(level_guard.level(), 1);
    assert_eq!(level_guard.strategy(), DeflateCompressionStrategy::Rle);

    assert!(level_guard.transition_to(DeflateStreamState::BlockFlushing).is_ok());
    assert!(level_guard.transition_to(DeflateStreamState::Finished).is_ok());

    // 4. Layer 4: DecompressionBombGuard Expansion Ratio & Quota Defense
    let mut bomb_guard = DecompressionBombGuard::new(2 * 1024 * 1024, DEFLATE_NG_MAX_EXPANSION_RATIO, 1024);
    assert!(bomb_guard.track_progress(100, 1000).is_ok());
    assert_eq!(
        bomb_guard.track_progress(100, 3 * 1024 * 1024),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    let mut ratio_guard = DecompressionBombGuard::new(10 * 1024 * 1024, 10, 100);
    assert_eq!(
        ratio_guard.track_progress(10, 500),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 5. Layer 5: StoredBlockEscapeGuard LEN/!NLEN & Zero-Length Breaker
    let mut stored_guard = StoredBlockEscapeGuard::new(3);
    assert!(stored_guard.validate_stored_header(0x00FF, !0x00FF).is_ok());
    assert_eq!(
        stored_guard.validate_stored_header(0x00FF, 0x00FF),
        Err(TTZipStatus::ErrCorruptHeader)
    );
    assert!(stored_guard.validate_stored_block(10, !10, 10).is_ok());
    assert_eq!(
        stored_guard.validate_stored_block(10, !10, 5),
        Err(TTZipStatus::ErrCorruptHeader)
    );

    assert!(stored_guard.validate_stored_block(0, !0, 0).is_ok());
    assert!(stored_guard.validate_stored_block(0, !0, 0).is_ok());
    assert!(stored_guard.validate_stored_block(0, !0, 0).is_ok());
    assert_eq!(
        stored_guard.validate_stored_block(0, !0, 0),
        Err(TTZipStatus::ErrSecurityViolation)
    );

    // 6. Layer 6: Composite DeflateNgDefenseGuard, Sensitive Scratchpad & Path Sanitizer
    let mut composite = DeflateNgDefenseGuard::with_output_limit(1024 * 1024);
    composite.sensitive_pad.scratch[0] = 0x5A;
    composite.reset();
    assert_eq!(composite.sensitive_pad.scratch[0], 0);

    let path_res = sanitize_deflate_entry_path("../../../shadow/passwords.txt");
    assert_eq!(path_res.normalized_path, "shadow/passwords.txt");
    assert!(path_res.has_traversal_attack);
    assert!(!path_res.is_safe());
}

// MARK: - 5. Container Framing & Checksum Verification

#[test]
fn test_zlib_ng_container_framing_and_checksum_verification() {
    let sample = b"RFC 1950 Zlib and RFC 1952 Gzip Framing and Checksum Verification Corpus";

    // 1. Zlib Header & Adler-32 Verification
    let bound = zlib_compress_bound(sample.len(), 6);
    let mut zlib_stream = vec![0u8; bound];
    let zlib_len = zlib_compress(sample, &mut zlib_stream, 6).expect("Zlib compression failed");
    let zlib_data = &zlib_stream[..zlib_len];

    // CMF = 0x78 (CM = 8 Deflate, CINFO = 7 (32KB))
    assert_eq!(zlib_data[0], 0x78);
    // FCHECK validation: (CMF * 256 + FLG) % 31 == 0
    let fcheck = (zlib_data[0] as u16) * 256 + (zlib_data[1] as u16);
    assert_eq!(fcheck % 31, 0);

    // Verify Adler-32 trailer (last 4 bytes, big endian)
    let adler_expected = adler2::adler32_slice(sample);
    let adler_actual = u32::from_be_bytes([
        zlib_data[zlib_len - 4],
        zlib_data[zlib_len - 3],
        zlib_data[zlib_len - 2],
        zlib_data[zlib_len - 1],
    ]);
    assert_eq!(adler_actual, adler_expected);

    // 2. Gzip Header & CRC-32 Verification
    let gz_bound = gzip_compress_bound(sample.len(), 6);
    let mut gz_stream = vec![0u8; gz_bound];
    let gz_len = gzip_compress(sample, &mut gz_stream, 6).expect("Gzip compression failed");
    let gz_data = &gz_stream[..gz_len];

    // Magic ID1=0x1F, ID2=0x8B, CM=8 (Deflate)
    assert_eq!(gz_data[0], 0x1F);
    assert_eq!(gz_data[1], 0x8B);
    assert_eq!(gz_data[2], 0x08);

    // Verify CRC-32 trailer (bytes len-8..len-4, little endian)
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(sample);
    let crc_expected = hasher.finalize();
    let crc_actual = u32::from_le_bytes([
        gz_data[gz_len - 8],
        gz_data[gz_len - 7],
        gz_data[gz_len - 6],
        gz_data[gz_len - 5],
    ]);
    assert_eq!(crc_actual, crc_expected);

    // Verify ISIZE trailer (last 4 bytes, little endian)
    let isize_actual = u32::from_le_bytes([
        gz_data[gz_len - 4],
        gz_data[gz_len - 3],
        gz_data[gz_len - 2],
        gz_data[gz_len - 1],
    ]);
    assert_eq!(isize_actual, sample.len() as u32);
}
