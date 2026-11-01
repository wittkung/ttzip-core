// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R1–R3 targeted reproduction and regression tests for cleanup object ownership,
//! explicit finalization across failure paths, backup vs uncommitted stage separation,
//! and post-commit cleanup failure reporting alongside committed success.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use ttzip_archive::archive::unified::create::{
    create_archive_detailed, recover_preserved_create_backup, run_libarchive_staged_write,
    with_staged_archive_destination_detailed, CreateCommitMode, CreateStageCommitError,
};
use ttzip_archive::archive::unified::create_staging::set_create_stage_after_work_hook;
use ttzip_archive::fs::abort_guard::{
    set_abort_guard_after_check_hook, set_abort_guard_after_quarantine_hook,
};
use ttzip_archive::fs::collision::staging::{
    set_staging_after_check_hook, set_staging_after_quarantine_hook,
};
use ttzip_archive::fs::collision::StagingArea;
use ttzip_archive::fs::FileAbortGuard;
use ttzip_archive::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipStatus, TTZIP_ABI_VERSION_3,
};
use ttzip_engine::uniffi_api::archive::{
    create_archive_advanced_with_options, UniFFICreateArchiveConfig,
    UniFFICreateArchiveExtraOptions,
};
use ttzip_engine::uniffi_api::extraction_session::{
    CollisionAction, CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession,
    OutcomeStatus, ResolutionSet,
};
use ttzip_engine::uniffi_api::types::{
    CancellationToken, TTZipArchiveFormat as UniFFIFormat,
};

fn write_minimal_zip(path: &Path, entry_name: &str, payload: &[u8]) {
    let mut file = fs::File::create(path).expect("create zip");
    let name_bytes = entry_name.as_bytes();
    let crc = crc32fast::hash(payload);
    let size = payload.len() as u32;

    let mut lfh = Vec::new();
    lfh.extend_from_slice(&0x04034b50u32.to_le_bytes());
    lfh.extend_from_slice(&20u16.to_le_bytes());
    lfh.extend_from_slice(&0x0800u16.to_le_bytes());
    lfh.extend_from_slice(&0u16.to_le_bytes());
    lfh.extend_from_slice(&0u16.to_le_bytes());
    lfh.extend_from_slice(&0u16.to_le_bytes());
    lfh.extend_from_slice(&crc.to_le_bytes());
    lfh.extend_from_slice(&size.to_le_bytes());
    lfh.extend_from_slice(&size.to_le_bytes());
    lfh.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    lfh.extend_from_slice(&0u16.to_le_bytes());
    lfh.extend_from_slice(name_bytes);
    file.write_all(&lfh).unwrap();
    file.write_all(payload).unwrap();
    let cd_offset = (lfh.len() + payload.len()) as u32;

    let mut cdfh = Vec::new();
    cdfh.extend_from_slice(&0x02014b50u32.to_le_bytes());
    cdfh.extend_from_slice(&20u16.to_le_bytes());
    cdfh.extend_from_slice(&20u16.to_le_bytes());
    cdfh.extend_from_slice(&0x0800u16.to_le_bytes());
    cdfh.extend_from_slice(&0u16.to_le_bytes());
    cdfh.extend_from_slice(&0u16.to_le_bytes());
    cdfh.extend_from_slice(&0u16.to_le_bytes());
    cdfh.extend_from_slice(&crc.to_le_bytes());
    cdfh.extend_from_slice(&size.to_le_bytes());
    cdfh.extend_from_slice(&size.to_le_bytes());
    cdfh.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    cdfh.extend_from_slice(&0u16.to_le_bytes());
    cdfh.extend_from_slice(&0u16.to_le_bytes());
    cdfh.extend_from_slice(&0u16.to_le_bytes());
    cdfh.extend_from_slice(&0u16.to_le_bytes());
    cdfh.extend_from_slice(&0u32.to_le_bytes());
    cdfh.extend_from_slice(&0u32.to_le_bytes());
    cdfh.extend_from_slice(name_bytes);
    file.write_all(&cdfh).unwrap();
    let cd_size = cdfh.len() as u32;

    let mut eocd = Vec::new();
    eocd.extend_from_slice(&0x06054b50u32.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes());
    eocd.extend_from_slice(&1u16.to_le_bytes());
    eocd.extend_from_slice(&1u16.to_le_bytes());
    eocd.extend_from_slice(&cd_size.to_le_bytes());
    eocd.extend_from_slice(&cd_offset.to_le_bytes());
    eocd.extend_from_slice(&0u16.to_le_bytes());
    file.write_all(&eocd).unwrap();
    file.flush().unwrap();
}

#[test]
fn test_r1_case1_moved_staged_file_and_sentinel_at_same_path_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let staged_file = tmp.path().join("staged_partial.zip");
    let moved_file = tmp.path().join("moved_partial.zip");

    fs::write(&staged_file, b"PARTIAL_INFLIGHT_BYTES").unwrap();
    let guard = FileAbortGuard::new(&staged_file);

    // Move original staged file away and place a sentinel file at the exact same path
    fs::rename(&staged_file, &moved_file).unwrap();
    fs::write(&staged_file, b"SENTINEL_MUST_SURVIVE").unwrap();

    let cleanup_res = guard.abort_checked();
    assert!(
        cleanup_res.is_err(),
        "abort_checked must refuse to claim clean removal when target path was replaced by another inode"
    );
    assert_eq!(
        fs::read(&staged_file).unwrap(),
        b"SENTINEL_MUST_SURVIVE",
        "Sentinel file at the reused path must be preserved"
    );
}

#[test]
fn test_r1_case2_moved_staging_dir_and_same_named_sentinel_dir_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let dest_root = tmp.path().join("dest_root");
    fs::create_dir_all(&dest_root).unwrap();

    let staging = StagingArea::create(&dest_root).unwrap();
    let staging_dir_path = staging.staging_dir.clone();
    fs::write(staging.payload_dir().path().join("staged.txt"), b"STAGED_DATA").unwrap();

    // Move original staging directory away and create a new directory with the same name containing a sentinel
    let displaced_staging = tmp.path().join("displaced_staging_dir");
    fs::rename(&staging_dir_path, &displaced_staging).unwrap();
    fs::create_dir_all(&staging_dir_path).unwrap();
    let sentinel_file = staging_dir_path.join("sentinel.txt");
    fs::write(&sentinel_file, b"DIR_SENTINEL_PAYLOAD").unwrap();

    let cleanup_res = staging.abort_checked();
    assert!(
        cleanup_res.is_err(),
        "StagingArea::abort_checked must fail when staging_dir was replaced by a different directory"
    );
    assert_eq!(
        fs::read(&sentinel_file).unwrap(),
        b"DIR_SENTINEL_PAYLOAD",
        "Sentinel inside replacement directory must be preserved"
    );
}

#[test]
fn test_r1_case3_replaced_parent_directory_and_reconstructed_tree_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let parent_dir = tmp.path().join("parent_root");
    fs::create_dir_all(&parent_dir).unwrap();

    let staging = StagingArea::create(&parent_dir).unwrap();
    let staging_dir_name = staging.staging_dir.file_name().unwrap().to_os_string();

    // Move parent_dir away and reconstruct a new parent_dir at the original path with same child tree and sentinel
    let moved_parent = tmp.path().join("moved_parent_root");
    fs::rename(&parent_dir, &moved_parent).unwrap();
    let new_same_named_staging = parent_dir.join(&staging_dir_name);
    fs::create_dir_all(&new_same_named_staging).unwrap();
    let sentinel_in_new_parent = new_same_named_staging.join("parent_sentinel.txt");
    fs::write(&sentinel_in_new_parent, b"NEW_PARENT_SENTINEL").unwrap();

    let cleanup_res = staging.abort_checked();
    assert!(
        cleanup_res.is_err(),
        "StagingArea::abort_checked must refuse cleanup when parent directory was replaced"
    );
    assert_eq!(
        fs::read(&sentinel_in_new_parent).unwrap(),
        b"NEW_PARENT_SENTINEL",
        "Sentinel in reconstructed parent directory tree must be preserved"
    );
}

#[test]
fn test_r2_uncommitted_stage_dir_is_not_backup_dir_and_cannot_be_recovered_as_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    let target_zip = out_dir.join("archive.zip");
    fs::write(&target_zip, b"VALID_ORIGINAL_ARCHIVE").unwrap();

    let mut captured_txn_dir = None;
    let res = with_staged_archive_destination_detailed(
        &target_zip,
        CreateCommitMode::SingleFile,
        |staged_dest| {
            fs::write(staged_dest, b"CORRUPTED_UNCOMMITTED_PARTIAL_BYTES").unwrap();
            let txn_dir = staged_dest.parent().unwrap().to_path_buf();
            captured_txn_dir = Some(txn_dir.clone());
            // Make txn_dir read-only so stage cleanup fails
            fs::set_permissions(&txn_dir, fs::Permissions::from_mode(0o500)).unwrap();
            Err(TTZipStatus::Cancelled)
        },
    );

    let txn_dir = captured_txn_dir.unwrap();
    fs::set_permissions(&txn_dir, fs::Permissions::from_mode(0o700)).unwrap();

    let err = res.expect_err("stage failure with cleanup error must return Err");
    assert_eq!(
        err.backup_dir(),
        None,
        "Uncommitted transaction staging directory (.ttzip_create_txn_*) must NEVER be labeled as backup_dir"
    );
    assert!(
        recover_preserved_create_backup(&txn_dir).is_err(),
        "recover_preserved_create_backup must reject uncommitted .ttzip_create_txn_* directory"
    );
    assert_eq!(
        fs::read(&target_zip).unwrap(),
        b"VALID_ORIGINAL_ARCHIVE",
        "Original archive must not be overwritten by uncommitted partial file"
    );
}

#[test]
fn test_r3_extraction_session_commit_reports_post_commit_cleanup_failure_without_losing_success() {
    let tmp = tempfile::tempdir().unwrap();
    let zip_path = tmp.path().join("r3.zip");
    write_minimal_zip(&zip_path, "committed.txt", b"COMMITTED_EXTRACTED_PAYLOAD");

    let dest_root = tmp.path().join("dest");
    fs::create_dir_all(&dest_root).unwrap();

    let session = ExtractionSession::new(ExtractionRequest {
        archive_path: zip_path.to_string_lossy().into_owned(),
        destination_root: dest_root.to_string_lossy().into_owned(),
        password: None,
        entry_filter: None,
        root_at_common_parent: false,
        policy: CollisionPolicy::Replace,
    })
    .unwrap();

    session.stage(None, None).unwrap();

    // Place an unremovable subdir inside `.ttzip_staging_*/displaced` (mode 0o500 with a file inside)
    // so `commit_staged` succeeds in moving `payload/committed.txt` to `dest_root/committed.txt`,
    // but post-commit cleanup of `.ttzip_staging_*` fails with EACCES!
    let mut locked_subdir = None;
    for entry in fs::read_dir(&dest_root).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".ttzip_staging_") {
            let sub = entry.path().join("displaced").join("locked_guard");
            fs::create_dir_all(&sub).unwrap();
            fs::write(sub.join("pin.txt"), b"pin").unwrap();
            fs::set_permissions(&sub, fs::Permissions::from_mode(0o500)).unwrap();
            locked_subdir = Some(sub);
        }
    }
    let locked_subdir = locked_subdir.expect("found staging dir");

    let commit_res = session.commit(ResolutionSet {
        default_action: Some(CollisionAction::Replace),
        overrides: vec![],
    });

    // Restore permissions for tempdir cleanup
    let _ = fs::set_permissions(&locked_subdir, fs::Permissions::from_mode(0o700));

    let CommitResult::Finished { outcome } = commit_res.expect("commit must succeed and preserve committed items") else {
        panic!("expected CommitResult::Finished");
    };
    assert_eq!(outcome.extracted, 1, "Committed file count must be preserved");
    assert_eq!(
        fs::read(dest_root.join("committed.txt")).unwrap(),
        b"COMMITTED_EXTRACTED_PAYLOAD"
    );
    assert_eq!(
        outcome.status,
        OutcomeStatus::CompletedWithFailures,
        "Post-commit staging cleanup failure must be surfaced as CompletedWithFailures"
    );
    assert!(
        !outcome.failed.is_empty(),
        "Post-commit staging cleanup failure must record the lingering staging path in outcome.failed"
    );
}

#[test]
fn test_r1_case4_after_check_hook_replacement_preserves_new_sentinel_file_and_dir() {
    let tmp = tempfile::tempdir().unwrap();

    // Part A: FileAbortGuard check-then-replace race before unlink (identity_mismatch_restored)
    let staged_file = tmp.path().join("race_staged.zip");
    let displaced_file = tmp.path().join("race_displaced.zip");
    fs::write(&staged_file, b"ORIGINAL_STAGED_INODE").unwrap();
    let guard = FileAbortGuard::new(&staged_file);
    let displaced_clone = displaced_file.clone();
    set_abort_guard_after_check_hook(move |target_path| {
        fs::rename(target_path, &displaced_clone).unwrap();
        fs::write(target_path, b"RACE_SENTINEL_FILE").unwrap();
    });
    let file_err = guard.abort_checked().unwrap_err().to_string();
    assert!(file_err.contains("outcome=identity_mismatch_restored"), "got: {file_err}");
    assert_eq!(fs::read(&staged_file).unwrap(), b"RACE_SENTINEL_FILE");

    // Part A2: FileAbortGuard restore_occupied (EEXIST) and quarantined_unlink_failed
    let occ_file = tmp.path().join("occ_staged.zip");
    let occ_displaced = tmp.path().join("occ_displaced.zip");
    fs::write(&occ_file, b"ORIG_OCC").unwrap();
    let occ_guard = FileAbortGuard::new(&occ_file);
    set_abort_guard_after_check_hook(move |p| {
        fs::rename(p, &occ_displaced).unwrap();
        fs::write(p, b"SENTINEL_AT_QUARANTINE").unwrap();
    });
    let captured_q_file = std::sync::Arc::new(std::sync::Mutex::new(None));
    let q_slot = std::sync::Arc::clone(&captured_q_file);
    set_abort_guard_after_quarantine_hook(move |orig, q_path| {
        *q_slot.lock().unwrap() = Some(q_path.to_path_buf());
        fs::write(orig, b"NEW_OCCUPANT_AT_ORIG").unwrap();
    });
    let occ_err = occ_guard.abort_checked().unwrap_err().to_string();
    assert!(occ_err.contains("outcome=identity_mismatch_restore_occupied"), "got: {occ_err}");
    assert_eq!(fs::read(&occ_file).unwrap(), b"NEW_OCCUPANT_AT_ORIG");
    assert_eq!(fs::read(captured_q_file.lock().unwrap().clone().unwrap()).unwrap(), b"SENTINEL_AT_QUARANTINE");

    let qfail_dir = tmp.path().join("qfail_dir");
    fs::create_dir_all(&qfail_dir).unwrap();
    let qfail_file = qfail_dir.join("qfail.zip");
    fs::write(&qfail_file, b"OWNED_BYTES").unwrap();
    let qfail_guard = FileAbortGuard::new(&qfail_file);
    let qfail_dir_c = qfail_dir.clone();
    set_abort_guard_after_quarantine_hook(move |_, _| {
        fs::set_permissions(&qfail_dir_c, fs::Permissions::from_mode(0o500)).unwrap();
    });
    let qf_err = qfail_guard.abort_checked().unwrap_err().to_string();
    fs::set_permissions(&qfail_dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(qf_err.contains("outcome=quarantined_unlink_failed"), "got: {qf_err}");

    // Part B: StagingArea check-then-replace race before rmdir (identity_mismatch_restored, restore_occupied, quarantined_rmdir_failed)
    let dest_root = tmp.path().join("race_dest");
    fs::create_dir_all(&dest_root).unwrap();
    let staging = StagingArea::create(&dest_root).unwrap();
    let staging_dir_path = staging.staging_dir.clone();
    let displaced_dir = tmp.path().join("race_displaced_dir");
    set_staging_after_check_hook(move |dir_path| {
        fs::rename(dir_path, &displaced_dir).unwrap();
        fs::create_dir_all(dir_path).unwrap();
        fs::write(dir_path.join("sentinel.txt"), b"RACE_DIR_SENTINEL").unwrap();
    });
    let dir_err = staging.abort_checked().unwrap_err().to_string();
    assert!(dir_err.contains("outcome=identity_mismatch_restored"), "got: {dir_err}");
    assert_eq!(fs::read(staging_dir_path.join("sentinel.txt")).unwrap(), b"RACE_DIR_SENTINEL");

    let staging2 = StagingArea::create(&dest_root).unwrap();
    let displaced_dir2 = tmp.path().join("race_displaced_dir2");
    set_staging_after_check_hook(move |dir_path| {
        fs::rename(dir_path, &displaced_dir2).unwrap();
        fs::create_dir_all(dir_path).unwrap();
        fs::write(dir_path.join("q_sentinel.txt"), b"DIR_AT_Q").unwrap();
    });
    set_staging_after_quarantine_hook(|orig_dir, _q_dir| {
        fs::create_dir_all(orig_dir).unwrap();
        fs::write(orig_dir.join("occ.txt"), b"DIR_OCC").unwrap();
    });
    let dir_occ_err = staging2.abort_checked().unwrap_err().to_string();
    assert!(dir_occ_err.contains("outcome=identity_mismatch_restore_occupied"), "got: {dir_occ_err}");

    let staging3 = StagingArea::create(&dest_root).unwrap();
    set_staging_after_quarantine_hook(|_orig_dir, q_dir| {
        fs::write(q_dir.join("pin.txt"), b"NON_EMPTY_AFTER_QUARANTINE").unwrap();
    });
    let rmdir_err = staging3.abort_checked().unwrap_err().to_string();
    assert!(rmdir_err.contains("outcome=quarantined_rmdir_failed"), "got: {rmdir_err}");
}

#[test]
fn test_r2_libarchive_staged_write_configure_open_write_close_and_cleanup_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let base_opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Tar,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 64,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    // 1. configure_archive_write_format failure (Brotli is rejected by configure_archive_write_format)
    let cfg_fail_dest = tmp.path().join("cfg_fail.tar");
    let mut bad_opts = base_opts;
    bad_opts.format = TTZipArchiveFormat::Brotli;
    let r1 = run_libarchive_staged_write(
        &cfg_fail_dest,
        TTZipArchiveFormat::Brotli,
        &bad_opts,
        false,
        0,
        |_| Ok(()),
    );
    assert_eq!(r1, Err(TTZipStatus::ErrUnsupportedFeature));
    assert!(!cfg_fail_dest.exists());

    // 2. open_libarchive_destination failure (non-existent parent directory)
    let open_fail_dest = tmp.path().join("missing_parent_dir").join("open_fail.tar");
    let r2 = run_libarchive_staged_write(
        &open_fail_dest,
        TTZipArchiveFormat::Tar,
        &base_opts,
        false,
        0,
        |_| Ok(()),
    );
    assert_eq!(r2, Err(TTZipStatus::ErrOpenFailed));

    // 3. write_fn failure after file creation cleans up staged file via explicit abort_checked
    let write_fail_dest = tmp.path().join("write_fail.tar");
    let r3 = run_libarchive_staged_write(
        &write_fail_dest,
        TTZipArchiveFormat::Tar,
        &base_opts,
        false,
        0,
        |_a| Err(TTZipStatus::Cancelled),
    );
    assert_eq!(r3, Err(TTZipStatus::Cancelled));
    assert!(
        !write_fail_dest.exists(),
        "run_libarchive_staged_write must remove partial file on write_fn failure"
    );

    // 4. write_fn failure combined with abort_checked failure (parent made 0o500) surfaces ErrCompressionFailed
    let locked_dir = tmp.path().join("locked_tar_dir");
    fs::create_dir_all(&locked_dir).unwrap();
    let locked_dest = locked_dir.join("locked_fail.tar");
    let locked_dir_clone = locked_dir.clone();
    let r4 = run_libarchive_staged_write(
        &locked_dest,
        TTZipArchiveFormat::Tar,
        &base_opts,
        false,
        0,
        move |_a| {
            fs::set_permissions(&locked_dir_clone, fs::Permissions::from_mode(0o500)).unwrap();
            Err(TTZipStatus::Cancelled)
        },
    );
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        r4,
        Err(TTZipStatus::ErrCompressionFailed),
        "run_libarchive_staged_write must not swallow abort_checked failure as clean Cancelled"
    );
}

#[test]
fn test_r2_zip_streaming_parallel_preserves_inner_and_outer_cleanup_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let src_file = tmp.path().join("payload.txt");
    fs::write(&src_file, vec![b'Z'; 16 * 1024]).unwrap();

    let out_dir = tmp.path().join("out_zip");
    fs::create_dir_all(&out_dir).unwrap();
    let target_zip = out_dir.join("out.zip");

    static mut TARGET_DIR_PTR: Option<std::path::PathBuf> = None;
    unsafe extern "C" fn cancel_and_lock_cb(
        _processed: u64,
        _total: u64,
        _entry: *const libc::c_char,
        _user_data: *mut libc::c_void,
    ) -> bool {
        if let Some(out_dir) = unsafe { &*std::ptr::addr_of!(TARGET_DIR_PTR) } {
            if let Ok(entries) = fs::read_dir(out_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with(".ttzip_create_txn_") {
                        let _ = fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o500));
                    }
                }
            }
        }
        false
    }

    unsafe {
        TARGET_DIR_PTR = Some(out_dir.clone());
    }
    let opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 64,
        progress_callback: Some(cancel_and_lock_cb),
        user_data: std::ptr::null_mut(),
    };

    let res = create_archive_detailed(std::slice::from_ref(&src_file), &target_zip, &opts, 0);
    for entry in fs::read_dir(&out_dir).unwrap().flatten() {
        let _ = fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o700));
    }

    match res {
        Err(ref err @ CreateStageCommitError::TransactionIncomplete(ref report)) => {
            assert_eq!(report.primary_status, TTZipStatus::Cancelled);
            assert!(report.was_primary_cancelled);
            assert_eq!(err.backup_dir(), None);
            assert_eq!(err.uncommitted_stage_dir(), Some(report.stage_dir.as_path()));
            assert!(report.stage_dir_preserved);
            assert!(!report.cleanup_failures.is_empty());
        }
        other => panic!("Expected CreateStageCommitError::TransactionIncomplete, got: {other:?}"),
    }
}

#[test]
fn test_r3_create_archive_post_commit_txn_and_backup_cleanup_failures_preserve_success() {
    use std::sync::Arc;
    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("doc1.txt"), vec![b'1'; 24 * 1024]).unwrap();
    fs::write(src_dir.join("doc2.txt"), vec![b'2'; 24 * 1024]).unwrap();

    // Case A: Single-file commit succeeds, then `.ttzip_create_txn_*` post-commit cleanup fails
    let out_dir = tmp.path().join("out_single");
    fs::create_dir_all(&out_dir).unwrap();
    let target_zip = out_dir.join("committed_single.zip");

    let locked_pin_dir = Arc::new(std::sync::Mutex::new(None));
    let locked_pin_clone = Arc::clone(&locked_pin_dir);
    set_create_stage_after_work_hook(move |txn_dir, _res| {
        let parent_dir = txn_dir.parent().unwrap().to_path_buf();
        let pin_slot = Arc::clone(&locked_pin_clone);
        set_staging_after_check_hook(move |_dir| {
            let _ = fs::set_permissions(&parent_dir, fs::Permissions::from_mode(0o500));
            *pin_slot.lock().unwrap() = Some(parent_dir);
        });
    });

    let token = CancellationToken::new();
    let cfg = UniFFICreateArchiveConfig {
        source_paths: vec![src_dir.to_string_lossy().into_owned()],
        output_path: target_zip.to_string_lossy().into_owned(),
        format: UniFFIFormat::Zip,
        level: 6,
        password: None,
        split_volume_size_bytes: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 1,
    };

    let report = create_archive_advanced_with_options(
        cfg,
        UniFFICreateArchiveExtraOptions::default(),
        None,
        Some(Arc::clone(&token)),
    )
    .expect("Committed archive creation must succeed even when post-commit txn_dir cleanup fails");

    token.cancel();
    if let Some(locked_parent) = locked_pin_dir.lock().unwrap().take() {
        let _ = fs::set_permissions(&locked_parent, fs::Permissions::from_mode(0o700));
    }
    assert!(target_zip.exists() && report.compressed_bytes > 0);
    let outcome = report.outcome.expect("Post-commit cleanup failure must populate report.outcome");
    assert_eq!(outcome.status, OutcomeStatus::CompletedWithFailures);
    assert_eq!(outcome.extracted, 1);
    assert_eq!(outcome.failed.len(), 1);
    assert!(outcome.failed[0].relative_path.contains(".ttzip_create_txn_"));

    // Case B: Split-volume replacement succeeds, then `.ttzip_create_bak_*` post-commit cleanup fails
    let split_dir = tmp.path().join("out_split");
    fs::create_dir_all(&split_dir).unwrap();
    let split_target = split_dir.join("archive.7z");
    fs::write(split_dir.join("archive.7z.001"), b"OLD_VOL_1").unwrap();
    fs::write(split_dir.join("archive.7z.002"), b"OLD_VOL_2").unwrap();

    let split_dir_clone = split_dir.clone();
    set_staging_after_check_hook(move |dir_path| {
        if dir_path.file_name().and_then(|n| n.to_str()).is_some_and(|s| s.starts_with(".ttzip_create_bak_")) {
            let _ = fs::set_permissions(&split_dir_clone, fs::Permissions::from_mode(0o500));
        }
    });

    let split_cfg = UniFFICreateArchiveConfig {
        source_paths: vec![src_dir.to_string_lossy().into_owned()],
        output_path: split_target.to_string_lossy().into_owned(),
        format: UniFFIFormat::SevenZip,
        level: 1,
        password: None,
        split_volume_size_bytes: Some(16 * 1024),
        exclude_patterns: vec![],
        include_patterns: vec![],
        exclude_vcs: false,
        no_mac_metadata: true,
        thread_budget: 1,
    };

    let split_report = create_archive_advanced_with_options(
        split_cfg,
        UniFFICreateArchiveExtraOptions::default(),
        None,
        None,
    )
    .expect("Committed split archive creation must succeed even when .ttzip_create_bak_* cleanup fails");

    let _ = fs::set_permissions(&split_dir, fs::Permissions::from_mode(0o700));
    assert!(split_dir.join("archive.7z.001").exists());
    let split_outcome = split_report.outcome.expect("Post-commit .ttzip_create_bak_* cleanup failure must populate outcome");
    assert_eq!(split_outcome.status, OutcomeStatus::CompletedWithFailures);
    assert!(split_outcome.failed.iter().any(|f| f.relative_path.contains(".ttzip_create_bak_")));
}

#[test]
fn test_c1_outer_staging_cleanup_must_not_delete_inner_preserved_sentinel() {
    let tmp = tempfile::tempdir().unwrap();
    let src_file = tmp.path().join("src.txt");
    fs::write(&src_file, vec![b'A'; 16 * 1024]).unwrap();
    let out_dir = tmp.path().join("out_c1");
    fs::create_dir_all(&out_dir).unwrap();
    let target_zip = out_dir.join("c1_target.zip");

    static mut REPLACED_SENTINEL_PATH: Option<std::path::PathBuf> = None;
    unsafe extern "C" fn cancel_after_swapping_inner_staged_file(
        _processed: u64,
        _total: u64,
        _entry: *const libc::c_char,
        _user_data: *mut libc::c_void,
    ) -> bool {
        if let Some(out_dir) = unsafe { &*std::ptr::addr_of!(REPLACED_SENTINEL_PATH) } {
            if let Ok(entries) = fs::read_dir(out_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with(".ttzip_create_txn_") {
                        let staged_zip = entry.path().join("c1_target.zip");
                        let displaced = entry.path().join("displaced_partial.zip");
                        if staged_zip.exists() {
                            let _ = fs::rename(&staged_zip, &displaced);
                            let _ = fs::write(&staged_zip, b"INNER_SENTINEL_MUST_SURVIVE_OUTER_CLEANUP");
                        }
                    }
                }
            }
        }
        false
    }

    unsafe {
        REPLACED_SENTINEL_PATH = Some(out_dir.clone());
    }
    let opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 64,
        progress_callback: Some(cancel_after_swapping_inner_staged_file),
        user_data: std::ptr::null_mut(),
    };

    let res = create_archive_detailed(std::slice::from_ref(&src_file), &target_zip, &opts, 0);
    let Err(CreateStageCommitError::TransactionIncomplete(ref report)) = res else {
        panic!("Expected TransactionIncomplete when inner sentinel replaced staged file, got: {res:?}");
    };
    let sentinel_on_disk = report.stage_dir.join("c1_target.zip");
    assert!(
        sentinel_on_disk.exists(),
        "Outer CreateStagingGuard must NOT delete inner preserved sentinel at {}",
        sentinel_on_disk.display()
    );
    assert_eq!(
        fs::read(&sentinel_on_disk).unwrap(),
        b"INNER_SENTINEL_MUST_SURVIVE_OUTER_CLEANUP"
    );
}

#[test]
fn test_c2_preexisting_file_before_libarchive_early_failure_must_not_be_deleted() {
    let tmp = tempfile::tempdir().unwrap();
    let preexisting = tmp.path().join("preexisting_user_archive.tar");
    fs::write(&preexisting, b"PREEXISTING_USER_FILE_BYTES").unwrap();

    let bad_opts = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Brotli,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 1,
        solid_block_size_mb: 64,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let res = run_libarchive_staged_write(
        &preexisting,
        TTZipArchiveFormat::Brotli,
        &bad_opts,
        false,
        0,
        |_| Ok(()),
    );
    assert!(
        res.is_err(),
        "run_libarchive_staged_write must fail when format is unsupported and pre-existing file is refused cleanup"
    );
    assert!(
        preexisting.exists(),
        "Pre-existing file at staged_dest must NOT be adopted and deleted when run_libarchive_staged_write fails before creation"
    );
    assert_eq!(
        fs::read(&preexisting).unwrap(),
        b"PREEXISTING_USER_FILE_BYTES"
    );
    let (recorded_path, recorded_err) =
        ttzip_archive::archive::unified::create_staging::take_last_inner_stage_cleanup_error()
            .expect("inner cleanup refusal on pre-existing file must be recorded");
    assert_eq!(recorded_path, preexisting);
    assert!(recorded_err.contains("untracked or pre-existing file occupies staged path"));

    // Also verify when configure_archive_write_format succeeds (Tar), run_libarchive_staged_write
    // refuses to overwrite/truncate or delete the pre-existing file at staged_dest.
    let mut tar_opts = bad_opts;
    tar_opts.format = TTZipArchiveFormat::Tar;
    let res_tar = run_libarchive_staged_write(
        &preexisting,
        TTZipArchiveFormat::Tar,
        &tar_opts,
        false,
        0,
        |_| Ok(()),
    );
    assert!(res_tar.is_err());
    assert_eq!(
        fs::read(&preexisting).unwrap(),
        b"PREEXISTING_USER_FILE_BYTES",
        "Pre-existing file must not be truncated or deleted even when format configuration succeeds"
    );
    let _ = ttzip_archive::archive::unified::create_staging::take_last_inner_stage_cleanup_error();

    // Verify FileAbortGuard::from_created_file binds directly to created File fd and detects replacement before bind
    let bound_path = tmp.path().join("bound_created.bin");
    let created_f = fs::File::create(&bound_path).unwrap();
    fs::remove_file(&bound_path).unwrap();
    fs::write(&bound_path, b"REPLACEMENT_BEFORE_GUARD_BIND").unwrap();
    let replaced_guard = FileAbortGuard::from_created_file(&bound_path, &created_f);
    assert!(replaced_guard.abort_checked().is_err());
    assert_eq!(fs::read(&bound_path).unwrap(), b"REPLACEMENT_BEFORE_GUARD_BIND");
}
