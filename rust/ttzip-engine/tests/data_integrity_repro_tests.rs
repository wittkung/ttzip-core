// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Task D: Core Engine Data Integrity Bug Reproduction Suite.
//!
//! Isolates 6 reproducible data-integrity violations discovered during Task D:
//! 1. `repro_bug_1`: Unencrypted 7z (`Store` / `LZMA2`) ignores CRC32 mismatch on extraction
//!    (`sevenz/decoder/archive.rs:327` & `sevenz/decoder/stream.rs:89`), returning `Ok` and
//!    writing corrupted files to disk.
//! 2. `repro_bug_2`: `extract_from_archive_handle` (`archive/unified/extract.rs:467`) and
//!    `inspect_from_handle` (`archive/unified/inspect.rs:331`) use
//!    `while archive_read_next_header(a, &mut entry) == 0`, ignoring negative `ARCHIVE_FATAL`
//!    errors after the first entry and returning `Ok` on truncated/corrupted TAR/TAR.GZ streams.
//! 3. `repro_bug_3`: `detect_volume_chain` (`archive/split/reader.rs:64-73`) breaks on the first
//!    missing volume without invoking `PreFlightVolumeProber::probe`, and `extract_archive` /
//!    `ExtractionSession` silently extracts partial files and returns `Ok` when trailing/middle
//!    split volumes are missing.
//! 4. `repro_bug_4`: `ZipArchive::extract_single_file_to_disk` (`zip/reader.rs:421`) creates/
//!    truncates destination files on disk BEFORE verifying the archive password in
//!    `extract_entry_to_sink_with_sheet` (`zip/reader.rs:480`), and 0-byte files in encrypted
//!    ZIPs are marked `is_encrypted: false` (`zip/writer/parallel_batch.rs:118`).
//! 5. `repro_bug_5`: Direct `UnifiedArchiveOrchestrator::create_archive` (`archive/unified/create.rs:219`)
//!    truncates pre-existing target files immediately and leaves partial broken archive bytes on
//!    disk when cancelled mid-stream.
//! 6. `repro_bug_6`: `DFSCommitWalker::commit_file` (`fs/collision/commit.rs:333`) swallows
//!    `renameatx` errors (`let _ = self.fs.renameatx(...)`) during `CommitRecheck` with
//!    `CollisionAction::Replace` and falsely reports `OutcomeStatus::Completed` (`replaced: 1`).

use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::os::unix::io::{AsRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use tempfile::tempdir;

use ttzip_engine::archive::split::{detect_volume_chain, PreFlightVolumeProber};
use ttzip_engine::archive::unified::{verify_archive_stream, UnifiedArchiveOrchestrator};
use ttzip_engine::fs::collision::table::ConflictTableBuilder;
use ttzip_engine::fs::collision::{
    CollisionAction as CoreCollisionAction, DFSCommitWalker, FsOps, ItemMeta, LibcFsOps,
    OutcomeStatus as CoreOutcomeStatus, ResolutionSet as CoreResolutionSet, StagingArea,
    StagingRoot, VolCaps,
};
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipExtractOptions, TTZipStatus, TTZIP_ABI_VERSION_3,
};
use ttzip_engine::uniffi_api::archive::{create_archive_advanced, UniFFICreateArchiveConfig};
use ttzip_engine::uniffi_api::types::ArchiveFormat as UniFFIArchiveFormat;
use ttzip_engine::uniffi_api::vfs_session_status::{open_archive_session, VfsSessionStatus};

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

fn collect_relative_files(root: &Path) -> BTreeMap<String, u64> {
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
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                map.insert(rel, meta.len());
            }
        }
    }
    map
}

#[test]
fn repro_bug_1_unencrypted_7z_corrupted_payload_ignores_crc_mismatch() {
    let tmp = tempdir().unwrap();
    let src_file = tmp.path().join("important_contract.txt");
    fs::write(&src_file, b"CRITICAL_FINANCIAL_RECORD_1234567890_ABCDEF".repeat(64)).unwrap();

    let archive_path = tmp.path().join("unencrypted_store.7z");
    create_archive_simple(
        &[src_file],
        &archive_path,
        TTZipArchiveFormat::SevenZip,
        TTZipCompressionLevel::Store,
        TTZipEncryptionMethod::None,
        None,
    )
    .unwrap();

    let mut raw = fs::read(&archive_path).unwrap();
    for b in &mut raw[40..72] {
        *b ^= 0xAA;
    }
    fs::write(&archive_path, &raw).unwrap();

    let verify_res = verify_archive_stream(&archive_path, None, None, std::ptr::null_mut());
    let detected_corrupt =
        verify_res.as_ref().map(|r| r.corrupted_entries_count > 0).unwrap_or(true);
    assert!(detected_corrupt, "verify_archive_stream should detect CRC mismatch: {verify_res:?}");

    let out_dir = tmp.path().join("extracted_7z_corrupt");
    fs::create_dir_all(&out_dir).unwrap();
    let extract_res = extract_archive_simple(&archive_path, &out_dir, None);
    assert!(
        extract_res.is_err(),
        "BUG REPRO #1 (sevenz/decoder/archive.rs:327 & stream.rs:89): \
         unencrypted 7z with corrupted payload CRC mismatch returned Ok({extract_res:?}) \
         and wrote silently corrupted file to disk!"
    );
}

#[test]
fn repro_bug_2_truncated_or_corrupted_tar_and_targz_after_first_entry_return_false_ok() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("multi_tar_src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("01_first.txt"), vec![b'A'; 4096]).unwrap();
    fs::write(src_dir.join("02_second.txt"), vec![b'B'; 16_384]).unwrap();

    let tar_path = tmp.path().join("truncated.tar");
    create_archive_simple(
        std::slice::from_ref(&src_dir),
        &tar_path,
        TTZipArchiveFormat::Tar,
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::None,
        None,
    )
    .unwrap();
    let raw_tar = fs::read(&tar_path).unwrap();
    fs::write(&tar_path, &raw_tar[..5500]).unwrap();

    let out_tar = tmp.path().join("out_truncated_tar");
    fs::create_dir_all(&out_tar).unwrap();
    let res_tar = extract_archive_simple(&tar_path, &out_tar, None);

    let tgz_path = tmp.path().join("corrupt.tar.gz");
    let tgz_src = tmp.path().join("tgz_src");
    fs::create_dir_all(&tgz_src).unwrap();
    fs::write(tgz_src.join("01_head.bin"), pseudo_random_bytes(1, 4096)).unwrap();
    fs::write(tgz_src.join("02_tail.bin"), pseudo_random_bytes(2, 24_576)).unwrap();
    create_archive_simple(
        &[tgz_src],
        &tgz_path,
        TTZipArchiveFormat::TarGz,
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::None,
        None,
    )
    .unwrap();
    let mut raw_tgz = fs::read(&tgz_path).unwrap();
    let mid = (raw_tgz.len() * 2) / 3;
    for b in &mut raw_tgz[mid..mid + 32] {
        *b ^= 0xFF;
    }
    fs::write(&tgz_path, &raw_tgz).unwrap();

    let out_tgz = tmp.path().join("out_corrupt_tgz");
    fs::create_dir_all(&out_tgz).unwrap();
    let res_tgz = extract_archive_simple(&tgz_path, &out_tgz, None);

    assert!(
        res_tar.is_err() && res_tgz.is_err(),
        "BUG REPRO #2 (archive/unified/extract.rs:467): \
         while archive_read_next_header(a, &mut entry) == 0 ignores negative error returns \
         after first entry! truncated.tar={res_tar:?}, corrupt.tar.gz={res_tgz:?}"
    );
}

#[test]
fn repro_bug_3_split_volume_missing_middle_or_last_volume_silent_truncation() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("split_src");
    fs::create_dir_all(&src_dir).unwrap();
    for idx in 1..=3 {
        fs::write(
            src_dir.join(format!("part_file_{idx}.bin")),
            pseudo_random_bytes(idx as u64 * 999, 40_960),
        )
        .unwrap();
    }

    let out_base = tmp.path().join("multi.tar");
    let cfg = UniFFICreateArchiveConfig {
        source_paths: vec![src_dir.to_str().unwrap().to_string()],
        output_path: out_base.to_str().unwrap().to_string(),
        format: UniFFIArchiveFormat::Tar,
        level: 0,
        password: None,
        split_volume_size_bytes: Some(41_984),
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: false,
        thread_budget: 1,
    };
    create_archive_advanced(cfg, None, None).unwrap();

    let vol1 = PathBuf::from(format!("{}.001", out_base.display()));
    let vol2 = PathBuf::from(format!("{}.002", out_base.display()));
    let vol3 = PathBuf::from(format!("{}.003", out_base.display()));
    assert!(vol1.exists() && vol2.exists() && vol3.exists());

    // 1. Missing last volume (.003): extract_archive silently succeeds with only 2 of 3 files!
    let vol3_bytes = fs::read(&vol3).unwrap();
    fs::remove_file(&vol3).unwrap();
    let extract_missing_3 = tmp.path().join("extract_missing_vol3");
    fs::create_dir_all(&extract_missing_3).unwrap();
    let ext_res_missing_3 = extract_archive_simple(&vol1, &extract_missing_3, None);

    // 2. Restore .003 and delete middle volume (.002): detect_volume_chain returns Ok([.001])
    // AND extract_archive silently succeeds with only 1 of 3 files!
    fs::write(&vol3, vol3_bytes).unwrap();
    fs::remove_file(&vol2).unwrap();
    assert!(PreFlightVolumeProber::probe(&vol1).is_some());
    assert!(matches!(
        open_archive_session(vol1.to_str().unwrap().to_string(), None),
        Ok(VfsSessionStatus::MissingVolume { .. })
    ));
    let chain_missing_2 = detect_volume_chain(&vol1);
    let extract_missing_2 = tmp.path().join("extract_missing_vol2");
    fs::create_dir_all(&extract_missing_2).unwrap();
    let ext_res_missing_2 = extract_archive_simple(&vol1, &extract_missing_2, None);

    assert!(
        chain_missing_2.is_err() && ext_res_missing_2.is_err() && ext_res_missing_3.is_err(),
        "BUG REPRO #3 (archive/split/reader.rs:64-73 & unified/extract.rs:467): \
         missing .002 detect_volume_chain={chain_missing_2:?}, extract={ext_res_missing_2:?} \
         (files={:?}); missing .003 extract={ext_res_missing_3:?} (files={:?})!",
        collect_relative_files(&extract_missing_2),
        collect_relative_files(&extract_missing_3)
    );
}

#[test]
fn repro_bug_4_encrypted_zip_direct_extract_truncates_or_leaks_files_before_password_check() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("enc_src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("secret_a.txt"), b"TOP_SECRET_DATA_ALPHA_12345").unwrap();
    fs::write(src_dir.join("secret_b.txt"), b"TOP_SECRET_DATA_BETA_67890").unwrap();

    let zip_path = tmp.path().join("encrypted_payload.zip");
    create_archive_simple(
        &[src_dir],
        &zip_path,
        TTZipArchiveFormat::Zip,
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::Aes256,
        Some("SecretPassword123"),
    )
    .unwrap();

    let out_dir = tmp.path().join("direct_extract_no_pw");
    let preexisting_dir = out_dir.join("enc_src");
    fs::create_dir_all(&preexisting_dir).unwrap();
    let preexisting_file = preexisting_dir.join("secret_a.txt");
    let preexisting_bytes = b"PRECIOUS_PREEXISTING_USER_FILE_MUST_SURVIVE";
    fs::write(&preexisting_file, preexisting_bytes).unwrap();

    for bad_pw in [None, Some("WrongPassword999")] {
        let res = extract_archive_simple(&zip_path, &out_dir, bad_pw);
        assert_eq!(res, Err(TTZipStatus::ErrInvalidPassword));

        let actual_a = fs::read(&preexisting_file).unwrap_or_default();
        assert_eq!(
            actual_a, preexisting_bytes,
            "BUG REPRO #4 (zip/reader.rs:421): direct ZIP extraction with bad password={bad_pw:?} \
             truncated or modified pre-existing file before password verification!"
        );
        assert!(
            !preexisting_dir.join("secret_b.txt").exists(),
            "BUG REPRO #4: unauthenticated entry secret_b.txt was created on disk with bad password={bad_pw:?}!"
        );
    }
}

unsafe extern "C" fn cancel_after_first_ffi_cb(
    _processed: u64,
    _total: u64,
    _entry: *const libc::c_char,
    user_data: *mut libc::c_void,
) -> bool {
    let counter = &*(user_data as *const AtomicUsize);
    counter.fetch_add(1, Ordering::SeqCst) == 0
}

#[test]
fn repro_bug_5_direct_create_archive_clobbers_existing_file_on_midway_cancel() {
    let tmp = tempdir().unwrap();
    let src_dir = tmp.path().join("cancel_src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("f1.bin"), pseudo_random_bytes(10, 32_768)).unwrap();
    fs::write(src_dir.join("f2.bin"), pseudo_random_bytes(20, 32_768)).unwrap();

    let target_archive = tmp.path().join("direct_output.tar.gz");
    let original_bytes = b"ORIGINAL_TAR_GZ_ARCHIVE_CONTENT_BEFORE_CANCEL";
    fs::write(&target_archive, original_bytes).unwrap();

    let calls = AtomicUsize::new(0);
    let opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::TarGz,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 0,
        progress_callback: Some(cancel_after_first_ffi_cb),
        user_data: &calls as *const AtomicUsize as *mut libc::c_void,
    };

    let res = UnifiedArchiveOrchestrator::create_archive(&[src_dir], &target_archive, &opts, 0);
    assert_eq!(res, Err(TTZipStatus::Cancelled));

    let after_bytes = fs::read(&target_archive).unwrap_or_default();
    assert_eq!(
        after_bytes, original_bytes,
        "BUG REPRO #5 (archive/unified/create.rs:219): direct create_archive truncated/clobbered \
         pre-existing target file on mid-way cancellation and left {} partial bytes!",
        after_bytes.len()
    );
}

struct FailSecondRenameFsOps {
    inner: LibcFsOps,
    rename_calls: AtomicUsize,
}

impl FsOps for FailSecondRenameFsOps {
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
        if self.rename_calls.fetch_add(1, Ordering::SeqCst) >= 1 {
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
fn repro_bug_6_commit_recheck_replace_swallows_rename_failure_and_reports_completed() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path().join("dest_recheck");
    fs::create_dir_all(&dest).unwrap();
    fs::write(dest.join("late_conflict.txt"), b"EXISTING_LATE_CONFLICT").unwrap();

    let staging = StagingArea::create(&dest).unwrap();
    let payload_dir = staging.payload_dir();
    fs::write(payload_dir.path().join("late_conflict.txt"), b"INCOMING_NEW_DATA").unwrap();

    let table = ConflictTableBuilder::new().build().unwrap();
    let ops = FailSecondRenameFsOps { inner: LibcFsOps, rename_calls: AtomicUsize::new(0) };
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

    assert_eq!(
        outcome.status,
        CoreOutcomeStatus::CompletedWithFailures,
        "BUG REPRO #6 (fs/collision/commit.rs:333): `let _ = self.fs.renameatx(...)` swallowed \
         ENOSPC error on Replace and falsely reported status={:?}, replaced={}, failed={:?}!",
        outcome.status,
        outcome.replaced,
        outcome.failed
    );
    assert_eq!(
        outcome.replaced, 0,
        "Failed Replace must not increment outcome.replaced"
    );
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(
        fs::read(dest.join("late_conflict.txt")).unwrap(),
        b"EXISTING_LATE_CONFLICT"
    );
}

#[test]
fn repro_7z_single_entry_no_crc_premature_eof_and_overflow_rejected() {
    use std::io::Cursor;
    use ttzip_engine::sevenz::decoder::stream::{
        extract_7z_single_entry_from_reader, extract_single_entry_bounded,
    };
    use ttzip_engine::sevenz::format::METHOD_COPY;
    use ttzip_engine::sevenz::header::{
        SevenZCoder, SevenZEntryLocation, SevenZFileMeta, SevenZFolder, SevenZHeaderInfo,
        SevenZSeekIndex,
    };

    let short_payload = b"PREMATURE_EOF_PAYLOAD_ONLY_32_B!";
    let declared_size = 128u64;
    let folder = SevenZFolder {
        coders: vec![SevenZCoder {
            method_id: METHOD_COPY,
            num_in_streams: 1,
            num_out_streams: 1,
            properties: Vec::new(),
        }],
        bind_pairs: Vec::new(),
        packed_streams: vec![0],
        unpack_sizes: vec![short_payload.len() as u64],
        crc: None,
        num_unpack_streams: 1,
        packed_offset: 0,
        packed_len: short_payload.len(),
    };
    let info = SevenZHeaderInfo {
        payload_offset: 0,
        payload_len: short_payload.len(),
        pack_sizes: vec![short_payload.len() as u64],
        folders: vec![folder],
        stream_sizes: vec![declared_size],
        stream_crcs: Vec::new(),
        files: vec![SevenZFileMeta {
            rel_path: "no_crc_truncated.bin".to_string(),
            is_directory: false,
            is_empty_stream: false,
            mtime_epoch_secs: None,
            mode: 0o644,
        }],
        primary_method_id: METHOD_COPY,
        coder_props: Vec::new(),
        is_encrypted: false,
        aes_salt: [0u8; 16],
        aes_salt_len: 0,
        aes_iv: [0u8; 16],
        aes_iv_len: 0,
        aes_num_cycles_power: 0,
    };
    let seek_index = SevenZSeekIndex {
        entries: vec![SevenZEntryLocation {
            file_index: 0,
            rel_path: "no_crc_truncated.bin".to_string(),
            is_directory: false,
            is_empty_stream: false,
            folder_index: Some(0),
            stream_index: Some(0),
            offset_in_folder: 0,
            uncompressed_size: declared_size,
            crc: None,
        }],
    };

    assert_eq!(
        extract_single_entry_bounded(short_payload, &info, &seek_index, 0, None, 0),
        Err(TTZipStatus::ErrCorruptHeader),
        "extract_single_entry_bounded must reject short output when CRC is None"
    );
    let mut reader = Cursor::new(short_payload.as_slice());
    assert_eq!(
        extract_7z_single_entry_from_reader(&mut reader, &info, &seek_index, 0, None),
        Err(TTZipStatus::ErrCorruptHeader),
        "extract_7z_single_entry_from_reader must reject short output when CRC is None"
    );

    let overflow_index = SevenZSeekIndex {
        entries: vec![SevenZEntryLocation {
            offset_in_folder: u64::MAX - 8,
            ..seek_index.entries[0].clone()
        }],
    };
    assert_eq!(
        extract_single_entry_bounded(short_payload, &info, &overflow_index, 0, None, 0),
        Err(TTZipStatus::ErrCorruptHeader),
        "extract_single_entry_bounded must reject offset_in_folder + uncompressed_size overflow"
    );
    let mut reader2 = Cursor::new(short_payload.as_slice());
    assert_eq!(
        extract_7z_single_entry_from_reader(&mut reader2, &info, &overflow_index, 0, None),
        Err(TTZipStatus::ErrCorruptHeader),
        "extract_7z_single_entry_from_reader must reject offset_in_folder + uncompressed_size overflow"
    );
}

#[test]
fn test_multivolume_commit_and_restore_failure_preserves_backup_and_allows_recovery() {
    use ttzip_engine::archive::split::VolumeNamingScheme;
    use ttzip_engine::archive::unified::create::{
        recover_preserved_create_backup, with_staged_archive_destination_detailed_using,
        CreateCommitMode, CreateStageCommitError,
    };

    let tmp = tempdir().unwrap();
    let out_dir = tmp.path().join("rollback_preserve_bak");
    fs::create_dir_all(&out_dir).unwrap();
    let target_archive = out_dir.join("bundle.zip");

    let old_vol1 = out_dir.join("bundle.zip.001");
    let old_vol2 = out_dir.join("bundle.zip.002");
    let old_vol3 = out_dir.join("bundle.zip.003");
    fs::write(&old_vol1, b"OLD_VOLUME_1_INTACT").unwrap();
    fs::write(&old_vol2, b"OLD_VOLUME_2_INTACT").unwrap();
    fs::write(&old_vol3, b"OLD_VOLUME_3_INTACT").unwrap();

    let staged_calls = AtomicUsize::new(0);
    let res = with_staged_archive_destination_detailed_using(
        &target_archive,
        CreateCommitMode::SplitVolume(VolumeNamingScheme::NumberedExtension),
        |staged_target| {
            let staged_dir = staged_target.parent().unwrap();
            fs::write(staged_dir.join("bundle.zip.001"), b"NEW_VOLUME_1_PARTIAL").unwrap();
            fs::write(staged_dir.join("bundle.zip.002"), b"NEW_VOLUME_2_FAILS").unwrap();
            Ok(())
        },
        |from, to| {
            if staged_calls.fetch_add(1, Ordering::SeqCst) == 1 {
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
            fs::rename(from, to)
        },
        |bak_src, orig_dst| {
            // Simulate restore rename failure for bundle.zip.002 during rollback
            if orig_dst.file_name().and_then(|n| n.to_str()) == Some("bundle.zip.002") {
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
            fs::rename(bak_src, orig_dst)
        },
    );

    let (backup_dir, unrestored) = match res {
        Err(CreateStageCommitError::RollbackIncomplete {
            status,
            ref backup_dir,
            ref unrestored_origins,
            ref lingering_installed,
        }) => {
            assert_eq!(status, TTZipStatus::ErrOpenFailed);
            assert!(lingering_installed.is_empty(), "Partially installed vol1 must be removed");
            let msg = res.as_ref().unwrap_err().to_string();
            assert!(msg.contains(&backup_dir.display().to_string()));
            assert!(msg.contains("bundle.zip.002"));
            (backup_dir.clone(), unrestored_origins.clone())
        }
        other => panic!("Expected RollbackIncomplete error, got {other:?}"),
    };

    assert_eq!(unrestored, vec![old_vol2.clone()]);
    assert!(backup_dir.exists(), "Backup directory MUST be preserved when restore fails");
    assert_eq!(
        fs::read(backup_dir.join("bundle.zip.002")).unwrap(),
        b"OLD_VOLUME_2_INTACT",
        "Unique backup of bundle.zip.002 must survive inside backup_dir"
    );
    assert_eq!(fs::read(&old_vol1).unwrap(), b"OLD_VOLUME_1_INTACT");
    assert_eq!(fs::read(&old_vol3).unwrap(), b"OLD_VOLUME_3_INTACT");

    let recovered = recover_preserved_create_backup(&backup_dir).unwrap();
    assert_eq!(recovered, vec![old_vol2.clone()]);
    assert!(!backup_dir.exists(), "Backup directory must be cleaned up after recovery succeeds");
    assert_eq!(fs::read(&old_vol2).unwrap(), b"OLD_VOLUME_2_INTACT");
}

fn create_7z_advanced(src: &Path, dest: &Path, split_volume_size_bytes: Option<u64>) {
    create_archive_advanced(
        UniFFICreateArchiveConfig {
            source_paths: vec![src.to_str().unwrap().to_string()],
            output_path: dest.to_str().unwrap().to_string(),
            format: UniFFIArchiveFormat::SevenZip,
            level: 0,
            password: None,
            split_volume_size_bytes,
            exclude_patterns: vec![],
            include_patterns: vec![],
            exclude_vcs: false,
            no_mac_metadata: true,
            thread_budget: 1,
        },
        None,
        None,
    )
    .unwrap();
}

#[test]
fn test_single_file_and_multivolume_archive_creation_preserve_unrelated_numeric_suffixed_neighbors() {
    use ttzip_engine::archive::unified::create::{
        with_staged_archive_destination_detailed_using, CreateCommitMode,
    };

    let tmp = tempdir().unwrap();
    let out_dir = tmp.path().join("neighbor_protection");
    fs::create_dir_all(&out_dir).unwrap();
    let src_file = tmp.path().join("payload.bin");
    fs::write(&src_file, pseudo_random_bytes(77, 92_160)).unwrap();

    // 1. Single-file atomic replacement: must NOT move existing target to .ttzip_create_bak_*
    // and must NEVER touch report.zip.1, report.zip.2026, report.zip.001, or report.z01.
    let report_zip = out_dir.join("report.zip");
    let neighbor_1 = out_dir.join("report.zip.1");
    let neighbor_2026 = out_dir.join("report.zip.2026");
    let neighbor_001 = out_dir.join("report.zip.001");
    let neighbor_z01 = out_dir.join("report.z01");
    fs::write(&report_zip, b"OLD_REPORT_ZIP").unwrap();
    fs::write(&neighbor_1, b"ROTATED_LOG_1").unwrap();
    fs::write(&neighbor_2026, b"ANNUAL_ARCHIVE_2026").unwrap();
    fs::write(&neighbor_001, b"UNRELATED_SPLIT_001").unwrap();
    fs::write(&neighbor_z01, b"UNRELATED_Z01").unwrap();

    let restore_called = AtomicUsize::new(0);
    let fail_single = with_staged_archive_destination_detailed_using(
        &report_zip,
        CreateCommitMode::SingleFile,
        |staged_target| {
            fs::write(staged_target, b"STAGED_SINGLE_FILE").unwrap();
            Ok(())
        },
        |_from, _to| Err(std::io::Error::from_raw_os_error(libc::EACCES)),
        |_bak, _orig| {
            restore_called.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    );
    assert!(fail_single.is_err());
    assert_eq!(
        restore_called.load(Ordering::SeqCst),
        0,
        "Single-file replace must use direct atomic rename without moving target to backup first"
    );
    assert_eq!(fs::read(&report_zip).unwrap(), b"OLD_REPORT_ZIP");

    create_archive_simple(
        std::slice::from_ref(&src_file),
        &report_zip,
        TTZipArchiveFormat::Zip,
        TTZipCompressionLevel::Normal,
        TTZipEncryptionMethod::None,
        None,
    )
    .unwrap();
    assert_ne!(fs::read(&report_zip).unwrap(), b"OLD_REPORT_ZIP");
    assert_eq!(fs::read(&neighbor_1).unwrap(), b"ROTATED_LOG_1");
    assert_eq!(fs::read(&neighbor_2026).unwrap(), b"ANNUAL_ARCHIVE_2026");
    assert_eq!(fs::read(&neighbor_001).unwrap(), b"UNRELATED_SPLIT_001");
    assert_eq!(fs::read(&neighbor_z01).unwrap(), b"UNRELATED_Z01");

    // 2. Multi-volume replacement: retires confirmed contiguous .003 but preserves .1, .2026, and non-contiguous .005.
    let split_7z = out_dir.join("split.7z");
    fs::write(out_dir.join("split.7z.001"), b"OLD_7Z_001").unwrap();
    fs::write(out_dir.join("split.7z.002"), b"OLD_7Z_002").unwrap();
    fs::write(out_dir.join("split.7z.003"), b"OLD_7Z_003_CONTIGUOUS_TAIL").unwrap();
    fs::write(out_dir.join("split.7z.1"), b"NEIGHBOR_DOT_1").unwrap();
    fs::write(out_dir.join("split.7z.2026"), b"NEIGHBOR_YEAR_2026").unwrap();
    fs::write(out_dir.join("split.7z.005"), b"UNLINKED_GAP_005").unwrap();

    create_7z_advanced(&src_file, &split_7z, Some(65_536));

    assert!(out_dir.join("split.7z.001").exists());
    assert!(out_dir.join("split.7z.002").exists());
    assert!(!out_dir.join("split.7z.003").exists(), "Confirmed contiguous .003 must be retired");
    assert_eq!(fs::read(out_dir.join("split.7z.1")).unwrap(), b"NEIGHBOR_DOT_1");
    assert_eq!(fs::read(out_dir.join("split.7z.2026")).unwrap(), b"NEIGHBOR_YEAR_2026");
    assert_eq!(fs::read(out_dir.join("split.7z.005")).unwrap(), b"UNLINKED_GAP_005");
}

#[test]
fn test_dot_001_single_file_vs_single_volume_split_mode_distinction() {
    let tmp = tempdir().unwrap();
    let small_src = tmp.path().join("small.txt");
    fs::write(&small_src, b"SMALL_PAYLOAD_FITS_IN_FIRST_VOLUME").unwrap();

    // Case A: Unsplit single-file archive explicitly named `report.001` (`split_volume_size_bytes: None`).
    // Must commit via SingleFile atomic rename and MUST preserve adjacent `report.002` and `report.003`.
    let single_dir = tmp.path().join("single_dot_001");
    fs::create_dir_all(&single_dir).unwrap();
    let single_target = single_dir.join("report.001");
    fs::write(&single_target, b"OLD_SINGLE_REPORT_001").unwrap();
    fs::write(single_dir.join("report.002"), b"ADJACENT_REPORT_002_MUST_SURVIVE").unwrap();
    fs::write(single_dir.join("report.003"), b"ADJACENT_REPORT_003_MUST_SURVIVE").unwrap();

    create_7z_advanced(&small_src, &single_target, None);
    assert_ne!(fs::read(&single_target).unwrap(), b"OLD_SINGLE_REPORT_001");
    assert_eq!(fs::read(single_dir.join("report.002")).unwrap(), b"ADJACENT_REPORT_002_MUST_SURVIVE");
    assert_eq!(fs::read(single_dir.join("report.003")).unwrap(), b"ADJACENT_REPORT_003_MUST_SURVIVE");

    // Case B: Split mode explicitly enabled (`split_volume_size_bytes: Some(65_536)`) targeting `report.001`,
    // where the compressed output fits within the first volume alone (`report.001`).
    // Must retire confirmed old volumes `report.002` and `report.003` while preserving `report.1`, `report.2026`, `report.005`.
    let split_dir = tmp.path().join("split_single_vol_dot_001");
    fs::create_dir_all(&split_dir).unwrap();
    let split_target = split_dir.join("report.001");
    fs::write(&split_target, b"OLD_SPLIT_VOL_001").unwrap();
    fs::write(split_dir.join("report.002"), b"OLD_SPLIT_VOL_002_RETIRED").unwrap();
    fs::write(split_dir.join("report.003"), b"OLD_SPLIT_VOL_003_RETIRED").unwrap();
    fs::write(split_dir.join("report.1"), b"UNRELATED_DOT_1").unwrap();
    fs::write(split_dir.join("report.2026"), b"UNRELATED_YEAR_2026").unwrap();
    fs::write(split_dir.join("report.005"), b"DISJOINT_GAP_005").unwrap();

    create_7z_advanced(&small_src, &split_target, Some(65_536));
    assert_ne!(fs::read(&split_target).unwrap(), b"OLD_SPLIT_VOL_001");
    assert!(!split_dir.join("report.002").exists(), "Confirmed old volume report.002 must be retired");
    assert!(!split_dir.join("report.003").exists(), "Confirmed old volume report.003 must be retired");
    assert_eq!(fs::read(split_dir.join("report.1")).unwrap(), b"UNRELATED_DOT_1");
    assert_eq!(fs::read(split_dir.join("report.2026")).unwrap(), b"UNRELATED_YEAR_2026");
    assert_eq!(fs::read(split_dir.join("report.005")).unwrap(), b"DISJOINT_GAP_005");
}

