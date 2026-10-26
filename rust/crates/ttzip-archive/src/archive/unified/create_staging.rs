// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Transactional staging, single-file atomic replacement, and multi-volume verified rollback
//! with backup preservation for archive creation.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::archive::split::VolumeNamingScheme;
use crate::types::{TTZipArchiveFormat, TTZipStatus};

static CREATE_TXN_SEQ: AtomicU64 = AtomicU64::new(1);

/// Explicit commit mode and volume naming scheme supplied by the archive creation caller.
///
/// Never inferred from filenames on disk: unsplit creation (`split_volume_size_bytes == 0`)
/// always uses `SingleFile` even when the target filename ends with `.001`, whereas split-volume
/// creation (`split_volume_size_bytes > 0`) always uses `SplitVolume(scheme)` even when the
/// compressed output fits into the first volume alone.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CreateCommitMode {
    /// Unsplit single-file archive creation (`split_volume_size_bytes == 0`).
    /// Commits via direct POSIX atomic `rename(staged_file, destination_path)` without scanning
    /// or touching any adjacent files.
    SingleFile,
    /// Multi-volume split archive creation (`split_volume_size_bytes > 0`) with an explicit
    /// `VolumeNamingScheme`. Commits via the two-phase `.ttzip_create_bak_*` backup/rollback
    /// transaction and retires stale tail volumes belonging to the confirmed contiguous chain.
    SplitVolume(VolumeNamingScheme),
}

impl CreateCommitMode {
    /// Resolves the canonical split base path (stripping a trailing `.001` if present) and
    /// `VolumeNamingScheme` for split-volume creation.
    pub fn resolve_split_target(
        destination_path: &Path,
        resolved_format: TTZipArchiveFormat,
    ) -> (PathBuf, VolumeNamingScheme) {
        if destination_path.extension().and_then(|e| e.to_str()) == Some("001") {
            (
                destination_path.with_extension(""),
                VolumeNamingScheme::NumberedExtension,
            )
        } else if matches!(
            resolved_format,
            TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto
        ) {
            (
                destination_path.to_path_buf(),
                VolumeNamingScheme::PkzipSpanned,
            )
        } else {
            (
                destination_path.to_path_buf(),
                VolumeNamingScheme::NumberedExtension,
            )
        }
    }

    /// Resolves the explicit `CreateCommitMode` from `destination_path`, `resolved_format`,
    /// and `split_volume_size_bytes`.
    pub fn for_creation(
        destination_path: &Path,
        resolved_format: TTZipArchiveFormat,
        split_volume_size_bytes: u64,
    ) -> Self {
        if split_volume_size_bytes == 0 {
            return Self::SingleFile;
        }
        let (_, scheme) = Self::resolve_split_target(destination_path, resolved_format);
        Self::SplitVolume(scheme)
    }
}

/// Detailed error returned when staging or committing a newly created archive fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateStageCommitError {
    /// Compression or commit failed with all pre-existing destination files intact or cleanly restored.
    Status(TTZipStatus),
    /// Multi-volume commit failed and one or more pre-existing volumes could not be restored from
    /// `backup_dir`. `backup_dir` is preserved on disk so no backup copy is lost.
    RollbackIncomplete {
        status: TTZipStatus,
        backup_dir: PathBuf,
        unrestored_origins: Vec<PathBuf>,
        lingering_installed: Vec<PathBuf>,
    },
}

impl CreateStageCommitError {
    /// Returns the underlying `TTZipStatus` code.
    pub fn status(&self) -> TTZipStatus {
        match self {
            Self::Status(s) => *s,
            Self::RollbackIncomplete { status, .. } => *status,
        }
    }

    /// Returns the preserved backup directory path when rollback could not restore all volumes.
    pub fn backup_dir(&self) -> Option<&Path> {
        match self {
            Self::Status(_) => None,
            Self::RollbackIncomplete { backup_dir, .. } => Some(backup_dir.as_path()),
        }
    }

    /// Returns the original target paths of volumes that remain inside `backup_dir`.
    pub fn unrestored_origins(&self) -> &[PathBuf] {
        match self {
            Self::Status(_) => &[],
            Self::RollbackIncomplete { unrestored_origins, .. } => unrestored_origins.as_slice(),
        }
    }
}

impl fmt::Display for CreateStageCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Status(status) => write!(f, "Archive creation failed with status {:?}", status),
            Self::RollbackIncomplete {
                status,
                backup_dir,
                unrestored_origins,
                lingering_installed,
            } => write!(
                f,
                "Multi-volume archive commit failed ({:?}) and rollback was incomplete; preserved backup directory '{}' contains unrestored volume(s): {:?} (lingering installed: {:?})",
                status,
                backup_dir.display(),
                unrestored_origins,
                lingering_installed
            ),
        }
    }
}

fn next_txn_token() -> (u32, u128, u64) {
    let seq = CREATE_TXN_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (std::process::id(), nanos, seq)
}

fn strip_first_volume_suffix(name: &str) -> &str {
    name.strip_suffix(".001")
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name)
}

fn is_non_dir_path(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| !m.file_type().is_dir())
        .unwrap_or(false)
}

fn collect_staged_files(txn_dir: &Path) -> Result<Vec<PathBuf>, CreateStageCommitError> {
    let entries = fs::read_dir(txn_dir)
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    let mut staged_files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
        staged_files.push(entry.path());
    }
    if staged_files.is_empty() {
        return Err(CreateStageCommitError::Status(TTZipStatus::ErrCompressionFailed));
    }
    staged_files.sort();
    Ok(staged_files)
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

/// Collects only the existing files in `parent` that are confirmed members of the multi-volume
/// split archive being replaced under the caller-supplied `VolumeNamingScheme`:
/// 1. Existing non-directory files whose names match one of `staged_names`.
/// 2. When `scheme` is `NumberedExtension` or `RawSplit` and `<canon_base>.001` exists on disk,
///    the unbroken contiguous `.001, .002, ...` chain starting at `.001`.
/// 3. When `scheme` is `PkzipSpanned` and `<stem>.z01` exists on disk,
///    the unbroken contiguous `.z01, .z02, ...` chain starting at `.z01` plus `<stem>.zip`.
///
/// Unrelated numeric-suffixed files (`report.zip.1`, `report.zip.2026`, or non-contiguous gaps)
/// are never matched.
fn collect_confirmed_existing_split_volumes(
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

fn rollback_volume_commit_with<B>(
    installed: &[PathBuf],
    backed_up: &[(PathBuf, PathBuf)],
    backup_dir: &Path,
    cause_status: TTZipStatus,
    mut restore_backup: B,
) -> CreateStageCommitError
where
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let mut lingering_installed = Vec::new();
    for dst in installed.iter().rev() {
        if is_non_dir_path(dst) && fs::remove_file(dst).is_err() {
            lingering_installed.push(dst.clone());
        }
    }

    let mut unrestored_origins = Vec::new();
    for (orig_dst, bak_src) in backed_up.iter().rev() {
        if restore_backup(bak_src, orig_dst).is_err() {
            unrestored_origins.push(orig_dst.clone());
        }
    }

    if unrestored_origins.is_empty() && lingering_installed.is_empty() {
        let _ = fs::remove_dir_all(backup_dir);
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
pub fn recover_preserved_create_backup(backup_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
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

fn commit_single_staged_file_with<R>(
    staged_files: &[PathBuf],
    parent: &Path,
    base_file_name: &str,
    mut rename_staged: R,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    if staged_files.len() != 1
        || staged_files[0].file_name().and_then(|n| n.to_str()) != Some(base_file_name)
    {
        return Err(CreateStageCommitError::Status(
            TTZipStatus::ErrCompressionFailed,
        ));
    }
    let dst_file = parent.join(base_file_name);
    rename_staged(&staged_files[0], &dst_file)
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    Ok(vec![dst_file])
}

fn commit_split_staged_volumes_with<R, B>(
    staged_files: Vec<PathBuf>,
    parent: &Path,
    base_file_name: &str,
    scheme: VolumeNamingScheme,
    mut rename_staged: R,
    restore_backup: B,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
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
    let backup_dir = parent.join(format!(".ttzip_create_bak_{pid}_{nanos}_{seq}"));
    let mut backed_up: Vec<(PathBuf, PathBuf)> = Vec::new();

    if !existing_files.is_empty() {
        fs::create_dir_all(&backup_dir)
            .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
        for orig_path in existing_files {
            let Some(fname) = orig_path.file_name() else {
                continue;
            };
            let bak_path = backup_dir.join(fname);
            if fs::rename(&orig_path, &bak_path).is_err() {
                return Err(rollback_volume_commit_with(
                    &[],
                    &backed_up,
                    &backup_dir,
                    TTZipStatus::ErrOpenFailed,
                    restore_backup,
                ));
            }
            backed_up.push((orig_path, bak_path));
        }
    }

    let mut installed: Vec<PathBuf> = Vec::with_capacity(staged_files.len());
    for src_file in staged_files {
        let Some(fname) = src_file.file_name() else {
            continue;
        };
        let dst_file = parent.join(fname);
        if rename_staged(&src_file, &dst_file).is_err() {
            return Err(rollback_volume_commit_with(
                &installed,
                &backed_up,
                &backup_dir,
                TTZipStatus::ErrOpenFailed,
                restore_backup,
            ));
        }
        installed.push(dst_file);
    }

    if !backed_up.is_empty() {
        let _ = fs::remove_dir_all(&backup_dir);
    }
    Ok(installed)
}

/// Commits staged archive output into `parent` according to the explicit caller-supplied `mode`:
/// - `CreateCommitMode::SingleFile`: direct POSIX atomic `rename_staged(staged_file, dst_file)`
///   with zero missing-target window and zero neighbor scanning, even if `base_file_name` ends with `.001`.
/// - `CreateCommitMode::SplitVolume(scheme)`: two-phase backup of confirmed contiguous split volumes
///   matching `scheme` into `.ttzip_create_bak_*` (even if only 1 volume was generated), sequential
///   installation of new volumes, and verified rollback via `restore_backup`.
pub fn commit_staged_archive_files_with<R, B>(
    txn_dir: &Path,
    parent: &Path,
    base_file_name: &str,
    mode: CreateCommitMode,
    rename_staged: R,
    restore_backup: B,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let staged_files = collect_staged_files(txn_dir)?;
    match mode {
        CreateCommitMode::SingleFile => {
            commit_single_staged_file_with(&staged_files, parent, base_file_name, rename_staged)
        }
        CreateCommitMode::SplitVolume(scheme) => commit_split_staged_volumes_with(
            staged_files,
            parent,
            base_file_name,
            scheme,
            rename_staged,
            restore_backup,
        ),
    }
}

struct CreateStagingGuard<'a> {
    dir: &'a Path,
    committed: bool,
}

impl<'a> Drop for CreateStagingGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(self.dir);
        }
    }
}

/// Executes `work` against a staged destination path and commits generated output using explicit
/// `mode` and custom `rename_staged` / `restore_backup` hooks, returning detailed rollback diagnostics.
pub fn with_staged_archive_destination_detailed_using<F, R, B>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
    rename_staged: R,
    restore_backup: B,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    B: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let file_name = destination_path
        .file_name()
        .ok_or(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam))?;
    let base_file_name = file_name
        .to_str()
        .ok_or(CreateStageCommitError::Status(TTZipStatus::ErrInvalidParam))?;
    let parent = destination_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;
    }

    let (pid, nanos, seq) = next_txn_token();
    let txn_dir = parent.join(format!(".ttzip_create_txn_{pid}_{nanos}_{seq}"));
    fs::create_dir_all(&txn_dir)
        .map_err(|_| CreateStageCommitError::Status(TTZipStatus::ErrOpenFailed))?;

    let mut guard = CreateStagingGuard {
        dir: &txn_dir,
        committed: false,
    };

    let staged_dest = txn_dir.join(file_name);
    work(&staged_dest).map_err(CreateStageCommitError::Status)?;

    let committed_files = commit_staged_archive_files_with(
        &txn_dir,
        parent,
        base_file_name,
        mode,
        rename_staged,
        restore_backup,
    )?;
    guard.committed = true;
    let _ = fs::remove_dir_all(&txn_dir);
    Ok(committed_files)
}

/// Executes `work` against a staged destination path and commits according to `mode`, returning detailed rollback diagnostics.
pub fn with_staged_archive_destination_detailed<F>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
) -> Result<Vec<PathBuf>, CreateStageCommitError>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
{
    with_staged_archive_destination_detailed_using(
        destination_path,
        mode,
        work,
        |from, to| fs::rename(from, to),
        |from, to| fs::rename(from, to),
    )
}

/// Executes `work` against a staged destination path with explicit `mode` and a custom `rename_staged` hook.
pub fn with_staged_archive_destination_using<F, R>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
    rename_staged: R,
) -> Result<Vec<PathBuf>, TTZipStatus>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    with_staged_archive_destination_detailed_using(
        destination_path,
        mode,
        work,
        rename_staged,
        |from, to| fs::rename(from, to),
    )
    .map_err(|e| e.status())
}

/// Executes `work` against a staged destination path and commits generated output according to `mode`.
pub fn with_staged_archive_destination<F>(
    destination_path: &Path,
    mode: CreateCommitMode,
    work: F,
) -> Result<Vec<PathBuf>, TTZipStatus>
where
    F: FnOnce(&Path) -> Result<(), TTZipStatus>,
{
    with_staged_archive_destination_detailed(destination_path, mode, work).map_err(|e| e.status())
}

// ---------------------------------------------------------------------------
// Bounded Streaming XAR 1.0 Primitives (Temporary Heap File + Chunked Zlib)
// ---------------------------------------------------------------------------

pub(crate) struct ScopedTempFile(pub(crate) PathBuf);

impl ScopedTempFile {
    pub(crate) fn in_parent_of(destination_path: &Path, prefix: &str) -> Self {
        let (pid, nanos, seq) = next_txn_token();
        let parent = destination_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        Self(parent.join(format!(".{prefix}_{pid}_{nanos}_{seq}.tmp")))
    }
}

impl Drop for ScopedTempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) enum XarNodeKind {
    Directory {
        children: std::collections::BTreeMap<String, XarTreeNode>,
    },
    File {
        offset: u64,
        length: u64,
        size: u64,
        ext_sha256: String,
        arc_sha256: String,
    },
    Symlink {
        target: String,
    },
}

pub(crate) struct XarTreeNode {
    pub(crate) mode: u32,
    pub(crate) kind: XarNodeKind,
}

struct XarHashingSink<'a> {
    out: &'a mut fs::File,
    hasher: sha2::Sha256,
    written: u64,
}

impl std::io::Write for XarHashingSink<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use sha2::Digest;
        self.out.write_all(buf)?;
        self.hasher.update(buf);
        self.written += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

pub(crate) fn check_xar_progress(
    options: &crate::types::TTZipCreateOptions,
    processed: u64,
    total: u64,
    entry_name: &str,
) -> Result<(), TTZipStatus> {
    if let Some(cb) = options.progress_callback {
        let c_name = std::ffi::CString::new(entry_name).unwrap_or_default();
        let keep_going = unsafe { cb(processed, total, c_name.as_ptr(), options.user_data) };
        if !keep_going {
            return Err(TTZipStatus::Cancelled);
        }
    }
    Ok(())
}

pub(crate) struct XarStreamState<'a> {
    pub(crate) heap_file: &'a mut fs::File,
    pub(crate) heap_pos: u64,
    pub(crate) comp_level: flate2::Compression,
    pub(crate) options: &'a crate::types::TTZipCreateOptions,
    pub(crate) processed_bytes: u64,
    pub(crate) total_bytes: u64,
}

pub(crate) fn stream_compress_xar_file(
    abs_path: &Path,
    clean_rel: &str,
    state: &mut XarStreamState<'_>,
) -> Result<XarNodeKind, TTZipStatus> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};

    check_xar_progress(state.options, state.processed_bytes, state.total_bytes, clean_rel)?;
    let mut src = fs::File::open(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let start_offset = 32 + state.heap_pos;
    let mut ext_hasher = Sha256::new();
    let mut uncomp_size = 0u64;
    let sink = XarHashingSink {
        out: state.heap_file,
        hasher: Sha256::new(),
        written: 0,
    };
    let mut encoder = flate2::write::ZlibEncoder::new(sink, state.comp_level);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = src
            .read(&mut buf)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        if n == 0 {
            break;
        }
        ext_hasher.update(&buf[..n]);
        encoder
            .write_all(&buf[..n])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        uncomp_size += n as u64;
        state.processed_bytes = state.processed_bytes.saturating_add(n as u64);
        check_xar_progress(state.options, state.processed_bytes, state.total_bytes, clean_rel)?;
    }
    let finished = encoder
        .finish()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    state.heap_pos += finished.written;
    Ok(XarNodeKind::File {
        offset: start_offset,
        length: finished.written,
        size: uncomp_size,
        ext_sha256: hex_lower(&ext_hasher.finalize()),
        arc_sha256: hex_lower(&finished.hasher.finalize()),
    })
}

fn xml_escape_xar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn insert_xar_tree_node(
    root: &mut std::collections::BTreeMap<String, XarTreeNode>,
    parts: &[&str],
    leaf: XarTreeNode,
) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        let key = parts[0].to_string();
        if let (Some(existing), XarNodeKind::Directory { .. }) = (root.get_mut(&key), &leaf.kind) {
            existing.mode = leaf.mode;
            return;
        }
        root.insert(key, leaf);
        return;
    }
    let dir_entry = root.entry(parts[0].to_string()).or_insert_with(|| XarTreeNode {
        mode: 0o755,
        kind: XarNodeKind::Directory { children: std::collections::BTreeMap::new() },
    });
    if let XarNodeKind::Directory { ref mut children } = dir_entry.kind {
        insert_xar_tree_node(children, &parts[1..], leaf);
    }
}

pub(crate) fn render_xar_tree_xml(
    nodes: &std::collections::BTreeMap<String, XarTreeNode>,
    next_id: &mut u64,
    out: &mut String,
) {
    for (name, node) in nodes {
        let id = *next_id;
        *next_id += 1;
        let esc_name = xml_escape_xar(name);
        let perm = node.mode & 0o7777;
        match &node.kind {
            XarNodeKind::Directory { children } => {
                out.push_str(&format!(
                    "  <file id=\"{id}\"><type>directory</type><mode>{perm:04o}</mode><name>{esc_name}</name>\n"
                ));
                render_xar_tree_xml(children, next_id, out);
                out.push_str("  </file>\n");
            }
            XarNodeKind::Symlink { target } => {
                let esc_target = xml_escape_xar(target);
                out.push_str(&format!(
                    "  <file id=\"{id}\"><type>symlink</type><mode>{perm:04o}</mode><name>{esc_name}</name><link type=\"file\">{esc_target}</link><target>{esc_target}</target></file>\n"
                ));
            }
            XarNodeKind::File { offset, length, size, ext_sha256, arc_sha256 } => {
                out.push_str(&format!(
                    "  <file id=\"{id}\"><data><length>{length}</length><offset>{offset}</offset><size>{size}</size><encoding style=\"application/x-gzip\"/><extracted-checksum style=\"sha256\">{ext_sha256}</extracted-checksum><archived-checksum style=\"sha256\">{arc_sha256}</archived-checksum></data><mode>{perm:04o}</mode><type>file</type><name>{esc_name}</name></file>\n"
                ));
            }
        }
    }
}



