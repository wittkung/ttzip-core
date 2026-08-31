// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive compliance test suite verifying 25+ official XZ scenarios against the
//! Tukaani XZ specification and reference test files.
//!
//! Covers:
//! 1. Official upstream good files: Check types (None, CRC32, CRC64, SHA-256), Block Headers,
//!    LZMA2 chunk transitions, and filter combinations (ARM64, Delta, PowerPC BCJ).
//! 2. Cross-architecture Multi-Block scenarios (x86, ARM, ARM64, RISC-V).
//! 3. Bigsize and concatenated multi-stream padding compliance.
//! 4. Zero-panic error interception for official corrupted (bad-*) and unsupported archives.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use ttzip_engine::xz::decoder::{xz_decompress, XzStreamDecoder};
use ttzip_engine::xz::types::XzCheckType;
use ttzip_engine::xz::writer::{
    xz_compress, XzBcjType, XzEncoderOptions, XzParallelStreamWriter,
};

/// Compute standard SHA-256 hexadecimal string of byte slice.
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Locate official test directory with fallback search paths.
fn get_official_test_dir() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("../../upstream/xz/pr1-arm64-bcj/tests/files"),
        manifest_dir.join("../../upstream/xz/pr2-arm64-crc64/tests/files"),
        manifest_dir.join("../../upstream/xz/pr3-arm64-neon-memcmplen/tests/files"),
        manifest_dir.join("vendor/xz/tests/files"),
    ];

    for candidate in &candidates {
        if candidate.is_dir() {
            return candidate.clone();
        }
    }

    panic!("Could not locate official XZ test directory in workspace");
}

/// Read test file contents from the official test directory.
fn read_test_file(filename: &str) -> Vec<u8> {
    let dir = get_official_test_dir();
    let file_path = dir.join(filename);
    fs::read(&file_path).unwrap_or_else(|e| {
        panic!("Failed to read test file {}: {}", file_path.display(), e)
    })
}

// -----------------------------------------------------------------------------
// 1. Official Good Files: Checksum Variants & Empty Streams
// -----------------------------------------------------------------------------

#[test]
fn test_official_good_empty_stream_variants() {
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let empty_cases = [
        "good-0-empty.xz",
        "good-0cat-empty.xz",
        "good-0catpad-empty.xz",
        "good-0pad-empty.xz",
        "good-1-lzma2-5.xz",
    ];

    for name in empty_cases {
        let compressed = read_test_file(name);
        let decompressed = xz_decompress(&compressed)
            .unwrap_or_else(|e| panic!("Failed to decompress official empty file {}: {:?}", name, e));

        assert!(decompressed.is_empty(), "Expected empty output for {}", name);
        assert_eq!(compute_sha256(&decompressed), EMPTY_SHA256);
    }
}

#[test]
fn test_official_good_checksum_matrix() {
    // Expected SHA-256 for standard 2-chunk uncompressed test payload
    const EXPECTED_HASH: &str = "8e5935e7e13368cd9688fe8f48a0955293676a021562582c7e848dafe13fb046";
    let check_cases = [
        "good-1-check-none.xz",
        "good-1-check-crc32.xz",
        "good-1-check-crc64.xz",
        "good-1-check-sha256.xz",
    ];

    for name in check_cases {
        let compressed = read_test_file(name);
        let decompressed = xz_decompress(&compressed)
            .unwrap_or_else(|e| panic!("Failed to decompress checksum case {}: {:?}", name, e));

        assert_eq!(
            compute_sha256(&decompressed),
            EXPECTED_HASH,
            "Fidelity mismatch on checksum test file {}",
            name
        );
    }
}

// -----------------------------------------------------------------------------
// 2. Official Good Files: Block Headers & LZMA2 Chunk Transitions
// -----------------------------------------------------------------------------

#[test]
fn test_official_good_block_header_variants() {
    const EXPECTED_HASH: &str = "8e5935e7e13368cd9688fe8f48a0955293676a021562582c7e848dafe13fb046";
    let header_cases = [
        "good-1-block_header-1.xz",
        "good-1-block_header-2.xz",
        "good-1-block_header-3.xz",
        "good-2-lzma2.xz",
    ];

    for name in header_cases {
        let compressed = read_test_file(name);
        let decompressed = xz_decompress(&compressed)
            .unwrap_or_else(|e| panic!("Failed to decompress block header file {}: {:?}", name, e));

        assert_eq!(
            compute_sha256(&decompressed),
            EXPECTED_HASH,
            "Fidelity mismatch on block header test file {}",
            name
        );
    }
}

#[test]
fn test_official_good_lzma2_chunk_transitions() {
    const EXPECTED_HASH: &str = "a643326fcc1346e96252c48931c32085ac7e3304adebd2e6e25390108c0b649e";
    let lzma2_cases = [
        "good-1-lzma2-1.xz",
        "good-1-lzma2-2.xz",
        "good-1-lzma2-3.xz",
        "good-1-lzma2-4.xz",
    ];

    for name in lzma2_cases {
        let compressed = read_test_file(name);
        let decompressed = xz_decompress(&compressed)
            .unwrap_or_else(|e| panic!("Failed to decompress LZMA2 transition file {}: {:?}", name, e));

        assert_eq!(
            compute_sha256(&decompressed),
            EXPECTED_HASH,
            "Fidelity mismatch on LZMA2 chunk transition test file {}",
            name
        );
    }
}

// -----------------------------------------------------------------------------
// 3. Official Good Files: Architecture BCJ & Delta Filters
// -----------------------------------------------------------------------------

#[test]
fn test_official_good_arm64_and_delta_filters() {
    // 1. ARM64 BCJ + LZMA2
    const ARM64_HASH: &str = "022bfaefea07d1647ac4ffdebd3a3e511202664207a3cd67f5af5b32e87ef524";
    for name in ["good-1-arm64-lzma2-1.xz", "good-1-arm64-lzma2-2.xz"] {
        let compressed = read_test_file(name);
        let decompressed = xz_decompress(&compressed)
            .unwrap_or_else(|e| panic!("Failed to decompress ARM64 filter file {}: {:?}", name, e));
        assert_eq!(
            compute_sha256(&decompressed),
            ARM64_HASH,
            "Fidelity mismatch on ARM64 test file {}",
            name
        );
    }

    // 2. Delta + LZMA2 (TIFF Image)
    const TIFF_HASH: &str = "ecfff4f7718dc4fcf0c5dba8813c29a2648053f9efbbc17d7946d2c37092885a";
    let tiff_comp = read_test_file("good-1-delta-lzma2.tiff.xz");
    let tiff_decomp = xz_decompress(&tiff_comp)
        .expect("Failed to decompress Delta+LZMA2 tiff file");
    assert_eq!(compute_sha256(&tiff_decomp), TIFF_HASH);

    // 3. 3x Delta + LZMA2
    const THREE_DELTA_HASH: &str = "a643326fcc1346e96252c48931c32085ac7e3304adebd2e6e25390108c0b649e";
    let delta3_comp = read_test_file("good-1-3delta-lzma2.xz");
    let delta3_decomp = xz_decompress(&delta3_comp)
        .expect("Failed to decompress 3delta file");
    assert_eq!(compute_sha256(&delta3_decomp), THREE_DELTA_HASH);

    // 4. PowerPC BCJ empty block
    let ppc_comp = read_test_file("good-1-empty-bcj-lzma2.xz");
    let ppc_decomp = xz_decompress(&ppc_comp)
        .expect("Failed to decompress PowerPC empty block");
    assert!(ppc_decomp.is_empty());
}

// -----------------------------------------------------------------------------
// 4. Extended Compliance Matrix: Multi-Block Architectures (x86, ARM, ARM64, RISC-V)
// -----------------------------------------------------------------------------

/// Deterministic pseudo-random instruction generator.
fn generate_code_payload(size: usize, seed: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut state = seed;
    for i in 0..size {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let byte = ((state >> 16) ^ (i as u32)) as u8;
        out.push(byte);
    }
    out
}

#[test]
fn test_compliance_good_2_x86_lzma2() {
    let mut code_data = generate_code_payload(128 * 1024, 0x11223344);
    for i in (0..code_data.len().saturating_sub(5)).step_by(32) {
        code_data[i] = 0xE8;
        code_data[i + 1] = 0x20;
        code_data[i + 2] = 0x40;
        code_data[i + 3] = 0x00;
        code_data[i + 4] = 0x00;
    }
    let expected_hash = compute_sha256(&code_data);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Crc32)
        .with_bcj(XzBcjType::X86)
        .with_block_size(64 * 1024);

    let compressed = xz_compress(&code_data, &options).expect("compress x86 multi-block");
    let decompressed = xz_decompress(&compressed).expect("decompress x86 multi-block");

    assert_eq!(decompressed.len(), code_data.len());
    assert_eq!(compute_sha256(&decompressed), expected_hash);
}

#[test]
fn test_compliance_good_2_arm_lzma2() {
    let mut code_data = generate_code_payload(128 * 1024, 0x55667788);
    for i in (0..code_data.len().saturating_sub(4)).step_by(16) {
        code_data[i] = 0x34;
        code_data[i + 1] = 0x12;
        code_data[i + 2] = 0x00;
        code_data[i + 3] = 0xEB; // ARM BL opcode
    }
    let expected_hash = compute_sha256(&code_data);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Crc64)
        .with_bcj(XzBcjType::Arm)
        .with_block_size(64 * 1024);

    let compressed = xz_compress(&code_data, &options).expect("compress ARM multi-block");
    let decompressed = xz_decompress(&compressed).expect("decompress ARM multi-block");

    assert_eq!(decompressed.len(), code_data.len());
    assert_eq!(compute_sha256(&decompressed), expected_hash);
}

#[test]
fn test_compliance_good_2_arm64_lzma2() {
    let mut code_data = generate_code_payload(128 * 1024, 0x99AABBCC);
    for i in (0..code_data.len().saturating_sub(4)).step_by(16) {
        let bl_instr = 0x9400_0200u32; // ARM64 BL +0x800
        code_data[i..i + 4].copy_from_slice(&bl_instr.to_le_bytes());
    }
    let expected_hash = compute_sha256(&code_data);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Sha256)
        .with_bcj(XzBcjType::Arm64)
        .with_block_size(64 * 1024);

    let compressed = xz_compress(&code_data, &options).expect("compress ARM64 multi-block");
    let decompressed = xz_decompress(&compressed).expect("decompress ARM64 multi-block");

    assert_eq!(decompressed.len(), code_data.len());
    assert_eq!(compute_sha256(&decompressed), expected_hash);
}

#[test]
fn test_compliance_good_2_riscv_lzma2() {
    let mut code_data = generate_code_payload(128 * 1024, 0xDDEEFF00);
    for i in (0..code_data.len().saturating_sub(8)).step_by(24) {
        let jal_ra = [0xEF, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00];
        code_data[i..i + 8].copy_from_slice(&jal_ra);
    }
    let expected_hash = compute_sha256(&code_data);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Crc32)
        .with_bcj(XzBcjType::Riscv)
        .with_block_size(64 * 1024);

    let compressed = xz_compress(&code_data, &options).expect("compress RISC-V multi-block");
    let decompressed = xz_decompress(&compressed).expect("decompress RISC-V multi-block");

    assert_eq!(decompressed.len(), code_data.len());
    assert_eq!(compute_sha256(&decompressed), expected_hash);
}

// -----------------------------------------------------------------------------
// 5. Extended Compliance Matrix: Bigsize & Concatenated Multi-Stream
// -----------------------------------------------------------------------------

#[test]
fn test_compliance_good_2_bigsize() {
    // 512 KiB payload across 4 blocks of 128 KiB each
    let original = generate_code_payload(512 * 1024, 0x7E57B165);
    let expected_hash = compute_sha256(&original);

    let options = XzEncoderOptions::new()
        .with_check_type(XzCheckType::Crc64)
        .with_dict_size(1024 * 1024)
        .with_block_size(128 * 1024);

    let mut compressed_sink = Vec::new();
    let mut writer = XzParallelStreamWriter::new(&mut compressed_sink, options)
        .expect("create parallel writer");
    writer.write_parallel(&original).expect("write parallel");
    writer.finish().expect("finish writer");

    let mut decoder = XzStreamDecoder::new(Cursor::new(&compressed_sink));
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).expect("streaming read bigsize");

    assert_eq!(decompressed.len(), original.len());
    assert_eq!(compute_sha256(&decompressed), expected_hash);
    assert_eq!(decoder.cumulative_records().len(), 4);
}

#[test]
fn test_compliance_good_multi_stream_padding() {
    let s1 = b"TTZip Compliance Stream Alpha: Verifying official multi-stream boundary.".to_vec();
    let s2 = b"TTZip Compliance Stream Beta: Testing 4-byte stream padding multiples.".to_vec();

    let c1 = xz_compress(&s1, &XzEncoderOptions::new().with_check_type(XzCheckType::Crc32))
        .expect("compress s1");
    let c2 = xz_compress(&s2, &XzEncoderOptions::new().with_check_type(XzCheckType::Sha256))
        .expect("compress s2");

    let mut chained = Vec::new();
    chained.extend_from_slice(&c1);
    chained.extend_from_slice(&[0x00; 8]); // 2 words padding
    chained.extend_from_slice(&c2);
    chained.extend_from_slice(&[0x00; 4]); // 1 word trailing padding

    let mut decoder = XzStreamDecoder::new(Cursor::new(&chained));
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).expect("decode multi stream chained");

    let mut expected = Vec::new();
    expected.extend_from_slice(&s1);
    expected.extend_from_slice(&s2);

    assert_eq!(decompressed, expected);
}

// -----------------------------------------------------------------------------
// 6. Official Corrupted (bad-*) & Unsupported Archives Zero-Panic Rejection
// -----------------------------------------------------------------------------

#[test]
fn test_official_bad_header_and_footer_magic_rejection() {
    let bad_magic_files = [
        "bad-0-header_magic.xz",
        "bad-0-footer_magic.xz",
        "bad-0cat-header_magic.xz",
    ];

    for name in bad_magic_files {
        let comp = read_test_file(name);
        let res = xz_decompress(&comp);
        assert!(res.is_err(), "Bad magic file {} must be rejected", name);
    }
}

#[test]
fn test_official_bad_padding_and_truncation_rejection() {
    let bad_padding_files = [
        "bad-0pad-empty.xz",
        "bad-0catpad-empty.xz",
        "bad-0-empty-truncated.xz",
        "bad-2-compressed_data_padding.xz",
    ];

    for name in bad_padding_files {
        let comp = read_test_file(name);
        let res = xz_decompress(&comp);
        assert!(res.is_err(), "Bad padding/truncation file {} must be rejected", name);
    }
}

#[test]
fn test_official_bad_indices_rejection() {
    let bad_index_files = [
        "bad-0-nonempty_index.xz",
        "bad-0-backward_size.xz",
        "bad-0-index-1.xz",
        "bad-2-index-1.xz",
        "bad-2-index-2.xz",
        "bad-2-index-3.xz",
        "bad-2-index-4.xz",
        "bad-2-index-5.xz",
    ];

    for name in bad_index_files {
        let comp = read_test_file(name);
        let res = xz_decompress(&comp);
        assert!(res.is_err(), "Bad index file {} must be rejected", name);
    }
}

#[test]
fn test_official_bad_block_headers_and_vli_rejection() {
    let bad_header_files = [
        "bad-1-block_header-1.xz",
        "bad-1-block_header-2.xz",
        "bad-1-block_header-3.xz",
        "bad-1-block_header-4.xz",
        "bad-1-block_header-5.xz",
        "bad-1-block_header-6.xz",
        "bad-1-vli-1.xz",
        "bad-1-vli-2.xz",
    ];

    for name in bad_header_files {
        let comp = read_test_file(name);
        let res = xz_decompress(&comp);
        assert!(res.is_err(), "Bad block header / VLI file {} must be rejected", name);
    }
}

#[test]
fn test_official_bad_checks_and_payloads_rejection() {
    let bad_check_and_payload_files = [
        "bad-1-check-crc32.xz",
        "bad-1-check-crc32-2.xz",
        "bad-1-check-crc64.xz",
        "bad-1-check-sha256.xz",
        "bad-1-lzma2-1.xz",
        "bad-1-lzma2-2.xz",
        "bad-1-lzma2-3.xz",
        "bad-1-lzma2-4.xz",
        "bad-1-lzma2-5.xz",
        "bad-1-lzma2-6.xz",
        "bad-1-lzma2-7.xz",
        "bad-1-lzma2-8.xz",
        "bad-1-lzma2-9.xz",
        "bad-1-lzma2-10.xz",
        "bad-1-lzma2-11.xz",
    ];

    for name in bad_check_and_payload_files {
        let comp = read_test_file(name);
        let res = xz_decompress(&comp);
        assert!(res.is_err(), "Bad checksum or LZMA2 payload {} must be rejected", name);
    }
}

#[test]
fn test_official_unsupported_archives_interception() {
    let unsupported_files = [
        "unsupported-block_header.xz",
        "unsupported-filter_flags-1.xz",
        "unsupported-filter_flags-2.xz",
        "unsupported-filter_flags-3.xz",
    ];

    for name in unsupported_files {
        let comp = read_test_file(name);
        let res = xz_decompress(&comp);
        assert!(res.is_err(), "Unsupported feature file {} must return Err", name);
    }
}
