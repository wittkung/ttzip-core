// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive integration tests for 50+ archive format magic numbers and 3-state sniffer.

use std::fs;
use tempfile::tempdir;
use ttzip_engine::archive::unified::format_sniffer::{
    ArchiveFormat, FormatSniffer, SniffResult,
};

#[test]
fn test_sniffer_50_plus_synthetic_headers() {
    let test_cases: Vec<(&str, Vec<u8>, ArchiveFormat)> = vec![
        // 1. Primary Archive Containers
        ("7z", {
            let mut b = vec![0u8; 32];
            b[..6].copy_from_slice(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]);
            b
        }, ArchiveFormat::SevenZip),
        ("zip_standard", {
            let mut b = vec![0u8; 30];
            b[..4].copy_from_slice(b"PK\x03\x04");
            b
        }, ArchiveFormat::Zip),
        ("zip_empty", {
            let mut b = vec![0u8; 22];
            b[..4].copy_from_slice(b"PK\x05\x06");
            b
        }, ArchiveFormat::Zip),
        ("zip64_locator", {
            let mut b = vec![0u8; 64];
            b[..4].copy_from_slice(b"PK\x03\x04");
            b[20..24].copy_from_slice(b"PK\x06\x07"); // Zip64 locator
            b
        }, ArchiveFormat::Zip64),
        ("tar_ustar", {
            let mut b = vec![0u8; 512];
            b[257..263].copy_from_slice(b"ustar\0");
            b
        }, ArchiveFormat::Tar),
        ("tar_gnu", {
            let mut b = vec![0u8; 512];
            b[257..265].copy_from_slice(b"ustar  \0");
            b
        }, ArchiveFormat::Tar),
        ("rar4", {
            let mut b = vec![0u8; 16];
            b[..7].copy_from_slice(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00]);
            b
        }, ArchiveFormat::Rar4),
        ("rar5", {
            let mut b = vec![0u8; 16];
            b[..8].copy_from_slice(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00]);
            b
        }, ArchiveFormat::Rar5),
        ("cab", {
            let mut b = vec![0u8; 32];
            b[..4].copy_from_slice(b"MSCF");
            b
        }, ArchiveFormat::Cab),
        ("wim", {
            let mut b = vec![0u8; 208];
            b[..8].copy_from_slice(b"MSWIM\0\0\0");
            b
        }, ArchiveFormat::Wim),
        ("iso_9660", {
            let mut b = vec![0u8; 32768 + 2048];
            b[0x8001..0x8001 + 5].copy_from_slice(b"CD001");
            b
        }, ArchiveFormat::Iso),
        ("udf_bea01", {
            let mut b = vec![0u8; 32768 + 2048];
            b[0x8001..0x8001 + 5].copy_from_slice(b"BEA01");
            b
        }, ArchiveFormat::Udf),
        ("udf_nsr02", {
            let mut b = vec![0u8; 40960];
            b[0x9001..0x9001 + 5].copy_from_slice(b"NSR02");
            b
        }, ArchiveFormat::Udf),
        ("udf_nsr03", {
            let mut b = vec![0u8; 40960];
            b[0x9001..0x9001 + 5].copy_from_slice(b"NSR03");
            b
        }, ArchiveFormat::Udf),
        ("dmg_koly_trailer", {
            let mut b = vec![0u8; 1024];
            b[512..516].copy_from_slice(b"koly"); // Tail 512
            b
        }, ArchiveFormat::Dmg),
        ("arj", {
            let mut b = vec![0u8; 32];
            b[..2].copy_from_slice(&[0x60, 0xEA]);
            b
        }, ArchiveFormat::Arj),
        ("lzh_lh", {
            let mut b = vec![0u8; 32];
            b[2..5].copy_from_slice(b"-lh");
            b
        }, ArchiveFormat::Lzh),
        ("lzh_lz", {
            let mut b = vec![0u8; 32];
            b[2..5].copy_from_slice(b"-lz");
            b
        }, ArchiveFormat::Lzh),
        ("cpio_newc", {
            let mut b = vec![0u8; 110];
            b[..6].copy_from_slice(b"070701");
            b
        }, ArchiveFormat::Cpio),
        ("cpio_crc", {
            let mut b = vec![0u8; 110];
            b[..6].copy_from_slice(b"070702");
            b
        }, ArchiveFormat::Cpio),
        ("cpio_odc", {
            let mut b = vec![0u8; 76];
            b[..6].copy_from_slice(b"070707");
            b
        }, ArchiveFormat::Cpio),
        ("cpio_bin_le", {
            let mut b = vec![0u8; 32];
            b[..2].copy_from_slice(&[0xC7, 0x71]);
            b
        }, ArchiveFormat::Cpio),
        ("chm", {
            let mut b = vec![0u8; 32];
            b[..4].copy_from_slice(b"ITSF");
            b
        }, ArchiveFormat::Chm),
        ("msi_cfb", {
            let mut b = vec![0u8; 512];
            b[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
            b
        }, ArchiveFormat::Msi),
        ("nsis_installer", {
            let mut b = vec![0u8; 256];
            b[16..32].copy_from_slice(b"\xEF\xBE\xAD\xDENullsoftInst");
            b
        }, ArchiveFormat::Nsis),

        // 2. Stream Compression Formats
        ("gzip", {
            let mut b = vec![0u8; 16];
            b[..2].copy_from_slice(&[0x1F, 0x8B]);
            b
        }, ArchiveFormat::Gzip),
        ("bzip2", {
            let mut b = vec![0u8; 16];
            b[..3].copy_from_slice(b"BZh");
            b
        }, ArchiveFormat::Bzip2),
        ("xz", {
            let mut b = vec![0u8; 16];
            b[..6].copy_from_slice(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]);
            b
        }, ArchiveFormat::Xz),
        ("zstd", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(&[0x28, 0xB5, 0x2F, 0xFD]);
            b
        }, ArchiveFormat::Zstd),
        ("lz4_frame", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(&[0x04, 0x22, 0x4D, 0x18]);
            b
        }, ArchiveFormat::Lz4),
        ("snappy_framed", {
            let mut b = vec![0u8; 16];
            b[..10].copy_from_slice(b"\xFF\x06\x00\x00sNaPpY");
            b
        }, ArchiveFormat::Snappy),
        ("lzfse_bvx", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(b"bvx-");
            b
        }, ArchiveFormat::Lzfse),
        ("brotli", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(b"\xCE\xB2\xCF\x81");
            b
        }, ArchiveFormat::Brotli),
        ("lzip", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(b"LZIP");
            b
        }, ArchiveFormat::Lzip),
        ("lrzip", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(b"LRZI");
            b
        }, ArchiveFormat::Lrzip),
        ("mslz", {
            let mut b = vec![0u8; 16];
            b[..8].copy_from_slice(&[0x53, 0x5A, 0x44, 0x44, 0x88, 0xF0, 0x27, 0x33]);
            b
        }, ArchiveFormat::MsLz),

        // 3. Virtual Disk & Filesystem Images
        ("apfs", {
            let mut b = vec![0u8; 512];
            b[..4].copy_from_slice(b"NXSB");
            b
        }, ArchiveFormat::Apfs),
        ("hfs_plus", {
            let mut b = vec![0u8; 1536];
            b[1024..1026].copy_from_slice(b"H+");
            b
        }, ArchiveFormat::HfsPlus),
        ("vhd_header", {
            let mut b = vec![0u8; 512];
            b[..8].copy_from_slice(b"conectix");
            b
        }, ArchiveFormat::Vhd),
        ("vhdx", {
            let mut b = vec![0u8; 65536];
            b[..8].copy_from_slice(b"vhdxfile");
            b
        }, ArchiveFormat::Vhdx),
        ("vmdk_binary", {
            let mut b = vec![0u8; 512];
            b[..4].copy_from_slice(b"KDMV");
            b
        }, ArchiveFormat::Vmdk),
        ("vmdk_text", {
            b"# Disk DescriptorFile\nversion=1\nCID=fffffffe".to_vec()
        }, ArchiveFormat::Vmdk),
        ("qcow2", {
            let mut b = vec![0u8; 72];
            b[..4].copy_from_slice(b"QFI\xFB");
            b
        }, ArchiveFormat::Qcow2),
        ("ext4", {
            let mut b = vec![0u8; 2048];
            b[1080..1082].copy_from_slice(&[0x53, 0xEF]);
            b
        }, ArchiveFormat::Ext4),
        ("fat32", {
            let mut b = vec![0u8; 512];
            b[510] = 0x55;
            b[511] = 0xAA;
            b[82..90].copy_from_slice(b"FAT32   ");
            b
        }, ArchiveFormat::Fat),
        ("fat16", {
            let mut b = vec![0u8; 512];
            b[510] = 0x55;
            b[511] = 0xAA;
            b[54..62].copy_from_slice(b"FAT16   ");
            b
        }, ArchiveFormat::Fat),
        ("exfat", {
            let mut b = vec![0u8; 512];
            b[3..11].copy_from_slice(b"EXFAT   ");
            b
        }, ArchiveFormat::Fat),
        ("ntfs", {
            let mut b = vec![0u8; 512];
            b[3..11].copy_from_slice(b"NTFS    ");
            b
        }, ArchiveFormat::Ntfs),
        ("squashfs_le", {
            let mut b = vec![0u8; 96];
            b[..4].copy_from_slice(b"hsqs");
            b
        }, ArchiveFormat::Squashfs),
        ("cramfs_le", {
            let mut b = vec![0u8; 64];
            b[..4].copy_from_slice(&[0x28, 0xCD, 0x3D, 0x45]);
            b
        }, ArchiveFormat::Cramfs),

        // 4. Packages & Apple Formats
        ("aar", {
            let mut b = vec![0u8; 32];
            b[..4].copy_from_slice(b"AA01");
            b
        }, ArchiveFormat::Aar),
        ("deb", {
            let mut b = vec![0u8; 32];
            b[..8].copy_from_slice(b"!<arch>\n");
            b[8..21].copy_from_slice(b"debian-binary");
            b
        }, ArchiveFormat::Deb),
        ("ar_unix", {
            let mut b = vec![0u8; 32];
            b[..8].copy_from_slice(b"!<arch>\n");
            b[8..16].copy_from_slice(b"custom_f");
            b
        }, ArchiveFormat::Ar),
        ("rpm", {
            let mut b = vec![0u8; 96];
            b[..4].copy_from_slice(&[0xED, 0xAB, 0xEE, 0xDB]);
            b
        }, ArchiveFormat::Rpm),
        ("xar", {
            let mut b = vec![0u8; 28];
            b[..4].copy_from_slice(b"xar!");
            b
        }, ArchiveFormat::Xar),

        // 5. Executables, Audio & Media Formats
        ("pe_exe", {
            let mut b = vec![0u8; 64];
            b[..2].copy_from_slice(b"MZ");
            b
        }, ArchiveFormat::PeExe),
        ("elf", {
            let mut b = vec![0u8; 52];
            b[..4].copy_from_slice(b"\x7FELF");
            b
        }, ArchiveFormat::Elf),
        ("macho_64", {
            let mut b = vec![0u8; 32];
            b[..4].copy_from_slice(&[0xFE, 0xED, 0xFA, 0xCF]);
            b
        }, ArchiveFormat::MachO),
        ("macho_fat", {
            let mut b = vec![0u8; 32];
            b[..4].copy_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
            b
        }, ArchiveFormat::MachO),
        ("swf_fws", {
            let mut b = vec![0u8; 16];
            b[..3].copy_from_slice(b"FWS");
            b
        }, ArchiveFormat::Swf),
        ("flac", {
            let mut b = vec![0u8; 42];
            b[..4].copy_from_slice(b"fLaC");
            b
        }, ArchiveFormat::Flac),
        ("dcx", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(&[0xB1, 0x68, 0xDE, 0x3A]);
            b
        }, ArchiveFormat::Dcx),
        ("zoo", {
            let mut b = vec![0u8; 34];
            b[20..24].copy_from_slice(b"ZOO ");
            b
        }, ArchiveFormat::Zoo),
        ("pak", {
            let mut b = vec![0u8; 16];
            b[..4].copy_from_slice(b"PACK");
            b
        }, ArchiveFormat::Pak),
    ];

    for (name, buffer, expected_format) in test_cases {
        let res = FormatSniffer::sniff(&buffer);
        match res {
            SniffResult::Yes { format, confidence } => {
                assert_eq!(
                    format, expected_format,
                    "Failed signature match for format {}: expected {:?}, got {:?}",
                    name, expected_format, format
                );
                assert!(confidence >= 50, "Confidence too low ({}) for {}", confidence, name);
            }
            other => {
                panic!("Expected SniffResult::Yes for {}, got {:?}", name, other);
            }
        }
    }
}

#[test]
fn test_sniffer_need_more_truncated_buffers() {
    // 1. Empty buffer
    assert_eq!(
        FormatSniffer::sniff(&[]),
        SniffResult::NeedMore { required_bytes: 4 }
    );

    // 2. Partial 7z prefix (3 bytes of 6)
    let partial_7z = b"7z\xBC";
    let res = FormatSniffer::sniff(partial_7z);
    match res {
        SniffResult::NeedMore { required_bytes } => {
            assert!(required_bytes >= 6, "Expected at least 6 bytes, got {}", required_bytes);
        }
        other => panic!("Expected NeedMore for partial 7z, got {:?}", other),
    }

    // 3. Short random 3-byte slice
    let short_random = &[0xAA, 0xBB, 0xCC];
    let res = FormatSniffer::sniff(short_random);
    match res {
        SniffResult::NeedMore { required_bytes } => {
            assert!(required_bytes >= 16);
        }
        other => panic!("Expected NeedMore for short buffer, got {:?}", other),
    }
}

#[test]
fn test_sniffer_noise_rejection() {
    // Generate deterministic pseudo-random noise
    let mut rng_state: u64 = 0xDEADBEEFCAFEBABE;
    for iteration in 0..100 {
        let mut noise = vec![0u8; 1024];
        for b in &mut noise {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            *b = (rng_state >> 33) as u8;
        }

        let res = FormatSniffer::sniff(&noise);
        assert_eq!(
            res,
            SniffResult::No,
            "Failed noise rejection on iteration {}",
            iteration
        );
    }
}

#[test]
fn test_sniffer_sliding_window_sfx_pe() {
    let mut pe_sfx = vec![0u8; 4096];
    pe_sfx[..2].copy_from_slice(b"MZ"); // PE header
    // Place 7z archive signature at offset 512
    pe_sfx[512..518].copy_from_slice(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]);

    let res = FormatSniffer::sniff(&pe_sfx);
    match res {
        SniffResult::Yes { format, .. } => {
            assert_eq!(format, ArchiveFormat::SevenZip);
        }
        other => panic!("Expected 7z SFX detected, got {:?}", other),
    }
}

#[test]
fn test_sniffer_physical_file() {
    let dir = tempdir().unwrap();
    let sample_file = dir.path().join("archive.7z");
    let mut data = vec![0u8; 128];
    data[..6].copy_from_slice(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]);
    fs::write(&sample_file, &data).unwrap();

    let res = FormatSniffer::sniff_file(&sample_file).unwrap();
    match res {
        SniffResult::Yes { format, confidence } => {
            assert_eq!(format, ArchiveFormat::SevenZip);
            assert_eq!(confidence, 100);
        }
        other => panic!("Expected 7z format from file, got {:?}", other),
    }
}

#[test]
fn test_archive_format_properties_and_mappings() {
    let formats = [
        ArchiveFormat::SevenZip,
        ArchiveFormat::Zip,
        ArchiveFormat::Zip64,
        ArchiveFormat::Tar,
        ArchiveFormat::Gzip,
        ArchiveFormat::Bzip2,
        ArchiveFormat::Xz,
        ArchiveFormat::Zstd,
        ArchiveFormat::Rar4,
        ArchiveFormat::Rar5,
        ArchiveFormat::Cab,
        ArchiveFormat::Wim,
        ArchiveFormat::Iso,
        ArchiveFormat::Udf,
        ArchiveFormat::Dmg,
        ArchiveFormat::Apfs,
        ArchiveFormat::HfsPlus,
        ArchiveFormat::Vhd,
        ArchiveFormat::Vhdx,
        ArchiveFormat::Vmdk,
        ArchiveFormat::Qcow2,
        ArchiveFormat::Ext4,
        ArchiveFormat::Fat,
        ArchiveFormat::Ntfs,
        ArchiveFormat::Lzfse,
        ArchiveFormat::Brotli,
    ];

    for fmt in formats {
        assert!(!fmt.as_str().is_empty());
        assert!(!fmt.primary_extension().is_empty());
        assert!(!fmt.mime_type().is_empty());
    }
}
