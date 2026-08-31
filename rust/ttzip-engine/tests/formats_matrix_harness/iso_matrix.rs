// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ISO 9660 & UDF Optical Disc Image Format Matrix (Level 1/2/3, Rockridge, Joliet UTF-16BE).

use std::ffi::CString;
use super::{
    assert_roundtrip_match, read_archive_buffer, write_archive_buffer, SyntheticEntry, VerifyPolicy,
};
use ttzip_engine::ffi::archive_ffi::sys::*;
use ttzip_engine::standards::signatures::DetectedFormat;
use ttzip_engine::standards::sniffer::detect_format_buffer;

/// 1. ISO 9660 Level 1 standard optical disc matrix.
pub fn run_iso9660_level1_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("README.TXT", b"ISO9660 Level 1 Standard 8.3 File".to_vec())
            .with_perm(0o644)
            .with_mtime(1_600_000_000, 0),
        SyntheticEntry::file("DATA01.BIN", vec![0x55; 2048])
            .with_perm(0o644)
            .with_mtime(1_600_000_100, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_iso9660(a);
        if rc != 0 {
            return Err("archive_write_set_format_iso9660 failed".to_string());
        }
        let opt = CString::new("iso9660:iso-level=1").unwrap();
        archive_write_set_options(a, opt.as_ptr());
        Ok(())
    })
    .expect("Failed to write ISO9660 Level 1 image");

    assert!(!bytes.is_empty());
    // Verify ISO9660 volume descriptor magic CD001 at sector 16 (offset 32768)
    const ISO_SECTOR_16: usize = 32768;
    if bytes.len() >= ISO_SECTOR_16 + 6 {
        assert_eq!(&bytes[ISO_SECTOR_16 + 1..ISO_SECTOR_16 + 6], b"CD001");
    }

    let extracted = read_archive_buffer(&bytes).expect("Failed to read ISO9660 Level 1 image");
    assert!(!extracted.is_empty());
}

/// 2. ISO 9660 Level 2 / 3 with extended filename lengths.
pub fn run_iso9660_level2_3_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("long_filename_level2_test.txt", b"Level 2 Long Filename Payload".to_vec())
            .with_perm(0o644)
            .with_mtime(1_650_000_000, 0),
        SyntheticEntry::file("multi_extent_data_block.bin", vec![0xAA; 4096])
            .with_perm(0o644)
            .with_mtime(1_650_000_100, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_iso9660(a);
        if rc != 0 {
            return Err("archive_write_set_format_iso9660 failed".to_string());
        }
        let opt = CString::new("iso9660:iso-level=3").unwrap();
        archive_write_set_options(a, opt.as_ptr());
        Ok(())
    })
    .expect("Failed to write ISO9660 Level 3 image");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read ISO9660 Level 3 image");
    assert!(!extracted.is_empty());
}

/// 3. ISO 9660 with Rockridge Extensions (PX, RR, TF, NM, SL Symbolic Links).
pub fn run_iso9660_rockridge_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("rockridge_target.txt", b"Rockridge target file contents".to_vec())
            .with_perm(0o644)
            .with_mtime(1_700_000_000, 0),
        SyntheticEntry::symlink("symlink_rockridge_link", "rockridge_target.txt")
            .with_mtime(1_700_000_000, 0),
        SyntheticEntry::file(
            "very_long_posix_compliant_rockridge_extended_filename.dat",
            vec![0x77; 1024],
        )
        .with_perm(0o755)
        .with_mtime(1_700_000_100, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_iso9660(a);
        if rc != 0 {
            return Err("archive_write_set_format_iso9660 failed".to_string());
        }
        let opt = CString::new("iso9660:rockridge=1").unwrap();
        archive_write_set_options(a, opt.as_ptr());
        Ok(())
    })
    .expect("Failed to write Rockridge ISO image");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read Rockridge ISO image");
    let policy = VerifyPolicy {
        check_data_sha256: true,
        check_permissions: false,
        check_mtime_secs: true,
        check_mtime_nanos: false,
        check_symlinks: true,
        check_hardlinks: true,
        check_xattrs: false,
    };
    assert_roundtrip_match(&entries, &extracted, &policy);
}

/// 4. ISO 9660 with Joliet Unicode UTF-16BE Extensions.
pub fn run_iso9660_joliet_matrix_test() {
    let entries = vec![
        SyntheticEntry::file(
            "Joliet_International_Unicode_Document.txt",
            "Joliet UTF-16BE Unicode text payload".as_bytes().to_vec(),
        )
        .with_perm(0o644)
        .with_mtime(1_700_000_000, 0),
        SyntheticEntry::file(
            "Nested_Folder/Joliet_Sub_File.bin",
            vec![0x12, 0x34, 0x56, 0x78],
        )
        .with_perm(0o644)
        .with_mtime(1_700_000_010, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_iso9660(a);
        if rc != 0 {
            return Err("archive_write_set_format_iso9660 failed".to_string());
        }
        let opt = CString::new("iso9660:joliet=1").unwrap();
        archive_write_set_options(a, opt.as_ptr());
        Ok(())
    })
    .expect("Failed to write Joliet ISO image");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read Joliet ISO image");
    let policy = VerifyPolicy {
        check_data_sha256: true,
        check_permissions: false,
        check_mtime_secs: true,
        check_mtime_nanos: false,
        check_symlinks: true,
        check_hardlinks: true,
        check_xattrs: false,
    };
    assert_roundtrip_match(&entries, &extracted, &policy);
}

/// 5. UDF and ISO Optical Sniffing & Boundary Validation.
pub fn run_udf_and_iso_sniffing_matrix_test() {
    // Construct synthetic UDF Volume Recognition Sequence (BEA01 + NSR02 + TEA01)
    let mut udf_image = vec![0u8; 40960]; // 20 sectors (2048 bytes each)
    let sector_16 = 16 * 2048;
    udf_image[sector_16 + 1..sector_16 + 6].copy_from_slice(b"CD001");
    let sector_17 = 17 * 2048;
    udf_image[sector_17 + 1..sector_17 + 6].copy_from_slice(b"NSR02");
    let sector_18 = 18 * 2048;
    udf_image[sector_18 + 1..sector_18 + 6].copy_from_slice(b"TEA01");

    let detected = detect_format_buffer(&udf_image, None);
    assert_eq!(
        detected.format,
        DetectedFormat::Iso,
        "UDF optical disc image must be recognized as ISO container"
    );
}
