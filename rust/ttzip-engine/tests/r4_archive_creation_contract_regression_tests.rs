// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R4 Regression Test Suite: Archive Creation Capabilities, 7z Header Encryption,
//! 7z POSIX Metadata Preservation, and Single-Stream vs TAR Container Contracts.
//!
//! Covers:
//! - Q1-ISSUE-04 (`C3-03`, `C5-03c`): 7z `encryptFileNames` (`kEncodedHeader = 0x17`) & POSIX symlinks/permissions/mtime.
//! - Q1-ISSUE-05 (`C1-21`..`C1-29`): Single-file compression streams vs `.tar.*` containers.
//! - Q1-ISSUE-06 (`C1-30`..`C1-36`): Removal of fake PKZIP fallback, authentic `Aar`/`Ar`/`Iso`/`Cpio`/`Xar` creation,
//!   and explicit rejection of non-creatable formats (`Dmg`, `Cab`, `Rpm`, `Deb`, `Wim`, `Rar`, `Squashfs`, `Lzh`).

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

use ttzip_engine::archive::unified::create::{create_archive, create_archive_detailed};
use ttzip_engine::archive::unified::inspect::inspect_archive;
use ttzip_engine::sevenz::decoder::SevenZArchive;
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions, TTZipStatus, TTZIP_ABI_VERSION_3,
};
use ttzip_engine::uniffi_api::archive::{
    create_archive_advanced, create_archive_advanced_with_options,
    ttzip_uniffi_stage_next_create_extra_options, UniFFICreateArchiveConfig,
    UniFFICreateArchiveExtraOptions,
};
use ttzip_engine::uniffi_api::types::{
    ArchiveFormat, CancellationToken, ProgressHandler, TTZipError, UniFFIEntryMetadata,
};
use ttzip_engine::uniffi_api::vfs::load_archive_vfs_tree;

fn inspect_entries_vec(path: &Path, password: Option<&str>) -> Result<Vec<UniFFIEntryMetadata>, TTZipStatus> {
    let mut entries = Vec::new();
    inspect_archive(path, password, true, |e| {
        entries.push(UniFFIEntryMetadata {
            path: e.path.to_string(),
            uncompressed_size: e.uncompressed_size,
            compressed_size: e.compressed_size,
            crc32: e.crc32,
            mtime_epoch_secs: e.mtime_epoch_secs,
            mode: e.mode,
            is_directory: e.is_directory,
            is_encrypted: e.is_encrypted,
            compression_method: e.compression_method.to_string(),
            detected_encoding: e.detected_encoding.map(|s| s.to_string()),
        });
        true
    })?;
    Ok(entries)
}

fn default_create_options(format: TTZipArchiveFormat) -> TTZipCreateOptions {
    TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 2,
        solid_block_size_mb: 16,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    }
}

fn make_extract_options(password_cstr: Option<&std::ffi::CString>) -> TTZipExtractOptions {
    TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        destination_path: std::ptr::null(),
        password: password_cstr.map(|c| c.as_ptr()).unwrap_or(std::ptr::null()),
        thread_budget: 2,
        collision_policy: 0,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    }
}

fn set_file_mtime(path: &Path, epoch_secs: i64) {
    let c_path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let tv = [
        libc::timeval {
            tv_sec: epoch_secs as libc::time_t,
            tv_usec: 0,
        },
        libc::timeval {
            tv_sec: epoch_secs as libc::time_t,
            tv_usec: 0,
        },
    ];
    unsafe {
        assert_eq!(libc::lutimes(c_path.as_ptr(), tv.as_ptr()), 0);
    }
}

fn make_7z_cfg(src_dir: &Path, out: &Path, pwd: Option<String>, split: Option<u64>) -> UniFFICreateArchiveConfig {
    UniFFICreateArchiveConfig {
        source_paths: vec![src_dir.to_string_lossy().to_string()],
        output_path: out.to_string_lossy().to_string(),
        format: ArchiveFormat::SevenZip,
        level: 6,
        password: pwd,
        split_volume_size_bytes: split,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 2,
    }
}

#[test]
fn test_r4_7z_header_encryption_on_off_and_external_verification() {
    let dir = tempdir().expect("create tempdir");
    let src_dir = dir.path().join("payload_root");
    fs::create_dir_all(src_dir.join("empty_folder")).unwrap();
    let unicode_file = src_dir.join("机密_αβγ_🔒.txt");
    fs::write(&unicode_file, b"Top secret 7z header encryption payload 2026\n").unwrap();

    let password = "Accept$2026_密码_🔒".to_string();
    let pwd_cstr = std::ffi::CString::new(password.as_str()).unwrap();

    // Case 1: encrypt_file_names = Some(true) via typed options (and via Swift C-ABI staging helper)
    let out_mhe_on = dir.path().join("c3_mhe_on.7z");
    ttzip_uniffi_stage_next_create_extra_options(1);
    create_archive_advanced(
        make_7z_cfg(&src_dir, &out_mhe_on, Some(password.clone()), None),
        None,
        None,
    )
    .expect("7z create with mhe=on should succeed");

    let raw_bytes_on = fs::read(&out_mhe_on).unwrap();
    assert!(raw_bytes_on.starts_with(b"7z\xBC\xAF\x27\x1C"));
    let next_hdr_off = u64::from_le_bytes(raw_bytes_on[12..20].try_into().unwrap()) as usize;
    assert_eq!(raw_bytes_on[32 + next_hdr_off], 0x17, "Expected kEncodedHeader (0x17)");

    let utf16_needle: Vec<u8> = "empty_folder".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    assert!(!raw_bytes_on.windows(utf16_needle.len()).any(|w| w == utf16_needle.as_slice()));

    assert_eq!(inspect_entries_vec(&out_mhe_on, None).unwrap_err(), TTZipStatus::ErrInvalidPassword);
    assert_eq!(inspect_entries_vec(&out_mhe_on, Some("wrong")).unwrap_err(), TTZipStatus::ErrInvalidPassword);
    assert!(matches!(
        load_archive_vfs_tree(out_mhe_on.to_string_lossy().to_string(), None),
        Err(TTZipError::InvalidPassword)
    ));

    let ok_entries = inspect_entries_vec(&out_mhe_on, Some(&password)).unwrap();
    assert!(ok_entries.iter().any(|e| e.path.contains("机密_αβγ_🔒.txt")));
    assert!(ok_entries.iter().any(|e| e.path.contains("empty_folder") && e.is_directory));

    let ext_dir_on = dir.path().join("ext_on");
    let sz_on = SevenZArchive::open_slice_with_password(&raw_bytes_on, Some(&password)).unwrap();
    sz_on.extract_all(&ext_dir_on, &make_extract_options(Some(&pwd_cstr))).unwrap();
    assert!(ext_dir_on.join("payload_root/empty_folder").is_dir());
    assert_eq!(
        fs::read(ext_dir_on.join("payload_root/机密_αβγ_🔒.txt")).unwrap(),
        b"Top secret 7z header encryption payload 2026\n"
    );

    assert!(Path::new("/opt/homebrew/bin/7z").exists(), "/opt/homebrew/bin/7z required for cross-verification");
    let list_no_pwd = Command::new("/opt/homebrew/bin/7z")
        .args(["l", "-p", out_mhe_on.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!list_no_pwd.status.success(), "7z l with empty password must fail on mhe=on archive");
    let test_with_pwd = Command::new("/opt/homebrew/bin/7z")
        .args(["t", &format!("-p{}", password), out_mhe_on.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(test_with_pwd.status.success());

    // Case 2: encrypt_file_names = Some(false) (-mhe=off) with password
    let out_mhe_off = dir.path().join("c3_mhe_off.7z");
    create_archive_advanced_with_options(
        make_7z_cfg(&src_dir, &out_mhe_off, Some(password.clone()), None),
        UniFFICreateArchiveExtraOptions { encrypt_file_names: Some(false) },
        None,
        None,
    )
    .expect("7z create with mhe=off should succeed");

    let raw_bytes_off = fs::read(&out_mhe_off).unwrap();
    let next_hdr_off_plain = u64::from_le_bytes(raw_bytes_off[12..20].try_into().unwrap()) as usize;
    assert_eq!(raw_bytes_off[32 + next_hdr_off_plain], 0x01, "Expected plaintext kHeader (0x01)");
    let entries_without_pwd = inspect_entries_vec(&out_mhe_off, None).unwrap();
    assert!(entries_without_pwd.iter().any(|e| e.path.contains("机密_αβγ_🔒.txt") && e.is_encrypted));

    let sz_off = SevenZArchive::open_slice(&raw_bytes_off).unwrap();
    let ext_dir_off = dir.path().join("ext_off");
    assert!(sz_off.extract_all(&ext_dir_off, &make_extract_options(None)).is_err());
    sz_off.extract_all(&ext_dir_off, &make_extract_options(Some(&pwd_cstr))).unwrap();
}

#[test]
fn test_r4_encryption_without_password_fails_and_exclude_patterns_remain_pure() {
    let dir = tempdir().expect("create tempdir");
    let src_dir = dir.path().join("enc_guard_src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("normal.txt"), b"hello world\n").unwrap();
    fs::write(src_dir.join("__ttzip_opt:encrypt_file_names=1"), b"must be excluded when pattern matches\n").unwrap();

    // 1. encrypt_file_names = Some(true) with password = None MUST fail with InvalidPassword and preserve destination
    let pre_existing = dir.path().join("must_not_overwrite.7z");
    let sentinel = b"PRE_EXISTING_DESTINATION_SENTINEL";
    fs::write(&pre_existing, sentinel).unwrap();

    let err_none = create_archive_advanced_with_options(
        make_7z_cfg(&src_dir, &pre_existing, None, None),
        UniFFICreateArchiveExtraOptions { encrypt_file_names: Some(true) },
        None,
        None,
    )
    .expect_err("encrypt_file_names=true with password=None must fail");
    assert!(matches!(err_none, TTZipError::InvalidPassword), "Expected InvalidPassword, got {:?}", err_none);
    assert_eq!(fs::read(&pre_existing).unwrap(), sentinel, "Pre-existing destination must be untouched");

    // 2. encrypt_file_names = Some(true) or Some(false) with empty password Some("") MUST fail and leave zero split volumes
    let split_target = dir.path().join("no_residual_split.7z");
    for enc_names in [Some(true), Some(false), None] {
        let err_empty = create_archive_advanced_with_options(
            make_7z_cfg(&src_dir, &split_target, Some(String::new()), Some(4096)),
            UniFFICreateArchiveExtraOptions { encrypt_file_names: enc_names },
            None,
            None,
        )
        .expect_err("Empty password must be rejected");
        assert!(matches!(err_empty, TTZipError::InvalidPassword));
        assert!(!split_target.exists());
        assert!(!dir.path().join("no_residual_split.7z.001").exists(), "Must not leave .001 volume");
    }

    // 3. Literal exclude pattern "__ttzip_opt:encrypt_file_names=1" must NOT be swallowed as an option!
    let filtered_7z = dir.path().join("literal_exclude.7z");
    let mut cfg_literal = make_7z_cfg(&src_dir, &filtered_7z, None, None);
    cfg_literal.exclude_patterns = vec!["*__ttzip_opt:encrypt_file_names=1*".to_string()];
    create_archive_advanced(cfg_literal, None, None).expect("Unencrypted 7z with literal exclude pattern must succeed");
    let entries = inspect_entries_vec(&filtered_7z, None).unwrap();
    assert!(entries.iter().any(|e| e.path.ends_with("normal.txt")));
    assert!(
        !entries.iter().any(|e| e.path.contains("__ttzip_opt:encrypt_file_names=1")),
        "Literal exclude pattern was swallowed instead of filtering matching file"
    );
}


#[test]
fn test_r4_7z_metadata_preservation_symlinks_permissions_and_mtime() {
    let dir = tempdir().expect("create tempdir");
    let src_dir = dir.path().join("meta_fixture");
    fs::create_dir_all(&src_dir).unwrap();

    let regular_file = src_dir.join("regular.txt");
    fs::write(&regular_file, b"regular target content\n").unwrap();
    let fixed_mtime: i64 = 1_704_067_200; // 2024-01-01T00:00:00Z
    set_file_mtime(&regular_file, fixed_mtime);

    let exec_file = src_dir.join("run_script.sh");
    fs::write(&exec_file, b"#!/bin/sh\necho ok\n").unwrap();
    fs::set_permissions(&exec_file, fs::Permissions::from_mode(0o755)).unwrap();
    set_file_mtime(&exec_file, fixed_mtime);

    let readonly_file = src_dir.join("readonly.txt");
    fs::write(&readonly_file, b"immutable config\n").unwrap();
    fs::set_permissions(&readonly_file, fs::Permissions::from_mode(0o444)).unwrap();
    set_file_mtime(&readonly_file, fixed_mtime);

    let symlink_file = src_dir.join("symlink_to_regular.txt");
    symlink("regular.txt", &symlink_file).unwrap();
    set_file_mtime(&symlink_file, fixed_mtime);

    let out_7z = dir.path().join("meta_preserved.7z");
    let cfg = UniFFICreateArchiveConfig {
        source_paths: vec![src_dir.to_string_lossy().to_string()],
        output_path: out_7z.to_string_lossy().to_string(),
        format: ArchiveFormat::SevenZip,
        level: 6,
        password: None,
        split_volume_size_bytes: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 2,
    };
    create_archive_advanced(cfg, None, None).expect("7z metadata archive creation should succeed");

    // Verify metadata via inspect_entries_vec
    let entries = inspect_entries_vec(&out_7z, None).unwrap();
    let sym_entry = entries
        .iter()
        .find(|e| e.path.ends_with("symlink_to_regular.txt"))
        .expect("symlink entry must exist");
    assert_eq!(
        sym_entry.mode & 0o170000,
        0o120000,
        "Symlink entry must carry S_IFLNK (0o120000) in mode, got 0o{:o}",
        sym_entry.mode
    );
    assert_eq!(
        sym_entry.uncompressed_size,
        "regular.txt".len() as u64,
        "Symlink stream size must equal target path length, not target file length"
    );

    let exec_entry = entries
        .iter()
        .find(|e| e.path.ends_with("run_script.sh"))
        .expect("exec entry must exist");
    assert_eq!(exec_entry.mode & 0o777, 0o755);
    assert_eq!(exec_entry.mtime_epoch_secs, fixed_mtime);

    let ro_entry = entries
        .iter()
        .find(|e| e.path.ends_with("readonly.txt"))
        .expect("readonly entry must exist");
    assert_eq!(ro_entry.mode & 0o777, 0o444);
    assert_eq!(ro_entry.mtime_epoch_secs, fixed_mtime);

    // Extract and verify filesystem state
    let ext_dir = dir.path().join("extracted_meta");
    let raw_7z = fs::read(&out_7z).unwrap();
    let sz = SevenZArchive::open_slice(&raw_7z).unwrap();
    sz.extract_all(&ext_dir, &make_extract_options(None)).unwrap();

    let ext_sym = ext_dir.join("meta_fixture/symlink_to_regular.txt");
    let sym_meta = fs::symlink_metadata(&ext_sym).expect("extracted symlink must exist");
    assert!(
        sym_meta.file_type().is_symlink(),
        "Extracted symlink must remain a symbolic link, not a dereferenced regular file"
    );
    assert_eq!(
        fs::read_link(&ext_sym).unwrap().to_str().unwrap(),
        "regular.txt"
    );

    let ext_exec_meta = fs::symlink_metadata(ext_dir.join("meta_fixture/run_script.sh")).unwrap();
    assert_eq!(ext_exec_meta.permissions().mode() & 0o777, 0o755);

    let ext_ro_meta = fs::symlink_metadata(ext_dir.join("meta_fixture/readonly.txt")).unwrap();
    assert_eq!(ext_ro_meta.permissions().mode() & 0o777, 0o444);
    let ext_ro_mtime = ext_ro_meta
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert_eq!(ext_ro_mtime, fixed_mtime);

    assert!(Path::new("/opt/homebrew/bin/7z").exists(), "/opt/homebrew/bin/7z required");
    let test_out = Command::new("/opt/homebrew/bin/7z")
        .args(["t", out_7z.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(test_out.status.success());
}

#[test]
fn test_r4_single_stream_vs_tar_container_contracts() {
    let dir = tempdir().expect("create tempdir");
    let file_a = dir.path().join("alpha.txt");
    let file_b = dir.path().join("beta.txt");
    let payload_a = b"Pure single-stream payload without any ustar TAR header!\n";
    fs::write(&file_a, payload_a).unwrap();
    fs::write(&file_b, b"Second file payload\n").unwrap();

    let stream_cases: &[(ArchiveFormat, &str, &[u8])] = &[
        (ArchiveFormat::Gzip, "alpha.txt.gz", &[0x1F, 0x8B]),
        (ArchiveFormat::Bzip2, "alpha.txt.bz2", b"BZh"),
        (ArchiveFormat::Xz, "alpha.txt.xz", &[0xFD, b'7', b'z', b'X', b'Z', 0x00]),
        (ArchiveFormat::Zstd, "alpha.txt.zst", &[0x28, 0xB5, 0x2F, 0xFD]),
        (ArchiveFormat::Lz4, "alpha.txt.lz4", &[0x04, 0x22, 0x4D, 0x18]),
        (ArchiveFormat::Brotli, "alpha.txt.br", &[]),
        (ArchiveFormat::Lzip, "alpha.txt.lz", b"LZIP"),
        (ArchiveFormat::Lrzip, "alpha.txt.lrz", b"LRZI"),
        (ArchiveFormat::Snappy, "alpha.txt.sz", &[0xFF, 0x06, 0x00, 0x00, b's', b'N', b'a', b'P', b'p', b'Y']),
    ];

    for (fmt, out_name, expected_magic) in stream_cases {
        let out_path = dir.path().join(out_name);
        let cfg = UniFFICreateArchiveConfig {
            source_paths: vec![file_a.to_string_lossy().to_string()],
            output_path: out_path.to_string_lossy().to_string(),
            format: *fmt,
            level: 6,
            password: None,
            split_volume_size_bytes: None,
            exclude_patterns: vec![],
            include_patterns: vec![],
            exclude_vcs: false,
            no_mac_metadata: true,
            thread_budget: 2,
        };
        create_archive_advanced(cfg, None, None)
            .unwrap_or_else(|e| panic!("Failed to create single stream {:?}: {:?}", fmt, e));

        let raw = fs::read(&out_path).unwrap();
        if !expected_magic.is_empty() {
            assert!(raw.starts_with(expected_magic), "Format {:?} magic mismatch", fmt);
        }
    }

    let gz_out = Command::new("/usr/bin/gzip")
        .args(["-dc", dir.path().join("alpha.txt.gz").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(gz_out.status.success());
    assert_eq!(gz_out.stdout.as_slice(), payload_a);

    let pre_existing_gz = dir.path().join("multi_rejected.gz");
    let sentinel = b"DO_NOT_CORRUPT_EXISTING_GZIP_TARGET";
    fs::write(&pre_existing_gz, sentinel).unwrap();

    let bad_multi_cfg = UniFFICreateArchiveConfig {
        source_paths: vec![file_a.to_string_lossy().to_string(), file_b.to_string_lossy().to_string()],
        output_path: pre_existing_gz.to_string_lossy().to_string(),
        format: ArchiveFormat::Gzip,
        level: 6,
        password: None,
        split_volume_size_bytes: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 2,
    };
    assert!(create_archive_advanced(bad_multi_cfg, None, None).is_err());
    assert_eq!(fs::read(&pre_existing_gz).unwrap().as_slice(), sentinel);
}

#[test]
fn test_r4_authentic_containers_and_readonly_format_rejection() {
    let dir = tempdir().expect("create tempdir");
    let long_name_file = dir.path().join("single_payload.bin");
    fs::write(&long_name_file, b"Authentic container verification payload 2026\n").unwrap();

    let ar_path = dir.path().join("probe.ar");
    create_archive(
        std::slice::from_ref(&long_name_file),
        &ar_path,
        &default_create_options(TTZipArchiveFormat::Ar),
        0,
    )
    .expect("BSD AR creation with 18-char filename must succeed");
    assert!(fs::read(&ar_path).unwrap().starts_with(b"!<arch>\n"));
    assert!(Path::new("/usr/bin/ar").exists(), "/usr/bin/ar required");
    let ar_list = Command::new("/usr/bin/ar").args(["-t", ar_path.to_str().unwrap()]).output().unwrap();
    assert!(ar_list.status.success());
    assert!(String::from_utf8_lossy(&ar_list.stdout).contains("single_payload.bin"));

    let aar_path = dir.path().join("probe.aar");
    create_archive(
        std::slice::from_ref(&long_name_file),
        &aar_path,
        &default_create_options(TTZipArchiveFormat::Aar),
        0,
    )
    .expect("Apple Archive (.aar) creation must succeed on macOS");
    assert_eq!(&fs::read(&aar_path).unwrap()[..4], b"pbze");
    assert!(Path::new("/usr/bin/aa").exists(), "/usr/bin/aa required");
    let aa_list = Command::new("/usr/bin/aa")
        .args(["list", "-i", aar_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(aa_list.status.success());
    assert!(String::from_utf8_lossy(&aa_list.stdout).contains("single_payload.bin"));

    let cpio_path = dir.path().join("probe.cpio");
    create_archive(
        std::slice::from_ref(&long_name_file),
        &cpio_path,
        &default_create_options(TTZipArchiveFormat::Cpio),
        0,
    )
    .expect("CPIO creation must succeed");
    assert!(fs::read(&cpio_path).unwrap().starts_with(b"070701"));

    let non_creatable = [
        (TTZipArchiveFormat::Dmg, "reject.dmg"),
        (TTZipArchiveFormat::Cab, "reject.cab"),
        (TTZipArchiveFormat::Rpm, "reject.rpm"),
        (TTZipArchiveFormat::Deb, "reject.deb"),
        (TTZipArchiveFormat::Wim, "reject.wim"),
        (TTZipArchiveFormat::Rar, "reject.rar"),
        (TTZipArchiveFormat::Squashfs, "reject.squashfs"),
        (TTZipArchiveFormat::Lzh, "reject.lzh"),
        (TTZipArchiveFormat::Lzfse, "reject.lzfse"),
    ];
    for (fmt, filename) in non_creatable {
        let dest = dir.path().join(filename);
        let sentinel = format!("SENTINEL_FOR_{:?}", fmt);
        fs::write(&dest, sentinel.as_bytes()).unwrap();
        let res = create_archive_detailed(
            std::slice::from_ref(&long_name_file),
            &dest,
            &default_create_options(fmt),
            0,
        );
        assert!(res.is_err(), "Non-creatable format {:?} must be rejected", fmt);
        assert_eq!(fs::read(&dest).unwrap(), sentinel.as_bytes());
    }
}

struct CancelAfterBytesHandler {
    threshold: u64,
    token: std::sync::Arc<CancellationToken>,
}

impl ProgressHandler for CancelAfterBytesHandler {
    fn on_progress(&self, processed: u64, _total: u64, _name: Option<String>) -> bool {
        if processed >= self.threshold {
            self.token.cancel();
            return false;
        }
        true
    }
}

#[test]
fn test_r4_bounded_xar_streaming_extraction_cancellation_and_failure_cleanup() {
    use sha2::{Digest, Sha256};

    let dir = tempdir().expect("create tempdir");
    let src_root = dir.path().join("xar_src");
    fs::create_dir_all(src_root.join("empty_xar_dir")).unwrap();
    fs::create_dir_all(src_root.join("nested")).unwrap();

    // Generate 2.5 MiB incompressible payload via deterministic xorshift64 + second file
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut big_payload = vec![0u8; 2_621_440];
    for chunk in big_payload.chunks_exact_mut(8) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        chunk.copy_from_slice(&state.to_le_bytes());
    }
    let expected_big_sha256 = Sha256::digest(&big_payload);
    let big_file = src_root.join("nested/incompressible.bin");
    fs::write(&big_file, &big_payload).unwrap();
    let second_file = src_root.join("nested/second_note.txt");
    fs::write(&second_file, b"XAR multi-file cumulative verification\n").unwrap();
    symlink("nested/incompressible.bin", src_root.join("link_to_nested.bin")).unwrap();

    // 1. Mid-stream cancellation must abort cleanly, preserve pre-existing destination, and remove .xar_heap_*.tmp
    let cancel_dest = dir.path().join("cancelled.xar");
    let cancel_sentinel = b"KEEP_EXISTING_XAR_ON_CANCEL";
    fs::write(&cancel_dest, cancel_sentinel).unwrap();
    let token = CancellationToken::new();
    let handler = Box::new(CancelAfterBytesHandler {
        threshold: 128 * 1024,
        token: token.clone(),
    });
    let cancel_cfg = UniFFICreateArchiveConfig {
        source_paths: vec![src_root.to_string_lossy().to_string()],
        output_path: cancel_dest.to_string_lossy().to_string(),
        format: ArchiveFormat::Xar,
        level: 6,
        password: None,
        split_volume_size_bytes: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 2,
    };
    let cancel_res = create_archive_advanced(cancel_cfg, Some(handler), Some(token));
    assert!(matches!(cancel_res, Err(TTZipError::Cancelled)));
    assert_eq!(fs::read(&cancel_dest).unwrap(), cancel_sentinel);
    let leftover_tmp = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().contains("xar_heap"));
    assert!(!leftover_tmp, "Temporary .xar_heap file must be cleaned up on cancellation");

    // 2. Write / missing-source failure must preserve pre-existing destination
    let fail_cfg = UniFFICreateArchiveConfig {
        source_paths: vec![dir.path().join("nonexistent_input.bin").to_string_lossy().to_string()],
        output_path: cancel_dest.to_string_lossy().to_string(),
        format: ArchiveFormat::Xar,
        level: 6,
        password: None,
        split_volume_size_bytes: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 2,
    };
    assert!(create_archive_advanced(fail_cfg, None, None).is_err());
    assert_eq!(fs::read(&cancel_dest).unwrap(), cancel_sentinel);

    // 3. Full bounded streaming XAR creation & external `/usr/bin/xar -xf` extraction verification
    let xar_out = dir.path().join("verified_bounded.xar");
    let ok_cfg = UniFFICreateArchiveConfig {
        source_paths: vec![src_root.to_string_lossy().to_string()],
        output_path: xar_out.to_string_lossy().to_string(),
        format: ArchiveFormat::Xar,
        level: 6,
        password: None,
        split_volume_size_bytes: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 2,
    };
    let report = create_archive_advanced(ok_cfg, None, None).expect("Bounded XAR creation must succeed");
    assert!(report.uncompressed_bytes >= 2_621_440);
    assert!(Path::new("/usr/bin/xar").exists(), "/usr/bin/xar required for extraction verification");

    let ext_dir = dir.path().join("xar_extracted");
    fs::create_dir_all(&ext_dir).unwrap();
    let xar_extract = Command::new("/usr/bin/xar")
        .current_dir(&ext_dir)
        .args(["-xf", xar_out.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        xar_extract.status.success(),
        "/usr/bin/xar -xf failed: {}",
        String::from_utf8_lossy(&xar_extract.stderr)
    );
    assert!(ext_dir.join("xar_src/empty_xar_dir").is_dir(), "Empty directory must be extracted by xar");
    let ext_sym = ext_dir.join("xar_src/link_to_nested.bin");
    assert!(
        fs::symlink_metadata(&ext_sym).unwrap().file_type().is_symlink(),
        "Symlink must be extracted as symbolic link by /usr/bin/xar"
    );
    assert_eq!(fs::read_link(&ext_sym).unwrap().to_str().unwrap(), "nested/incompressible.bin");
    assert_eq!(
        fs::read(ext_dir.join("xar_src/nested/second_note.txt")).unwrap(),
        b"XAR multi-file cumulative verification\n"
    );
    let extracted_big = fs::read(ext_dir.join("xar_src/nested/incompressible.bin")).unwrap();
    assert_eq!(Sha256::digest(&extracted_big), expected_big_sha256);
}

#[test]
fn test_r4_7z_split_volumes_and_c_ffi_creation_contracts() {
    use ttzip_engine::ffi::archive_ffi::ttzip_rust_create_archive;
    extern "C" {
        fn ttzip_rust_create_archive_v2(
            source_paths: *const *const libc::c_char,
            source_count: usize,
            destination_path: *const libc::c_char,
            options: *const TTZipCreateOptions,
            encrypt_file_names_mode: i32,
        ) -> TTZipStatus;
    }

    let dir = tempdir().expect("create tempdir");
    let src_dir = dir.path().join("split_src");
    fs::create_dir_all(src_dir.join("empty_sub")).unwrap();
    let payload: Vec<u8> = (0..16384).map(|i: usize| ((i * 73 + 19) % 251) as u8).collect();
    fs::write(src_dir.join("blob_αβγ.bin"), &payload).unwrap();

    let password = "Split$2026_密码".to_string();
    let split_mhe_off = dir.path().join("split_off.7z");
    let mut cfg_split_off = make_7z_cfg(&src_dir, &split_mhe_off, Some(password.clone()), Some(6144));
    cfg_split_off.level = 0;
    create_archive_advanced_with_options(
        cfg_split_off,
        UniFFICreateArchiveExtraOptions { encrypt_file_names: Some(false) },
        None,
        None,
    )
    .expect("Split 7z mhe=off should succeed");
    let vol1_off = dir.path().join("split_off.7z.001");
    let vol2_off = dir.path().join("split_off.7z.002");
    assert!(vol1_off.exists() && vol2_off.exists());

    let entries_off = inspect_entries_vec(&vol1_off, None).expect("Split mhe=off inspect without pwd");
    assert!(entries_off.iter().any(|e| e.path.contains("blob_αβγ.bin") && e.is_encrypted));

    let split_mhe_on = dir.path().join("split_on.7z");
    let mut cfg_split_on = make_7z_cfg(&src_dir, &split_mhe_on, Some(password.clone()), Some(6144));
    cfg_split_on.level = 0;
    create_archive_advanced_with_options(
        cfg_split_on,
        UniFFICreateArchiveExtraOptions { encrypt_file_names: Some(true) },
        None,
        None,
    )
    .expect("Split 7z mhe=on should succeed");
    let vol1_on = dir.path().join("split_on.7z.001");
    assert!(inspect_entries_vec(&vol1_on, None).is_err());
    let entries_on = inspect_entries_vec(&vol1_on, Some(&password)).expect("Split mhe=on inspect with pwd");
    assert!(entries_on.iter().any(|e| e.path.contains("blob_αβγ.bin")));

    assert!(Path::new("/opt/homebrew/bin/7z").exists(), "/opt/homebrew/bin/7z required");
    for v1 in [&vol1_off, &vol1_on] {
        let out = Command::new("/opt/homebrew/bin/7z")
            .args(["t", &format!("-p{}", password), v1.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(out.status.success(), "7z t failed on {:?}", v1);
    }

    let src_file = src_dir.join("blob_αβγ.bin");
    let src_c = std::ffi::CString::new(src_file.to_str().unwrap()).unwrap();
    let ptrs = [src_c.as_ptr()];

    let dmg_dest = dir.path().join("ffi_reject.dmg");
    fs::write(&dmg_dest, b"KEEP_DMG").unwrap();
    let dmg_c = std::ffi::CString::new(dmg_dest.to_str().unwrap()).unwrap();
    let dmg_opts = default_create_options(TTZipArchiveFormat::Dmg);
    let dmg_status = unsafe { ttzip_rust_create_archive(ptrs.as_ptr(), 1, dmg_c.as_ptr(), &dmg_opts) };
    assert_eq!(dmg_status, TTZipStatus::ErrUnsupportedFeature);
    assert_eq!(fs::read(&dmg_dest).unwrap(), b"KEEP_DMG");

    // C-FFI v2 with encrypt_file_names_mode=1 and null password must fail with ErrInvalidPassword
    let ffi_7z_dest = dir.path().join("ffi_no_pwd.7z");
    fs::write(&ffi_7z_dest, b"KEEP_7Z").unwrap();
    let ffi_7z_c = std::ffi::CString::new(ffi_7z_dest.to_str().unwrap()).unwrap();
    let sz_opts = default_create_options(TTZipArchiveFormat::SevenZip);
    let sz_status = unsafe { ttzip_rust_create_archive_v2(ptrs.as_ptr(), 1, ffi_7z_c.as_ptr(), &sz_opts, 1) };
    assert_eq!(sz_status, TTZipStatus::ErrInvalidPassword);
    assert_eq!(fs::read(&ffi_7z_dest).unwrap(), b"KEEP_7Z");

    let aar_dest = dir.path().join("ffi_valid.aar");
    let aar_c = std::ffi::CString::new(aar_dest.to_str().unwrap()).unwrap();
    let aar_opts = default_create_options(TTZipArchiveFormat::Aar);
    let aar_status = unsafe { ttzip_rust_create_archive(ptrs.as_ptr(), 1, aar_c.as_ptr(), &aar_opts) };
    assert_eq!(aar_status, TTZipStatus::Ok);
    assert_eq!(&fs::read(&aar_dest).unwrap()[..4], b"pbze");
}


