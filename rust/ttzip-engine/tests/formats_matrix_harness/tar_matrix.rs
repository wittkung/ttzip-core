// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TAR Family Format Matrix (V7, USTAR, POSIX.1-2001 PAX, GNU Tar LongLink).

use super::{
    assert_roundtrip_match, read_archive_buffer, write_archive_buffer, SyntheticEntry, VerifyPolicy,
};
use ttzip_engine::ffi::archive_ffi::sys::*;

/// 1. V7 Traditional Unix Tar matrix verification.
pub fn run_tar_v7_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("v7_test/file1.txt", b"Hello V7 Unix Tar World!".to_vec())
            .with_perm(0o644)
            .with_mtime(1_600_000_000, 0),
        SyntheticEntry::file("v7_test/file2.bin", vec![0xAB; 4096])
            .with_perm(0o755)
            .with_mtime(1_600_000_100, 0),
        SyntheticEntry::dir("v7_test/dir/")
            .with_perm(0o755)
            .with_mtime(1_600_000_200, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_v7tar(a);
        if rc != 0 {
            Err("archive_write_set_format_v7tar failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write V7 tar archive");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read V7 tar archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::default());
}

/// 2. POSIX.1-1988 USTAR matrix verification (split prefix/name, modes, directory headers).
pub fn run_tar_ustar_matrix_test() {
    let entries = vec![
        SyntheticEntry::dir("ustar_root/")
            .with_perm(0o755)
            .with_mtime(1_650_000_000, 0),
        SyntheticEntry::file("ustar_root/short.txt", b"USTAR short filename entry".to_vec())
            .with_perm(0o600)
            .with_mtime(1_650_000_100, 0),
        SyntheticEntry::file(
            "ustar_root/sub_directory/ustar_medium_length_filename_90_bytes_payload_test.dat",
            vec![0x42; 2048],
        )
        .with_perm(0o644)
        .with_mtime(1_650_000_200, 0),
        SyntheticEntry::symlink("ustar_root/symlink_to_short", "short.txt")
            .with_mtime(1_650_000_300, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_ustar(a);
        if rc != 0 {
            Err("archive_write_set_format_ustar failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write USTAR archive");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read USTAR archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::default());
}

/// 3. POSIX.1-2001 PAX with Nanosecond Timestamps.
pub fn run_tar_pax_nanosecond_matrix_test() {
    let entries = vec![
        SyntheticEntry::file("pax_nano/clock_1.bin", vec![0x11; 512])
            .with_mtime(1_700_000_000, 123_456_789),
        SyntheticEntry::file("pax_nano/clock_2.bin", vec![0x22; 1024])
            .with_mtime(1_700_000_001, 987_654_321),
        SyntheticEntry::file("pax_nano/clock_3.bin", vec![0x33; 256])
            .with_mtime(1_700_000_002, 500_000_000),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_pax(a);
        if rc != 0 {
            Err("archive_write_set_format_pax failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write PAX archive with nanos");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read PAX archive with nanos");

    let policy = VerifyPolicy {
        check_data_sha256: true,
        check_permissions: true,
        check_mtime_secs: true,
        check_mtime_nanos: true,
        check_symlinks: true,
        check_hardlinks: true,
        check_xattrs: false,
    };
    assert_roundtrip_match(&entries, &extracted, &policy);
}

/// 4. POSIX.1-2001 PAX with UTF-8 paths and Extended Attributes (xattrs).
pub fn run_tar_pax_utf8_and_xattrs_matrix_test() {
    let entries = vec![
        SyntheticEntry::file(
            "pax_utf8/🚀_ttzip_测试_日本語_папка.txt",
            "TTZip UTF-8 international text: 简体中文 繁體中文 日本語 한국어 Русский".as_bytes().to_vec(),
        )
        .with_perm(0o644)
        .with_mtime(1_700_000_000, 42_000_000)
        .with_xattr("user.ttzip.tag", b"matrix_utf8_verified")
        .with_xattr("user.digest.sha256", b"d41d8cd98f00b204e9800998ecf8427e"),
        SyntheticEntry::file(
            "pax_utf8/symbols_@#$%^&*()_+{}|:<>?.dat",
            vec![0x7F; 128],
        )
        .with_perm(0o755)
        .with_mtime(1_700_000_010, 0)
        .with_xattr("user.mime", b"application/octet-stream"),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_pax(a);
        if rc != 0 {
            Err("archive_write_set_format_pax failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write PAX UTF-8 archive");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read PAX UTF-8 archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::strict_all());
}

/// 5. POSIX.1-2001 PAX Negative Epoch Timestamps (Pre-1970).
pub fn run_tar_pax_negative_timestamps_matrix_test() {
    let entries = vec![
        // 1901-12-13 20:45:52 UTC (Negative timestamp)
        SyntheticEntry::file("pax_neg/pre_1970_1.dat", b"Historical data 1901".to_vec())
            .with_mtime(-2_147_483_600, 0),
        // 1969-12-31 23:59:59 UTC
        SyntheticEntry::file("pax_neg/pre_1970_2.dat", b"Dec 31 1969".to_vec())
            .with_mtime(-1, 500_000_000),
        // 1800-01-01 00:00:00 UTC
        SyntheticEntry::file("pax_neg/pre_1970_3.dat", b"Ancient archive data".to_vec())
            .with_mtime(-5_364_662_400, 123_000),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_pax(a);
        if rc != 0 {
            Err("archive_write_set_format_pax failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write PAX negative timestamps archive");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read PAX negative timestamps archive");

    let policy = VerifyPolicy {
        check_data_sha256: true,
        check_permissions: true,
        check_mtime_secs: true,
        check_mtime_nanos: false,
        check_symlinks: true,
        check_hardlinks: true,
        check_xattrs: false,
    };
    assert_roundtrip_match(&entries, &extracted, &policy);
}

/// 6. GNU Tar with Ultra-Long Filenames and Hardlinks (>100 Bytes `@LongLink`).
pub fn run_tar_gnutar_longlink_matrix_test() {
    let long_name = "gnutar_long/".to_string()
        + "nested_sub_directory_layer_one_abcdefghijklmnopqrstuvwxyz/"
        + "nested_sub_directory_layer_two_123456789012345678901234567890/"
        + "ultra_long_filename_exceeding_100_bytes_boundary_gnutar_longlink_extension_test.bin";

    let long_hardlink_target = long_name.clone();
    let long_hardlink_name = "gnutar_long/hardlink_pointing_to_long_path_also_exceeding_100_bytes_limit_for_verification.bin";

    let entries = vec![
        SyntheticEntry::file(&long_name, vec![0xEE; 8192])
            .with_perm(0o644)
            .with_mtime(1_700_000_000, 0),
        SyntheticEntry::hardlink(long_hardlink_name, &long_hardlink_target)
            .with_mtime(1_700_000_000, 0),
        SyntheticEntry::symlink(
            "gnutar_long/symlink_to_long_target.lnk",
            &long_hardlink_target,
        )
        .with_mtime(1_700_000_000, 0),
    ];

    let bytes = write_archive_buffer(&entries, |a| unsafe {
        let rc = archive_write_set_format_gnutar(a);
        if rc != 0 {
            Err("archive_write_set_format_gnutar failed".to_string())
        } else {
            Ok(())
        }
    })
    .expect("Failed to write GNU Tar archive with LongLink");

    assert!(!bytes.is_empty());
    let extracted = read_archive_buffer(&bytes).expect("Failed to read GNU Tar archive");
    assert_roundtrip_match(&entries, &extracted, &VerifyPolicy::default());
}
