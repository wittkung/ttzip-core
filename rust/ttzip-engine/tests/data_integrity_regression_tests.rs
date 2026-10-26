// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Task D: Compression & Decompression Data Integrity Baseline Regression Suite.
//!
//! Covers:
//! 1. Full writable format matrix roundtrip (`zip`, `7z` LZMA2/Store, `tar`, `tar.gz`,
//!    `tar.bz2`, `tar.xz`, `tar.zst`, `tar.lz4`, `tar.br`, `snappy`).
//! 2. Complex topologies: Chinese (UTF-8), Emoji, long filenames, 0-byte empty files,
//!    deep nested directories, and executable permissions (`0o755`).
//! 3. Encryption (`AES-256` for ZIP/7z) roundtrip and staged wrong-password rejection.
//! 4. Multi-volume split archives (`.001..`, `.z01..zip`) creation & extraction roundtrip.
//! 5. Collision policies (`Replace`, `KeepBoth`, `Skip`, `Stop`), extracting into
//!    existing directories, overwriting existing archives on cancellation, and `ENOSPC`.

use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use tempfile::tempdir;

use ttzip_engine::archive::unified::{verify_archive_stream, UnifiedArchiveOrchestrator};
use ttzip_engine::fs::collision::table::{ConflictTableBuilder, NewConflict};
use ttzip_engine::fs::collision::{
    CollisionAction as CoreCollisionAction, DFSCommitWalker, FsOps, ItemKind, ItemMeta, LibcFsOps,
    OutcomeStatus as CoreOutcomeStatus, ResolutionSet as CoreResolutionSet, StagingArea,
    StagingRoot, VolCaps,
};
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions, TTZipStatus, TTZIP_ABI_VERSION_3,
};
use ttzip_engine::uniffi_api::archive::{
    create_archive_advanced, create_archive_stream, UniFFICreateArchiveConfig,
};
use ttzip_engine::uniffi_api::extraction_session::{
    CollisionAction, CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession,
    OutcomeStatus, ResolutionSet,
};
use ttzip_engine::uniffi_api::types::{
    ArchiveFormat as UniFFIArchiveFormat, CancellationToken, ProgressHandler, TTZipError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSnapshot {
    sha256: [u8; 32],
    size: u64,
    is_exec: bool,
    symlink_target: Option<PathBuf>,
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn pseudo_random_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push((state & 0xFF) as u8);
    }
    out
}

fn create_archive_simple(
    sources: &[PathBuf],
    dest: &Path,
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
) -> Result<(), TTZipStatus> {
    let c_pw = password.map(|p| CString::new(p).unwrap());
    let opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format,
        level,
        encryption,
        password: c_pw.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
        thread_budget: 2,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    UnifiedArchiveOrchestrator::create_archive(sources, dest, &opts, 0)
}

fn extract_archive_simple(
    archive: &Path,
    dest_dir: &Path,
    password: Option<&str>,
) -> Result<(), TTZipStatus> {
    let c_pw = password.map(|p| CString::new(p).unwrap());
    let opts = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        destination_path: std::ptr::null(),
        password: c_pw.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
        thread_budget: 2,
        collision_policy: 0,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };
    let staging = StagingRoot(dest_dir.to_path_buf());
    UnifiedArchiveOrchestrator::extract_archive(archive, &staging, &opts)
}

fn create_synthetic_corpus(root: &Path) -> PathBuf {
    let src = root.join("corpus");
    let zh_dir = src.join("中文目录/子目录_测试");
    let emoji_dir = src.join("🚀_发布包_🎉");
    let long_dir = src.join("long_names");
    let empty_dir = src.join("empty_files");
    let deep_dir = src.join("L1/L2/L3/L4/L5/L6/L7/L8");
    let bin_dir = src.join("bin");

    for d in [&zh_dir, &emoji_dir, &long_dir, &empty_dir, &deep_dir, &bin_dir] {
        fs::create_dir_all(d).unwrap();
    }

    fs::write(
        zh_dir.join("唐詩三百首_测试.txt"),
        "床前明月光，疑是地上霜。\n舉頭望明月，低頭思故鄉。\n".repeat(64),
    )
    .unwrap();
    fs::write(emoji_dir.join("✨_数据_🧪.bin"), pseudo_random_bytes(0xA5A5, 24_576)).unwrap();

    let long_name = format!("长文件名_{}_end.dat", "a_b_c_1234567890_".repeat(9));
    fs::write(long_dir.join(long_name), b"long-filename-integrity-payload-9876543210").unwrap();
    fs::write(empty_dir.join("zero_length.empty"), b"").unwrap();
    fs::write(deep_dir.join("deep_leaf.txt"), b"deep-hierarchy-leaf-content").unwrap();

    let script_path = bin_dir.join("run_verify.sh");
    fs::write(&script_path, b"#!/bin/sh\necho ok\n").unwrap();
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
    src
}

fn snapshot_tree(root: &Path) -> BTreeMap<String, FileSnapshot> {
    let mut map = BTreeMap::new();
    if !root.exists() {
        return map;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                map.insert(
                    rel,
                    FileSnapshot {
                        sha256: [0u8; 32],
                        size: 0,
                        is_exec: false,
                        symlink_target: Some(fs::read_link(&path).unwrap()),
                    },
                );
            } else if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                let bytes = fs::read(&path).unwrap();
                map.insert(
                    rel,
                    FileSnapshot {
                        sha256: sha256_bytes(&bytes),
                        size: bytes.len() as u64,
                        is_exec: (meta.mode() & 0o111) != 0,
                        symlink_target: None,
                    },
                );
            }
        }
    }
    map
}

#[test]
fn test_matrix_roundtrip_all_writable_multi_entry_formats() {
    let tmp = tempdir().unwrap();
    let corpus = create_synthetic_corpus(tmp.path());
    let expected = snapshot_tree(&corpus);
    assert_eq!(expected.len(), 6);

    let formats = [
        (TTZipArchiveFormat::Zip, TTZipCompressionLevel::Normal, "matrix.zip"),
        (TTZipArchiveFormat::SevenZip, TTZipCompressionLevel::Normal, "matrix_lzma2.7z"),
        (TTZipArchiveFormat::SevenZip, TTZipCompressionLevel::Store, "matrix_store.7z"),
        (TTZipArchiveFormat::Tar, TTZipCompressionLevel::Normal, "matrix.tar"),
        (TTZipArchiveFormat::TarGz, TTZipCompressionLevel::Normal, "matrix.tar.gz"),
        (TTZipArchiveFormat::TarBz2, TTZipCompressionLevel::Normal, "matrix.tar.bz2"),
        (TTZipArchiveFormat::TarXz, TTZipCompressionLevel::Normal, "matrix.tar.xz"),
        (TTZipArchiveFormat::TarZstd, TTZipCompressionLevel::Normal, "matrix.tar.zst"),
        (TTZipArchiveFormat::TarLz4, TTZipCompressionLevel::Normal, "matrix.tar.lz4"),
        (TTZipArchiveFormat::TarBrotli, TTZipCompressionLevel::Normal, "matrix.tar.br"),
    ];

    for (fmt, level, filename) in formats {
        let archive_path = tmp.path().join(filename);
        create_archive_simple(
            std::slice::from_ref(&corpus),
            &archive_path,
            fmt,
            level,
            TTZipEncryptionMethod::None,
            None,
        )
        .unwrap_or_else(|e| panic!("create failed for {filename}: {e:?}"));

        let verify_res =
            verify_archive_stream(&archive_path, None, None, std::ptr::null_mut()).unwrap();
        assert_eq!(verify_res.corrupted_entries_count, 0, "verify failed for {filename}");

        let out_dir = tmp.path().join(format!("out_{filename}"));
        fs::create_dir_all(&out_dir).unwrap();
        extract_archive_simple(&archive_path, &out_dir, None)
            .unwrap_or_else(|e| panic!("extract failed for {filename}: {e:?}"));

        let actual = snapshot_tree(&out_dir.join("corpus"));
        assert_eq!(actual.keys().collect::<Vec<_>>(), expected.keys().collect::<Vec<_>>());
        for (rel_path, exp_snap) in &expected {
            let act_snap = &actual[rel_path];
            assert_eq!(act_snap.size, exp_snap.size, "size mismatch in {filename}: {rel_path}");
            assert_eq!(act_snap.sha256, exp_snap.sha256, "hash mismatch in {filename}: {rel_path}");
            if fmt != TTZipArchiveFormat::SevenZip {
                assert_eq!(act_snap.is_exec, exp_snap.is_exec, "mode mismatch in {filename}");
            }
        }
    }
}

#[test]
fn test_single_file_snappy_roundtrip_integrity() {
    let tmp = tempdir().unwrap();
    let src_file = tmp.path().join("payload_中文_🚀.bin");
    let data = pseudo_random_bytes(42, 65_536);
    fs::write(&src_file, &data).unwrap();

    let sz_path = tmp.path().join("payload.sz");
    create_archive_simple(
        &[src_file],
        &sz_path,
        TTZipArchiveFormat::Snappy,
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::None,
        None,
    )
    .unwrap();

    let out_dir = tmp.path().join("out_snappy");
    fs::create_dir_all(&out_dir).unwrap();
    extract_archive_simple(&sz_path, &out_dir, None).unwrap();

    let extracted = fs::read(out_dir.join("payload")).unwrap();
    assert_eq!(sha256_bytes(&extracted), sha256_bytes(&data));
}

#[test]
fn test_encryption_aes256_zip_and_7z_roundtrip_and_staged_wrong_password() {
    let tmp = tempdir().unwrap();
    let corpus = create_synthetic_corpus(tmp.path());
    let expected = snapshot_tree(&corpus);
    let secret = "Correct$Horse#Battery@Staple_密码_🔒";

    for (fmt, name) in [
        (TTZipArchiveFormat::Zip, "encrypted.zip"),
        (TTZipArchiveFormat::SevenZip, "encrypted.7z"),
    ] {
        let archive_path = tmp.path().join(name);
        create_archive_simple(
            std::slice::from_ref(&corpus),
            &archive_path,
            fmt,
            TTZipCompressionLevel::Normal,
            TTZipEncryptionMethod::Aes256,
            Some(secret),
        )
        .unwrap();

        let wrong_pw_dir = tmp.path().join(format!("wrong_pw_{name}"));
        fs::create_dir_all(&wrong_pw_dir).unwrap();
        let session_wrong = ExtractionSession::new(ExtractionRequest {
            archive_path: archive_path.to_str().unwrap().to_string(),
            destination_root: wrong_pw_dir.to_str().unwrap().to_string(),
            password: Some("WrongPassword_错误".to_string()),
            entry_filter: None,
            root_at_common_parent: false,
            policy: CollisionPolicy::Replace,
        })
        .unwrap();
        let stage_res = session_wrong.stage(None, None);
        assert!(matches!(stage_res, Err(TTZipError::InvalidPassword)));
        assert!(snapshot_tree(&wrong_pw_dir).is_empty(), "{name} leaked files on wrong password");

        let ok_dir = tmp.path().join(format!("ok_{name}"));
        fs::create_dir_all(&ok_dir).unwrap();
        extract_archive_simple(&archive_path, &ok_dir, Some(secret)).unwrap();
        let actual = snapshot_tree(&ok_dir.join("corpus"));
        for (k, exp) in &expected {
            assert_eq!(actual[k].sha256, exp.sha256, "{name} hash mismatch on {k}");
        }
    }
}

#[test]
fn test_split_volumes_creation_and_extraction_roundtrip() {
    let tmp = tempdir().unwrap();
    let corpus = create_synthetic_corpus(tmp.path());
    let expected = snapshot_tree(&corpus);

    for (fmt, ext) in [
        (UniFFIArchiveFormat::SevenZip, "split_test.7z"),
        (UniFFIArchiveFormat::Zip, "split_test.zip"),
        (UniFFIArchiveFormat::TarZstd, "split_test.tar.zst"),
    ] {
        let out_base = tmp.path().join(ext);
        let cfg = UniFFICreateArchiveConfig {
            source_paths: vec![corpus.to_str().unwrap().to_string()],
            output_path: out_base.to_str().unwrap().to_string(),
            format: fmt,
            level: 5,
            password: None,
            split_volume_size_bytes: Some(8_192),
            exclude_patterns: vec![],
            include_patterns: vec![],
            exclude_vcs: false,
            no_mac_metadata: false,
            thread_budget: 2,
        };
        create_archive_advanced(cfg, None, None).unwrap();

        let first_vol = if fmt == UniFFIArchiveFormat::Zip {
            tmp.path().join("split_test.z01")
        } else {
            PathBuf::from(format!("{}.001", out_base.display()))
        };
        assert!(first_vol.exists(), "first volume missing for {ext}");

        let extract_dir = tmp.path().join(format!("extract_{ext}"));
        fs::create_dir_all(&extract_dir).unwrap();
        let session = ExtractionSession::new(ExtractionRequest {
            archive_path: first_vol.to_str().unwrap().to_string(),
            destination_root: extract_dir.to_str().unwrap().to_string(),
            password: None,
            entry_filter: None,
            root_at_common_parent: false,
            policy: CollisionPolicy::Replace,
        })
        .unwrap();
        session.preflight().unwrap();
        session.stage(None, None).unwrap();
        let res = session
            .commit(ResolutionSet { default_action: Some(CollisionAction::Replace), overrides: vec![] })
            .unwrap();
        match res {
            CommitResult::Finished { outcome } => assert_eq!(outcome.status, OutcomeStatus::Completed),
            other => panic!("unexpected commit result: {other:?}"),
        }

        let actual = snapshot_tree(&extract_dir.join("corpus"));
        assert_eq!(actual.len(), expected.len(), "file count mismatch for split {ext}");
        for (k, exp) in &expected {
            assert_eq!(actual[k].sha256, exp.sha256, "hash mismatch in split {ext}: {k}");
        }
    }
}

#[test]
fn test_extract_to_existing_dir_collision_policies_and_failure_safety() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("shared.txt"), b"NEW_FROM_ARCHIVE").unwrap();
    fs::write(src_dir.join("fresh.txt"), b"FRESH_FILE").unwrap();

    let zip_path = tmp.path().join("collision.zip");
    create_archive_simple(
        &[src_dir.join("shared.txt"), src_dir.join("fresh.txt")],
        &zip_path,
        TTZipArchiveFormat::Zip,
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::None,
        None,
    )
    .unwrap();

    for (sub, policy, act, exp_shared) in [
        ("dest_skip", CollisionPolicy::Skip, CollisionAction::Skip, b"EXISTING_USER_DATA".as_slice()),
        ("dest_replace", CollisionPolicy::Replace, CollisionAction::Replace, b"NEW_FROM_ARCHIVE".as_slice()),
    ] {
        let dest = tmp.path().join(sub);
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("shared.txt"), b"EXISTING_USER_DATA").unwrap();
        fs::write(dest.join("unrelated.txt"), b"UNRELATED_BYSTANDER").unwrap();

        let session = ExtractionSession::new(ExtractionRequest {
            archive_path: zip_path.to_str().unwrap().to_string(),
            destination_root: dest.to_str().unwrap().to_string(),
            password: None,
            entry_filter: None,
            root_at_common_parent: false,
            policy,
        })
        .unwrap();
        session.preflight().unwrap();
        session.stage(None, None).unwrap();
        session
            .commit(ResolutionSet { default_action: Some(act), overrides: vec![] })
            .unwrap();
        assert_eq!(fs::read(dest.join("shared.txt")).unwrap(), exp_shared);
        assert_eq!(fs::read(dest.join("fresh.txt")).unwrap(), b"FRESH_FILE");
        assert_eq!(fs::read(dest.join("unrelated.txt")).unwrap(), b"UNRELATED_BYSTANDER");
    }
}

#[test]
fn test_failed_extraction_into_existing_dir_never_corrupts_existing_files() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("target.txt"), b"SECRET_PAYLOAD").unwrap();

    let enc_zip = tmp.path().join("enc.zip");
    create_archive_simple(
        &[src_dir.join("target.txt")],
        &enc_zip,
        TTZipArchiveFormat::Zip,
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::Aes256,
        Some("RightPassword"),
    )
    .unwrap();

    let dest = tmp.path().join("existing_dest");
    fs::create_dir_all(&dest).unwrap();
    let precious_bytes = b"PRECIOUS_EXISTING_CONTENT_DO_NOT_CORRUPT";
    fs::write(dest.join("target.txt"), precious_bytes).unwrap();

    let session = ExtractionSession::new(ExtractionRequest {
        archive_path: enc_zip.to_str().unwrap().to_string(),
        destination_root: dest.to_str().unwrap().to_string(),
        password: Some("WrongPassword".to_string()),
        entry_filter: None,
        root_at_common_parent: false,
        policy: CollisionPolicy::Replace,
    })
    .unwrap();
    session.preflight().unwrap();
    assert!(session.stage(None, None).is_err(), "stage with wrong password must fail");
    assert_eq!(fs::read(dest.join("target.txt")).unwrap(), precious_bytes);
}

struct CancelImmediatelyProgress;
impl ProgressHandler for CancelImmediatelyProgress {
    fn on_progress(&self, _processed: u64, _total: u64, _current_file: Option<String>) -> bool {
        false
    }
}

#[test]
fn test_create_archive_advanced_cancel_preserves_preexisting_target_archive() {
    let tmp = tempdir().unwrap();
    let corpus = create_synthetic_corpus(tmp.path());
    let target_archive = tmp.path().join("existing_output.zip");
    let original_archive_bytes = b"PRE_EXISTING_VALID_ARCHIVE_BYTES_DO_NOT_CLOBBER";
    fs::write(&target_archive, original_archive_bytes).unwrap();

    let token = CancellationToken::new();
    token.cancel();

    let cfg = UniFFICreateArchiveConfig {
        source_paths: vec![corpus.to_str().unwrap().to_string()],
        output_path: target_archive.to_str().unwrap().to_string(),
        format: UniFFIArchiveFormat::Zip,
        level: 5,
        password: None,
        split_volume_size_bytes: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: false,
        thread_budget: 1,
    };
    let res = create_archive_advanced(cfg, Some(Box::new(CancelImmediatelyProgress)), Some(token));
    assert!(matches!(res, Err(TTZipError::Cancelled)));
    assert_eq!(fs::read(&target_archive).unwrap(), original_archive_bytes);
}

struct EnospcMidCommitFsOps {
    inner: LibcFsOps,
    rename_calls: AtomicUsize,
    fail_from_call: usize,
}

impl FsOps for EnospcMidCommitFsOps {
    fn openat_dir_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<OwnedFd, std::io::Error> {
        self.inner.openat_dir_nofollow(dirfd, path)
    }
    fn fstatat_nofollow(&self, dirfd: RawFd, path: &Path) -> Result<Option<ItemMeta>, std::io::Error> {
        self.inner.fstatat_nofollow(dirfd, path)
    }
    fn getattrlistat_name(&self, dirfd: RawFd, path: &Path) -> Result<Option<String>, std::io::Error> {
        self.inner.getattrlistat_name(dirfd, path)
    }
    fn renameatx(
        &self,
        from_fd: RawFd,
        from: &Path,
        to_fd: RawFd,
        to: &Path,
        flags: u32,
    ) -> Result<(), std::io::Error> {
        if self.rename_calls.fetch_add(1, Ordering::SeqCst) >= self.fail_from_call {
            return Err(std::io::Error::from_raw_os_error(libc::ENOSPC));
        }
        self.inner.renameatx(from_fd, from, to_fd, to, flags)
    }
    fn unlinkat(&self, dirfd: RawFd, path: &Path, is_dir: bool) -> Result<(), std::io::Error> {
        self.inner.unlinkat(dirfd, path, is_dir)
    }
    fn remove_tree(&self, dirfd: RawFd, path: &Path) -> Result<(), std::io::Error> {
        self.inner.remove_tree(dirfd, path)
    }
    fn probe_vol_caps(&self, path: &Path) -> Result<VolCaps, std::io::Error> {
        self.inner.probe_vol_caps(path)
    }
}

#[test]
fn test_enospc_mid_commit_reports_completed_with_failures_and_preserves_existing() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path().join("dest_enospc");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("existing.txt"), b"EXISTING_MUST_SURVIVE_ENOSPC").unwrap();

    let staging = StagingArea::create(&dest).unwrap();
    let payload_dir = staging.payload_dir();
    fs::write(payload_dir.path().join("new_1.txt"), b"NEW_1").unwrap();
    fs::write(payload_dir.path().join("new_2.txt"), b"NEW_2").unwrap();

    let table = ConflictTableBuilder::new().build().unwrap();
    let ops = EnospcMidCommitFsOps {
        inner: LibcFsOps,
        rename_calls: AtomicUsize::new(0),
        fail_from_call: 1,
    };
    let resolutions = CoreResolutionSet::from_action(CoreCollisionAction::Replace);

    let dest_fd = fs::File::open(&dest).unwrap();
    let payload_fd = fs::File::open(payload_dir.path()).unwrap();
    let displaced_fd = fs::File::open(staging.displaced_dir()).unwrap();
    let caps = ops.probe_vol_caps(&dest).unwrap_or_default();

    let walker = DFSCommitWalker::new(
        &ops,
        dest_fd.as_raw_fd(),
        payload_fd.as_raw_fd(),
        displaced_fd.as_raw_fd(),
        caps,
    );
    let outcome = walker.commit(&table, &resolutions).unwrap();

    assert_eq!(outcome.status, CoreOutcomeStatus::CompletedWithFailures);
    assert_eq!(outcome.extracted, 1);
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].errno, libc::ENOSPC);
    assert_eq!(fs::read(dest.join("existing.txt")).unwrap(), b"EXISTING_MUST_SURVIVE_ENOSPC");
}

#[test]
fn test_create_archive_and_stream_preserve_preexisting_target_on_failure_and_cancel() {
    let tmp = tempdir().unwrap();
    let corpus = create_synthetic_corpus(tmp.path());
    let missing = tmp.path().join("does_not_exist.txt");

    for (ext, fmt, ufmt) in [
        ("zip", TTZipArchiveFormat::Zip, UniFFIArchiveFormat::Zip),
        ("7z", TTZipArchiveFormat::SevenZip, UniFFIArchiveFormat::SevenZip),
        ("tar.gz", TTZipArchiveFormat::TarGz, UniFFIArchiveFormat::TarGz),
    ] {
        let target = tmp.path().join(format!("existing_target.{ext}"));
        let precious = format!("PRECIOUS_ORIGINAL_ARCHIVE_{ext}").into_bytes();
        fs::write(&target, &precious).unwrap();

        let err = create_archive_simple(
            std::slice::from_ref(&missing),
            &target,
            fmt,
            TTZipCompressionLevel::Normal,
            TTZipEncryptionMethod::None,
            None,
        );
        assert!(err.is_err(), "create_archive with missing source must fail for {ext}");
        assert_eq!(fs::read(&target).unwrap(), precious);

        let cancel_res = create_archive_stream(
            vec![corpus.to_str().unwrap().to_string()],
            target.to_str().unwrap().to_string(),
            ufmt,
            5,
            None,
            Some(Box::new(CancelImmediatelyProgress)),
            None,
        );
        assert!(matches!(cancel_res, Err(TTZipError::Cancelled)));
        assert_eq!(fs::read(&target).unwrap(), precious);
    }
}

#[test]
fn test_commit_preflight_replace_failure_and_dynamic_recheck_conflicts() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path().join("dest_conflicts");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("preflight_conflict.txt"), b"EXISTING_PREFLIGHT").unwrap();
    fs::write(dest.join("dynamic_conflict.txt"), b"EXISTING_DYNAMIC").unwrap();

    let staging = StagingArea::create(&dest).unwrap();
    let payload_dir = staging.payload_dir();
    fs::write(payload_dir.path().join("preflight_conflict.txt"), b"NEW_PREFLIGHT").unwrap();
    fs::write(payload_dir.path().join("dynamic_conflict.txt"), b"NEW_DYNAMIC").unwrap();

    // Only register preflight_conflict.txt in table; dynamic_conflict.txt hits CommitRecheck.
    let mut builder = ConflictTableBuilder::new();
    builder
        .push(NewConflict {
            relative_path: "preflight_conflict.txt",
            existing_name: "preflight_conflict.txt",
            existing: ItemMeta { kind: ItemKind::File, size: 18, mtime_unix_ns: 90 },
            incoming: ItemMeta { kind: ItemKind::File, size: 13, mtime_unix_ns: 100 },
        })
        .unwrap();
    let table = builder.build().unwrap();

    // Call 0: dynamic_conflict.txt RENAME_EXCL -> EEXIST (enters CommitRecheck)
    // Call 1: dynamic_conflict.txt CommitRecheck RENAME_SWAP -> succeeds (replaced += 1)
    // Call 2: preflight_conflict.txt RENAME_SWAP -> fails with ENOSPC (recorded in failed)
    let ops = EnospcMidCommitFsOps {
        inner: LibcFsOps,
        rename_calls: AtomicUsize::new(0),
        fail_from_call: 2,
    };
    let resolutions = CoreResolutionSet::from_action(CoreCollisionAction::Replace);

    let dest_fd = fs::File::open(&dest).unwrap();
    let payload_fd = fs::File::open(payload_dir.path()).unwrap();
    let displaced_fd = fs::File::open(staging.displaced_dir()).unwrap();
    let caps = ops.probe_vol_caps(&dest).unwrap_or_default();

    let walker = DFSCommitWalker::new(
        &ops,
        dest_fd.as_raw_fd(),
        payload_fd.as_raw_fd(),
        displaced_fd.as_raw_fd(),
        caps,
    );
    let outcome = walker.commit(&table, &resolutions).unwrap();

    assert_eq!(outcome.status, CoreOutcomeStatus::CompletedWithFailures);
    assert_eq!(outcome.replaced, 1, "Only the single successful replace may increment replaced");
    assert_eq!(outcome.failed.len(), 1, "Failed replace must be recorded in outcome.failed");
    assert_eq!(outcome.failed[0].relative_path, "preflight_conflict.txt");
    assert_eq!(outcome.failed[0].errno, libc::ENOSPC);
    assert_eq!(fs::read(dest.join("dynamic_conflict.txt")).unwrap(), b"NEW_DYNAMIC");
    assert_eq!(fs::read(dest.join("preflight_conflict.txt")).unwrap(), b"EXISTING_PREFLIGHT");
}

#[test]
fn test_multivolume_second_volume_commit_failure_rolls_back_all_preexisting_volumes() {
    use ttzip_engine::archive::split::VolumeNamingScheme;
    use ttzip_engine::archive::unified::create::{
        with_staged_archive_destination_using, CreateCommitMode,
    };

    let tmp = tempdir().unwrap();
    let out_dir = tmp.path().join("split_rollback");
    fs::create_dir_all(&out_dir).unwrap();
    let target_archive = out_dir.join("bundle.zip");

    let old_vol1 = out_dir.join("bundle.zip.001");
    let old_vol2 = out_dir.join("bundle.zip.002");
    let old_vol3 = out_dir.join("bundle.zip.003");
    fs::write(&old_vol1, b"OLD_VOLUME_1_INTACT").unwrap();
    fs::write(&old_vol2, b"OLD_VOLUME_2_INTACT").unwrap();
    fs::write(&old_vol3, b"OLD_VOLUME_3_INTACT").unwrap();

    let staged_calls = AtomicUsize::new(0);
    let res = with_staged_archive_destination_using(
        &target_archive,
        CreateCommitMode::SplitVolume(VolumeNamingScheme::NumberedExtension),
        |staged_target| {
            let staged_dir = staged_target.parent().unwrap();
            fs::write(staged_dir.join("bundle.zip.001"), b"NEW_VOLUME_1_PARTIAL").unwrap();
            fs::write(staged_dir.join("bundle.zip.002"), b"NEW_VOLUME_2_FAILS").unwrap();
            Ok(())
        },
        |from, to| {
            // Only inject failure on the 2nd staged volume installation (from staging dir to out_dir)
            if from.parent() != to.parent() && staged_calls.fetch_add(1, Ordering::SeqCst) == 1 {
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
            fs::rename(from, to)
        },
    );

    assert_eq!(res, Err(TTZipStatus::ErrOpenFailed));
    assert_eq!(fs::read(&old_vol1).unwrap(), b"OLD_VOLUME_1_INTACT");
    assert_eq!(fs::read(&old_vol2).unwrap(), b"OLD_VOLUME_2_INTACT");
    assert_eq!(fs::read(&old_vol3).unwrap(), b"OLD_VOLUME_3_INTACT");
}

#[test]
fn test_multivolume_replacement_with_fewer_volumes_retires_stale_tail_volumes() {
    let tmp = tempdir().unwrap();
    let out_dir = tmp.path().join("split_fewer");
    fs::create_dir_all(&out_dir).unwrap();
    let src_file = tmp.path().join("payload.bin");
    // ~90 KiB deterministic incompressible payload
    let payload: Vec<u8> = (0..92_160u32)
        .map(|i| ((i.wrapping_mul(1664525).wrapping_add(1013904223)) >> 24) as u8)
        .collect();
    fs::write(&src_file, &payload).unwrap();

    // 1. Numeric volume family (.7z.001, .7z.002, .7z.003) replaced by 2 volumes (.7z.001, .7z.002)
    let target_7z = out_dir.join("archive.7z");
    fs::write(out_dir.join("archive.7z.001"), b"STALE_7Z_VOL_1").unwrap();
    fs::write(out_dir.join("archive.7z.002"), b"STALE_7Z_VOL_2").unwrap();
    fs::write(out_dir.join("archive.7z.003"), b"STALE_7Z_VOL_3_MUST_BE_RETIRED").unwrap();

    let cfg_7z = UniFFICreateArchiveConfig {
        source_paths: vec![src_file.to_str().unwrap().to_string()],
        output_path: target_7z.to_str().unwrap().to_string(),
        format: UniFFIArchiveFormat::SevenZip,
        level: 0,
        password: None,
        split_volume_size_bytes: Some(65_536),
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 1,
    };
    create_archive_advanced(cfg_7z, None, None).unwrap();

    assert!(out_dir.join("archive.7z.001").exists(), ".7z.001 must exist");
    assert!(out_dir.join("archive.7z.002").exists(), ".7z.002 must exist");
    assert!(
        !out_dir.join("archive.7z.003").exists(),
        "Stale tail volume .7z.003 must be retired when new archive has fewer volumes"
    );
    assert_ne!(fs::read(out_dir.join("archive.7z.001")).unwrap(), b"STALE_7Z_VOL_1");

    // 2. Split ZIP volume family (.z01, .z02, .zip) replaced by 2 volumes (.z01, .zip)
    let target_zip = out_dir.join("archive.zip");
    fs::write(out_dir.join("archive.z01"), b"STALE_ZIP_Z01").unwrap();
    fs::write(out_dir.join("archive.z02"), b"STALE_ZIP_Z02_MUST_BE_RETIRED").unwrap();
    fs::write(out_dir.join("archive.zip"), b"STALE_ZIP_TAIL").unwrap();

    let cfg_zip = UniFFICreateArchiveConfig {
        source_paths: vec![src_file.to_str().unwrap().to_string()],
        output_path: target_zip.to_str().unwrap().to_string(),
        format: UniFFIArchiveFormat::Zip,
        level: 0,
        password: None,
        split_volume_size_bytes: Some(65_536),
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 1,
    };
    create_archive_advanced(cfg_zip, None, None).unwrap();

    assert!(out_dir.join("archive.z01").exists(), ".z01 must exist");
    assert!(out_dir.join("archive.zip").exists(), ".zip must exist");
    assert!(
        !out_dir.join("archive.z02").exists(),
        "Stale split ZIP volume .z02 must be retired when new archive has fewer volumes"
    );
}



