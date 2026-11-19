// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive creation lifecycle submodule for Unified Orchestrator.

use std::fs;
use std::path::{Path, PathBuf};

use super::create_dispatch::{
    write_staged_explicit_archive_in_txn, write_staged_source_archive_in_txn,
};
pub use super::create_staging::{
    commit_staged_archive_files_in_txn_with, commit_staged_archive_files_with,
    recover_preserved_create_backup, with_staged_archive_destination,
    with_staged_archive_destination_detailed, with_staged_archive_destination_detailed_outcome,
    with_staged_archive_destination_detailed_using,
    with_staged_archive_destination_detailed_using_outcome, with_staged_archive_destination_using,
    CreateCommitMode, CreateStageCommitError, CreateStagedCommitOutput,
};
pub use super::create_to_writer::{
    create_archive_to_writer, run_libarchive_staged_write, run_libarchive_staged_write_in_txn,
};
use super::create_transaction::{
    ArchiveCleanupDisposition, CreateTransactionContext, ResidualPathRole, StageCleanupFailure,
};
use crate::types::{TTZipArchiveFormat, TTZipCreateOptions, TTZipStatus};

#[inline]
pub(crate) fn is_pure_single_stream_format(fmt: TTZipArchiveFormat) -> bool {
    matches!(
        fmt,
        TTZipArchiveFormat::Gzip
            | TTZipArchiveFormat::Bzip2
            | TTZipArchiveFormat::Xz
            | TTZipArchiveFormat::Zstd
            | TTZipArchiveFormat::Lz4
            | TTZipArchiveFormat::Brotli
            | TTZipArchiveFormat::Snappy
            | TTZipArchiveFormat::Lzip
            | TTZipArchiveFormat::Lrzip
    )
}

#[inline]
fn is_single_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_file() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

#[inline]
pub(crate) fn should_use_raw_stream(
    source_paths: &[PathBuf],
    resolved_format: TTZipArchiveFormat,
) -> bool {
    source_paths.len() == 1
        && is_single_regular_file(&source_paths[0])
        && is_pure_single_stream_format(resolved_format)
}

#[inline]
pub(super) fn should_use_raw_stream_explicit(
    entries: &[(PathBuf, String)],
    resolved_format: TTZipArchiveFormat,
) -> bool {
    entries.len() == 1
        && is_single_regular_file(&entries[0].0)
        && is_pure_single_stream_format(resolved_format)
}

fn prepare_staged_create(
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    is_explicit: bool,
    encrypt_file_names: Option<bool>,
    raw_stream_ok: impl FnOnce(TTZipArchiveFormat) -> bool,
) -> Result<(TTZipArchiveFormat, CreateCommitMode), CreateStageCommitError> {
    let resolved_format =
        super::format_config::effective_create_format(options.format, destination_path);
    super::format_config::validate_creatable_format(resolved_format, is_explicit)
        .map_err(CreateStageCommitError::Status)?;
    super::create_staging::validate_create_encryption_options(
        resolved_format,
        options,
        encrypt_file_names,
    )
    .map_err(CreateStageCommitError::Status)?;
    if is_pure_single_stream_format(resolved_format) && !raw_stream_ok(resolved_format) {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    Ok((
        resolved_format,
        CreateCommitMode::for_creation(destination_path, resolved_format, split_volume_size_bytes),
    ))
}

/// Recursively compresses source paths into the destination archive path with a cancellation probe,
/// returning `CreateStagedCommitOutput` or `CreateStageCommitError`.
pub fn create_archive_detailed_outcome_with_cancel<C>(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    cancel_probe: C,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    C: FnOnce() -> bool,
{
    if source_paths.is_empty() {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    for src_path in source_paths {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(CreateStageCommitError::Status(TTZipStatus::ErrFileNotFound));
        }
    }
    let (resolved_format, commit_mode) = prepare_staged_create(
        destination_path,
        options,
        split_volume_size_bytes,
        false,
        None,
        |fmt| should_use_raw_stream(source_paths, fmt),
    )?;
    with_staged_archive_destination_detailed_outcome(
        destination_path,
        commit_mode,
        |staged_dest, txn_ctx| {
            write_staged_source_archive_in_txn(
                source_paths,
                staged_dest,
                resolved_format,
                options,
                split_volume_size_bytes,
                Some(txn_ctx),
            )
        },
        cancel_probe,
    )
}

/// Recursively compresses source paths into the destination archive path, returning `CreateStagedCommitOutput`.
pub fn create_archive_detailed_outcome(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError> {
    create_archive_detailed_outcome_with_cancel(
        source_paths,
        destination_path,
        options,
        split_volume_size_bytes,
        || false,
    )
}

/// Recursively compresses source paths into the destination archive path with detailed rollback diagnostics.
pub fn create_archive_detailed(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    create_archive_detailed_outcome(
        source_paths,
        destination_path,
        options,
        split_volume_size_bytes,
    )
    .map(|out| out.committed_files)
}

/// Recursively compresses source paths into the destination archive path.
pub fn create_archive(
    source_paths: &[PathBuf],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    create_archive_detailed(source_paths, destination_path, options, split_volume_size_bytes)
        .map(|_| ())
        .map_err(CreateStageCommitError::log_and_into_status)
}

/// Returns whether the resolved archive format supports an embedded `TTZR`/`TTRC` recovery record trailer.
#[must_use]
pub fn supports_embedded_recovery_record(resolved_format: TTZipArchiveFormat) -> bool {
    matches!(
        resolved_format,
        TTZipArchiveFormat::Zip | TTZipArchiveFormat::SevenZip | TTZipArchiveFormat::Tar
    )
}

fn append_staged_recovery_record_in_txn(
    staged_dest: &Path,
    redundancy_percent: u8,
    txn_ctx: &CreateTransactionContext,
) -> Result<(), TTZipStatus> {
    let Err(status) = crate::crypto::rs_fec::append_recovery_record_to_file(
        staged_dest,
        f64::from(redundancy_percent),
        4096,
    ) else {
        return Ok(());
    };
    if let Err(rm_err) = fs::remove_file(staged_dest)
        && rm_err.kind() != std::io::ErrorKind::NotFound
    {
        let disposition = if rm_err.kind() == std::io::ErrorKind::InvalidData {
            ArchiveCleanupDisposition::RefusedOwnershipMismatch
        } else {
            ArchiveCleanupDisposition::UnlinkOrRmdirIoError
        };
        txn_ctx.record_inner_failure(StageCleanupFailure {
            role: ResidualPathRole::StagedArtifact,
            disposition,
            original_path: staged_dest.to_path_buf(),
            final_known_path: staged_dest.exists().then(|| staged_dest.to_path_buf()),
            expected_identity: None,
            observed_identity: None,
            os_error_code: rm_err.raw_os_error(),
            detail: format!(
                "append_recovery_record_to_file cleanup failed for '{}': {rm_err} (kind={:?}, os_err={:?})",
                staged_dest.display(),
                rm_err.kind(),
                rm_err.raw_os_error(),
            ),
        });
    }
    Err(status)
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with a
/// cancellation probe, optional 7z header encryption override, and optional RS-FEC recovery record.
pub fn create_archive_explicit_with_files_detailed_full_opts_outcome_with_cancel<C>(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    recovery_record_redundancy_percent: u8,
    cancel_probe: C,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    C: FnOnce() -> bool,
{
    if entries.is_empty() || recovery_record_redundancy_percent > 50 {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam));
    }
    for (src_path, _) in entries {
        if !src_path.exists() && fs::symlink_metadata(src_path).is_err() {
            return Err(CreateStageCommitError::Status(TTZipStatus::ErrFileNotFound));
        }
    }

    let (resolved_format, commit_mode) = prepare_staged_create(
        destination_path,
        options,
        split_volume_size_bytes,
        true,
        encrypt_file_names,
        |fmt| should_use_raw_stream_explicit(entries, fmt),
    )?;
    if recovery_record_redundancy_percent > 0
        && (!supports_embedded_recovery_record(resolved_format) || split_volume_size_bytes > 0)
    {
        return Err(CreateStageCommitError::Status(
            TTZipStatus::ErrUnsupportedFeature,
        ));
    }
    with_staged_archive_destination_detailed_outcome(
        destination_path,
        commit_mode,
        |staged_dest, txn_ctx| {
            write_staged_explicit_archive_in_txn(
                entries,
                staged_dest,
                resolved_format,
                options,
                split_volume_size_bytes,
                encrypt_file_names,
                Some(txn_ctx),
            )?;
            if recovery_record_redundancy_percent > 0 {
                append_staged_recovery_record_in_txn(
                    staged_dest,
                    recovery_record_redundancy_percent,
                    txn_ctx,
                )?;
            }
            Ok(())
        },
        cancel_probe,
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with a
/// cancellation probe and optional 7z header encryption override, returning `CreateStagedCommitOutput`.
pub fn create_archive_explicit_with_files_detailed_opts_outcome_with_cancel<C>(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    cancel_probe: C,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError>
where
    C: FnOnce() -> bool,
{
    create_archive_explicit_with_files_detailed_full_opts_outcome_with_cancel(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        0,
        cancel_probe,
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with optional
/// 7z header encryption override, returning `CreateStagedCommitOutput`.
pub fn create_archive_explicit_with_files_detailed_opts_outcome(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<CreateStagedCommitOutput, CreateStageCommitError> {
    create_archive_explicit_with_files_detailed_opts_outcome_with_cancel(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        || false,
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path with optional 7z header encryption override.
pub fn create_archive_explicit_with_files_detailed_opts(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    create_archive_explicit_with_files_detailed_opts_outcome(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
    )
    .map(|out| out.committed_files)
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path,
/// returning committed volume paths or detailed rollback/backup preservation diagnostics.
pub fn create_archive_explicit_with_files_detailed(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    create_archive_explicit_with_files_detailed_opts(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
        None,
    )
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path and returns committed volume paths.
pub fn create_archive_explicit_with_files(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<Vec<PathBuf>, TTZipStatus> {
    create_archive_explicit_with_files_detailed(
        entries,
        destination_path,
        options,
        split_volume_size_bytes,
    )
    .map_err(CreateStageCommitError::log_and_into_status)
}

/// Recursively compresses pre-filtered explicit entries into the destination archive path.
pub fn create_archive_explicit(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    create_archive_explicit_with_files(entries, destination_path, options, split_volume_size_bytes)
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_zip_with_recovery_record_keeps_eocd_within_standard_64k_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src_file = dir.path().join("payload.bin");
        // ~800 KB deterministic pseudo-random incompressible payload
        let mut payload = vec![0u8; 800_000];
        let mut state = 0x1234_5678u32;
        for b in &mut payload {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (state >> 24) as u8;
        }
        fs::write(&src_file, &payload).expect("write payload");

        let zip_path = dir.path().join("with_recovery.zip");
        let opts = TTZipCreateOptions {
            format: TTZipArchiveFormat::Zip,
            level: crate::types::TTZipCompressionLevel::Store,
            ..Default::default()
        };
        create_archive_explicit_with_files_detailed_full_opts_outcome_with_cancel(
            &[(src_file, "payload.bin".to_string())],
            &zip_path,
            &opts,
            0,
            None,
            10,
            || false,
        )
        .expect("create zip with recovery");

        let zip_bytes = fs::read(&zip_path).expect("read zip");
        // Verify recovery record is present and valid
        let info = crate::crypto::rs_fec::inspect_recovery_record(&zip_bytes)
            .expect("inspect recovery record")
            .expect("recovery record must exist");
        assert!(info.parity_slices_count >= 1);

        // Verify a standard ZIP reader (which only searches the last 65,557 bytes of the raw file
        // without stripping the recovery record) finds the EOCD record.
        let search_window = zip_bytes.len().min(65535 + 22);
        let tail = &zip_bytes[zip_bytes.len() - search_window..];
        let found_eocd = super::super::zip_central_directory::find_eocd_in_tail(tail);
        assert!(
            found_eocd.is_some(),
            "Standard ZIP EOCD must remain within the 65,557-byte tail search window (total_len={}, protected_len={})",
            zip_bytes.len(),
            info.protected_payload_length
        );

        let unzip_out = std::process::Command::new("/usr/bin/unzip")
            .args(["-t", zip_path.to_str().expect("utf8 path")])
            .output()
            .expect("run unzip -t");
        assert!(
            matches!(unzip_out.status.code(), Some(0 | 1))
                && String::from_utf8_lossy(&unzip_out.stdout).contains("No errors detected"),
            "External /usr/bin/unzip -t must validate large ZIP with recovery record"
        );

        let py_out = std::process::Command::new("python3")
            .args([
                "-c",
                "import sys, zipfile\nwith zipfile.ZipFile(sys.argv[1], 'r') as zf:\n    assert zf.testzip() is None\n    assert zf.namelist() == ['payload.bin']\n",
                zip_path.to_str().expect("utf8 path"),
            ])
            .output()
            .expect("run python3 zipfile check");
        assert!(
            py_out.status.success(),
            "Python zipfile.ZipFile must accept large ZIP with recovery record: {}",
            String::from_utf8_lossy(&py_out.stderr)
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_append_staged_recovery_record_records_cleanup_failure_in_txn_without_tls() {
        use std::os::unix::fs::PermissionsExt;
        super::super::clear_create_stage_diagnostics();
        let dir = tempfile::tempdir().expect("tempdir");
        let stage_dir = dir.path().join("stage");
        fs::create_dir(&stage_dir).expect("mkdir stage");
        let staged_dest = stage_dir.join("empty.zip");
        fs::write(&staged_dest, b"").expect("write empty staged file");
        fs::set_permissions(&stage_dir, fs::Permissions::from_mode(0o500)).expect("chmod 500");

        let txn_ctx = CreateTransactionContext::new(stage_dir.clone());
        let res = append_staged_recovery_record_in_txn(&staged_dest, 10, &txn_ctx);
        let _ = fs::set_permissions(&stage_dir, fs::Permissions::from_mode(0o700));

        assert_eq!(res, Err(TTZipStatus::ErrInvalidParam));
        assert!(txn_ctx.has_failures_for_path(&staged_dest));
        assert!(txn_ctx.should_preserve_outer_stage_dir());
        assert!(super::super::take_last_inner_stage_cleanup_record().is_none());
    }
}

