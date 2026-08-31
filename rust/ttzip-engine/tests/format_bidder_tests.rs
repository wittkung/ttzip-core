// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive test suite for strongly typed FormatBidderRegistry,
//! libarchive-inspired scoring hierarchy, and 20+ format candidate disambiguation.

use std::io::Cursor;
use ttzip_engine::archive::unified::format_sniffer::{
    ArchiveFormat, BidScore, FormatBidderRegistry,
};

/// Helper to generate synthetic valid TAR header blocks with verified octal checksums.
fn create_valid_tar_header(name: &str, magic: Option<&[u8]>) -> Vec<u8> {
    let mut block = vec![0u8; 512];
    let name_bytes = name.as_bytes();
    block[..name_bytes.len().min(100)].copy_from_slice(&name_bytes[..name_bytes.len().min(100)]);
    block[100..108].copy_from_slice(b"0000644\0");
    block[108..116].copy_from_slice(b"0001750\0");
    block[116..124].copy_from_slice(b"0001750\0");
    block[124..136].copy_from_slice(b"00000000000\0");
    block[136..148].copy_from_slice(b"14000000000\0");

    if let Some(m) = magic {
        block[257..257 + m.len()].copy_from_slice(m);
    }

    let mut sum: u32 = 0;
    for (i, &b) in block.iter().enumerate() {
        let val = if (148..156).contains(&i) { b' ' } else { b };
        sum += val as u32;
    }
    let chksum_str = format!("{:06o}\0 ", sum);
    block[148..156].copy_from_slice(chksum_str.as_bytes());

    block
}

#[test]
fn test_all_20_plus_format_bidders_head_probe() {
    let registry = FormatBidderRegistry::new();

    let test_matrix: Vec<(&str, Vec<u8>, ArchiveFormat, BidScore)> = vec![
        // 1. Tar formats (USTAR, GNU, V7)
        (
            "tar_ustar",
            create_valid_tar_header("test.txt", Some(b"ustar\0")),
            ArchiveFormat::Tar,
            BidScore::TAR_USTAR,
        ),
        (
            "tar_gnu",
            create_valid_tar_header("test.txt", Some(b"ustar  \0")),
            ArchiveFormat::Tar,
            BidScore::TAR_GNU,
        ),
        (
            "tar_v7",
            create_valid_tar_header("test.txt", None),
            ArchiveFormat::Tar,
            BidScore::TAR_V7,
        ),

        // 2. High confidence containers
        (
            "7z",
            vec![0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, 0x00, 0x04],
            ArchiveFormat::SevenZip,
            BidScore::SEVEN_ZIP,
        ),
        (
            "xar",
            b"xar!\x00\x1c\x00\x01\x00\x00\x00\x00".to_vec(),
            ArchiveFormat::Xar,
            BidScore::XAR,
        ),
        (
            "cab",
            b"MSCF\x00\x00\x00\x00\x20\x00\x00\x00".to_vec(),
            ArchiveFormat::Cab,
            BidScore::CAB,
        ),
        (
            "rar4",
            vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00, 0x00],
            ArchiveFormat::Rar4,
            BidScore::RAR4,
        ),
        (
            "rar5",
            vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00],
            ArchiveFormat::Rar5,
            BidScore::RAR5,
        ),
        (
            "warc",
            b"WARC/1.0\r\nWARC-Type: warcinfo\r\n".to_vec(),
            ArchiveFormat::Warc,
            BidScore::WARC,
        ),
        (
            "wim",
            b"MSWIM\0\0\0\x00\x00\x00\x00".to_vec(),
            ArchiveFormat::Wim,
            BidScore::WIM,
        ),
        (
            "ar",
            b"!<arch>\nfile.txt        1400000000  501   20    100644  0         `\n".to_vec(),
            ArchiveFormat::Ar,
            BidScore::AR,
        ),
        (
            "squashfs_le",
            b"hsqs\x00\x00\x00\x00".to_vec(),
            ArchiveFormat::Squashfs,
            BidScore::SQUASHFS,
        ),
        (
            "aar",
            b"AA01\x00\x00\x00\x00".to_vec(),
            ArchiveFormat::Aar,
            BidScore::AAR,
        ),

        // 3. Single-stream compression
        (
            "xz",
            vec![0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00],
            ArchiveFormat::Xz,
            BidScore::XZ,
        ),
        (
            "gzip",
            vec![0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00],
            ArchiveFormat::Gzip,
            BidScore::GZIP,
        ),
        (
            "bzip2",
            b"BZh91AY&SY".to_vec(),
            ArchiveFormat::Bzip2,
            BidScore::BZIP2,
        ),
        (
            "zstd",
            vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00],
            ArchiveFormat::Zstd,
            BidScore::ZSTD,
        ),
        (
            "lzip",
            b"LZIP\x01\x0c".to_vec(),
            ArchiveFormat::Lzip,
            BidScore::new(48),
        ),
        (
            "lz4_frame",
            vec![0x04, 0x22, 0x4D, 0x18, 0x64, 0x70, 0xB9],
            ArchiveFormat::Lz4,
            BidScore::new(48),
        ),
        (
            "snappy",
            b"\xFF\x06\x00\x00sNaPpY\x00\x00".to_vec(),
            ArchiveFormat::Snappy,
            BidScore::new(48),
        ),
        (
            "lzfse",
            b"bvx-\x00\x00\x00\x00".to_vec(),
            ArchiveFormat::Lzfse,
            BidScore::new(48),
        ),
        (
            "squashfs",
            b"hsqs\x00\x00\x00\x00".to_vec(),
            ArchiveFormat::Squashfs,
            BidScore::SQUASHFS,
        ),
        (
            "arj",
            vec![0x60, 0xEA, 0x1E, 0x00],
            ArchiveFormat::Arj,
            BidScore::ARJ,
        ),

        // 4. Special containers and text manifests
        (
            "cpio_newc",
            b"07070100000000".to_vec(),
            ArchiveFormat::Cpio,
            BidScore::CPIO,
        ),
        (
            "cpio_bin_le",
            vec![0xC7, 0x71, 0x00, 0x00],
            ArchiveFormat::Cpio,
            BidScore::CPIO,
        ),
        (
            "lha",
            b"\x00\x00-lh5-\x00\x00".to_vec(),
            ArchiveFormat::Lzh,
            BidScore::LHA,
        ),
        (
            "mtree",
            b"#mtree\n/set type=file uid=0 gid=0\n".to_vec(),
            ArchiveFormat::Mtree,
            BidScore::MTREE,
        ),
        (
            "zip_stream",
            b"PK\x03\x04\x14\x00\x00\x00".to_vec(),
            ArchiveFormat::Zip,
            BidScore::ZIP_STREAMABLE,
        ),
        (
            "iso9660_head",
            {
                let mut b = vec![0u8; 32768 + 16];
                b[32769..32769 + 5].copy_from_slice(b"CD001");
                b
            },
            ArchiveFormat::Iso,
            BidScore::ISO9660,
        ),
    ];

    for (name, buf, expected_fmt, expected_score) in test_matrix {
        let result = registry.bid(&buf);
        assert_eq!(
            result.format, expected_fmt,
            "Format mismatch for {}: expected {:?}, got {:?}",
            name, expected_fmt, result.format
        );
        assert_eq!(
            result.score, expected_score,
            "Score mismatch for {}: expected {:?}, got {:?}",
            name, expected_score, result.score
        );
        assert!(result.is_matched(), "Result must be matched for {}", name);
    }
}

#[test]
fn test_seekable_zip_and_dmg_arbitration() {
    let registry = FormatBidderRegistry::new();

    // 1. ZIP with Local Header at start (score 29) + EOCD at end (score 32)
    let mut zip_data = vec![0u8; 1024];
    zip_data[..4].copy_from_slice(b"PK\x03\x04");
    zip_data[1024 - 22..1024 - 18].copy_from_slice(b"PK\x05\x06");

    let mut cursor = Cursor::new(zip_data);
    let result = registry.bid_seekable(&mut cursor).unwrap();

    assert_eq!(result.format, ArchiveFormat::Zip);
    assert_eq!(result.score, BidScore::ZIP_SEEKABLE); // 32 beats streamable 29!

    // 2. DMG with 'koly' trailer at the last 512 bytes
    let mut dmg_data = vec![0u8; 2048];
    dmg_data[2048 - 512..2048 - 508].copy_from_slice(b"koly");

    let mut dmg_cursor = Cursor::new(dmg_data);
    let dmg_res = registry.bid_seekable(&mut dmg_cursor).unwrap();

    assert_eq!(dmg_res.format, ArchiveFormat::Dmg);
    assert_eq!(dmg_res.score, BidScore::DMG);
}

#[test]
fn test_empty_and_zero_noise_rejection() {
    let registry = FormatBidderRegistry::new();

    // 1. Completely empty buffer
    let empty_res = registry.bid(&[]);
    assert_eq!(empty_res.format, ArchiveFormat::Empty);
    assert_eq!(empty_res.score, BidScore::EMPTY);

    // 2. 512-byte block of all zeros (e.g. tar padding or zeroed file)
    let zero_512 = vec![0u8; 512];
    let zero_res = registry.bid(&zero_512);
    assert_eq!(zero_res.format, ArchiveFormat::Empty);
    assert_eq!(zero_res.score, BidScore::EMPTY); // 10 points

    // 3. Random noise (does not match any magic, falls back to Raw with score 1)
    let random_noise = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xFE, 0xED, 0xFA, 0xCE, 0x12, 0x34];
    let noise_res = registry.bid(&random_noise);
    assert_eq!(noise_res.format, ArchiveFormat::Raw);
    assert_eq!(noise_res.score, BidScore::FALLBACK);
}

#[test]
fn test_short_circuit_ordering_and_tar_checksum_rejection() {
    let registry = FormatBidderRegistry::new();

    // Corrupted TAR checksum: valid USTAR magic, but invalid checksum
    let mut corrupted_tar = create_valid_tar_header("test.txt", Some(b"ustar\0"));
    corrupted_tar[148] = b'9'; // Break octal checksum

    let result = registry.bid(&corrupted_tar);
    // Because checksum is invalid, TAR bidder returns NONE, falling through to Raw
    assert_ne!(result.format, ArchiveFormat::Tar);
    assert_eq!(result.format, ArchiveFormat::Raw);
    assert_eq!(result.score, BidScore::FALLBACK);
}
