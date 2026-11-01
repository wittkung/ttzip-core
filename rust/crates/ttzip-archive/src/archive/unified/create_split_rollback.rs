// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multi-volume split archive discovery, pre-commit backup creation, and verified rollback
//! with structured `CreateBackupRecoveryAttempt` reporting.

use std::collections::BTreeSet;
use std::ffi::{CString, OsString};
use std::fs;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};

use crate::archive::split::VolumeNamingScheme;
use crate::fs::abort_guard::open_dir_nofollow;
use crate::fs::collision::staging::verify_and_remove_owned_dir;
use crate::types::TTZipStatus;

use super::create_staging::{next_txn_token, record_post_commit_cleanup_issue, CreateStageCommitError};
use super::create_transaction::{
    ArchiveCleanupDisposition, BackupRecoveryEntryFailure, CreateBackupRecoveryAttempt,
    CreateBackupRecoveryState, CreateTransactionContext, ResidualPathRole, StageCleanupFailure,
};
use super::PostCommitCleanupIssue;

fn strip_first_volume_suffix(name: &str) -> &str {
    name.strip_suffix(".001")
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name)
}

pub(crate) fn is_non_dir_path(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| !m.file_type().is_dir())
        .unwrap_or(false)
}

fn collect_contiguous_numeric_chain(parent: &Path, canon_base: &str, out: &mut BTreeSet<PathBuf>) {
    let first = parent.join(format!("{canon_base}.001"));
    if !is_non_dir_path(&first) {
        return;
    }
    for idx in 1usize..=999 {
        let candidate = parent.join(format!("{canon_base}.{idx:03}"));
        if is_non_dir_path(&candidate) {
            out.insert(candidate);
        } else {
            break;
        }
    }
}

fn collect_contiguous_split_zip_chain(parent: &Path, canon_base: &str, out: &mut BTreeSet<PathBuf>) {
    let lower = canon_base.to_ascii_lowercase();
    if !lower.ends_with(".zip") || canon_base.len() <= 4 {
        return;
    }
    let stem = &canon_base[..canon_base.len() - 4];
    let first_z = parent.join(format!("{stem}.z01"));
    if !is_non_dir_path(&first_z) {
        return;
    }
    for idx in 1usize..=99 {
        let candidate = parent.join(format!("{stem}.z{idx:02}"));
        if is_non_dir_path(&candidate) {
            out.insert(candidate);
        } else {
            break;
        }
    }
    let zip_tail = parent.join(canon_base);
    if is_non_dir_path(&zip_tail) {
        out.insert(zip_tail);
    }
}

pub(crate) fn collect_confirmed_existing_split_volumes(
    parent: &Path,
    base_file_name: &str,
    staged_names: &[OsString],
    scheme: VolumeNamingScheme,
) -> Vec<PathBuf> {
    let mut confirmed = BTreeSet::new();
    for staged_name in staged_names {
        let candidate = parent.join(staged_name);
        if is_non_dir_path(&candidate) {
            confirmed.insert(candidate);
        }
    }

    let canon_base = strip_first_volume_suffix(base_file_name);
    match scheme {
        VolumeNamingScheme::NumberedExtension | VolumeNamingScheme::RawSplit => {
            collect_contiguous_numeric_chain(parent, canon_base, &mut confirmed);
        }
        VolumeNamingScheme::PkzipSpanned => {
            collect_contiguous_split_zip_chain(parent, canon_base, &mut confirmed);
        }
    }

    confirmed.into_iter().collect()
}

fn remove_lingering_installed_volumes(
    installed: &[PathBuf],
    txn_ctx: Option<&CreateTransactionContext>,
) -> Vec<PathBuf> {
    let mut lingering_installed = Vec::new();
    for dst in installed.iter().rev() {
        if let Err(e) = fs::remove_file(dst) {
            if e.kind() != std::io::ErrorKind::NotFound {
                lingering_installed.push(dst.clone());
                if let Some(ctx) = txn_ctx {
                    ctx.record_inner_failure(StageCleanupFailure {
                        role: ResidualPathRole::FinalDestinationTarget,
                        disposition: ArchiveCleanupDisposition::UnlinkOrRmdirIoError,
                        original_path: dst.clone(),
                        final_known_path: Some(dst.clone()),
                        expected_identity: None,
                        observed_identity: None,
                        os_error_code: e.raw_os_error(),
                        detail: format!(
                            "Failed to remove partially installed split volume '{}': {e}",
                            dst.display()
                        ),
                    });
                }
            }
        }
    }
    lingering_installed
}

fn cleanup_restored_backup_dir(
    backup_dir: &Path,
    txn_ctx: Option<&CreateTransactionContext>,
    lingering_installed: &mut Vec<PathBuf>,
) -> Option<String> {
    match fs::remove_dir_all(backup_dir) {
        Ok(()) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            lingering_installed.push(backup_dir.to_path_buf());
            let msg = format!(
                "Failed to remove restored backup directory '{}': {e}",
                backup_dir.display()
            );
            if let Some(ctx) = txn_ctx {
                ctx.record_inner_failure(StageCleanupFailure {
                    role: ResidualPathRole::PreexistingBackupDirectory,
                    disposition: ArchiveCleanupDisposition::UnlinkOrRmdirIoError,
                    original_path: backup_dir.to_path_buf(),
                    final_known_path: Some(backup_dir.to_path_buf()),
                    expected_identity: None,
                    observed_identity: None,
                    os_error_code: e.raw_os_error(),
                    detail: msg.clone(),
                });
            }
            Some(msg)
        }
    }
}

pub(crate) fn rollback_volume_commit_in_txn_with<B>(
    installed: &[PathBuf],
    backed_up: &[(PathBuf, PathBuf)],
    backup_dir: &Path,
    cause_status: TTZipStatus,
    txn_ctx: Option<&CreateTransactionContext>,
    mut restore_backup: B,
) -> CreateStageCommitError
where
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let mut lingering_installed = remove_lingering_installed_volumes(installed, txn_ctx);
    let mut unrestored_origins = Vec::new();
    let mut restored_targets = Vec::new();
    let mut failed_entries = Vec::new();

    for (orig_dst, bak_src) in backed_up.iter().rev() {
        match restore_backup(bak_src, orig_dst) {
            Ok(()) => restored_targets.push(orig_dst.clone()),
            Err(e) => {
                unrestored_origins.push(orig_dst.clone());
                failed_entries.push(BackupRecoveryEntryFailure {
                    backup_source_path: bak_src.clone(),
                    target_path: orig_dst.clone(),
                    os_error_code: e.raw_os_error(),
                    detail: format!(
                        "Failed to restore backup '{}' -> '{}': {e}",
                        bak_src.display(),
                        orig_dst.display()
                    ),
                });
            }
        }
    }

    let mut backup_dir_cleanup_error = None;
    let rec_state = if backed_up.is_empty() {
        let _ = fs::remove_dir_all(backup_dir);
        CreateBackupRecoveryState::NotApplicable
    } else if unrestored_origins.is_empty() && lingering_installed.is_empty() {
        backup_dir_cleanup_error =
            cleanup_restored_backup_dir(backup_dir, txn_ctx, &mut lingering_installed);
        CreateBackupRecoveryState::RestoredAndVerified
    } else {
        CreateBackupRecoveryState::RollbackIncompletePreservedInBackup
    };

    if let Some(ctx) = txn_ctx {
        if !backed_up.is_empty() {
            ctx.set_recovery_attempt(CreateBackupRecoveryAttempt {
                state: rec_state,
                backup_dir: Some(backup_dir.to_path_buf()),
                restored_targets,
                failed_entries,
                backup_dir_cleanup_error,
            });
        }
    }

    if unrestored_origins.is_empty() && lingering_installed.is_empty() {
        CreateStageCommitError::Status(cause_status)
    } else {
        CreateStageCommitError::RollbackIncomplete {
            status: cause_status,
            backup_dir: backup_dir.to_path_buf(),
            unrestored_origins,
            lingering_installed,
        }
    }
}

/// Restores any remaining backed-up volumes from a preserved `.ttzip_create_bak_*` directory
/// back into its parent directory and removes `backup_dir` once all volumes are restored.
///
/// Rejects `.ttzip_create_txn_*` or any directory whose name does not start with `.ttzip_create_bak_`
/// so uncommitted partial staging artifacts can never be restored over user targets.
pub fn recover_preserved_create_backup(backup_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let is_valid_backup = backup_dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.starts_with(".ttzip_create_bak_"));
    if !is_valid_backup {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "recover_preserved_create_backup requires a .ttzip_create_bak_* directory; uncommitted .ttzip_create_txn_* staging directories cannot be restored",
        ));
    }
    let parent = backup_dir.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "backup_dir has no parent")
    })?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(backup_dir)? {
        entries.push(entry?.path());
    }
    entries.sort();

    let mut restored = Vec::with_capacity(entries.len());
    for bak_path in entries {
        let Some(fname) = bak_path.file_name() else {
            continue;
        };
        let orig_dst = parent.join(fname);
        fs::rename(&bak_path, &orig_dst)?;
        restored.push(orig_dst);
    }
    fs::remove_dir_all(backup_dir)?;
    Ok(restored)
}

struct BackupDirHandles {
    parent_fd: OwnedFd,
    parent_id: (u64, u64),
    dir_fd: OwnedFd,
    dir_id: (u64, u64),
}

struct SplitBackupState {
    backed_up: Vec<(PathBuf, PathBuf)>,
    handles: Option<BackupDirHandles>,
}

fn backup_existing_split_volumes<B>(
    existing_files: Vec<PathBuf>,
    parent: &Path,
    backup_dir: &Path,
    bak_name_c: &CString,
    txn_ctx: Option<&CreateTransactionContext>,
    restore_backup: &mut B,
) -> Result<SplitBackupState, CreateStageCommitError>
where
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    if existing_files.is_empty() {
        return Ok(SplitBackupState {
            backed_up: Vec::new(),
            handles: None,
        });
    }
    let (parent_fd, parent_id) = open_dir_nofollow(parent)
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    if unsafe { libc::mkdirat(parent_fd.as_raw_fd(), bak_name_c.as_ptr(), 0o700) } != 0 {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed));
    }
    let (dir_fd, dir_id) = open_dir_nofollow(backup_dir)
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    let mut backed_up: Vec<(PathBuf, PathBuf)> = Vec::new();
    for orig_path in existing_files {
        let Some(fname) = orig_path.file_name() else {
            continue;
        };
        let bak_path = backup_dir.join(fname);
        if fs::rename(&orig_path, &bak_path).is_err() {
            return Err(rollback_volume_commit_in_txn_with(
                &[],
                &backed_up,
                backup_dir,
                TTZipStatus::ErrOpenFailed,
                txn_ctx,
                restore_backup,
            ));
        }
        backed_up.push((orig_path, bak_path));
    }
    Ok(SplitBackupState {
        backed_up,
        handles: Some(BackupDirHandles {
            parent_fd,
            parent_id,
            dir_fd,
            dir_id,
        }),
    })
}

pub(crate) fn commit_split_staged_volumes_in_txn_with<R, B>(
    staged_files: Vec<PathBuf>,
    parent: &Path,
    base_file_name: &str,
    scheme: VolumeNamingScheme,
    txn_ctx: Option<&CreateTransactionContext>,
    mut rename_staged: R,
    mut restore_backup: B,
) -> Result<(Vec<PathBuf>, Vec<PostCommitCleanupIssue>), CreateStageCommitError>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let staged_names: Vec<OsString> = staged_files
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_os_string()))
        .collect();
    let existing_files =
        collect_confirmed_existing_split_volumes(parent, base_file_name, &staged_names, scheme);

    let (pid, nanos, seq) = next_txn_token();
    let bak_name = format!(".ttzip_create_bak_{pid}_{nanos}_{seq}");
    let bak_name_c = CString::new(bak_name.as_bytes())
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam))?;
    let backup_dir = parent.join(&bak_name);
    let state = backup_existing_split_volumes(
        existing_files,
        parent,
        &backup_dir,
        &bak_name_c,
        txn_ctx,
        &mut restore_backup,
    )?;

    let mut installed: Vec<PathBuf> = Vec::with_capacity(staged_files.len());
    for src_file in staged_files {
        let Some(fname) = src_file.file_name() else {
            continue;
        };
        let dst_file = parent.join(fname);
        if rename_staged(&src_file, &dst_file).is_err() {
            return Err(rollback_volume_commit_in_txn_with(
                &installed,
                &state.backed_up,
                &backup_dir,
                TTZipStatus::ErrOpenFailed,
                txn_ctx,
                restore_backup,
            ));
        }
        installed.push(dst_file);
    }

    let mut post_commit_issues = Vec::new();
    if let Some(h) = state.handles {
        if let Err(e) = verify_and_remove_owned_dir(
            parent,
            h.parent_fd.as_raw_fd(),
            h.parent_id,
            &backup_dir,
            &bak_name_c,
            h.dir_fd.as_raw_fd(),
            h.dir_id,
        ) {
            record_post_commit_cleanup_issue(backup_dir.clone(), "committed_split_backup_dir", &e);
            post_commit_issues.push(PostCommitCleanupIssue {
                path: backup_dir,
                role: "committed_split_backup_dir",
                message: format!("committed_split_backup_dir: {e}"),
                errno: e.raw_os_error().unwrap_or(libc::EIO),
            });
        }
    }
    Ok((installed, post_commit_issues))
}
