// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Three-phase extraction metadata plan, `DirJail` FD-bound metadata application, and
//! post-commit failure propagation (spec 229 FR-024).
//!
//! Enforces:
//! 1. Phase 1 (`stage`) keeps all directories in `staging/payload` at `0o700` and regular
//!    files at `0o600`, capturing each staged object's `(st_dev, st_ino)` identity.
//! 2. Phase 2 (`DFSCommitWalker`) records actual committed destination paths and directory
//!    dispositions in [`CommitPathMapping`].
//! 3. Phase 3 resolves every target object step-by-step from `DirJail::root_fd()` without
//!    following symlinks, verifies the target's `(st_dev, st_ino)` matches the committed object,
//!    applies `fchmod`/`futimens` on the held FD (and `O_SYMLINK | O_NOFOLLOW` / `AT_SYMLINK_NOFOLLOW`
//!    for symlinks), and merges any metadata failure into [`ExtractionOutcome`] as
//!    [`OutcomeStatus::CompletedWithFailures`] without deleting committed file contents.

use std::collections::HashMap;
use std::ffi::{CString, OsStr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use rayon::prelude::*;
use ttzip_types::scheduler::LocalWorkStealingPool;

use crate::fs::collision::fs_ops::map_errno;
use crate::fs::collision::{CommitPathMapping, ExtractionOutcome, FailedItem, FailureReason, OutcomeStatus};
use crate::fs::dir_jail::{DirJail, VerifiedDirHandle};
use crate::fs::safe_extract::{DeferredEntryMetadata, SafeExtractEngine};
use crate::types::TTZipStatus;

/// Error returned when FD-bound metadata application fails on a single entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetadataApplyError {
    pub status: TTZipStatus,
    pub errno: i32,
}

impl MetadataApplyError {
    #[inline]
    const fn security(errno: i32) -> Self {
        Self { status: TTZipStatus::ErrSecurityViolation, errno }
    }

    #[inline]
    fn from_last_os_error(default_status: TTZipStatus) -> Self {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(libc::EIO);
        let status = match errno {
            libc::ENOENT => TTZipStatus::ErrFileNotFound,
            libc::ELOOP | libc::ENOTDIR => TTZipStatus::ErrSecurityViolation,
            _ => default_status,
        };
        Self { status, errno }
    }

    #[inline]
    fn failure_reason(self) -> FailureReason {
        if self.status == TTZipStatus::ErrSecurityViolation || self.errno == libc::ELOOP {
            FailureReason::PermissionDenied
        } else {
            map_errno(self.errno)
        }
    }
}

/// Deferred metadata plan captured at the end of Phase 1 (`stage`) for a staging payload root.
#[derive(Debug, Clone)]
pub struct StagedMetadataPlan {
    pub staging_payload_root: PathBuf,
    pub preserve_permissions: bool,
    pub creation_mode_mask: u32,
    pub files: Vec<DeferredEntryMetadata>,
    pub explicit_dirs: Vec<DeferredEntryMetadata>,
    pub implicit_dirs: Vec<DeferredEntryMetadata>,
}

type PreMetadataHook = Box<dyn FnOnce(&Path) + Send>;

fn plan_registry() -> &'static Mutex<HashMap<PathBuf, StagedMetadataPlan>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, StagedMetadataPlan>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn hook_registry() -> &'static Mutex<Vec<(PathBuf, PreMetadataHook)>> {
    static HOOKS: OnceLock<Mutex<Vec<(PathBuf, PreMetadataHook)>>> = OnceLock::new();
    HOOKS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Registers a one-shot callback invoked at the start of Phase 3 (`apply_committed_metadata`)
/// for `dest_root` (after Phase 2 `DFSCommitWalker` has placed items into `dest_root` and
/// before `DirJail` applies final permissions and timestamps).
pub fn register_pre_metadata_hook_for_dest(dest_root: &Path, hook: PreMetadataHook) {
    let mut guard = hook_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.push((dest_root.to_path_buf(), hook));
}

fn take_pre_metadata_hook(dest_root: &Path) -> Option<PreMetadataHook> {
    let canon_dest = std::fs::canonicalize(dest_root).ok();
    let mut guard = hook_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let idx = guard.iter().position(|(registered, _)| {
        if registered == dest_root {
            return true;
        }
        match (&canon_dest, std::fs::canonicalize(registered).ok()) {
            (Some(a), Some(b)) => a == &b,
            _ => false,
        }
    })?;
    Some(guard.remove(idx).1)
}

/// Returns `true` when `root` is the `payload` subdirectory of a transactional `.ttzip_staging_*` area.
#[inline]
pub fn is_transactional_staging_payload(root: &Path) -> bool {
    if root.file_name() != Some(OsStr::new("payload")) {
        return false;
    }
    root.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.starts_with(".ttzip_staging_"))
}

/// Stores a `StagedMetadataPlan` for later Phase 3 application after `commit` finishes.
pub fn store_staged_metadata_plan(plan: StagedMetadataPlan) {
    let mut guard = plan_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.insert(plan.staging_payload_root.clone(), plan);
}

/// Discards any stored `StagedMetadataPlan` for `staging_payload_root` (called on `StagingArea::drop`).
pub fn discard_staged_metadata_plan(staging_payload_root: &Path) {
    let mut guard = plan_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.remove(staging_payload_root);
}

/// Splits raw registered entries into `(files, explicit_dirs)` with Last-Entry-Wins deduplication
/// by path so intra-archive duplicate entries preserve the final entry's `(mode, mtime)`.
pub fn dedup_last_entry_wins(
    entries: Vec<DeferredEntryMetadata>,
) -> (Vec<DeferredEntryMetadata>, Vec<DeferredEntryMetadata>) {
    let mut file_index_by_path: HashMap<PathBuf, usize> = HashMap::new();
    let mut files: Vec<DeferredEntryMetadata> = Vec::new();
    let mut dir_index_by_path: HashMap<PathBuf, usize> = HashMap::new();
    let mut dirs: Vec<DeferredEntryMetadata> = Vec::new();

    for entry in entries {
        if entry.is_directory {
            if let Some(&idx) = dir_index_by_path.get(&entry.path) {
                dirs[idx] = entry;
            } else {
                let idx = dirs.len();
                dir_index_by_path.insert(entry.path.clone(), idx);
                dirs.push(entry);
            }
        } else if let Some(&idx) = file_index_by_path.get(&entry.path) {
            files[idx] = entry;
        } else {
            let idx = files.len();
            file_index_by_path.insert(entry.path.clone(), idx);
            files.push(entry);
        }
    }

    (files, dirs)
}

struct RawFdGuard {
    fd: libc::c_int,
}

impl RawFdGuard {
    #[inline]
    const fn owned(fd: libc::c_int) -> Self {
        Self { fd }
    }
}

impl Drop for RawFdGuard {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

fn fstatat_nofollow_raw(dir_fd: libc::c_int, name_c: &CString) -> Result<libc::stat, MetadataApplyError> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::fstatat(dir_fd, name_c.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
    if rc != 0 {
        return Err(MetadataApplyError::from_last_os_error(TTZipStatus::ErrExtractionFailed));
    }
    Ok(st)
}

fn fstat_raw(fd: libc::c_int) -> Result<libc::stat, MetadataApplyError> {
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        return Err(MetadataApplyError::from_last_os_error(TTZipStatus::ErrExtractionFailed));
    }
    Ok(st)
}

fn resolve_parent_dir_in_jail(
    jail: &DirJail,
    parent_comps: &[String],
) -> Result<Option<Arc<VerifiedDirHandle>>, MetadataApplyError> {
    jail.resolve_dir_handle_detailed(parent_comps, false)
        .map_err(|e| MetadataApplyError {
            status: e.status,
            errno: e.errno,
        })
}

/// Resolves `(st_dev, st_ino)` of an entry inside `jail` step-by-step without following symlinks.
pub fn stat_entry_identity_in_jail(
    jail: &DirJail,
    path: &Path,
) -> Result<(u64, u64), MetadataApplyError> {
    let comps = jail
        .resolve_path_components(path)
        .map_err(|_| MetadataApplyError::security(libc::EPERM))?;
    let Some((leaf, parent_comps)) = comps.split_last() else {
        let st = fstat_raw(jail.root_fd())?;
        return Ok((st.st_dev as u64, st.st_ino));
    };
    let parent_handle = resolve_parent_dir_in_jail(jail, parent_comps)?;
    let parent_fd = parent_handle
        .as_ref()
        .map_or(jail.root_fd(), |h| h.as_raw_fd());
    let leaf_c = CString::new(leaf.as_bytes()).map_err(|_| MetadataApplyError::security(libc::EINVAL))?;
    let st = fstatat_nofollow_raw(parent_fd, &leaf_c)?;
    Ok((st.st_dev as u64, st.st_ino))
}

/// Populates `expected_identity` (`(st_dev, st_ino)`) for each entry using an already-open `DirJail`.
pub(crate) fn populate_staged_identities_in_jail(
    jail: &DirJail,
    entries: &mut [DeferredEntryMetadata],
) -> Result<(), MetadataApplyError> {
    for entry in entries {
        let identity = stat_entry_identity_in_jail(jail, &entry.path)?;
        entry.expected_identity = Some(identity);
    }
    Ok(())
}

/// Populates `expected_identity` (`(st_dev, st_ino)`) for each entry inside `dest_root`.
///
/// Fails explicitly if `DirJail::open(dest_root)` fails or any entry's identity cannot be
/// resolved without following symlinks, preventing `expected_identity = None` from bypassing
/// Phase 3 object verification.
pub fn populate_staged_identities(
    dest_root: &Path,
    entries: &mut [DeferredEntryMetadata],
) -> Result<(), MetadataApplyError> {
    if entries.is_empty() {
        return Ok(());
    }
    let jail = DirJail::open(dest_root).map_err(|status| MetadataApplyError {
        status,
        errno: if status == TTZipStatus::ErrSecurityViolation {
            libc::ELOOP
        } else {
            libc::EACCES
        },
    })?;
    populate_staged_identities_in_jail(&jail, entries)
}

fn apply_symlink_mtime_nofollow(
    parent_fd: libc::c_int,
    leaf_c: &CString,
    pre_st: &libc::stat,
    entry: &DeferredEntryMetadata,
) -> Result<(), MetadataApplyError> {
    if entry.mtime_epoch_secs <= 0 {
        return Ok(());
    }
    let times = [
        libc::timespec {
            tv_sec: entry.mtime_epoch_secs as libc::time_t,
            tv_nsec: entry.mtime_nanos as libc::c_long,
        },
        libc::timespec {
            tv_sec: entry.mtime_epoch_secs as libc::time_t,
            tv_nsec: entry.mtime_nanos as libc::c_long,
        },
    ];
    #[cfg(target_os = "macos")]
    {
        let flags = libc::O_RDONLY | libc::O_SYMLINK | libc::O_CLOEXEC;
        let sym_fd = unsafe { libc::openat(parent_fd, leaf_c.as_ptr(), flags) };
        if sym_fd < 0 {
            return Err(MetadataApplyError::from_last_os_error(TTZipStatus::ErrExtractionFailed));
        }
        let guard = RawFdGuard::owned(sym_fd);
        let fd_st = fstat_raw(guard.fd)?;
        if (fd_st.st_mode & libc::S_IFMT) != libc::S_IFLNK
            || (fd_st.st_dev, fd_st.st_ino) != (pre_st.st_dev, pre_st.st_ino)
        {
            return Err(MetadataApplyError::security(libc::EPERM));
        }
        if unsafe { libc::futimens(guard.fd, times.as_ptr()) } != 0 {
            return Err(MetadataApplyError::from_last_os_error(TTZipStatus::ErrExtractionFailed));
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pre_st;
        let rc = unsafe {
            libc::utimensat(parent_fd, leaf_c.as_ptr(), times.as_ptr(), libc::AT_SYMLINK_NOFOLLOW)
        };
        if rc != 0 {
            return Err(MetadataApplyError::from_last_os_error(TTZipStatus::ErrExtractionFailed));
        }
        Ok(())
    }
}

fn open_target_fd_verified(
    parent_fd: libc::c_int,
    leaf_c: &CString,
    entry: &DeferredEntryMetadata,
) -> Result<(RawFdGuard, libc::stat), MetadataApplyError> {
    let expected_fmt = if entry.is_directory {
        libc::S_IFDIR
    } else {
        libc::S_IFREG
    };
    let mut flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK;
    if entry.is_directory {
        flags |= libc::O_DIRECTORY;
    }
    let fd = unsafe { libc::openat(parent_fd, leaf_c.as_ptr(), flags) };
    if fd < 0 {
        return Err(MetadataApplyError::from_last_os_error(
            TTZipStatus::ErrExtractionFailed,
        ));
    }
    let guard = RawFdGuard::owned(fd);
    let fd_st = fstat_raw(guard.fd)?;
    if (fd_st.st_mode & libc::S_IFMT) != expected_fmt {
        let errno = if entry.is_directory {
            libc::ENOTDIR
        } else {
            libc::EPERM
        };
        return Err(MetadataApplyError::security(errno));
    }
    if let Some((exp_dev, exp_ino)) = entry.expected_identity {
        if (fd_st.st_dev as u64, fd_st.st_ino) != (exp_dev, exp_ino) {
            return Err(MetadataApplyError::security(libc::EPERM));
        }
    }
    Ok((guard, fd_st))
}

/// Probes the calling environment's effective creation mode mask (`0o777 & !umask`)
/// by creating and immediately unlinking a transient exclusive probe file inside `dir_fd`
/// with mode `0o777` and reading its kernel-assigned `st_mode` via `fstat`.
///
/// This never calls `libc::umask`, avoiding any process-global state mutation or
/// multi-threaded concurrency race. Fails closed to `0o700` if probing cannot create an inode.
pub fn probe_creation_mode_mask_in_fd(dir_fd: libc::c_int) -> u32 {
    static PROBE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pid = std::process::id();
    for attempt in 0..8u32 {
        let seq = PROBE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = format!(".ttzip_mode_probe_{pid}_{seq}_{attempt}");
        let Ok(name_c) = CString::new(name.as_bytes()) else {
            break;
        };
        let flags =
            libc::O_RDONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = unsafe { libc::openat(dir_fd, name_c.as_ptr(), flags, 0o777 as libc::c_uint) };
        if fd < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if errno == libc::EEXIST {
                continue;
            }
            break;
        }
        let guard = RawFdGuard::owned(fd);
        let mask = fstat_raw(guard.fd)
            .map(|st| (st.st_mode as u32) & 0o0777)
            .unwrap_or(0o700);
        unsafe {
            libc::unlinkat(dir_fd, name_c.as_ptr(), 0);
        }
        return mask;
    }
    0o700
}

/// Opens `dir_path` via `DirJail` and probes the calling environment's effective creation mode mask.
pub fn probe_creation_mode_mask_for_path(dir_path: &Path) -> u32 {
    if dir_path.as_os_str().is_empty() {
        return 0o700;
    }
    DirJail::open(dir_path)
        .map(|jail| probe_creation_mode_mask_in_fd(jail.root_fd()))
        .unwrap_or(0o700)
}

fn apply_fd_mode_and_mtime(
    fd: libc::c_int,
    current_mode: u32,
    entry: &DeferredEntryMetadata,
    preserve_permissions: bool,
    creation_mode_mask: u32,
) -> Result<(), MetadataApplyError> {
    if entry.mtime_epoch_secs > 0 {
        let times = [
            libc::timespec {
                tv_sec: entry.mtime_epoch_secs as libc::time_t,
                tv_nsec: entry.mtime_nanos as libc::c_long,
            },
            libc::timespec {
                tv_sec: entry.mtime_epoch_secs as libc::time_t,
                tv_nsec: entry.mtime_nanos as libc::c_long,
            },
        ];
        if unsafe { libc::futimens(fd, times.as_ptr()) } != 0 {
            return Err(MetadataApplyError::from_last_os_error(TTZipStatus::ErrExtractionFailed));
        }
    }

    let mask = creation_mode_mask & 0o0777;
    let effective_mode = if preserve_permissions {
        if entry.mode != 0 {
            entry.mode
        } else if entry.is_directory {
            SafeExtractEngine::DEFAULT_IMPLICIT_DIR_MODE & mask
        } else {
            SafeExtractEngine::DEFAULT_REGULAR_FILE_MODE & mask
        }
    } else if entry.is_directory {
        SafeExtractEngine::DEFAULT_IMPLICIT_DIR_MODE & mask
    } else {
        SafeExtractEngine::DEFAULT_REGULAR_FILE_MODE & mask
    };
    if effective_mode != 0 {
        let target_mode = ((effective_mode & 0o0777) & !0o002) as libc::mode_t;
        if (current_mode & 0o0777) != (target_mode as u32)
            && unsafe { libc::fchmod(fd, target_mode) } != 0
        {
            return Err(MetadataApplyError::from_last_os_error(
                TTZipStatus::ErrExtractionFailed,
            ));
        }
    }

    Ok(())
}

/// Applies `entry` permissions and timestamps strictly via step-by-step `DirJail` FD traversal.
pub fn apply_entry_metadata_in_jail(
    jail: &DirJail,
    entry: &DeferredEntryMetadata,
    preserve_permissions: bool,
) -> Result<(), MetadataApplyError> {
    let creation_mode_mask = probe_creation_mode_mask_in_fd(jail.root_fd());
    apply_entry_metadata_in_jail_with_mask(jail, entry, preserve_permissions, creation_mode_mask)
}

/// Applies `entry` permissions and timestamps strictly via step-by-step `DirJail` FD traversal
/// with a pre-probed `creation_mode_mask` (`0o777 & !umask`).
pub fn apply_entry_metadata_in_jail_with_mask(
    jail: &DirJail,
    entry: &DeferredEntryMetadata,
    preserve_permissions: bool,
    creation_mode_mask: u32,
) -> Result<(), MetadataApplyError> {
    let comps = jail
        .resolve_path_components(&entry.path)
        .map_err(|_| MetadataApplyError::security(libc::EPERM))?;
    let Some((leaf, parent_comps)) = comps.split_last() else {
        return Ok(());
    };
    let parent_handle = resolve_parent_dir_in_jail(jail, parent_comps)?;
    let parent_fd = parent_handle
        .as_ref()
        .map_or(jail.root_fd(), |h| h.as_raw_fd());
    let leaf_c = CString::new(leaf.as_bytes()).map_err(|_| MetadataApplyError::security(libc::EINVAL))?;

    if entry.is_symlink {
        let pre_st = fstatat_nofollow_raw(parent_fd, &leaf_c)?;
        if let Some((exp_dev, exp_ino)) = entry.expected_identity {
            if (pre_st.st_dev as u64, pre_st.st_ino) != (exp_dev, exp_ino) {
                return Err(MetadataApplyError::security(libc::EPERM));
            }
        }
        if (pre_st.st_mode & libc::S_IFMT) != libc::S_IFLNK {
            return Err(MetadataApplyError::security(libc::EPERM));
        }
        return apply_symlink_mtime_nofollow(parent_fd, &leaf_c, &pre_st, entry);
    }

    let (target_fd, fd_st) = open_target_fd_verified(parent_fd, &leaf_c, entry)?;
    apply_fd_mode_and_mtime(
        target_fd.fd,
        fd_st.st_mode as u32,
        entry,
        preserve_permissions,
        creation_mode_mask,
    )
}

const PARALLEL_METADATA_MIN_FILES: usize = 64;

fn shared_metadata_pool() -> &'static LocalWorkStealingPool {
    static META_POOL: OnceLock<LocalWorkStealingPool> = OnceLock::new();
    META_POOL.get_or_init(|| {
        let threads = ttzip_types::recommended_compute_threads().clamp(2, 6);
        LocalWorkStealingPool::with_threads(threads)
    })
}

/// Applies non-directory file metadata in parallel across a bounded worker pool while preserving
/// deterministic first-error reporting.
pub(crate) fn apply_file_metadata_batch_in_jail(
    jail: &DirJail,
    files: &[DeferredEntryMetadata],
    preserve_permissions: bool,
    creation_mode_mask: u32,
) -> Result<(), MetadataApplyError> {
    if files.len() < PARALLEL_METADATA_MIN_FILES {
        for file in files {
            apply_entry_metadata_in_jail_with_mask(
                jail,
                file,
                preserve_permissions,
                creation_mode_mask,
            )?;
        }
        return Ok(());
    }
    let errors: Vec<Option<MetadataApplyError>> = shared_metadata_pool().install(|| {
        files
            .par_iter()
            .map(|file| {
                apply_entry_metadata_in_jail_with_mask(
                    jail,
                    file,
                    preserve_permissions,
                    creation_mode_mask,
                )
                .err()
            })
            .collect()
    });
    if let Some(first_err) = errors.into_iter().flatten().next() {
        return Err(first_err);
    }
    Ok(())
}

/// Applies `entry` metadata via `DirJail` rooted at `dest_root` (or `entry.path.parent()` if `dest_root` is unset).
pub fn apply_entry_metadata_via_jail(
    dest_root: &Path,
    entry: &DeferredEntryMetadata,
    preserve_permissions: bool,
) -> Result<(), MetadataApplyError> {
    let effective_root = if !dest_root.as_os_str().is_empty() && entry.path.starts_with(dest_root) {
        dest_root
    } else {
        entry.path.parent().unwrap_or_else(|| Path::new("."))
    };
    let jail = DirJail::open(effective_root).map_err(|status| MetadataApplyError {
        status,
        errno: libc::EACCES,
    })?;
    apply_entry_metadata_in_jail(&jail, entry, preserve_permissions)
}

/// Applies `entry` metadata via `DirJail` using a pre-probed `creation_mode_mask`.
pub fn apply_entry_metadata_via_jail_with_mask(
    dest_root: &Path,
    entry: &DeferredEntryMetadata,
    preserve_permissions: bool,
    creation_mode_mask: u32,
) -> Result<(), MetadataApplyError> {
    let effective_root = if !dest_root.as_os_str().is_empty() && entry.path.starts_with(dest_root) {
        dest_root
    } else {
        entry.path.parent().unwrap_or_else(|| Path::new("."))
    };
    let jail = DirJail::open(effective_root).map_err(|status| MetadataApplyError {
        status,
        errno: libc::EACCES,
    })?;
    apply_entry_metadata_in_jail_with_mask(&jail, entry, preserve_permissions, creation_mode_mask)
}

fn record_metadata_failure(
    outcome: &mut ExtractionOutcome,
    committed_rel: &Path,
    err: MetadataApplyError,
) {
    let rel_str = committed_rel.to_string_lossy().into_owned();
    if !outcome.failed.iter().any(|f| f.relative_path == rel_str) {
        outcome.failed.push(FailedItem {
            relative_path: rel_str,
            reason: err.failure_reason(),
            errno: err.errno,
        });
    }
    if outcome.status != OutcomeStatus::Stopped {
        outcome.status = OutcomeStatus::CompletedWithFailures;
    }
}

type MappedEntryList = Vec<(PathBuf, DeferredEntryMetadata)>;

fn collect_mapped_targets(
    plan: &StagedMetadataPlan,
    dest_root: &Path,
    mapping: &CommitPathMapping,
) -> (MappedEntryList, MappedEntryList) {
    let mut mapped_files = Vec::new();
    for file in &plan.files {
        let Ok(staged_rel) = file.path.strip_prefix(&plan.staging_payload_root) else {
            continue;
        };
        let Some(committed_rel) = mapping.resolve_committed_path(staged_rel) else {
            continue;
        };
        let mut mapped = file.clone();
        mapped.path = dest_root.join(&committed_rel);
        mapped_files.push((committed_rel, mapped));
    }

    let mut mapped_dirs = Vec::new();
    for dir in plan.explicit_dirs.iter().chain(plan.implicit_dirs.iter()) {
        let Ok(staged_rel) = dir.path.strip_prefix(&plan.staging_payload_root) else {
            continue;
        };
        if staged_rel.as_os_str().is_empty() {
            continue;
        }
        let Some(committed_rel) = mapping.resolve_committed_path(staged_rel) else {
            continue;
        };
        let mut mapped = dir.clone();
        mapped.path = dest_root.join(&committed_rel);
        mapped_dirs.push((committed_rel, mapped));
    }

    mapped_dirs.sort_by(|(_, a), (_, b)| {
        b.depth()
            .cmp(&a.depth())
            .then_with(|| b.path.cmp(&a.path))
    });

    (mapped_files, mapped_dirs)
}

/// Phase 3: Applies the staged metadata plan to `dest_root` via `DirJail` using the
/// [`CommitPathMapping`] produced by `DFSCommitWalker::commit_with_mapping`, recording any
/// per-entry metadata errors into `outcome`.
pub fn apply_committed_metadata(
    staging_payload_root: &Path,
    dest_root: &Path,
    mapping: &CommitPathMapping,
    outcome: &mut ExtractionOutcome,
) {
    let plan_opt = {
        let mut guard = plan_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.remove(staging_payload_root)
    };
    if let Some(hook) = take_pre_metadata_hook(dest_root) {
        hook(dest_root);
    }
    let Some(plan) = plan_opt else {
        return;
    };

    let (mapped_files, mapped_dirs) = collect_mapped_targets(&plan, dest_root, mapping);
    if mapped_files.is_empty() && mapped_dirs.is_empty() {
        return;
    }

    let commit_mask = DirJail::open(staging_payload_root)
        .map(|j| probe_creation_mode_mask_in_fd(j.root_fd()))
        .unwrap_or(plan.creation_mode_mask);
    let creation_mode_mask = plan.creation_mode_mask & commit_mask;

    let jail = match DirJail::open(dest_root) {
        Ok(j) => j,
        Err(status) => {
            let err = MetadataApplyError {
                status,
                errno: if status == TTZipStatus::ErrSecurityViolation {
                    libc::ELOOP
                } else {
                    libc::EACCES
                },
            };
            for (rel, _) in mapped_files.iter().chain(mapped_dirs.iter()) {
                record_metadata_failure(outcome, rel, err);
            }
            return;
        }
    };

    apply_committed_file_metadata(
        &jail,
        &mapped_files,
        plan.preserve_permissions,
        creation_mode_mask,
        outcome,
    );

    for (committed_rel, mapped) in &mapped_dirs {
        if mapped.expected_identity.is_none() {
            record_metadata_failure(outcome, committed_rel, MetadataApplyError::security(libc::EPERM));
            continue;
        }
        if let Err(err) = apply_entry_metadata_in_jail_with_mask(
            &jail,
            mapped,
            plan.preserve_permissions,
            creation_mode_mask,
        ) {
            record_metadata_failure(outcome, committed_rel, err);
        }
    }
}

fn apply_committed_file_metadata(
    jail: &DirJail,
    mapped_files: &MappedEntryList,
    preserve_permissions: bool,
    creation_mode_mask: u32,
    outcome: &mut ExtractionOutcome,
) {
    let apply_one = |mapped: &DeferredEntryMetadata| -> Option<MetadataApplyError> {
        if mapped.expected_identity.is_none() {
            return Some(MetadataApplyError::security(libc::EPERM));
        }
        apply_entry_metadata_in_jail_with_mask(jail, mapped, preserve_permissions, creation_mode_mask)
            .err()
    };

    if mapped_files.len() < PARALLEL_METADATA_MIN_FILES {
        for (committed_rel, mapped) in mapped_files {
            if let Some(err) = apply_one(mapped) {
                record_metadata_failure(outcome, committed_rel, err);
            }
        }
        return;
    }

    let errors: Vec<Option<MetadataApplyError>> = shared_metadata_pool().install(|| {
        mapped_files
            .par_iter()
            .map(|(_, mapped)| apply_one(mapped))
            .collect()
    });
    for ((committed_rel, _), err_opt) in mapped_files.iter().zip(errors) {
        if let Some(err) = err_opt {
            record_metadata_failure(outcome, committed_rel, err);
        }
    }
}
