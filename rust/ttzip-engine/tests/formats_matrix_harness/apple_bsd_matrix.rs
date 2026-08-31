// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple & BSD Format Matrix (AppleDouble FinderInfo, AR BSD/GNU variants, XAR).

use super::{
    assert_roundtrip_match, compute_sha256, read_archive_buffer, write_archive_buffer, SyntheticEntry,
    VerifyPolicy,
};
use ttzip_engine::archive::mac_metadata::appledouble::{
    AppleDoubleEntryDescriptor, AppleDoubleHeader,
};
use ttzip_engine::archive::mac_metadata::finder_info::FinderInfo;
use ttzip_engine::archive::mac_metadata::types::*;
use ttzip_engine::ffi::archive_ffi::sys::*;

/// 1. AppleDouble `._` Resource Fork and FinderInfo binary synthesis and roundtrip.
pub fn run_appledouble_finderinfo_matrix_test() {
    let mut finder_info_bytes = [0u8; 32];
    // Set OSType 'TEXT' and Creator 'TTZP'
    finder_info_bytes[0..4].copy_from_slice(b"TEXT");
    finder_info_bytes[4..8].copy_from_slice(b"TTZP");
    finder_info_bytes[8..10].copy_from_slice(&0x4000u16.to_be_bytes()); // HasCustomIcon flag

    let parsed_finder = FinderInfo::from_raw(finder_info_bytes);
    assert_eq!(&parsed_finder.file_type(), b"TEXT");
    assert_eq!(&parsed_finder.file_creator(), b"TTZP");

    let resource_fork_payload = b"Resource Fork Raw Byte Stream 2026";

    // Build AppleDouble V2 header with Finder Info and Resource Fork entries
    let header = AppleDoubleHeader {
        magic: APPLEDOUBLE_MAGIC,
        version: APPLEDOUBLE_VERSION_2,
        home_fs: *b"Mac OS X        ",
        num_entries: 2,
        entries: vec![
            AppleDoubleEntryDescriptor {
                entry_id: ENTRY_FINDER_INFO,
                offset: (APPLEDOUBLE_HEADER_BASE_SIZE + 2 * APPLEDOUBLE_ENTRY_DESCRIPTOR_SIZE) as u32,
                length: 32,
            },
            AppleDoubleEntryDescriptor {
                entry_id: ENTRY_RESOURCE_FORK,
                offset: (APPLEDOUBLE_HEADER_BASE_SIZE + 2 * APPLEDOUBLE_ENTRY_DESCRIPTOR_SIZE + 32) as u32,
                length: resource_fork_payload.len() as u32,
            },
        ],
    };

    let mut encoded_appledouble = header.encode();
    encoded_appledouble.extend_from_slice(&finder_info_bytes);
    encoded_appledouble.extend_from_slice(resource_fork_payload);

    // Decode and verify exact structure
    let decoded_header = AppleDoubleHeader::decode(&encoded_appledouble)
        .expect("AppleDouble decode must succeed");
    assert_eq!(decoded_header.magic, APPLEDOUBLE_MAGIC);
    assert_eq!(decoded_header.version, APPLEDOUBLE_VERSION_2);
    assert_eq!(decoded_header.entries.len(), 2);
    assert_eq!(decoded_header.entries[0].entry_id, ENTRY_FINDER_INFO);
    assert_eq!(decoded_header.entries[0].length, 32);
    assert_eq!(decoded_header.entries[1].entry_id, ENTRY_RESOURCE_FORK);
    assert_eq!(decoded_header.entries[1].length, resource_fork_payload.len() as u32);
}

/// 2. AR Archive BSD Variant (`!<arch>\n` with `#1/len` extended naming).
pub fn run_ar_bsd_variant_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("object1.o", vec![0xCF, 0xFA, 0xED, 0xFE, 0x07, 0x00, 0x00, 0x01])
            .with_perm(0o644)
            .with_mtime(1_600_000_000, 0),
        SyntheticEntry::file(
            "very_long_bsd_ar_member_name_exceeding_15_chars.o",
            vec![0x90; 256],
        )
        .with_perm(0o644)
        .with_mtime(1_600_000_100, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_ar_bsd(a);
        if rc != 0 {
            Err("archive_write_set_format_ar_bsd failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write BSD AR archive");

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"!<arch>\n"), "AR archive must start with !<arch>\\n");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read BSD AR archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::default());
}

/// 3. AR Archive GNU / SVR4 Variant (Debian Packages & static libraries).
pub fn run_ar_gnu_svr4_variant_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("debian-binary", b"2.0\n".to_vec())
            .with_perm(0o644)
            .with_mtime(1_650_000_000, 0),
        SyntheticEntry::file("control.tar.gz", vec![0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00])
            .with_perm(0o644)
            .with_mtime(1_650_000_100, 0),
        SyntheticEntry::file("data.tar.xz", vec![0xAA; 512])
            .with_perm(0o644)
            .with_mtime(1_650_000_200, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_ar_svr4(a);
        if rc != 0 {
            Err("archive_write_set_format_ar_svr4 failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write GNU/SVR4 AR archive");

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"!<arch>\n"), "GNU AR archive must start with !<arch>\\n");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read GNU/SVR4 AR archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::default());
}

/// 4. XAR (eXtensible ARchive) XML Table of Contents & Compressed Heap.
pub fn run_xar_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("xar_pkg/PackageInfo.xml", b"<pkg-info format-version=\"2\" identifier=\"com.ttzip.engine\"/>".to_vec())
            .with_perm(0o644)
            .with_mtime(1_700_000_000, 0),
        SyntheticEntry::file("xar_pkg/payload.bin", vec![0x55; 4096])
            .with_perm(0o755)
            .with_mtime(1_700_000_100, 0),
        SyntheticEntry::symlink("xar_pkg/symlink_payload", "payload.bin")
            .with_mtime(1_700_000_200, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_xar(a);
        if rc != 0 {
            Err("archive_write_set_format_xar failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write XAR archive");

    assert!(!bytes.is_empty());
    // Verify XAR magic "xar!" (0x78617221)
    assert!(bytes.starts_with(b"xar!"), "XAR archive must start with magic 'xar!'");

    let extracted = read_archive_buffer(&bytes).expect("Failed to read XAR archive");
    let pkg_info = extracted.iter().find(|e| e.path.contains("PackageInfo.xml")).expect("PackageInfo.xml missing");
    assert_eq!(pkg_info.data, b"<pkg-info format-version=\"2\" identifier=\"com.ttzip.engine\"/>");
    let payload = extracted.iter().find(|e| e.path.contains("payload.bin")).expect("payload.bin missing");
    assert_eq!(payload.data, vec![0x55; 4096]);
    assert_eq!(payload.sha256, compute_sha256(&vec![0x55; 4096]));
}
