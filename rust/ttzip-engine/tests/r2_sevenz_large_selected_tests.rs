// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! R2 regression test suite for 7z selected large file extraction to disk.
//!
//! Verifies:
//! 1. 70 MiB and 256 MiB selected-entry disk extraction succeeds with identical SHA-256.
//! 2. In-memory return interface (`extract_single_entry_memory`) strictly rejects entries > 64 MiB.
//! 3. Selective extraction of head, middle, and tail small files from a solid 7z folder.
//! 4. Encrypted archives with wrong password and corrupted/truncated streams clean up safely.
//! 5. Phased cancellation tests: during preceding discard phase and during mid-stream write.
//! 6. Decompression quota enforcement across discarded and written bytes (trip breaker), plus the
//!    compression-ratio breaker with a custom low ceiling while the byte budget stays untouched
//!    (discard phase and write phase), and the metered compressed denominator being charged.
//! 7. Sink write failure propagation and automatic staging cleanup.

use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;

use ttzip_engine::archive::stream_adapter::extract_sevenz_selected_from_reader_with_quota;
use ttzip_engine::archive::unified::selection::EntrySelection;
use ttzip_engine::archive::unified::{extract_selected_entries, extract_single_entry_memory};
use ttzip_engine::fs::collision::staging::StagingRoot;
use ttzip_engine::security::zip_defense::ExtractionQuotaGuard;
use ttzip_engine::sevenz::{
    create_7z_solid_archive_bytes, create_7z_solid_archive_bytes_with_password,
};
use ttzip_engine::types::{TTZipExtractOptions, TTZipStatus};
use ttzip_engine::zip::writer::ZipInputItem;

fn calculate_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn calculate_file_sha256(path: &Path) -> (u64, String) {
    let bytes = fs::read(path).expect("read extracted file");
    let len = bytes.len() as u64;
    (len, calculate_sha256(&bytes))
}

fn generate_test_payload(size: usize, pattern_id: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut block = [0u8; 65536];
    for (i, b) in block.iter_mut().enumerate() {
        *b = ((i as u8).wrapping_mul(31)).wrapping_add(pattern_id);
    }
    while out.len() < size {
        let rem = size - out.len();
        let chunk = rem.min(block.len());
        out.extend_from_slice(&block[..chunk]);
    }
    out
}

fn make_input_item(name: &str, data: Vec<u8>) -> ZipInputItem {
    ZipInputItem {
        rel_path: name.to_string(),
        data,
        mtime_epoch_secs: 1700000000,
        mode: 0o644,
        is_directory: false,
    }
}

fn write_archive_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write test archive");
    path
}

fn run_selected_extraction(
    archive: &Path,
    selected_name: &str,
    staging_dir: &Path,
    password: Option<&str>,
) -> Result<usize, TTZipStatus> {
    let staging = StagingRoot(staging_dir.to_path_buf());
    let selection = EntrySelection::new([selected_name], false);

    let pwd_c = password.and_then(|p| CString::new(p).ok());
    let opts = TTZipExtractOptions {
        preserve_permissions: true,
        password: pwd_c
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null()),
        ..Default::default()
    };

    extract_selected_entries(archive, &selection, &staging, &opts)
}

fn run_selected_extraction_with_guard(
    archive: &Path,
    selected_name: &str,
    staging_dir: &Path,
    guard: ExtractionQuotaGuard,
) -> Result<usize, TTZipStatus> {
    let staging = StagingRoot(staging_dir.to_path_buf());
    let selection = EntrySelection::new([selected_name], false);
    let opts = TTZipExtractOptions::default();
    let mut file = fs::File::open(archive).expect("open archive");
    extract_sevenz_selected_from_reader_with_quota(
        &mut file,
        &selection,
        &staging,
        &opts,
        None,
        Some(guard),
    )
}

#[test]
fn test_r2_selected_large_70mib_streaming_extract_and_memory_rejection() {
    let work = tempdir().expect("tempdir");
    let size_70m = 70 * 1024 * 1024;
    let payload_70m = generate_test_payload(size_70m, 0xA1);
    let expected_sha = calculate_sha256(&payload_70m);

    let items = [make_input_item("large_70m.bin", payload_70m)];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create 7z archive");
    let archive_path = write_archive_file(work.path(), "large_70m.7z", &archive_bytes);

    // Contract 1: In-memory return interface MUST strictly reject > 64 MiB payload
    let mem_res = extract_single_entry_memory(&archive_path, Some("large_70m.bin"), -1, None);
    assert_eq!(
        mem_res.err(),
        Some(TTZipStatus::ErrOutOfMemory),
        "Memory return interface must enforce 64 MiB limit"
    );

    // Contract 2: Streaming disk extraction MUST succeed with identical SHA-256
    let staging_dir = work.path().join("staging_70m");
    let count = run_selected_extraction(&archive_path, "large_70m.bin", &staging_dir, None)
        .expect("streaming extraction of 70 MiB entry to staging must succeed");
    assert_eq!(count, 1);

    let extracted_file = staging_dir.join("large_70m.bin");
    assert!(extracted_file.exists(), "Target file must exist in staging");
    let (disk_len, disk_sha) = calculate_file_sha256(&extracted_file);
    assert_eq!(disk_len, size_70m as u64);
    assert_eq!(disk_sha, expected_sha);
}

#[test]
fn test_r2_selected_large_256mib_streaming_extract_and_memory_rejection() {
    let work = tempdir().expect("tempdir");
    let size_256m = 256 * 1024 * 1024;
    let payload_256m = generate_test_payload(size_256m, 0xB2);
    let expected_sha = calculate_sha256(&payload_256m);

    let items = [make_input_item("large_256m.bin", payload_256m)];
    let archive_bytes =
        create_7z_solid_archive_bytes(&items, 1, 1).expect("create 256M 7z archive");
    let archive_path = write_archive_file(work.path(), "large_256m.7z", &archive_bytes);

    // Contract 1: In-memory return interface MUST strictly reject > 64 MiB payload
    let mem_res = extract_single_entry_memory(&archive_path, Some("large_256m.bin"), -1, None);
    assert_eq!(
        mem_res.err(),
        Some(TTZipStatus::ErrOutOfMemory),
        "Memory return interface must reject 256 MiB payload"
    );

    // Contract 2: Streaming disk extraction MUST succeed with identical SHA-256
    let staging_dir = work.path().join("staging_256m");
    let count = run_selected_extraction(&archive_path, "large_256m.bin", &staging_dir, None)
        .expect("streaming extraction of 256 MiB entry must succeed");
    assert_eq!(count, 1);

    let extracted_file = staging_dir.join("large_256m.bin");
    assert!(extracted_file.exists(), "Extracted file must exist");
    let (disk_len, disk_sha) = calculate_file_sha256(&extracted_file);
    assert_eq!(disk_len, size_256m as u64);
    assert_eq!(disk_sha, expected_sha);
}

#[test]
fn test_r2_solid_head_middle_tail_selective_extraction() {
    let work = tempdir().expect("tempdir");
    let head_data = b"HEAD_DATA_0123456789_ALPHA".to_vec();
    let mid_data = b"MIDDLE_DATA_9876543210_BETA".to_vec();
    let tail_data = b"TAIL_DATA_STAGED_DISPATCH_GAMMA".to_vec();

    let items = [
        make_input_item("head.txt", head_data.clone()),
        make_input_item("mid.txt", mid_data.clone()),
        make_input_item("tail.txt", tail_data.clone()),
    ];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create solid archive");
    let archive_path = write_archive_file(work.path(), "solid_three.7z", &archive_bytes);

    // 1. Extract Head only
    let stage_head = work.path().join("stage_head");
    let count_head = run_selected_extraction(&archive_path, "head.txt", &stage_head, None).unwrap();
    assert_eq!(count_head, 1);
    assert_eq!(fs::read(stage_head.join("head.txt")).unwrap(), head_data);
    assert!(!stage_head.join("mid.txt").exists());
    assert!(!stage_head.join("tail.txt").exists());

    // 2. Extract Middle only (head discarded)
    let stage_mid = work.path().join("stage_mid");
    let count_mid = run_selected_extraction(&archive_path, "mid.txt", &stage_mid, None).unwrap();
    assert_eq!(count_mid, 1);
    assert_eq!(fs::read(stage_mid.join("mid.txt")).unwrap(), mid_data);
    assert!(!stage_mid.join("head.txt").exists());
    assert!(!stage_mid.join("tail.txt").exists());

    // 3. Extract Tail only (head + mid discarded)
    let stage_tail = work.path().join("stage_tail");
    let count_tail = run_selected_extraction(&archive_path, "tail.txt", &stage_tail, None).unwrap();
    assert_eq!(count_tail, 1);
    assert_eq!(fs::read(stage_tail.join("tail.txt")).unwrap(), tail_data);
    assert!(!stage_tail.join("head.txt").exists());
    assert!(!stage_tail.join("mid.txt").exists());

    // 4. Extract Head and Tail (mid skipped)
    let stage_combo = work.path().join("stage_combo");
    let staging = StagingRoot(stage_combo.clone());
    let selection = EntrySelection::new(["head.txt", "tail.txt"], false);
    let opts = TTZipExtractOptions::default();
    let count_combo = extract_selected_entries(&archive_path, &selection, &staging, &opts).unwrap();
    assert_eq!(count_combo, 2);
    assert_eq!(fs::read(stage_combo.join("head.txt")).unwrap(), head_data);
    assert_eq!(fs::read(stage_combo.join("tail.txt")).unwrap(), tail_data);
    assert!(!stage_combo.join("mid.txt").exists());
}

#[test]
fn test_r2_encrypted_large_archive_wrong_password_and_protection() {
    let work = tempdir().expect("tempdir");
    let payload = generate_test_payload(70 * 1024 * 1024, 0xC3);
    let expected_sha = calculate_sha256(&payload);
    let items = [make_input_item("secret_70m.dat", payload)];

    let password = "CorrectPassphrase99";
    let archive_bytes = create_7z_solid_archive_bytes_with_password(&items, 1, 1, Some(password))
        .expect("create encrypted archive");
    let archive_path = write_archive_file(work.path(), "secret.7z", &archive_bytes);

    // Wrong password test
    let stage_err = work.path().join("stage_err");
    let err_res = run_selected_extraction(
        &archive_path,
        "secret_70m.dat",
        &stage_err,
        Some("WrongPass"),
    );
    assert!(
        matches!(err_res, Err(TTZipStatus::ErrInvalidPassword)),
        "Wrong password must result in ErrInvalidPassword"
    );
    assert!(
        !stage_err.join("secret_70m.dat").exists(),
        "No partial file leaked on password error"
    );

    // Correct password test
    let stage_ok = work.path().join("stage_ok");
    let ok_count =
        run_selected_extraction(&archive_path, "secret_70m.dat", &stage_ok, Some(password))
            .expect("extract with correct password must succeed");
    assert_eq!(ok_count, 1);
    let (disk_len, disk_sha) = calculate_file_sha256(&stage_ok.join("secret_70m.dat"));
    assert_eq!(disk_len, 70 * 1024 * 1024);
    assert_eq!(disk_sha, expected_sha);
}

#[test]
fn test_r2_truncated_archive_safe_cleanup() {
    let work = tempdir().expect("tempdir");
    let payload = generate_test_payload(70 * 1024 * 1024, 0xD4);
    let items = [make_input_item("truncated.bin", payload)];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create archive");

    let trunc_len = archive_bytes.len().saturating_sub(1024);
    let trunc_path = write_archive_file(work.path(), "trunc.7z", &archive_bytes[..trunc_len]);

    let stage_trunc = work.path().join("stage_trunc");
    let trunc_res = run_selected_extraction(&trunc_path, "truncated.bin", &stage_trunc, None);
    assert!(trunc_res.is_err(), "Truncated stream must fail extraction");
    assert!(
        !stage_trunc.join("truncated.bin").exists(),
        "No uncommitted partial file left on truncation"
    );
}

static DISCARD_CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn cancel_on_discard(
    _current: u64,
    _total: u64,
    item: *const std::ffi::c_char,
    _user: *mut std::ffi::c_void,
) -> bool {
    // When item is null, we are in the preceding discard window
    if item.is_null() {
        DISCARD_CALLBACK_COUNT.fetch_add(1, Ordering::SeqCst);
        return false; // Trigger cancel during discard phase
    }
    true
}

#[test]
fn test_r2_cancellation_during_preceding_discard_phase() {
    let work = tempdir().expect("tempdir");
    DISCARD_CALLBACK_COUNT.store(0, Ordering::SeqCst);

    let prefix_data = generate_test_payload(4 * 1024 * 1024, 0xE5);
    let target_data = b"TARGET_SELECTED_PAYLOAD_FINAL".to_vec();

    let items = [
        make_input_item("prefix_skip.bin", prefix_data),
        make_input_item("target_selected.txt", target_data),
    ];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create solid archive");
    let archive_path = write_archive_file(work.path(), "discard_cancel.7z", &archive_bytes);

    let stage_dir = work.path().join("stage_discard_cancel");
    let staging = StagingRoot(stage_dir.clone());
    let selection = EntrySelection::new(["target_selected.txt"], false);

    let cancel_opts = TTZipExtractOptions {
        progress_callback: Some(cancel_on_discard),
        ..Default::default()
    };

    let res = extract_selected_entries(&archive_path, &selection, &staging, &cancel_opts);
    assert_eq!(
        res.err(),
        Some(TTZipStatus::Cancelled),
        "Cancellation during preceding data discard must return Cancelled"
    );
    assert!(
        DISCARD_CALLBACK_COUNT.load(Ordering::SeqCst) >= 1,
        "Ensures callback was triggered during discard phase"
    );
    assert!(
        !stage_dir.join("target_selected.txt").exists(),
        "Target must not exist when cancelled during discard"
    );
}

static WRITTEN_CHUNK_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn cancel_after_two_megabytes(
    current: u64,
    _total: u64,
    item: *const std::ffi::c_char,
    _user: *mut std::ffi::c_void,
) -> bool {
    if !item.is_null() {
        WRITTEN_CHUNK_BYTES.store(current as usize, Ordering::SeqCst);
        if current >= 2 * 1024 * 1024 {
            return false; // Cancel after at least 2 MiB written out
        }
    }
    true
}

#[test]
fn test_r2_cancellation_during_mid_stream_write_phase() {
    let work = tempdir().expect("tempdir");
    WRITTEN_CHUNK_BYTES.store(0, Ordering::SeqCst);

    let payload = generate_test_payload(8 * 1024 * 1024, 0xF6);
    let items = [make_input_item("write_cancel.bin", payload)];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create archive");
    let archive_path = write_archive_file(work.path(), "write_cancel.7z", &archive_bytes);

    let stage_dir = work.path().join("stage_write_cancel");
    let staging = StagingRoot(stage_dir.clone());
    let selection = EntrySelection::new(["write_cancel.bin"], false);

    let cancel_opts = TTZipExtractOptions {
        progress_callback: Some(cancel_after_two_megabytes),
        ..Default::default()
    };

    let res = extract_selected_entries(&archive_path, &selection, &staging, &cancel_opts);
    assert_eq!(
        res.err(),
        Some(TTZipStatus::Cancelled),
        "Cancellation mid-stream must return Cancelled"
    );
    assert!(
        WRITTEN_CHUNK_BYTES.load(Ordering::SeqCst) >= 2 * 1024 * 1024,
        "Ensures writing phase was active before cancelling"
    );

    // Verify uncommitted inflight staging files were removed
    let entries: Vec<_> = fs::read_dir(&stage_dir)
        .map(|rd| rd.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    assert!(
        entries.is_empty(),
        "Staging directory must be completely clean with 0 leftover files"
    );
}

#[test]
fn test_r2_sink_write_failure_propagation_and_cleanup() {
    let work = tempdir().expect("tempdir");
    let payload = b"SAMPLE_PAYLOAD_FOR_FAIL".to_vec();
    let items = [make_input_item("subdir/file.bin", payload)];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create archive");
    let archive_path = write_archive_file(work.path(), "sink_fail.7z", &archive_bytes);

    let stage_dir = work.path().join("stage_sink_fail");
    fs::create_dir_all(&stage_dir).expect("create staging root");

    // Pre-create regular file with the same name as the subfolder to force open/create failure
    fs::write(stage_dir.join("subdir"), b"CONFLICTING_FILE").expect("write blocker file");

    let staging = StagingRoot(stage_dir.clone());
    let selection = EntrySelection::new(["subdir/file.bin"], false);
    let opts = TTZipExtractOptions::default();

    let res = extract_selected_entries(&archive_path, &selection, &staging, &opts);
    assert!(
        res.is_err(),
        "Sink directory/file creation failure must be propagated"
    );

    // Ensure the blocker file was untouched and no partial payload created
    assert!(stage_dir.join("subdir").is_file());
    assert!(!stage_dir.join("subdir/file.bin").exists());
}

#[test]
fn test_r2_discard_and_write_quota_enforcement() {
    let work = tempdir().expect("tempdir");
    let prefix_payload = generate_test_payload(3 * 1024 * 1024, 0x11);
    let target_payload = b"QUOTA_TARGET_TEST".to_vec();

    let items = [
        make_input_item("waste_prefix.bin", prefix_payload),
        make_input_item("quota_target.txt", target_payload),
    ];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create solid archive");
    let archive_path = write_archive_file(work.path(), "quota_test.7z", &archive_bytes);

    let stage_dir = work.path().join("stage_quota");
    let staging = StagingRoot(stage_dir.clone());
    let selection = EntrySelection::new(["quota_target.txt"], false);
    let opts = TTZipExtractOptions::default();

    // Set a strict 1 MiB quota. The preceding discarded file is 3 MiB, so it must trip the breaker
    let strict_quota = ExtractionQuotaGuard::new(1024 * 1024, 50000.0);

    let mut file = fs::File::open(&archive_path).expect("open archive");
    let res = extract_sevenz_selected_from_reader_with_quota(
        &mut file,
        &selection,
        &staging,
        &opts,
        None,
        Some(strict_quota),
    );

    assert_eq!(
        res.err(),
        Some(TTZipStatus::ErrSecurityViolation),
        "Decompressed data exceeding quota during discard must trip ErrSecurityViolation"
    );
    assert!(
        !stage_dir.join("quota_target.txt").exists(),
        "No output produced when quota tripped during discard"
    );
}

/// Ratio breaker must fire on discarded solid-block prefix bytes even though the absolute
/// byte budget is far from being exhausted.
#[test]
fn test_r2_quota_ratio_rejects_during_discard_phase() {
    let work = tempdir().expect("tempdir");
    let prefix_payload = generate_test_payload(4 * 1024 * 1024, 0x21);
    let target_payload = b"RATIO_TARGET_AFTER_DISCARD".to_vec();

    let items = [
        make_input_item("ratio_prefix.bin", prefix_payload),
        make_input_item("ratio_target.txt", target_payload),
    ];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create solid archive");
    let archive_path = write_archive_file(work.path(), "ratio_discard.7z", &archive_bytes);

    let stage_dir = work.path().join("stage_ratio_discard");
    // 1 GiB byte budget is never approached: only the ~4 MiB prefix is decoded before the
    // ratio ceiling (2:1) must reject the archive.
    let guard = ExtractionQuotaGuard::new(1024 * 1024 * 1024, 2.0);
    let res =
        run_selected_extraction_with_guard(&archive_path, "ratio_target.txt", &stage_dir, guard);

    assert_eq!(
        res.err(),
        Some(TTZipStatus::ErrSecurityViolation),
        "Compression ratio breach while discarding the solid prefix must be rejected \
         even though decoded bytes stay far below max_uncompressed_bytes"
    );
    assert!(
        !stage_dir.join("ratio_target.txt").exists(),
        "Target must never be created when the ratio breaker trips during the discard phase"
    );
}

/// Ratio breaker must also fire while payload bytes are being written to staging.
#[test]
fn test_r2_quota_ratio_rejects_during_write_phase() {
    let work = tempdir().expect("tempdir");
    let payload = generate_test_payload(8 * 1024 * 1024, 0x22);
    let items = [make_input_item("ratio_write.bin", payload)];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 1, 1).expect("create archive");
    let archive_path = write_archive_file(work.path(), "ratio_write.7z", &archive_bytes);

    let stage_dir = work.path().join("stage_ratio_write");
    fs::create_dir_all(&stage_dir).expect("create staging root");
    let guard = ExtractionQuotaGuard::new(1024 * 1024 * 1024, 2.0);
    let res =
        run_selected_extraction_with_guard(&archive_path, "ratio_write.bin", &stage_dir, guard);

    assert_eq!(
        res.err(),
        Some(TTZipStatus::ErrSecurityViolation),
        "Compression ratio breach while writing the target must be rejected \
         even though decoded bytes stay far below max_uncompressed_bytes"
    );
    assert!(
        !stage_dir.join("ratio_write.bin").exists(),
        "Partially written target must not survive a ratio breach"
    );
    let leftovers: Vec<_> = fs::read_dir(&stage_dir)
        .map(|rd| rd.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "No uncommitted staging file may be left behind after a ratio breach"
    );
}

/// The ratio denominator must be the folder's metered compressed input, not an implicit zero.
///
/// Both archives are extracted with the same guard (same byte budget, same 2:1 ceiling). The
/// COPY-coded archive has a true ratio of 1.0 and must be accepted; the LZMA2-coded archive of
/// the same size has a far higher ratio and must be rejected. If the compressed counter were
/// never charged, the COPY archive would be rejected as well (2 MiB / 1 byte >> 2:1).
#[test]
fn test_r2_quota_ratio_uses_metered_compressed_input() {
    let work = tempdir().expect("tempdir");
    let payload = generate_test_payload(2 * 1024 * 1024, 0x23);
    let expected_sha = calculate_sha256(&payload);

    // level 0 selects the COPY coder: packed_len == uncompressed_len, ratio == 1.0
    let copy_items = [make_input_item("copy_2m.bin", payload.clone())];
    let copy_bytes = create_7z_solid_archive_bytes(&copy_items, 0, 1).expect("create COPY archive");
    let copy_path = write_archive_file(work.path(), "copy_2m.7z", &copy_bytes);

    let copy_stage = work.path().join("stage_ratio_copy");
    let copy_guard = ExtractionQuotaGuard::new(1024 * 1024 * 1024, 2.0);
    let copy_res =
        run_selected_extraction_with_guard(&copy_path, "copy_2m.bin", &copy_stage, copy_guard);
    assert_eq!(
        copy_res,
        Ok(1),
        "COPY-coded entry has a 1:1 ratio and must not be rejected by the ratio breaker"
    );
    let (copy_len, copy_sha) = calculate_file_sha256(&copy_stage.join("copy_2m.bin"));
    assert_eq!(copy_len, 2 * 1024 * 1024);
    assert_eq!(copy_sha, expected_sha);

    // Same payload through LZMA2: same byte budget, only the ratio differs.
    let lzma_items = [make_input_item("lzma_2m.bin", payload)];
    let lzma_bytes =
        create_7z_solid_archive_bytes(&lzma_items, 1, 1).expect("create LZMA2 archive");
    let lzma_path = write_archive_file(work.path(), "lzma_2m.7z", &lzma_bytes);

    let lzma_stage = work.path().join("stage_ratio_lzma");
    let lzma_guard = ExtractionQuotaGuard::new(1024 * 1024 * 1024, 2.0);
    let lzma_res =
        run_selected_extraction_with_guard(&lzma_path, "lzma_2m.bin", &lzma_stage, lzma_guard);
    assert_eq!(
        lzma_res.err(),
        Some(TTZipStatus::ErrSecurityViolation),
        "LZMA2-coded entry of the same size exceeds the 2:1 ceiling and must be rejected"
    );
    assert!(!lzma_stage.join("lzma_2m.bin").exists());
}

/// The production default entry must charge compressed input to the quota guard it builds itself.
///
/// `extract_selected_entries` supplies `ExtractionQuotaGuard::default_limits()` (50,000:1,
/// 10 GiB, 1 MiB warm-up). A 2 MiB COPY-coded entry has a true ratio of 1.0 and must succeed.
/// If the production path never metered the folder's packed length, the same guard would compute
/// 2 MiB / 1 byte = 2,097,152 > 50,000 and reject a perfectly ordinary archive.
#[test]
fn test_r2_production_entry_default_guard_meters_compressed_input() {
    let work = tempdir().expect("tempdir");
    let payload = generate_test_payload(2 * 1024 * 1024, 0x24);
    let expected_sha = calculate_sha256(&payload);

    let items = [make_input_item("prod_copy_2m.bin", payload)];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 0, 1).expect("create COPY archive");
    let archive_path = write_archive_file(work.path(), "prod_copy_2m.7z", &archive_bytes);

    let stage_dir = work.path().join("stage_prod_default_guard");
    let count = run_selected_extraction(&archive_path, "prod_copy_2m.bin", &stage_dir, None)
        .expect("production entry must accept a 1:1 archive under default limits");
    assert_eq!(count, 1);

    let (disk_len, disk_sha) = calculate_file_sha256(&stage_dir.join("prod_copy_2m.bin"));
    assert_eq!(disk_len, 2 * 1024 * 1024);
    assert_eq!(disk_sha, expected_sha);
}

/// Builds two COPY folders, with two streams in the first and one in the second.
fn create_two_folder_copy_fixture(zero_first_pack: bool) -> Vec<u8> {
    use ttzip_engine::crypto::crc32::crc32_fast;
    use ttzip_engine::sevenz::{format::*, SevenZSignatureHeader};
    let entry_len = 2 * 1024 * 1024;
    let mut header = vec![K_HEADER, K_MAIN_STREAMS_INFO, K_PACK_INFO];
    write_varint(0, &mut header);
    write_varint(2, &mut header);
    header.push(K_SIZE);
    write_varint(if zero_first_pack { 0 } else { 2 * entry_len }, &mut header);
    write_varint(entry_len, &mut header);
    header.extend_from_slice(&[K_END, K_UNPACK_INFO, K_FOLDER]);
    write_varint(2, &mut header);
    header.push(0); // Inline folder descriptions.
    for _ in 0..2 {
        write_varint(1, &mut header);
        header.extend_from_slice(&[0x01, 0x00]); // One COPY coder.
    }
    header.push(K_CODERS_UNPACK_SIZE);
    write_varint(2 * entry_len, &mut header);
    write_varint(entry_len, &mut header);
    header.extend_from_slice(&[K_END, K_SUB_STREAMS_INFO, K_NUM_UNPACK_STREAM]);
    write_varint(2, &mut header);
    write_varint(1, &mut header);
    header.push(K_SIZE);
    write_varint(entry_len, &mut header);
    header.extend_from_slice(&[K_END, K_END, K_FILES_INFO]);
    write_varint(3, &mut header);
    let mut names = vec![0]; // Inline UTF-16 names.
    for name in ["first.bin", "second.bin", "third.bin"] {
        for unit in name.encode_utf16().chain(std::iter::once(0)) {
            names.extend_from_slice(&unit.to_le_bytes());
        }
    }
    header.push(K_NAME);
    write_varint(names.len() as u64, &mut header);
    header.extend_from_slice(&names);
    header.extend_from_slice(&[K_END, K_END]);
    let payload = generate_test_payload((3 * entry_len) as usize, 0x31);
    let signature = SevenZSignatureHeader {
        major_version: 0,
        minor_version: 4,
        start_header_crc: 0,
        next_header_offset: payload.len() as u64,
        next_header_size: header.len() as u64,
        next_header_crc: crc32_fast(0, &header),
    };
    let mut archive = signature.serialize().to_vec();
    archive.extend_from_slice(&payload);
    archive.extend_from_slice(&header);
    archive
}

#[test]
fn test_r2_quota_accumulates_across_folders_without_duplicate_denominator() {
    let work = tempdir().expect("tempdir");
    let bytes = create_two_folder_copy_fixture(false);
    let archive = ttzip_engine::sevenz::SevenZArchive::open_slice(&bytes).expect("parse fixture");
    assert_eq!(archive.solid_index().folder_count(), 2);
    assert_eq!(archive.len(), 3);
    let selection = EntrySelection::new(["first.bin", "second.bin", "third.bin"], false);
    let opts = TTZipExtractOptions::default();
    for (name, guard, expected) in [
        ("accepted", ExtractionQuotaGuard::new(8 << 20, 1.1), Ok(3)),
        (
            "total_limit",
            ExtractionQuotaGuard::new(5 << 20, 1.1),
            Err(TTZipStatus::ErrSecurityViolation),
        ),
        (
            "ratio_limit",
            ExtractionQuotaGuard::new(8 << 20, 0.75),
            Err(TTZipStatus::ErrSecurityViolation),
        ),
    ] {
        let stage = work.path().join(name);
        let result = extract_sevenz_selected_from_reader_with_quota(
            &mut std::io::Cursor::new(&bytes),
            &selection,
            &StagingRoot(stage.clone()),
            &opts,
            None,
            Some(guard),
        );
        assert_eq!(result, expected, "scenario {name}");
        let names = ["first.bin", "second.bin", "third.bin"];
        let expected_files = match name {
            "accepted" => 3,
            "total_limit" => 2,
            _ => 0,
        };
        let expected_sha = calculate_sha256(&generate_test_payload(2 << 20, 0x31));
        for (index, file) in names.iter().enumerate() {
            if index < expected_files {
                let (len, sha) = calculate_file_sha256(&stage.join(file));
                assert_eq!(len, 2 << 20);
                assert_eq!(sha, expected_sha);
            } else {
                assert!(
                    !stage.join(file).exists(),
                    "{name}: {file} must not survive"
                );
            }
        }
        assert_eq!(
            fs::read_dir(stage).expect("staging").count(),
            expected_files
        );
    }
}

#[test]
fn test_r2_nonempty_folder_with_zero_packed_length_is_rejected() {
    let work = tempdir().expect("tempdir");
    let bytes = create_two_folder_copy_fixture(true);
    let stage = work.path().join("zero_pack");
    let result = extract_sevenz_selected_from_reader_with_quota(
        &mut std::io::Cursor::new(bytes),
        &EntrySelection::new(["first.bin"], false),
        &StagingRoot(stage.clone()),
        &TTZipExtractOptions::default(),
        None,
        None,
    );
    assert_eq!(result, Err(TTZipStatus::ErrCorruptHeader));
    assert!(!stage.exists() || fs::read_dir(stage).expect("staging").count() == 0);
}

#[test]
fn test_r2_quota_charges_decoded_same_chunk_tail_after_selected_entry() {
    let work = tempdir().expect("tempdir");
    let bytes = create_7z_solid_archive_bytes(
        &[
            make_input_item("tiny.bin", vec![42]),
            make_input_item("unselected.bin", vec![7; 128 * 1024]),
        ],
        0,
        1,
    )
    .expect("create COPY archive");
    let archive = write_archive_file(work.path(), "same_chunk_tail.7z", &bytes);
    let stage = work.path().join("same_chunk_tail");
    // A one-byte target fits, but the COPY decoder has already produced a larger callback block.
    let result = run_selected_extraction_with_guard(
        &archive,
        "tiny.bin",
        &stage,
        ExtractionQuotaGuard::new(1, 50000.0),
    );
    assert_eq!(result, Err(TTZipStatus::ErrSecurityViolation));
    assert!(!stage.join("tiny.bin").exists());
    assert_eq!(fs::read_dir(stage).expect("staging").count(), 0);
}
