// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Corrupted Zip64 Modulo Overflow and LZMA 4GB OOM Dictionary Bomb Test Suite.
//!
//! Validates:
//! 1. Zip64 truncation and 32-bit modulo integer overflow attacks (2^32+5 uncompressed size).
//! 2. ZIP local header vs CDFH size mismatch and truncated payload boundaries.
//! 3. LZMA 4GB OOM dictionary bombs (0xFFFFFFFF dictionary size memory quota guards).
//! 4. 7z malformed entries/folders/numfiles OOM vector allocations.

use std::panic::{catch_unwind, AssertUnwindSafe};

use super::uudecode::{load_libarchive_asset, write_temp_archive};
use ttzip_engine::archive::unified::UnifiedArchiveOrchestrator;
use ttzip_engine::codecs::lzma::alone::{lzma1_decompress, LzmaAloneDecoder, MAX_LZMA_DICT_SIZE};
use ttzip_engine::sevenz::header::models::SevenZHeaderInfo;
use ttzip_engine::sevenz::header::parse_7z_header_stream;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::types::{TTZipExtractOptions, TTZipStatus};
use ttzip_engine::zip::extra::TAG_ZIP64;
use ttzip_engine::zip::parser::find_eocd;
use ttzip_engine::zip::reader::ZipArchive;

/// Builds a synthetic Zip64 archive containing an entry with declared size = 2^32 + 5.
fn build_synthetic_zip64_modulo_overflow() -> Vec<u8> {
    let mut out = Vec::new();

    // 1. Local File Header
    let lfh_start = out.len();
    out.extend_from_slice(&0x04034B50u32.to_le_bytes()); // LFH magic
    out.extend_from_slice(&45u16.to_le_bytes()); // version needed: 4.5
    out.extend_from_slice(&0u16.to_le_bytes()); // flags
    out.extend_from_slice(&0u16.to_le_bytes()); // method: STORE
    out.extend_from_slice(&0u16.to_le_bytes()); // mod time
    out.extend_from_slice(&0u16.to_le_bytes()); // mod date
    out.extend_from_slice(&0x3610A686u32.to_le_bytes()); // CRC32 of "hello"
    out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // compressed size sentinel
    out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // uncompressed size sentinel

    let filename = b"hello.txt";
    out.extend_from_slice(&(filename.len() as u16).to_le_bytes());

    // Zip64 extra in LFH: id (2) + size (2) + uncompressed (8) + compressed (8)
    let extra_len: u16 = 20;
    out.extend_from_slice(&extra_len.to_le_bytes());
    out.extend_from_slice(filename);

    out.extend_from_slice(&TAG_ZIP64.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(&(4294967301u64).to_le_bytes()); // 2^32 + 5
    out.extend_from_slice(&(5u64).to_le_bytes()); // 5 real bytes

    // Payload: only 5 real bytes "hello"
    out.extend_from_slice(b"hello");

    // 2. Central Directory File Header
    let cd_start = out.len();
    out.extend_from_slice(&0x02014B50u32.to_le_bytes()); // CDFH magic
    out.extend_from_slice(&45u16.to_le_bytes()); // version made by
    out.extend_from_slice(&45u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0u16.to_le_bytes()); // flags
    out.extend_from_slice(&0u16.to_le_bytes()); // method: STORE
    out.extend_from_slice(&0u16.to_le_bytes()); // mod time
    out.extend_from_slice(&0u16.to_le_bytes()); // mod date
    out.extend_from_slice(&0x3610A686u32.to_le_bytes()); // CRC32 of "hello"
    out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // comp size sentinel
    out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // uncomp size sentinel
    out.extend_from_slice(&(filename.len() as u16).to_le_bytes());

    let cd_extra_len: u16 = 28;
    out.extend_from_slice(&cd_extra_len.to_le_bytes()); // extra len
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out.extend_from_slice(&0u16.to_le_bytes()); // disk start
    out.extend_from_slice(&0u16.to_le_bytes()); // int attr
    out.extend_from_slice(&0u32.to_le_bytes()); // ext attr
    out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // lfh offset sentinel

    out.extend_from_slice(filename);

    // Zip64 extra in CDFH: uncomp (8) + comp (8) + lfh offset (8)
    out.extend_from_slice(&TAG_ZIP64.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&(4294967301u64).to_le_bytes()); // 2^32 + 5
    out.extend_from_slice(&(5u64).to_le_bytes());
    out.extend_from_slice(&(lfh_start as u64).to_le_bytes());

    let cd_end = out.len();
    let cd_size = (cd_end - cd_start) as u64;

    // 3. Zip64 EOCD Record
    let z64_eocd_start = out.len();
    out.extend_from_slice(&0x06064B50u32.to_le_bytes());
    out.extend_from_slice(&44u64.to_le_bytes()); // size of z64 eocd remaining
    out.extend_from_slice(&45u16.to_le_bytes()); // version made by
    out.extend_from_slice(&45u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0u32.to_le_bytes()); // disk num
    out.extend_from_slice(&0u32.to_le_bytes()); // disk cd start
    out.extend_from_slice(&1u64.to_le_bytes()); // entries on this disk
    out.extend_from_slice(&1u64.to_le_bytes()); // total entries
    out.extend_from_slice(&cd_size.to_le_bytes()); // cd size
    out.extend_from_slice(&(cd_start as u64).to_le_bytes()); // cd offset

    // 4. Zip64 EOCD Locator
    out.extend_from_slice(&0x07064B50u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // disk with z64 eocd
    out.extend_from_slice(&(z64_eocd_start as u64).to_le_bytes()); // z64 eocd offset
    out.extend_from_slice(&1u32.to_le_bytes()); // total disks

    // 5. Standard EOCD
    out.extend_from_slice(&0x06054B50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk num
    out.extend_from_slice(&0u16.to_le_bytes()); // disk cd start
    out.extend_from_slice(&0xFFFFu16.to_le_bytes()); // entries on disk
    out.extend_from_slice(&0xFFFFu16.to_le_bytes()); // total entries
    out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // cd size
    out.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // cd offset
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len

    out
}

#[test]
pub fn test_corrupted_zip64_uncompressed_size_off_by_4gib_real_asset() {
    let asset_bytes = load_libarchive_asset("test_read_format_zip_uncompressed_size_off_by_4gib.zip");
    assert!(
        asset_bytes.is_some(),
        "libarchive test_read_format_zip_uncompressed_size_off_by_4gib.zip fixture must load"
    );
    let bytes = asset_bytes.unwrap();

    // 1. Pure Rust Zip parser inspection
    let parse_res = ZipArchive::open_slice(&bytes);
    assert!(parse_res.is_ok(), "Archive header parsing should parse CDFH entry");
    let zip = parse_res.unwrap();
    assert_eq!(zip.len(), 1);
    let entry = &zip.entries()[0];
    assert_eq!(entry.rel_path, "hello.txt");
    assert_eq!(entry.uncompressed_size, 4294967301);

    // 2. Pure Rust extraction must be safely intercepted (OOM / size mismatch guard)
    let ext_res = zip.extract_entry_bytes(0, None);
    assert!(
        ext_res.is_err(),
        "Extraction of 2^32+5 entry must fail and never report success on truncated 5 bytes"
    );
    match ext_res {
        Err(TTZipStatus::ErrOutOfMemory) | Err(TTZipStatus::ErrCorruptHeader) => {}
        other => panic!("Expected ErrOutOfMemory or ErrCorruptHeader, got {:?}", other),
    }

    // 3. Unified Orchestrator extraction must reject and not write fraudulent gigabytes
    let (_tmp_dir, arch_path) = write_temp_archive("off_by_4gib.zip", &bytes);
    let dest_dir = tempfile::tempdir().unwrap();
    let options = TTZipExtractOptions::default();
    let orch_res = UnifiedArchiveOrchestrator::extract_archive(&arch_path, dest_dir.path(), &options);
    assert!(
        orch_res.is_err(),
        "Unified orchestrator must reject corrupted off-by-4gib archive"
    );
}

#[test]
pub fn test_corrupted_zip64_synthetic_modulo_overflow_defense() {
    let synthetic_bytes = build_synthetic_zip64_modulo_overflow();
    assert!(!synthetic_bytes.is_empty());

    let zip = ZipArchive::open_slice(&synthetic_bytes).expect("synthetic zip must parse headers");
    assert_eq!(zip.entries()[0].uncompressed_size, 4294967301);

    let res = zip.extract_entry_bytes(0, None);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrOutOfMemory),
        "Memory quota must reject > 256MB uncompressed entry allocation"
    );
}

#[test]
pub fn test_corrupted_zip64_bad_locator_and_truncated_eocd() {
    let base = build_synthetic_zip64_modulo_overflow();

    // Corrupt Zip64 locator offset to point past EOF
    let mut bad_locator = base.clone();
    let loc_pos = bad_locator.len() - 42; // Zip64 locator is 20 bytes before standard EOCD (22 bytes)
    bad_locator[loc_pos + 8..loc_pos + 16].copy_from_slice(&0x7FFFFFFFFFFFFFFFu64.to_le_bytes());

    let parse_res = find_eocd(&bad_locator);
    assert!(
        parse_res.is_ok(),
        "Should fall back gracefully or safely parse standard EOCD without panic"
    );

    // Truncate archive right at CDFH boundary
    let truncated = &base[..base.len() / 2];
    let trunc_res = ZipArchive::open_slice(truncated);
    assert!(
        trunc_res.is_err(),
        "Truncated ZIP missing EOCD must fail with ErrCorruptHeader"
    );
}

#[test]
pub fn test_corrupted_lzma_4gb_oom_dictionary_bomb_alone() {
    // LZMA1 Alone properties: [pb/lp/lc, dict_size: u32_le]
    // Dictionary size: 0xFFFFFFFF (4GiB)
    let bomb_props = [0x5d, 0xFF, 0xFF, 0xFF, 0xFF];
    let dummy_payload = vec![0x00; 16];
    let mut out_dst = vec![0u8; 32];

    let res = lzma1_decompress(&dummy_payload, &bomb_props, 32, &mut out_dst);
    assert_eq!(
        res,
        Err(TTZipStatus::ErrOutOfMemory),
        "LZMA 4GB dictionary bomb must trigger ErrOutOfMemory"
    );

    // Dictionary size: 2GiB (0x80000000)
    let bomb_2gb = [0x5d, 0x00, 0x00, 0x00, 0x80];
    let res_2gb = lzma1_decompress(&dummy_payload, &bomb_2gb, 32, &mut out_dst);
    assert_eq!(
        res_2gb,
        Err(TTZipStatus::ErrOutOfMemory),
        "LZMA 2GB dictionary bomb must trigger ErrOutOfMemory"
    );
}

#[test]
pub fn test_corrupted_lzma_alone_decoder_memlimit_guard() {
    let mut decoder = LzmaAloneDecoder::new_with_memlimit(MAX_LZMA_DICT_SIZE as u64)
        .expect("decoder creation with 64MB limit must succeed");

    // Feed 13-byte header with 4GiB dictionary size
    let mut bomb_hdr = [0u8; 13];
    bomb_hdr[..5].copy_from_slice(&[0x5d, 0xFF, 0xFF, 0xFF, 0xFF]);
    bomb_hdr[5..13].copy_from_slice(&100u64.to_le_bytes());

    let mut out = [0u8; 64];
    let decompress_res = decoder.decompress_chunk(&bomb_hdr, &mut out, false);
    assert!(
        decompress_res.is_err(),
        "Decompressing 4GiB header under 64MB memlimit must return error"
    );
    match decompress_res {
        Err(TTZipStatus::ErrOutOfMemory) | Err(TTZipStatus::ErrCorruptHeader) => {}
        other => panic!("Expected ErrOutOfMemory or ErrCorruptHeader, got {:?}", other),
    }
}

#[test]
pub fn test_corrupted_libarchive_zipx_lzma_oom_asset() {
    if let Some(asset) = load_libarchive_asset("test_read_format_zip_zipx_lzma_oom.zipx") {
        let (_tmp, path) = write_temp_archive("lzma_oom.zipx", &asset);
        let dest = tempfile::tempdir().unwrap();
        let options = TTZipExtractOptions::default();

        let res = UnifiedArchiveOrchestrator::extract_archive(&path, dest.path(), &options);
        assert!(
            res.is_err(),
            "ZIPX LZMA 4GB dictionary bomb must fail and not cause OOM"
        );
    }
}

#[test]
pub fn test_corrupted_libarchive_7z_oom_assets() {
    let oom_fixtures = [
        "test_read_format_7zip_entries_oom.7z",
        "test_read_format_7zip_folders_oom.7z",
        "test_read_format_7zip_malformed_numfiles_oom.7z",
        "test_read_format_7zip_malformed.7z",
        "test_read_format_7zip_malformed2.7z",
        "test_read_format_7zip_malformed3.7z",
        "test_read_format_7zip_malformed4.7z",
    ];

    for fixture in &oom_fixtures {
        if let Some(bytes) = load_libarchive_asset(fixture) {
            let res = catch_unwind(AssertUnwindSafe(|| {
                let _ = SevenZArchive::open_slice(&bytes);
                let mut info = SevenZHeaderInfo::default();
                let _ = parse_7z_header_stream(&bytes, &mut info);
            }));
            assert!(
                res.is_ok(),
                "Parsing 7z corrupted asset '{}' must never panic or abort",
                fixture
            );
        }
    }
}
