// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unit and integration tests for `check` subcommand and deep payload verification.

use super::stream_verify::deep_verify_stream_extract_with_quota;
use super::*;
use ttzip_engine::sevenz::writer::create_7z_solid_archive_bytes_with_password;
use ttzip_engine::types::TTZipEncryptionMethod;
use ttzip_engine::zip::writer::{ZipInputItem, assemble_zip_archive, compress_items_parallel};

#[test]
fn test_deep_verify_zip_and_sevenz_streaming() {
    let zip_items = vec![ZipInputItem {
        rel_path: "stream_check.txt".to_string(),
        data: b"Streaming verification test payload".repeat(32),
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];
    let compressed = compress_items_parallel(zip_items, 6, TTZipEncryptionMethod::None, None, 1)
        .expect("compress zip");
    let zip_bytes = assemble_zip_archive(&compressed).expect("assemble zip");
    let mut errors = Vec::new();
    assert_eq!(deep_verify_zip(&zip_bytes, None, &mut errors), 0);
    assert!(errors.is_empty());

    let sz_items = vec![ZipInputItem {
        rel_path: "solid_check.txt".to_string(),
        data: b"Solid 7z streaming verification payload".repeat(32),
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];
    let sz_bytes =
        create_7z_solid_archive_bytes_with_password(&sz_items, 6, 1, None).expect("assemble 7z");
    assert_eq!(deep_verify_sevenz(&sz_bytes, None, &mut errors), 0);
    assert!(errors.is_empty());

    let mut corrupted_zip = zip_bytes;
    corrupted_zip[72] ^= 0xFF;
    assert!(deep_verify_zip(&corrupted_zip, None, &mut errors) >= 1);
    assert!(!errors.is_empty());
}

#[test]
fn test_deep_verify_archive_streaming_other_formats() {
    use tempfile::tempdir;
    use ttzip_engine::archive::unified::create_archive;
    use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions};

    let dir = tempdir().expect("tempdir");
    let src_file = dir.path().join("payload.txt");
    std::fs::write(&src_file, b"Deep streaming payload check data ".repeat(256))
        .expect("write payload");

    let archive_path = dir.path().join("sample.tar.gz");
    let opts = TTZipCreateOptions {
        format: TTZipArchiveFormat::TarGz,
        level: TTZipCompressionLevel::Normal,
        thread_budget: 1,
        ..Default::default()
    };
    create_archive(std::slice::from_ref(&src_file), &archive_path, &opts, 0)
        .expect("create tar.gz");

    let valid_dto = check_archive_integrity(&archive_path, None, true).expect("check tar.gz");
    assert!(valid_dto.is_valid);
    assert_eq!(valid_dto.corrupted_entries, 0);
    assert!(valid_dto.errors.is_empty());

    #[cfg(unix)]
    {
        let link_path = dir.path().join("sym.txt");
        std::os::unix::fs::symlink("payload.txt", &link_path).expect("create symlink");
        let cpio_path = dir.path().join("with_link.cpio");
        let cpio_opts = TTZipCreateOptions {
            format: TTZipArchiveFormat::Cpio,
            ..opts
        };
        create_archive(&[src_file, link_path], &cpio_path, &cpio_opts, 0).expect("create cpio");
        let cpio_dto = check_archive_integrity(&cpio_path, None, true).expect("check cpio");
        assert!(cpio_dto.is_valid);
        assert_eq!(cpio_dto.corrupted_entries, 0);
    }

    let mut bytes = std::fs::read(&archive_path).expect("read tar.gz");
    let len = bytes.len();
    bytes[len - 4] ^= 0xFF;
    let corrupt_path = dir.path().join("corrupt.tar.gz");
    std::fs::write(&corrupt_path, &bytes).expect("write corrupt tar.gz");

    let mut errors = Vec::new();
    let count = deep_verify_archive(
        &corrupt_path,
        ContainerFormat::Gzip,
        &bytes,
        false,
        None,
        &mut errors,
    );
    assert!(count >= 1);
    assert!(!errors.is_empty());
}

#[cfg(unix)]
#[test]
fn test_deep_verify_sink_and_byte_quota_enforcement() {
    use tempfile::tempdir;
    use ttzip_archive::tar::header::{TarEntryType, TarHeader};
    use ttzip_engine::archive::tar::writer::TarWriter;
    use ttzip_engine::codecs::deflate::libdeflate_gzip_compress;

    let dir = tempdir().expect("tempdir");
    let archive_path = dir.path().join("locked_modes.tar.gz");
    let mut tar = TarWriter::new(Vec::new());
    tar.append_dir("locked_dir", 0o000, 1700000000)
        .expect("append locked_dir");
    let payload = b"payload inside mode 000 dir".repeat(512);
    tar.append_file("locked_dir/nested.txt", &payload, 0o400, 1700000000)
        .expect("append file");
    tar.finish().expect("finish tar");
    let gz_bytes = libdeflate_gzip_compress(&tar.into_inner(), 1).expect("gzip compress");
    std::fs::write(&archive_path, &gz_bytes).expect("write locked_modes.tar.gz");

    let dto = check_archive_integrity(&archive_path, None, true).expect("check mode-000");
    assert!(dto.is_valid);
    assert_eq!(dto.total_entries, 2);
    assert_eq!(dto.corrupted_entries, 0);

    let mut errs = Vec::new();
    let small_quota = deep_verify_stream_extract_with_quota(
        &archive_path,
        ContainerFormat::Gzip,
        None,
        1024,
        &mut errs,
    );
    assert_eq!(small_quota, 1);
    assert!(errs.iter().any(|e| e.contains("quota exceeded")));

    let raw_gz_path = dir.path().join("single_stream.gz");
    let raw_gz = libdeflate_gzip_compress(&vec![b'A'; 8192], 1).expect("gzip raw");
    std::fs::write(&raw_gz_path, raw_gz).expect("write single_stream.gz");
    errs.clear();
    let stream_quota = deep_verify_stream_extract_with_quota(
        &raw_gz_path,
        ContainerFormat::Gzip,
        None,
        4096,
        &mut errs,
    );
    assert_eq!(stream_quota, 1);
    assert!(errs.iter().any(|e| e.contains("quota exceeded")));

    let mut oversized_tar = Vec::with_capacity(1536);
    let mut hdr = TarHeader::new();
    hdr.set_ustar_magic();
    hdr.set_name("oversized.bin");
    hdr.set_mode(0o644);
    hdr.set_entry_type(TarEntryType::Regular);
    hdr.set_size(MAX_DEEP_VERIFY_OUTPUT_BYTES + 512);
    hdr.update_checksum();
    oversized_tar.extend_from_slice(hdr.as_bytes());
    oversized_tar.resize(1536, 0);
    let oversized_path = dir.path().join("oversized.tar.gz");
    let oversized_gz = libdeflate_gzip_compress(&oversized_tar, 1).expect("gzip oversized");
    std::fs::write(&oversized_path, oversized_gz).expect("write oversized.tar.gz");

    errs.clear();
    let oversized_res = deep_verify_stream_extract_with_quota(
        &oversized_path,
        ContainerFormat::Gzip,
        None,
        MAX_DEEP_VERIFY_OUTPUT_BYTES,
        &mut errs,
    );
    assert_eq!(oversized_res, 1);
    assert!(errs.iter().any(|e| e.contains("quota exceeded")));

    hdr.set_size(512);
    hdr.update_checksum();
    let part1 = dir.path().join("trunc.tar.001");
    std::fs::write(&part1, hdr.as_bytes()).expect("write part1");
    std::fs::write(dir.path().join("trunc.tar.002"), [b'X'; 512]).expect("write part2");
    errs.clear();
    let trunc_res =
        deep_verify_stream_extract_with_quota(&part1, ContainerFormat::Tar, None, 8192, &mut errs);
    assert_eq!(trunc_res, 1);
    assert!(errs.iter().any(|e| e.contains("trailer")));
}

#[test]
fn test_deep_verify_hardlink_target_resolution() {
    use tempfile::tempdir;
    use ttzip_archive::tar::header::{TarEntryType, TarHeader};
    use ttzip_engine::codecs::deflate::libdeflate_gzip_compress;

    let dir = tempdir().expect("tempdir");

    let mut dangling_tar = Vec::with_capacity(1536);
    let mut link_hdr = TarHeader::new();
    link_hdr.set_ustar_magic();
    link_hdr.set_name("orphan_link.txt");
    link_hdr.set_mode(0o644);
    link_hdr.set_size(0);
    link_hdr.set_entry_type(TarEntryType::Link);
    link_hdr.set_linkname("missing_target.txt");
    link_hdr.update_checksum();
    dangling_tar.extend_from_slice(link_hdr.as_bytes());
    dangling_tar.resize(1536, 0);

    let dangling_gz_path = dir.path().join("dangling_hardlink.tar.gz");
    let dangling_gz = libdeflate_gzip_compress(&dangling_tar, 1).expect("gzip dangling tar");
    std::fs::write(&dangling_gz_path, &dangling_gz).expect("write dangling_hardlink.tar.gz");

    let dangling_dto =
        check_archive_integrity(&dangling_gz_path, None, true).expect("check dangling tar.gz");
    assert!(!dangling_dto.is_valid);
    assert!(dangling_dto.corrupted_entries >= 1);
    assert!(
        dangling_dto
            .errors
            .iter()
            .any(|e| e.contains("unresolved hardlink"))
    );

    let mut tar_errors = Vec::new();
    assert_eq!(deep_verify_tar(&dangling_tar, &mut tar_errors), 1);
    assert!(tar_errors.iter().any(|e| e.contains("unresolved hardlink")));

    let mut valid_tar = Vec::with_capacity(2560);
    valid_tar.extend_from_slice(link_hdr.as_bytes());
    let mut file_hdr = TarHeader::new();
    file_hdr.set_ustar_magic();
    file_hdr.set_name("missing_target.txt");
    file_hdr.set_mode(0o644);
    file_hdr.set_size(12);
    file_hdr.set_entry_type(TarEntryType::Regular);
    file_hdr.update_checksum();
    valid_tar.extend_from_slice(file_hdr.as_bytes());
    let mut block = [0u8; 512];
    block[..12].copy_from_slice(b"hello target");
    valid_tar.extend_from_slice(&block);
    valid_tar.resize(2560, 0);

    let valid_gz_path = dir.path().join("valid_hardlink.tar.gz");
    let valid_gz = libdeflate_gzip_compress(&valid_tar, 1).expect("gzip valid tar");
    std::fs::write(&valid_gz_path, &valid_gz).expect("write valid_hardlink.tar.gz");

    let valid_dto =
        check_archive_integrity(&valid_gz_path, None, true).expect("check valid tar.gz");
    assert!(valid_dto.is_valid);
    assert_eq!(valid_dto.corrupted_entries, 0);
    assert!(valid_dto.errors.is_empty());
}

#[test]
fn test_deep_verify_symlink_safety_and_split_archives() {
    use tempfile::tempdir;
    use ttzip_archive::tar::header::{TarEntryType, TarHeader};
    use ttzip_engine::codecs::deflate::libdeflate_gzip_compress;

    let dir = tempdir().expect("tempdir");

    let mut escape_tar = Vec::with_capacity(1536);
    let mut sym_hdr = TarHeader::new();
    sym_hdr.set_ustar_magic();
    sym_hdr.set_name("escape_link.txt");
    sym_hdr.set_mode(0o777);
    sym_hdr.set_size(0);
    sym_hdr.set_entry_type(TarEntryType::Symlink);
    sym_hdr.set_linkname("../../outside");
    sym_hdr.update_checksum();
    escape_tar.extend_from_slice(sym_hdr.as_bytes());
    escape_tar.resize(1536, 0);

    let escape_gz_path = dir.path().join("escape_symlink.tar.gz");
    let escape_gz = libdeflate_gzip_compress(&escape_tar, 1).expect("gzip escape tar");
    std::fs::write(&escape_gz_path, &escape_gz).expect("write escape_symlink.tar.gz");

    let escape_dto =
        check_archive_integrity(&escape_gz_path, None, true).expect("check escape tar.gz");
    assert!(!escape_dto.is_valid);
    assert!(escape_dto.corrupted_entries >= 1);

    let sz_items = vec![ZipInputItem {
        rel_path: "secret.txt".to_string(),
        data: b"Encrypted split 7z payload".repeat(32),
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];
    let enc_7z =
        create_7z_solid_archive_bytes_with_password(&sz_items, 6, 1, Some("pass123"))
            .expect("create encrypted 7z");
    let mid = enc_7z.len() / 2;
    let sz_part1 = dir.path().join("enc_split.7z.001");
    let sz_part2 = dir.path().join("enc_split.7z.002");
    std::fs::write(&sz_part1, &enc_7z[..mid]).expect("write 7z.001");
    std::fs::write(&sz_part2, &enc_7z[mid..]).expect("write 7z.002");

    let ok_7z =
        check_archive_integrity(&sz_part1, Some("pass123"), true).expect("check split 7z");
    assert!(ok_7z.is_valid);
    assert_eq!(ok_7z.corrupted_entries, 0);

    let bad_7z =
        check_archive_integrity(&sz_part1, Some("wrong"), true).expect("check split 7z wrong");
    assert!(!bad_7z.is_valid);

    let empty_zip = assemble_zip_archive(&[]).expect("assemble empty zip");
    let zip_part1 = dir.path().join("empty.zip.001");
    let zip_part2 = dir.path().join("empty.zip.002");
    std::fs::write(&zip_part1, &empty_zip[..11]).expect("write zip.001");
    std::fs::write(&zip_part2, &empty_zip[11..]).expect("write zip.002");
    let mut zip_errs = Vec::new();
    let empty_zip_res = deep_verify_stream_extract_with_quota(
        &zip_part1,
        ContainerFormat::Zip,
        None,
        8192,
        &mut zip_errs,
    );
    assert_eq!(empty_zip_res, 0);
    assert!(zip_errs.is_empty());
}

#[test]
fn test_deep_verify_large_split_7z_and_reverse_hardlink_ancestors() {
    use tempfile::tempdir;
    use ttzip_archive::tar::header::{TarEntryType, TarHeader};
    use ttzip_engine::codecs::deflate::libdeflate_gzip_compress;

    let dir = tempdir().expect("tempdir");
    let large_7z_items = vec![ZipInputItem {
        rel_path: "over_64mb.bin".to_string(),
        data: vec![0u8; 65 * 1024 * 1024],
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }];
    let large_7z =
        create_7z_solid_archive_bytes_with_password(&large_7z_items, 1, 2, None)
            .expect("create >64MB 7z");
    let lmid = large_7z.len() / 2;
    let lpart1 = dir.path().join("large_split.7z.001");
    let lpart2 = dir.path().join("large_split.7z.002");
    std::fs::write(&lpart1, &large_7z[..lmid]).expect("write large 7z.001");
    std::fs::write(&lpart2, &large_7z[lmid..]).expect("write large 7z.002");
    let large_dto = check_archive_integrity(&lpart1, None, true).expect("check >64MB split 7z");
    assert!(large_dto.is_valid, "errors: {:?}", large_dto.errors);

    let mut rev_hl_tar = Vec::with_capacity(3072);
    let mut x_hdr = TarHeader::new();
    x_hdr.set_ustar_magic();
    x_hdr.set_name("x");
    x_hdr.set_mode(0o644);
    x_hdr.set_size(4);
    x_hdr.set_entry_type(TarEntryType::Regular);
    x_hdr.update_checksum();
    rev_hl_tar.extend_from_slice(x_hdr.as_bytes());
    let mut x_blk = [0u8; 512];
    x_blk[..4].copy_from_slice(b"data");
    rev_hl_tar.extend_from_slice(&x_blk);
    let mut dir_sym = TarHeader::new();
    dir_sym.set_ustar_magic();
    dir_sym.set_name("dir");
    dir_sym.set_mode(0o777);
    dir_sym.set_size(0);
    dir_sym.set_entry_type(TarEntryType::Symlink);
    dir_sym.set_linkname("x");
    dir_sym.update_checksum();
    rev_hl_tar.extend_from_slice(dir_sym.as_bytes());
    let mut hl_hdr = TarHeader::new();
    hl_hdr.set_ustar_magic();
    hl_hdr.set_name("x");
    hl_hdr.set_mode(0o644);
    hl_hdr.set_size(0);
    hl_hdr.set_entry_type(TarEntryType::Link);
    hl_hdr.set_linkname("dir/y");
    hl_hdr.update_checksum();
    rev_hl_tar.extend_from_slice(hl_hdr.as_bytes());
    rev_hl_tar.resize(3072, 0);
    let rev_hl_gz_path = dir.path().join("rev_hl_symlink_parent.tar.gz");
    let rev_hl_gz = libdeflate_gzip_compress(&rev_hl_tar, 1).expect("gzip rev_hl_tar");
    std::fs::write(&rev_hl_gz_path, rev_hl_gz).expect("write rev_hl_symlink_parent.tar.gz");
    let rev_hl_dto =
        check_archive_integrity(&rev_hl_gz_path, None, true).expect("check rev_hl_tar.gz");
    assert!(!rev_hl_dto.is_valid);
}

#[test]
fn test_keep_both_preserves_duplicate_archive_paths_and_existing_stage_dir() {
    use crate::cli::CliExtractParams;
    use crate::cli::handlers::extract::execute_extract_with_policy;
    use std::fs;
    use tempfile::tempdir;
    use ttzip_engine::archive::tar::writer::TarWriter;
    use ttzip_engine::codecs::deflate::libdeflate_gzip_compress;

    let dir = tempdir().expect("tempdir");
    let archive_path = dir.path().join("dups.tar.gz");
    let mut tar = TarWriter::new(Vec::new());
    tar.append_file("report.txt", b"first occurrence", 0o644, 1700000000)
        .expect("append first");
    tar.append_file("report.txt", b"second occurrence", 0o644, 1700000000)
        .expect("append second");
    tar.append_file("unique.txt", b"unique payload", 0o644, 1700000000)
        .expect("append unique");
    tar.finish().expect("finish tar");
    let gz_bytes = libdeflate_gzip_compress(&tar.into_inner(), 1).expect("gzip compress");
    fs::write(&archive_path, gz_bytes).expect("write dups.tar.gz");

    let out_dir = dir.path().join("out");
    let pid = std::process::id();
    for seq in 0..8 {
        let preexisting_stage = out_dir.join(format!(".ttzip_extract_stage_{}_{}", pid, seq));
        fs::create_dir_all(&preexisting_stage).expect("create preexisting stage");
        fs::write(preexisting_stage.join("keep_me.txt"), b"unrelated").expect("write keep_me");
    }

    let include = vec!["*".to_string()];
    execute_extract_with_policy(
        CliExtractParams {
            archive_path: &archive_path,
            output_dir: Some(&out_dir),
            password: None,
            threads: 1,
            verbose: false,
            dry_run: false,
            include: &include,
            exclude: &[],
        },
        Some("keep-both"),
    )
    .expect("extract duplicate archive entries with keep-both");

    assert_eq!(
        fs::read(out_dir.join("report.txt")).expect("read report.txt"),
        b"first occurrence"
    );
    assert_eq!(
        fs::read(out_dir.join("report 2.txt")).expect("read report 2.txt"),
        b"second occurrence"
    );
    assert_eq!(
        fs::read(out_dir.join("unique.txt")).expect("read unique.txt"),
        b"unique payload"
    );
    for seq in 0..8 {
        let preexisting_file = out_dir
            .join(format!(".ttzip_extract_stage_{}_{}", pid, seq))
            .join("keep_me.txt");
        assert_eq!(
            fs::read(&preexisting_file).expect("preexisting stage preserved"),
            b"unrelated"
        );
    }
}

#[test]
fn test_keep_both_reserves_planned_destinations_across_entries() {
    use crate::cli::CliExtractParams;
    use crate::cli::handlers::extract::execute_extract_with_policy;
    use std::fs;
    use tempfile::tempdir;
    use ttzip_engine::archive::unified::create_archive;
    use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions};

    let dir = tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("create src_dir");
    let f1 = src_dir.join("report.txt");
    let f2 = src_dir.join("report 2.txt");
    fs::write(&f1, b"archive entry one").expect("write f1");
    fs::write(&f2, b"archive entry two").expect("write f2");

    let archive_path = dir.path().join("reports.cpio");
    let opts = TTZipCreateOptions {
        format: TTZipArchiveFormat::Cpio,
        level: TTZipCompressionLevel::Normal,
        thread_budget: 1,
        ..Default::default()
    };
    create_archive(&[f1, f2], &archive_path, &opts, 0).expect("create reports.cpio");

    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("create out_dir");
    fs::write(out_dir.join("report.txt"), b"preexisting local").expect("write preexisting");

    execute_extract_with_policy(
        CliExtractParams {
            archive_path: &archive_path,
            output_dir: Some(&out_dir),
            password: None,
            threads: 1,
            verbose: false,
            dry_run: false,
            include: &[],
            exclude: &[],
        },
        Some("keep-both"),
    )
    .expect("keep-both extraction must succeed without overwriting planned destinations");

    assert_eq!(
        fs::read(out_dir.join("report.txt")).expect("read report.txt"),
        b"preexisting local"
    );
    assert_eq!(
        fs::read(out_dir.join("report 2.txt")).expect("read report 2.txt"),
        b"archive entry one"
    );
    assert_eq!(
        fs::read(out_dir.join("report 2 2.txt")).expect("read report 2 2.txt"),
        b"archive entry two"
    );
}
