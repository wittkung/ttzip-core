// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! DFS commit walker for extraction collisions.

use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;

use crate::fs::collision::fs_ops::{self, FsOps, Placement, RENAME_NOFOLLOW_ANY, RENAME_SWAP, VolCaps};
use crate::fs::collision::naming;
use crate::fs::collision::table::ConflictTable;
use crate::fs::collision::{
    CollisionAction, CommitPathMapping, CommittedEntryDisposition, CommittedPathEntry,
    ConflictKind, ExtractionOutcome, FailedItem, FailureReason, ItemKind, OutcomeStatus,
    ResolutionSet,
};

struct CommitWalkState<'a> {
    table: &'a ConflictTable,
    resolutions: &'a ResolutionSet,
    outcome: ExtractionOutcome,
    mapping: CommitPathMapping,
}

impl CommitWalkState<'_> {
    fn record_committed(
        &mut self,
        staged_rel: &Path,
        committed_rel: &Path,
        kind: ItemKind,
        disposition: CommittedEntryDisposition,
        whole_subtree: bool,
    ) {
        self.mapping.push(CommittedPathEntry {
            staged_rel_path: staged_rel.to_path_buf(),
            committed_rel_path: committed_rel.to_path_buf(),
            kind,
            disposition,
            whole_subtree,
        });
    }
}

pub struct DFSCommitWalker<'a, F: FsOps> {
    pub fs: &'a F,
    pub dest_dirfd: RawFd,
    pub payload_dirfd: RawFd,
    pub displaced_dirfd: RawFd,
    pub caps: VolCaps,
}

impl<'a, F: FsOps> DFSCommitWalker<'a, F> {
    pub fn new(
        fs: &'a F,
        dest_dirfd: RawFd,
        payload_dirfd: RawFd,
        displaced_dirfd: RawFd,
        caps: VolCaps,
    ) -> Self {
        Self {
            fs,
            dest_dirfd,
            payload_dirfd,
            displaced_dirfd,
            caps,
        }
    }

    pub fn commit(
        &self,
        table: &ConflictTable,
        resolutions: &ResolutionSet,
    ) -> Result<ExtractionOutcome, std::io::Error> {
        self.commit_with_mapping(table, resolutions).map(|(outcome, _)| outcome)
    }

    pub fn commit_with_mapping(
        &self,
        table: &ConflictTable,
        resolutions: &ResolutionSet,
    ) -> Result<(ExtractionOutcome, CommitPathMapping), std::io::Error> {
        let mut state = CommitWalkState {
            table,
            resolutions,
            outcome: ExtractionOutcome::default(),
            mapping: CommitPathMapping::new(),
        };
        self.walk_dir(
            self.payload_dirfd,
            self.dest_dirfd,
            Path::new(""),
            &mut state,
        )?;

        if state.outcome.failed.is_empty() {
            state.outcome.status = OutcomeStatus::Completed;
        } else {
            state.outcome.status = OutcomeStatus::CompletedWithFailures;
        }

        Ok((state.outcome, state.mapping))
    }

    fn walk_dir(
        &self,
        payload_fd: RawFd,
        dest_fd: RawFd,
        rel_prefix: &Path,
        state: &mut CommitWalkState<'_>,
    ) -> Result<(), std::io::Error> {
        // SAFETY: duping payload_fd to open with fdopendir
        let dup_fd = unsafe { libc::dup(payload_fd) };
        if dup_fd < 0 {
            return Err(std::io::Error::last_os_error());
        }

        // SAFETY: valid dup_fd
        let dir = unsafe { libc::fdopendir(dup_fd) };
        if dir.is_null() {
            // SAFETY: closing duplicated fd on error
            unsafe { libc::close(dup_fd) };
            return Err(std::io::Error::last_os_error());
        }

        struct DirGuard(*mut libc::DIR);
        impl Drop for DirGuard {
            fn drop(&mut self) {
                // SAFETY: closing open DIR pointer
                unsafe { libc::closedir(self.0) };
            }
        }
        let _guard = DirGuard(dir);

        loop {
            // SAFETY: clearing errno before readdir
            unsafe { *libc::__error() = 0 };
            // SAFETY: readdir on valid DIR
            let entry = unsafe { libc::readdir(dir) };
            if entry.is_null() {
                // SAFETY: reading errno after readdir
                let err = unsafe { *libc::__error() };
                if err != 0 {
                    return Err(std::io::Error::from_raw_os_error(err));
                }
                break;
            }

            // SAFETY: d_name is null-terminated C string
            let name_cstr = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
            let name_bytes = name_cstr.to_bytes();
            if name_bytes == b"." || name_bytes == b".." {
                continue;
            }

            let child_name = Path::new(std::ffi::OsStr::from_bytes(name_bytes));
            let item_rel_path = if rel_prefix.as_os_str().is_empty() {
                child_name.to_path_buf()
            } else {
                rel_prefix.join(child_name)
            };

            let item_rel_str = item_rel_path.to_str().unwrap_or_default();
            let meta = match self.fs.fstatat_nofollow(payload_fd, child_name) {
                Ok(Some(m)) => m,
                Ok(None) => continue,
                Err(e) => {
                    self.record_io_failure(&mut state.outcome, item_rel_str, e);
                    continue;
                }
            };

            if meta.kind == ItemKind::Directory {
                self.commit_directory(payload_fd, dest_fd, child_name, &item_rel_path, state);
            } else {
                self.commit_file(payload_fd, dest_fd, child_name, &item_rel_path, meta.kind, state);
            }
        }

        Ok(())
    }

    fn record_io_failure(&self, outcome: &mut ExtractionOutcome, rel_str: &str, err: std::io::Error) {
        let errno = err.raw_os_error().unwrap_or(libc::EIO);
        outcome.failed.push(FailedItem {
            relative_path: rel_str.to_string(),
            reason: fs_ops::map_errno(errno),
            errno,
        });
    }

    fn park_displaced_staging_entry(&self, payload_fd: RawFd, child_name: &Path, rel_str: &str) {
        static DISPLACED_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let seq = DISPLACED_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let displaced_name = format!("disp_{seq}_{}", child_name.to_string_lossy());
        if let Err(e) = self.fs.renameatx(
            payload_fd,
            child_name,
            self.displaced_dirfd,
            Path::new(&displaced_name),
            RENAME_NOFOLLOW_ANY,
        ) {
            log::warn!("Staging cleanup: failed to park displaced item {rel_str} ({e}), removing in place");
            if self.fs.unlinkat(payload_fd, child_name, false).is_err() {
                let _ = self.fs.remove_tree(payload_fd, child_name);
            }
        }
    }

    fn replace_via_swap(
        &self,
        payload_fd: RawFd,
        dest_fd: RawFd,
        child_name: &Path,
        item_rel_path: &Path,
        kind: ItemKind,
        state: &mut CommitWalkState<'_>,
    ) {
        let rel_str = item_rel_path.to_str().unwrap_or_default();
        if !self.caps.rename_swap {
            state.outcome.failed.push(FailedItem {
                relative_path: rel_str.to_string(),
                reason: FailureReason::NotSupportedByVolume,
                errno: libc::ENOTSUP,
            });
            return;
        }
        match self.fs.renameatx(
            payload_fd,
            child_name,
            dest_fd,
            child_name,
            RENAME_SWAP | RENAME_NOFOLLOW_ANY,
        ) {
            Ok(()) => {
                state.outcome.replaced += 1;
                state.record_committed(
                    item_rel_path,
                    item_rel_path,
                    kind,
                    CommittedEntryDisposition::Replaced,
                    kind == ItemKind::Directory,
                );
                self.park_displaced_staging_entry(payload_fd, child_name, rel_str);
            }
            Err(e) => self.record_io_failure(&mut state.outcome, rel_str, e),
        }
    }

    fn commit_directory(
        &self,
        payload_fd: RawFd,
        dest_fd: RawFd,
        child_name: &Path,
        item_rel_path: &Path,
        state: &mut CommitWalkState<'_>,
    ) {
        let rel_str = item_rel_path.to_str().unwrap_or_default();
        let dest_meta = match self.fs.fstatat_nofollow(dest_fd, child_name) {
            Ok(None) => {
                match self.place_exclusive(payload_fd, child_name, dest_fd, child_name, ItemKind::Directory) {
                    Ok(()) => {
                        state.outcome.extracted += 1;
                        state.record_committed(
                            item_rel_path,
                            item_rel_path,
                            ItemKind::Directory,
                            CommittedEntryDisposition::Extracted,
                            true,
                        );
                        return;
                    }
                    Err(e) if e.raw_os_error() == Some(libc::EEXIST) => {
                        match self.fs.fstatat_nofollow(dest_fd, child_name) {
                            Ok(Some(m)) => m,
                            Ok(None) => {
                                self.record_io_failure(&mut state.outcome, rel_str, e);
                                return;
                            }
                            Err(stat_err) => {
                                self.record_io_failure(&mut state.outcome, rel_str, stat_err);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        self.record_io_failure(&mut state.outcome, rel_str, e);
                        return;
                    }
                }
            }
            Ok(Some(m)) => m,
            Err(e) => {
                self.record_io_failure(&mut state.outcome, rel_str, e);
                return;
            }
        };
        if dest_meta.kind != ItemKind::Directory {
            self.resolve_dir_type_mismatch(payload_fd, dest_fd, child_name, item_rel_path, state);
            return;
        }

        // Directory onto directory: descend and merge (FR-007)
        let sub_payload = match self.fs.openat_dir_nofollow(payload_fd, child_name) {
            Ok(fd) => fd,
            Err(e) => {
                self.record_io_failure(&mut state.outcome, rel_str, e);
                return;
            }
        };
        let sub_dest = match self.fs.openat_dir_nofollow(dest_fd, child_name) {
            Ok(fd) => fd,
            Err(e) => {
                self.record_io_failure(&mut state.outcome, rel_str, e);
                return;
            }
        };
        state.record_committed(
            item_rel_path,
            item_rel_path,
            ItemKind::Directory,
            CommittedEntryDisposition::MergedExistingDir,
            false,
        );
        if let Err(e) = self.walk_dir(
            sub_payload.as_raw_fd(),
            sub_dest.as_raw_fd(),
            item_rel_path,
            state,
        ) {
            self.record_io_failure(&mut state.outcome, rel_str, e);
            return;
        }
        state.outcome.merged_dirs += 1;

        // Staging cleanup: remove drained payload directory (non-fatal if failed children remain).
        if let Err(e) = self.fs.unlinkat(payload_fd, child_name, true) {
            log::debug!("Staging directory cleanup deferred for {rel_str}: {e}");
        }
    }

    fn resolve_dir_type_mismatch(
        &self,
        payload_fd: RawFd,
        dest_fd: RawFd,
        child_name: &Path,
        item_rel_path: &Path,
        state: &mut CommitWalkState<'_>,
    ) {
        let rel_str = item_rel_path.to_str().unwrap_or_default();
        let action_opt = state
            .table
            .find_by_path(rel_str)
            .and_then(|record| state.resolutions.action_for(record.id))
            .or(state.resolutions.default_action);
        match action_opt {
            Some(CollisionAction::Skip) => match self.fs.remove_tree(payload_fd, child_name) {
                Ok(()) => state.outcome.skipped += 1,
                Err(e) => self.record_io_failure(&mut state.outcome, rel_str, e),
            },
            Some(CollisionAction::Replace) => {
                self.replace_via_swap(
                    payload_fd,
                    dest_fd,
                    child_name,
                    item_rel_path,
                    ItemKind::Directory,
                    state,
                );
            }
            Some(CollisionAction::KeepBoth) => {
                self.commit_keep_both(
                    payload_fd,
                    dest_fd,
                    child_name,
                    item_rel_path,
                    ItemKind::Directory,
                    state,
                );
            }
            None => {
                state.outcome.failed.push(FailedItem {
                    relative_path: rel_str.to_string(),
                    reason: FailureReason::Io,
                    errno: libc::EEXIST,
                });
            }
        }
    }

    fn resolve_file_action(
        &self,
        payload_fd: RawFd,
        dest_fd: RawFd,
        child_name: &Path,
        item_rel_path: &Path,
        kind: ItemKind,
        action: CollisionAction,
        same_type: bool,
        state: &mut CommitWalkState<'_>,
    ) {
        let rel_str = item_rel_path.to_str().unwrap_or_default();
        match action {
            CollisionAction::Skip => match self.fs.unlinkat(payload_fd, child_name, false) {
                Ok(()) => state.outcome.skipped += 1,
                Err(e) => self.record_io_failure(&mut state.outcome, rel_str, e),
            },
            CollisionAction::Replace => {
                if same_type {
                    match self.fs.renameatx(
                        payload_fd,
                        child_name,
                        dest_fd,
                        child_name,
                        RENAME_NOFOLLOW_ANY,
                    ) {
                        Ok(()) => {
                            state.outcome.replaced += 1;
                            state.record_committed(
                                item_rel_path,
                                item_rel_path,
                                kind,
                                CommittedEntryDisposition::Replaced,
                                false,
                            );
                        }
                        Err(e) if e.raw_os_error() == Some(libc::EISDIR) => {
                            self.replace_via_swap(
                                payload_fd,
                                dest_fd,
                                child_name,
                                item_rel_path,
                                kind,
                                state,
                            );
                        }
                        Err(e) => self.record_io_failure(&mut state.outcome, rel_str, e),
                    }
                } else {
                    self.replace_via_swap(
                        payload_fd,
                        dest_fd,
                        child_name,
                        item_rel_path,
                        kind,
                        state,
                    );
                }
            }
            CollisionAction::KeepBoth => {
                self.commit_keep_both(payload_fd, dest_fd, child_name, item_rel_path, kind, state);
            }
        }
    }

    fn commit_file(
        &self,
        payload_fd: RawFd,
        dest_fd: RawFd,
        child_name: &Path,
        item_rel_path: &Path,
        kind: ItemKind,
        state: &mut CommitWalkState<'_>,
    ) {
        let rel_str = item_rel_path.to_str().unwrap_or_default();
        if let Some(record) = state.table.find_by_path(rel_str) {
            if let Some(action) = state.resolutions.action_for(record.id) {
                self.resolve_file_action(
                    payload_fd,
                    dest_fd,
                    child_name,
                    item_rel_path,
                    kind,
                    action,
                    record.kind == ConflictKind::SameType,
                    state,
                );
            } else {
                state.outcome.failed.push(FailedItem {
                    relative_path: rel_str.to_string(),
                    reason: FailureReason::Io,
                    errno: libc::EEXIST,
                });
            }
            return;
        }

        // Non-conflicting item
        match self.place_exclusive(payload_fd, child_name, dest_fd, child_name, kind) {
            Ok(()) => {
                state.outcome.extracted += 1;
                state.record_committed(
                    item_rel_path,
                    item_rel_path,
                    kind,
                    CommittedEntryDisposition::Extracted,
                    false,
                );
            }
            Err(e) if e.raw_os_error() == Some(libc::EEXIST) => {
                // Dynamic collision detected at commit time (CommitRecheck)
                if let Some(act) = state.resolutions.default_action {
                    self.resolve_file_action(
                        payload_fd,
                        dest_fd,
                        child_name,
                        item_rel_path,
                        kind,
                        act,
                        true,
                        state,
                    );
                } else {
                    state.outcome.failed.push(FailedItem {
                        relative_path: rel_str.to_string(),
                        reason: FailureReason::Io,
                        errno: libc::EEXIST,
                    });
                }
            }
            Err(e) => {
                self.record_io_failure(&mut state.outcome, rel_str, e);
            }
        }
    }

    /// Places a staged item at a destination name only if that name is free (FR-022).
    fn place_exclusive(
        &self,
        payload_fd: RawFd,
        child_name: &Path,
        dest_fd: RawFd,
        target_name: &Path,
        kind: ItemKind,
    ) -> Result<(), std::io::Error> {
        let placement = Placement {
            from_fd: payload_fd,
            from: child_name,
            to_fd: dest_fd,
            to: target_name,
        };
        fs_ops::place_exclusive(self.fs, self.caps, placement, kind)
    }

    fn commit_keep_both(
        &self,
        payload_fd: RawFd,
        dest_fd: RawFd,
        child_name: &Path,
        item_rel_path: &Path,
        kind: ItemKind,
        state: &mut CommitWalkState<'_>,
    ) -> bool {
        let rel_str = item_rel_path.to_str().unwrap_or_default();
        let mut n = 2;
        while n <= 10000 {
            let cand = naming::candidate(child_name.as_os_str(), kind, n);
            let cand_path = Path::new(&cand);
            match self.place_exclusive(payload_fd, child_name, dest_fd, cand_path, kind) {
                Ok(()) => {
                    let committed_rel = match item_rel_path.parent() {
                        Some(parent) if !parent.as_os_str().is_empty() => parent.join(cand_path),
                        _ => cand_path.to_path_buf(),
                    };
                    state.outcome.kept_both += 1;
                    state.record_committed(
                        item_rel_path,
                        &committed_rel,
                        kind,
                        CommittedEntryDisposition::KeptBoth,
                        kind == ItemKind::Directory,
                    );
                    return true;
                }
                Err(e) if e.raw_os_error() == Some(libc::EEXIST) => {
                    n += 1;
                }
                Err(e) => {
                    state.outcome.failed.push(FailedItem {
                        relative_path: rel_str.to_string(),
                        reason: fs_ops::map_errno(e.raw_os_error().unwrap_or(0)),
                        errno: e.raw_os_error().unwrap_or(0),
                    });
                    return false;
                }
            }
        }

        state.outcome.failed.push(FailedItem {
            relative_path: rel_str.to_string(),
            reason: FailureReason::NameTooLong,
            errno: libc::EEXIST,
        });
        false
    }
}

#[cfg(test)]
#[path = "commit_exclusive_tests.rs"]
mod commit_exclusive_tests;
