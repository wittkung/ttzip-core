// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R3 Regression Suite: ZIP Timestamp Fidelity (Q1-ISSUE-03), In-Place Edit & External Tool Cross-Verification.
//!
//! Embedded Q1 C3-04 ZipCrypto fixture, CRC-32 boundary, WinZip AES AE-1/AE-2 version contracts,
//! and ZipCrypto error semantics live in `r3_zip_crc_and_aes_version_regression_tests.rs`.

use std::ffi::CString;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use ttzip_archive::archive::in_place_edit::{in_place_edit_zip, InPlaceAction};
use ttzip_archive::archive::unified::extract_single::extract_single_entry_memory;
use ttzip_archive::archive::unified::inspect::inspect_archive;
use ttzip_archive::archive::unified::UnifiedArchiveOrchestrator;
use ttzip_archive::crypto::crc32::crc32_fast;
use ttzip_archive::fs::collision::staging::StagingRoot;
use ttzip_archive::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions, TTZipStatus,
};
use ttzip_archive::zip::datetime::{
    dos_to_local_unix_epoch_secs, resolve_zip_mtime_secs, DosDateTime,
};
use ttzip_archive::zip::extra::{NtfsExtra, ZipExtraFields};
use ttzip_archive::zip::reader::ZipArchive;
use ttzip_archive::zip::reader_stream::extract_zip_from_reader;
use ttzip_archive::zip::writer::store_stream::create_zip_store_parallel;
use ttzip_archive::zip::writer::streaming_parallel::{
    create_zip_streaming_parallel_split, create_zip_streaming_parallel_with_sheet,
};
use ttzip_archive::zip::writer::{
    assemble_zip_archive, compress_items_parallel, unix_to_dos_time, ZipInputItem,
};
use ttzip_types::options::ProgressAtomicSheet;

const Q1_HIST_MTIME_2023_NOV: i64 = 1700001234; // 2023-11-14 22:33:54 UTC
const Q1_HIST_MTIME_2023_MAR: i64 = 1680000000; // 2023-03-28 10:40:00 UTC

#[derive(Debug)]
struct RawHeaderTimeInfo {
    name: String,
    lfh_dos_time: u16,
    lfh_dos_date: u16,
    lfh_ext_mtime: Option<i64>,
    cdfh_dos_time: u16,
    cdfh_dos_date: u16,
    cdfh_ext_mtime: Option<i64>,
}

#[derive(Debug, Clone)]
struct InspectedMeta {
    path: String,
    mtime_epoch_secs: i64,
}

fn zip_item(rel_path: &str, data: Vec<u8>, mtime: i64, mode: u32) -> ZipInputItem {
    ZipInputItem {
        rel_path: rel_path.to_string(),
        data,
        mtime_epoch_secs: mtime as u32,
        mode,
        is_directory: false,
    }
}

fn file_mtime(path: &Path) -> i64 {
    fs::metadata(path).unwrap().mtime()
}

fn collect_inspected_entries(archive_path: &Path, password: Option<&str>) -> Vec<InspectedMeta> {
    let mut out = Vec::new();
    inspect_archive(archive_path, password, true, |e| {
        out.push(InspectedMeta {
            path: e.path.to_string(),
            mtime_epoch_secs: e.mtime_epoch_secs,
        });
        true
    })
    .expect("inspect_archive must succeed");
    out
}

fn extract_archive_to_dir(
    archive_path: &Path,
    dest_dir: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    UnifiedArchiveOrchestrator::extract_archive_with_sheet(
        archive_path,
        &StagingRoot(dest_dir.to_path_buf()),
        options,
        sheet,
    )
}

fn parse_raw_lfh_and_cdfh_times(zip_bytes: &[u8]) -> Vec<RawHeaderTimeInfo> {
    let eocd = (0..=zip_bytes.len().saturating_sub(22))
        .rev()
        .find(|&i| &zip_bytes[i..i + 4] == b"PK\x05\x06")
        .expect("EOCD must exist");
    let count = u16::from_le_bytes([zip_bytes[eocd + 10], zip_bytes[eocd + 11]]) as usize;
    let cd_offset = u32::from_le_bytes(zip_bytes[eocd + 16..eocd + 20].try_into().unwrap()) as usize;

    let mut pos = cd_offset;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        assert_eq!(&zip_bytes[pos..pos + 4], b"PK\x01\x02");
        let cdfh_dos_time = u16::from_le_bytes([zip_bytes[pos + 12], zip_bytes[pos + 13]]);
        let cdfh_dos_date = u16::from_le_bytes([zip_bytes[pos + 14], zip_bytes[pos + 15]]);
        let name_len = u16::from_le_bytes([zip_bytes[pos + 28], zip_bytes[pos + 29]]) as usize;
        let extra_len = u16::from_le_bytes([zip_bytes[pos + 30], zip_bytes[pos + 31]]) as usize;
        let comment_len = u16::from_le_bytes([zip_bytes[pos + 32], zip_bytes[pos + 33]]) as usize;
        let lfh_off = u32::from_le_bytes(zip_bytes[pos + 42..pos + 46].try_into().unwrap()) as usize;

        let name = String::from_utf8_lossy(&zip_bytes[pos + 46..pos + 46 + name_len]).into_owned();
        let cd_extra = ZipExtraFields::parse(
            &zip_bytes[pos + 46 + name_len..pos + 46 + name_len + extra_len],
            true,
            false,
            false,
            false,
        );
        pos += 46 + name_len + extra_len + comment_len;

        assert_eq!(&zip_bytes[lfh_off..lfh_off + 4], b"PK\x03\x04");
        let lfh_dos_time = u16::from_le_bytes([zip_bytes[lfh_off + 10], zip_bytes[lfh_off + 11]]);
        let lfh_dos_date = u16::from_le_bytes([zip_bytes[lfh_off + 12], zip_bytes[lfh_off + 13]]);
        let lfh_name_len = u16::from_le_bytes([zip_bytes[lfh_off + 26], zip_bytes[lfh_off + 27]]) as usize;
        let lfh_extra_len = u16::from_le_bytes([zip_bytes[lfh_off + 28], zip_bytes[lfh_off + 29]]) as usize;
        let lfh_extra = ZipExtraFields::parse(
            &zip_bytes[lfh_off + 30 + lfh_name_len..lfh_off + 30 + lfh_name_len + lfh_extra_len],
            false,
            false,
            false,
            false,
        );

        out.push(RawHeaderTimeInfo {
            name,
            lfh_dos_time,
            lfh_dos_date,
            lfh_ext_mtime: lfh_extra.mod_time.map(i64::from),
            cdfh_dos_time,
            cdfh_dos_date,
            cdfh_ext_mtime: cd_extra.mod_time.map(i64::from),
        });
    }
    out
}

fn set_file_mtime(path: &Path, epoch_secs: i64) {
    let ts = [
        libc::timespec {
            tv_sec: epoch_secs,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: epoch_secs,
            tv_nsec: 0,
        },
    ];
    let c_path = CString::new(path.to_str().unwrap()).unwrap();
    let rc = unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            c_path.as_ptr(),
            ts.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    assert_eq!(rc, 0, "utimensat failed for {:?}", path);
}

fn write_file_with_mtime(path: &Path, data: &[u8], mode: u32, epoch_secs: i64) {
    fs::write(path, data).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    set_file_mtime(path, epoch_secs);
}

fn build_historical_source_tree(root: &Path) -> PathBuf {
    let src = root.join("corpus_posix");
    fs::create_dir_all(src.join("nested")).unwrap();
    write_file_with_mtime(
        &src.join("hist_2023_nov.txt"),
        b"Historical sample Nov 2023 payload\n",
        0o644,
        Q1_HIST_MTIME_2023_NOV,
    );
    write_file_with_mtime(
        &src.join("nested/hist_2023_mar.sh"),
        b"#!/bin/sh\necho 'Historical Mar 2023'\n",
        0o755,
        Q1_HIST_MTIME_2023_MAR,
    );
    set_file_mtime(&src.join("nested"), Q1_HIST_MTIME_2023_MAR);
    src
}

fn make_create_opts(
    level: TTZipCompressionLevel,
    encryption: TTZipEncryptionMethod,
    password: Option<&CString>,
    threads: u32,
) -> TTZipCreateOptions {
    TTZipCreateOptions {
        format: TTZipArchiveFormat::Zip,
        level,
        encryption,
        password: password.map_or(std::ptr::null(), |c| c.as_ptr()),
        thread_budget: threads,
        ..Default::default()
    }
}

fn make_extract_opts(password: Option<&CString>) -> TTZipExtractOptions {
    TTZipExtractOptions {
        password: password.map_or(std::ptr::null(), |c| c.as_ptr()),
        preserve_permissions: true,
        thread_budget: 2,
        ..Default::default()
    }
}

fn verify_raw_headers_for_historical_tree(zip_bytes: &[u8]) {
    let raw = parse_raw_lfh_and_cdfh_times(zip_bytes);
    for info in &raw {
        assert_eq!(
            info.lfh_dos_time, info.cdfh_dos_time,
            "LFH/CDFH dos_time mismatch for {}",
            info.name
        );
        assert_eq!(
            info.lfh_dos_date, info.cdfh_dos_date,
            "LFH/CDFH dos_date mismatch for {}",
            info.name
        );
        let dt = DosDateTime::from_dos(info.cdfh_dos_date, info.cdfh_dos_time)
            .expect("Raw DOS date/time must be valid calendar date");
        assert!(
            (1980..=2026).contains(&dt.year),
            "Unexpected future year {} for {}",
            dt.year,
            info.name
        );
        assert!((1..=12).contains(&dt.month) && (1..=31).contains(&dt.day));
        if info.name.ends_with("hist_2023_nov.txt") {
            assert_eq!(
                (info.cdfh_dos_time, info.cdfh_dos_date),
                unix_to_dos_time(Q1_HIST_MTIME_2023_NOV as u32)
            );
            assert_eq!(info.lfh_ext_mtime, Some(Q1_HIST_MTIME_2023_NOV));
            assert_eq!(info.cdfh_ext_mtime, Some(Q1_HIST_MTIME_2023_NOV));
        } else if info.name.ends_with("hist_2023_mar.sh") {
            assert_eq!(
                (info.cdfh_dos_time, info.cdfh_dos_date),
                unix_to_dos_time(Q1_HIST_MTIME_2023_MAR as u32)
            );
            assert_eq!(info.lfh_ext_mtime, Some(Q1_HIST_MTIME_2023_MAR));
            assert_eq!(info.cdfh_ext_mtime, Some(Q1_HIST_MTIME_2023_MAR));
        }
    }
}

fn verify_extracted_historical_tree(
    archive_path: &Path,
    pass_opt: Option<&CString>,
    out_dir: &Path,
    idx: usize,
) {
    let entries = collect_inspected_entries(archive_path, pass_opt.map(|p| p.to_str().unwrap()));
    for e in &entries {
        if e.path.ends_with("hist_2023_nov.txt") {
            assert_eq!(e.mtime_epoch_secs, Q1_HIST_MTIME_2023_NOV, "inspect mtime failed on case {idx}");
        } else if e.path.ends_with("hist_2023_mar.sh") {
            assert_eq!(e.mtime_epoch_secs, Q1_HIST_MTIME_2023_MAR, "inspect mtime failed on case {idx}");
        }
    }
    extract_archive_to_dir(archive_path, out_dir, &make_extract_opts(pass_opt), None).unwrap();
    let f1_meta = fs::metadata(out_dir.join("corpus_posix/hist_2023_nov.txt")).unwrap();
    let f2_meta = fs::metadata(out_dir.join("corpus_posix/nested/hist_2023_mar.sh")).unwrap();
    assert_eq!(f1_meta.mtime(), Q1_HIST_MTIME_2023_NOV);
    assert_eq!(f2_meta.mtime(), Q1_HIST_MTIME_2023_MAR);
    assert_eq!(f2_meta.permissions().mode() & 0o777, 0o755);
}

#[test]
fn test_q1_issue_03_all_creation_paths_header_and_roundtrip_fidelity() {
    let tmp = TempDir::new().unwrap();
    let src = build_historical_source_tree(tmp.path());
    let pwd = CString::new("Secret2026!").unwrap();

    let store_zip = tmp.path().join("store_stream.zip");
    create_zip_store_parallel(
        &store_zip,
        std::slice::from_ref(&src),
        &make_create_opts(TTZipCompressionLevel::Store, TTZipEncryptionMethod::None, None, 2),
    )
    .unwrap();
    verify_raw_headers_for_historical_tree(&fs::read(&store_zip).unwrap());

    let serial_zip = tmp.path().join("serial_stream.zip");
    create_zip_streaming_parallel_with_sheet(
        &serial_zip,
        std::slice::from_ref(&src),
        &make_create_opts(TTZipCompressionLevel::Normal, TTZipEncryptionMethod::None, None, 1),
        None,
    )
    .unwrap();
    verify_raw_headers_for_historical_tree(&fs::read(&serial_zip).unwrap());

    let par_stream_zip = tmp.path().join("parallel_stream.zip");
    create_zip_streaming_parallel_with_sheet(
        &par_stream_zip,
        std::slice::from_ref(&src),
        &make_create_opts(TTZipCompressionLevel::Normal, TTZipEncryptionMethod::None, None, 4),
        None,
    )
    .unwrap();
    verify_raw_headers_for_historical_tree(&fs::read(&par_stream_zip).unwrap());

    let zc_zip = tmp.path().join("zipcrypto_stream.zip");
    create_zip_streaming_parallel_with_sheet(
        &zc_zip,
        std::slice::from_ref(&src),
        &make_create_opts(TTZipCompressionLevel::Normal, TTZipEncryptionMethod::ZipCrypto, Some(&pwd), 4),
        None,
    )
    .unwrap();
    verify_raw_headers_for_historical_tree(&fs::read(&zc_zip).unwrap());

    let aes_zip = tmp.path().join("aes256_stream.zip");
    create_zip_streaming_parallel_with_sheet(
        &aes_zip,
        &[src],
        &make_create_opts(TTZipCompressionLevel::Normal, TTZipEncryptionMethod::Aes256, Some(&pwd), 4),
        None,
    )
    .unwrap();
    verify_raw_headers_for_historical_tree(&fs::read(&aes_zip).unwrap());

    let cases = [
        (&store_zip, None),
        (&serial_zip, None),
        (&par_stream_zip, None),
        (&zc_zip, Some(&pwd)),
        (&aes_zip, Some(&pwd)),
    ];
    for (idx, (archive_path, pass_opt)) in cases.into_iter().enumerate() {
        let out_dir = tmp.path().join(format!("extract_{idx}"));
        verify_extracted_historical_tree(archive_path, pass_opt, &out_dir, idx);
    }
}

#[test]
fn test_in_memory_parallel_and_split_volume_timestamp_and_crypto_fidelity() {
    let tmp = TempDir::new().unwrap();
    let items = vec![
        zip_item("corpus_posix/hist_2023_nov.txt", vec![b'A'; 4096], Q1_HIST_MTIME_2023_NOV, 0o100644),
        zip_item("corpus_posix/nested/hist_2023_mar.sh", vec![b'B'; 4096], Q1_HIST_MTIME_2023_MAR, 0o100755),
    ];
    let compressed =
        compress_items_parallel(items, 6, TTZipEncryptionMethod::ZipCrypto, Some("Secret2026!"), 2).unwrap();
    let zip_bytes = assemble_zip_archive(&compressed).unwrap();
    verify_raw_headers_for_historical_tree(&zip_bytes);

    let archive = ZipArchive::open_slice(&zip_bytes).unwrap();
    assert_eq!(archive.entries()[0].mtime_epoch_secs, Q1_HIST_MTIME_2023_NOV);
    assert_eq!(archive.entries()[1].mtime_epoch_secs, Q1_HIST_MTIME_2023_MAR);
    assert_eq!(archive.extract_entry_bytes(0, Some("Secret2026!")).unwrap(), vec![b'A'; 4096]);
    assert_eq!(archive.extract_entry_bytes(0, Some("WrongPass")), Err(TTZipStatus::ErrInvalidPassword));

    let src = build_historical_source_tree(tmp.path());
    fs::write(src.join("large_blob.bin"), vec![0x5Au8; 160 * 1024]).unwrap();
    set_file_mtime(&src.join("large_blob.bin"), Q1_HIST_MTIME_2023_NOV);

    let pwd = CString::new("SplitKey2026!").unwrap();
    let seg1 = tmp.path().join("split_vol.zip.001");
    let opts_split = make_create_opts(
        TTZipCompressionLevel::Store,
        TTZipEncryptionMethod::ZipCrypto,
        Some(&pwd),
        2,
    );
    create_zip_streaming_parallel_split(&seg1, &[src], &opts_split, 64 * 1024).unwrap();
    assert!(seg1.exists() && tmp.path().join("split_vol.zip.002").exists());

    let split_out = tmp.path().join("split_extracted");
    extract_archive_to_dir(&seg1, &split_out, &make_extract_opts(Some(&pwd)), None).unwrap();
    assert_eq!(file_mtime(&split_out.join("corpus_posix/hist_2023_nov.txt")), Q1_HIST_MTIME_2023_NOV);
    assert_eq!(file_mtime(&split_out.join("corpus_posix/large_blob.bin")), Q1_HIST_MTIME_2023_NOV);

    let bad_pwd = CString::new("WrongSplitKey").unwrap();
    assert_eq!(
        extract_archive_to_dir(&seg1, &tmp.path().join("split_bad"), &make_extract_opts(Some(&bad_pwd)), None),
        Err(TTZipStatus::ErrInvalidPassword)
    );
}

#[test]
fn test_calendar_boundaries_leap_years_and_priority_resolution() {
    let cases = [
        (1980, 1, 1, 0, 0, 0),
        (2000, 2, 29, 23, 59, 58),
        (2023, 12, 31, 23, 59, 58),
        (2024, 1, 1, 0, 0, 0),
        (2024, 2, 29, 12, 34, 56),
        (2026, 2, 28, 23, 59, 58),
        (2100, 2, 28, 12, 0, 0),
        (2107, 12, 31, 23, 59, 58),
    ];
    for (y, m, d, hh, mm, ss) in cases {
        let dt = DosDateTime::new(y, m, d, hh, mm, ss).unwrap();
        let local_epoch = dt.to_local_unix_epoch_secs();
        assert_eq!(DosDateTime::from_local_unix_epoch_secs(local_epoch), dt);
        assert_eq!(dos_to_local_unix_epoch_secs(dt.dos_time(), dt.dos_date()), local_epoch);
    }

    let base_dt = DosDateTime::new(2024, 2, 29, 12, 34, 54).unwrap();
    let base_epoch = base_dt.to_local_unix_epoch_secs();
    let odd_dt = DosDateTime::from_local_unix_epoch_secs(base_epoch + 1);
    assert_eq!(odd_dt.second, 54);
    assert_eq!(odd_dt.to_local_unix_epoch_secs(), base_epoch);
    assert_eq!(DosDateTime::from_local_unix_epoch_secs(0), DosDateTime::MIN);
    assert_eq!(DosDateTime::from_local_unix_epoch_secs(5_000_000_000), DosDateTime::MAX);

    let mut extra = ZipExtraFields::default();
    assert_eq!(resolve_zip_mtime_secs(&extra, base_dt.dos_time(), base_dt.dos_date()), base_epoch);
    let ticks = NtfsExtra::unix_secs_to_filetime(1700001235);
    let ntfs_bytes = (NtfsExtra {
        mtime_ticks: ticks,
        atime_ticks: ticks,
        ctime_ticks: ticks,
    })
    .build();
    extra = ZipExtraFields::parse(&ntfs_bytes, true, false, false, false);
    assert_eq!(resolve_zip_mtime_secs(&extra, base_dt.dos_time(), base_dt.dos_date()), 1700001235);
    extra.mod_time = Some(1700001234);
    assert_eq!(resolve_zip_mtime_secs(&extra, base_dt.dos_time(), base_dt.dos_date()), 1700001234);
}

#[test]
fn test_timezone_matrix_utc_vs_pure_dos_contract() {
    if let Ok(tz) = std::env::var("TTZIP_R3_TZ_WORKER") {
        let dt = DosDateTime::new(2023, 11, 14, 22, 33, 54).unwrap();
        let expected_local = dt.to_local_unix_epoch_secs();
        let pure_dos_item = build_pure_dos_archive(dt.dos_time(), dt.dos_date(), b"hello dos");
        let tmp = TempDir::new().unwrap();
        let pure_path = tmp.path().join("pure_dos.zip");
        fs::write(&pure_path, &pure_dos_item).unwrap();

        let inspected = collect_inspected_entries(&pure_path, None);
        assert_eq!(
            inspected[0].mtime_epoch_secs, expected_local,
            "Pure DOS inspect mismatch in TZ={tz}"
        );
        let archive = ZipArchive::open_slice(&pure_dos_item).unwrap();
        assert_eq!(archive.entries()[0].mtime_epoch_secs, expected_local);

        let items = vec![zip_item("utc.txt", b"utc".to_vec(), Q1_HIST_MTIME_2023_NOV, 0o100644)];
        let zip_utc = assemble_zip_archive(
            &compress_items_parallel(items, 0, TTZipEncryptionMethod::None, None, 1).unwrap(),
        )
        .unwrap();
        let utc_path = tmp.path().join("utc_ext.zip");
        fs::write(&utc_path, &zip_utc).unwrap();
        let utc_inspected = collect_inspected_entries(&utc_path, None);
        assert_eq!(
            utc_inspected[0].mtime_epoch_secs, Q1_HIST_MTIME_2023_NOV,
            "UTC 0x5455 must be exact in TZ={tz}"
        );
        return;
    }

    let exe = std::env::current_exe().unwrap();
    for tz in ["UTC0", "Asia/Shanghai", "America/New_York"] {
        let status = Command::new(&exe)
            .args(["--exact", "test_timezone_matrix_utc_vs_pure_dos_contract", "--nocapture"])
            .env("TZ", tz)
            .env("TTZIP_R3_TZ_WORKER", tz)
            .status()
            .unwrap();
        assert!(status.success(), "Timezone worker failed for TZ={tz}");
    }
}

fn build_pure_dos_archive(dos_time: u16, dos_date: u16, payload: &[u8]) -> Vec<u8> {
    let name = b"pure_dos.txt";
    let crc = crc32_fast(0, payload);
    let len = payload.len() as u32;
    let mut buf = Vec::new();
    buf.extend_from_slice(&0x04034b50u32.to_le_bytes());
    buf.extend_from_slice(&20u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&dos_time.to_le_bytes());
    buf.extend_from_slice(&dos_date.to_le_bytes());
    buf.extend_from_slice(&crc.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(name);
    buf.extend_from_slice(payload);

    let cd_off = buf.len() as u32;
    buf.extend_from_slice(&0x02014b50u32.to_le_bytes());
    buf.extend_from_slice(&0x031eu16.to_le_bytes());
    buf.extend_from_slice(&20u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&dos_time.to_le_bytes());
    buf.extend_from_slice(&dos_date.to_le_bytes());
    buf.extend_from_slice(&crc.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&(name.len() as u16).to_le_bytes());
    buf.extend_from_slice(&[0u8; 8]);
    buf.extend_from_slice(&(0o100644u32 << 16).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(name);

    let cd_size = (buf.len() as u32) - cd_off;
    buf.extend_from_slice(&0x06054b50u32.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&cd_size.to_le_bytes());
    buf.extend_from_slice(&cd_off.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf
}

#[test]
fn test_c3_04_external_tools_infozip_cross_verification() {
    let tmp = TempDir::new().unwrap();
    let pwd_ok = CString::new("Secret2026!").unwrap();
    let pwd_bad = CString::new("WrongPass123").unwrap();

    // 1. External /usr/bin/zip (-P ZipCrypto with bit 3 data descriptor) -> TTZip extract
    let src = build_historical_source_tree(tmp.path());
    let ext_created = tmp.path().join("infozip_created.zip");
    let st = Command::new("/usr/bin/zip")
        .current_dir(tmp.path())
        .args(["-r", "-q", "-P", "Secret2026!", ext_created.to_str().unwrap(), "corpus_posix"])
        .status()
        .unwrap();
    assert!(st.success(), "/usr/bin/zip failed");

    let ttzip_out = tmp.path().join("from_infozip");
    extract_archive_to_dir(&ext_created, &ttzip_out, &make_extract_opts(Some(&pwd_ok)), None).unwrap();
    assert_eq!(file_mtime(&ttzip_out.join("corpus_posix/hist_2023_nov.txt")), Q1_HIST_MTIME_2023_NOV);
    assert_eq!(file_mtime(&ttzip_out.join("corpus_posix/nested/hist_2023_mar.sh")), Q1_HIST_MTIME_2023_MAR);

    let ext_bytes = fs::read(&ext_created).unwrap();
    let ext_arch = ZipArchive::open_slice(&ext_bytes).unwrap();
    assert_eq!(
        ext_arch.extract_all(&tmp.path().join("bad_iz"), &make_extract_opts(Some(&pwd_bad))),
        Err(TTZipStatus::ErrInvalidPassword)
    );
    assert_eq!(
        extract_single_entry_memory(&ext_created, Some("corpus_posix/hist_2023_nov.txt"), -1, Some("Secret2026!")).unwrap(),
        b"Historical sample Nov 2023 payload\n"
    );

    // 2. TTZip creates ZipCrypto archive -> /usr/bin/unzip verify & extract
    let ttzip_zc = tmp.path().join("ttzip_created_zc.zip");
    create_zip_streaming_parallel_with_sheet(
        &ttzip_zc,
        &[src],
        &make_create_opts(TTZipCompressionLevel::Normal, TTZipEncryptionMethod::ZipCrypto, Some(&pwd_ok), 2),
        None,
    )
    .unwrap();
    let unzip_out = tmp.path().join("unzip_extracted");
    let st_unzip = Command::new("/usr/bin/unzip")
        .args(["-q", "-P", "Secret2026!", ttzip_zc.to_str().unwrap(), "-d", unzip_out.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(st_unzip.success(), "/usr/bin/unzip failed to extract TTZip ZipCrypto archive");
    assert_eq!(file_mtime(&unzip_out.join("corpus_posix/hist_2023_nov.txt")), Q1_HIST_MTIME_2023_NOV);
    assert_eq!(file_mtime(&unzip_out.join("corpus_posix/nested/hist_2023_mar.sh")), Q1_HIST_MTIME_2023_MAR);
}

fn verify_in_place_edited_archive(edited_zip: &Path, out_dir: &Path) {
    let edited_bytes = fs::read(edited_zip).unwrap();
    let raw_headers = parse_raw_lfh_and_cdfh_times(&edited_bytes);
    assert_eq!(raw_headers.len(), 3);
    for h in &raw_headers {
        assert_eq!(h.lfh_dos_time, h.cdfh_dos_time);
        assert_eq!(h.lfh_dos_date, h.cdfh_dos_date);
        let dt = DosDateTime::from_dos(h.cdfh_dos_date, h.cdfh_dos_time)
            .expect("valid DOS date after in_place_edit");
        assert_eq!(dt.year, 2023);
        let expected_ts = if h.name == "replace_mar.txt" {
            Q1_HIST_MTIME_2023_MAR
        } else {
            Q1_HIST_MTIME_2023_NOV
        };
        assert_eq!((h.cdfh_dos_time, h.cdfh_dos_date), unix_to_dos_time(expected_ts as u32));
        assert_eq!(h.lfh_ext_mtime, Some(expected_ts));
        assert_eq!(h.cdfh_ext_mtime, Some(expected_ts));
    }

    extract_archive_to_dir(edited_zip, out_dir, &make_extract_opts(None), None).unwrap();
    assert!(!out_dir.join("delete_me.txt").exists());
    assert_eq!(fs::read(out_dir.join("keep_nov.txt")).unwrap(), b"untouched nov 2023 payload");
    assert_eq!(file_mtime(&out_dir.join("keep_nov.txt")), Q1_HIST_MTIME_2023_NOV);
    assert_eq!(fs::read(out_dir.join("replace_mar.txt")).unwrap(), b"updated mar 2023 payload");
    assert_eq!(file_mtime(&out_dir.join("replace_mar.txt")), Q1_HIST_MTIME_2023_MAR);
    assert_eq!(fs::read(out_dir.join("added_nov.txt")).unwrap(), b"appended nov 2023 payload");
    assert_eq!(file_mtime(&out_dir.join("added_nov.txt")), Q1_HIST_MTIME_2023_NOV);
}

#[test]
fn test_in_place_edit_zip_preserves_timestamps_and_content() {
    let tmp = TempDir::new().unwrap();
    let items = vec![
        zip_item("keep_nov.txt", b"untouched nov 2023 payload".to_vec(), Q1_HIST_MTIME_2023_NOV, 0o100644),
        zip_item("replace_mar.txt", b"original mar payload".to_vec(), Q1_HIST_MTIME_2023_MAR, 0o100644),
        zip_item("delete_me.txt", b"to be deleted".to_vec(), Q1_HIST_MTIME_2023_MAR, 0o100644),
    ];
    let orig_bytes = assemble_zip_archive(
        &compress_items_parallel(items, 6, TTZipEncryptionMethod::None, None, 1).unwrap(),
    )
    .unwrap();
    let orig_zip = tmp.path().join("orig.zip");
    fs::write(&orig_zip, &orig_bytes).unwrap();

    let repl_src = tmp.path().join("repl_src.txt");
    write_file_with_mtime(&repl_src, b"updated mar 2023 payload", 0o644, Q1_HIST_MTIME_2023_MAR);
    let add_src = tmp.path().join("add_src.txt");
    write_file_with_mtime(&add_src, b"appended nov 2023 payload", 0o644, Q1_HIST_MTIME_2023_NOV);

    let edited_zip = tmp.path().join("edited.zip");
    let actions = [
        InPlaceAction::Delete {
            entry_path: "delete_me.txt".to_string(),
        },
        InPlaceAction::Replace {
            entry_path: "replace_mar.txt".to_string(),
            source_path: repl_src,
        },
        InPlaceAction::Append {
            entry_path: "added_nov.txt".to_string(),
            source_path: add_src,
        },
    ];
    in_place_edit_zip(&orig_zip, &edited_zip, &actions).expect("in_place_edit_zip must succeed");
    verify_in_place_edited_archive(&edited_zip, &tmp.path().join("edited_out"));
}

#[test]
fn test_sha256_permissions_symlinks_and_cancellation_non_regression() {
    let tmp = TempDir::new().unwrap();
    let src = build_historical_source_tree(tmp.path());
    let ro_file = src.join("readonly.bin");
    let payload = b"Read-only binary payload for integrity verification".repeat(64);
    write_file_with_mtime(&ro_file, &payload, 0o444, Q1_HIST_MTIME_2023_NOV);
    std::os::unix::fs::symlink("hist_2023_nov.txt", src.join("link_to_nov.txt")).unwrap();

    let zip_path = tmp.path().join("full_meta.zip");
    let opts = make_create_opts(
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::None,
        None,
        2,
    );
    create_zip_streaming_parallel_with_sheet(&zip_path, std::slice::from_ref(&src), &opts, None).unwrap();

    let cancelled_sheet = ProgressAtomicSheet::with_total(1024);
    cancelled_sheet.cancel();
    let cancel_dir = tmp.path().join("cancelled_out");
    let zip_bytes = fs::read(&zip_path).unwrap();
    let z_arch = ZipArchive::open_slice(&zip_bytes).unwrap();
    assert_eq!(
        z_arch.extract_all_with_sheet(&cancel_dir, &make_extract_opts(None), Some(&cancelled_sheet)),
        Err(TTZipStatus::Cancelled)
    );
    assert!(!cancel_dir.join("corpus_posix/readonly.bin").exists());

    let cancel_create_sheet = ProgressAtomicSheet::with_total(1024);
    cancel_create_sheet.cancel();
    assert_eq!(
        create_zip_streaming_parallel_with_sheet(
            &tmp.path().join("cancelled_create.zip"),
            &[src],
            &opts,
            Some(&cancel_create_sheet)
        ),
        Err(TTZipStatus::Cancelled)
    );

    let ok_dir = tmp.path().join("ok_out");
    extract_archive_to_dir(&zip_path, &ok_dir, &make_extract_opts(None), None).unwrap();
    let extracted_ro = fs::read(ok_dir.join("corpus_posix/readonly.bin")).unwrap();
    assert_eq!(extracted_ro, payload);
    assert_eq!(
        fs::metadata(ok_dir.join("corpus_posix/readonly.bin"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o444
    );
    assert!(
        fs::symlink_metadata(ok_dir.join("corpus_posix/link_to_nov.txt"))
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let mut file_reader = fs::File::open(&zip_path).unwrap();
    let file_len = file_reader.metadata().unwrap().len();
    let stream_dir = tmp.path().join("stream_reader_out");
    extract_zip_from_reader(&mut file_reader, file_len, &stream_dir, &make_extract_opts(None), None)
        .unwrap();
    assert_eq!(file_mtime(&stream_dir.join("corpus_posix/readonly.bin")), Q1_HIST_MTIME_2023_NOV);
}
