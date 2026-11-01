// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unified Archive Lifecycle Orchestrator Engine.
//!
//! Supports:
//! 1. Zero-copy / stream auto-detection across 17 archive/compression formats.
//! 2. Single-entry and multi-file recursive archive creation with optional encryption and split volumes.
//! 3. Two-stage secure extraction with ZipSlip defense, APFS extent preallocation, and bottom-up metadata.
//! 4. Non-destructive archive inspection with optional automatic charset detection.
//! 5. Self-healing archive repair integrating RS-FEC streaming and damaged header recovery.

pub mod container_writers;
pub mod create;
pub mod create_guard_settlement;
pub mod create_split_rollback;
pub mod create_staging;
pub mod create_to_writer;
pub mod create_transaction;
pub mod detect;
pub mod entry;
pub mod extract;
pub mod extract_single;
pub mod fallback_policy;
pub mod filter_pipeline;
pub mod format_config;
pub mod format_sniffer;
pub mod inspect;
pub mod repair;
pub mod selection;
pub mod stream_adapter;
pub mod verify;
pub mod zip_central_directory;
#[cfg(test)]
pub mod tests_detection;
#[cfg(test)]
pub mod tests_full_extract_ownership;
#[cfg(test)]
pub mod tests_lifecycle;
#[cfg(test)]
pub mod tests_multivolume_selective;

pub use create::{create_archive, create_archive_to_writer};
pub use create_transaction::{
    ArchiveCleanupDisposition, BackupRecoveryEntryFailure, CloseScopePrecondition,
    CreateBackupRecoveryAttempt, CreateBackupRecoveryState, CreateFileAccessMode,
    CreateTransactionContext, CreateTransactionFailureReport, DescriptorReleaseStatus,
    ExplicitFileFinishProof, FileFinishSyncPolicy, ProducerTicket, ResidualPathRole,
    StageCleanupFailure, StageFileFinishDiagnostic, StageFileFinishError,
};
pub use detect::{detect_archive_sniff, detect_format, resolve_create_format};
pub use entry::{
    coalesce_sparse_extents, clean_sparse_extents, EntryFields, LinkAction, LinkResolver,
    LinkResolverStrategy, SparseExtent, TTZipEntry, TTZipFileType, TTZipTimestamp,
};
pub use extract::{extract_archive, extract_archive_with_metrics, extract_archive_with_sheet};
pub use extract_single::{extract_selected_entries, extract_single_entry_memory};
pub use selection::{normalize_entry_path, EntrySelection};
pub use filter_pipeline::{
    FilterKind, FilterPipeline, FilterPipelineError, FilterPipelineResult,
    StreamFilter, MAX_FILTER_CHAIN_DEPTH,
};
pub use format_sniffer::{
    ArchiveFormat, ArchiveFormatBidder, BidResult, BidScore, FormatBidderRegistry, FormatSniffer,
    SeekableStream, SniffAnchor, SniffResult, SniffRule, SNIFF_RULES,
};
pub use inspect::{inspect_archive, inspect_archive_c_compat, ArchiveEntryRef};
pub use repair::repair_archive;
pub use stream_adapter::{
    LookaheadRead, SlidingLookaheadReader, DEFAULT_INITIAL_LOOKAHEAD_CAPACITY,
    MAX_LOOKAHEAD_CAPACITY, MICRO_BUFFER_CHUNK_SIZE,
};
pub use verify::{verify_archive_stream, ArchiveIntegrityReport, CorruptedEntryDetail};

use std::path::{Path, PathBuf};

use crate::standards::signatures::{CompoundFormat, DetectedFormat};
use crate::types::{
    TTZipArchiveFormat, TTZipCreateOptions, TTZipExtractOptions, TTZipInspectCallback, TTZipStatus,
};
use libc::c_void;

/// Unified Archive Orchestrator Engine.
pub struct UnifiedArchiveOrchestrator;

impl UnifiedArchiveOrchestrator {
    /// Three-state sniffing across 50+ archive, disk image, and compressed formats from memory buffer.
    #[inline]
    pub fn sniff_format(buffer: &[u8]) -> SniffResult {
        FormatSniffer::sniff(buffer)
    }

    /// Three-state sniffing across 50+ archive, disk image, and compressed formats from file.
    #[inline]
    pub fn sniff_file(path: &Path) -> std::io::Result<SniffResult> {
        FormatSniffer::sniff_file(path)
    }

    /// Detects format of an archive from file headers with extension fallback.
    #[inline]
    pub fn detect_format(path: &Path) -> Result<(DetectedFormat, Option<CompoundFormat>), TTZipStatus> {
        detect::detect_format(path)
    }

    /// Resolves target archive creation format from explicit format and destination path.
    #[inline]
    pub fn resolve_create_format(format: TTZipArchiveFormat, dest_path: &Path) -> TTZipArchiveFormat {
        detect::resolve_create_format(format, dest_path)
    }

    /// Recursively compresses source paths into the destination archive path.
    #[inline]
    pub fn create_archive(
        source_paths: &[PathBuf],
        destination_path: &Path,
        options: &TTZipCreateOptions,
        split_volume_size_bytes: u64,
    ) -> Result<(), TTZipStatus> {
        create::create_archive(source_paths, destination_path, options, split_volume_size_bytes)
    }

    /// Recursively compresses explicit pre-collected entries into the destination archive path.
    #[inline]
    pub fn create_archive_explicit(
        entries: &[(PathBuf, String)],
        destination_path: &Path,
        options: &TTZipCreateOptions,
        split_volume_size_bytes: u64,
    ) -> Result<(), TTZipStatus> {
        create::create_archive_explicit(entries, destination_path, options, split_volume_size_bytes)
    }

    /// Recursively compresses source paths into a streaming writer (0% disk write amplification).
    #[inline]
    pub fn create_archive_to_writer<W: std::io::Write>(
        source_paths: &[PathBuf],
        writer: W,
        options: &TTZipCreateOptions,
    ) -> Result<u64, TTZipStatus> {
        create::create_archive_to_writer(source_paths, writer, options)
    }

    /// Extracts an archive to destination directory with security verification and APFS preallocation.
    #[inline]
    pub fn extract_archive(
        archive_path: &Path,
        staging: &crate::fs::collision::staging::StagingRoot,
        options: &TTZipExtractOptions,
    ) -> Result<(), TTZipStatus> {
        extract::extract_archive(archive_path, staging, options)
    }

    /// Extracts an archive and returns total extracted uncompressed bytes.
    #[inline]
    pub fn extract_archive_with_metrics(
        archive_path: &Path,
        staging: &crate::fs::collision::staging::StagingRoot,
        options: &TTZipExtractOptions,
    ) -> Result<u64, TTZipStatus> {
        extract::extract_archive_with_metrics(archive_path, staging, options)
    }

    /// Extracts an archive with an optional atomic progress sheet for non-blocking cancellation and progress tracking.
    #[inline]
    pub fn extract_archive_with_sheet(
        archive_path: &Path,
        staging: &crate::fs::collision::staging::StagingRoot,
        options: &TTZipExtractOptions,
        sheet: Option<&crate::types::ProgressAtomicSheet>,
    ) -> Result<u64, TTZipStatus> {
        extract::extract_archive_with_sheet(archive_path, staging, options, sheet)
    }


    /// Inspects an archive and invokes the callback for every discovered entry metadata item.
    #[inline]
    pub fn inspect_archive(
        archive_path: &Path,
        password: Option<&str>,
        detect_encoding: bool,
        callback: TTZipInspectCallback,
        user_data: *mut c_void,
    ) -> Result<usize, TTZipStatus> {
        inspect::inspect_archive_c_compat(archive_path, password, detect_encoding, callback, user_data)
    }

    /// Inspects an archive using pure Safe Rust closure with zero-copy borrowed `ArchiveEntryRef`.
    #[inline]
    pub fn inspect_archive_safe<F>(
        archive_path: &Path,
        password: Option<&str>,
        detect_encoding: bool,
        callback: F,
    ) -> Result<usize, TTZipStatus>
    where
        F: FnMut(&ArchiveEntryRef<'_>) -> bool,
    {
        inspect::inspect_archive(archive_path, password, detect_encoding, callback)
    }

    /// Auto-detects format and repairs damaged archive.
    #[inline]
    pub fn repair_archive(
        damaged_path: &Path,
        repaired_path: &Path,
    ) -> Result<usize, TTZipStatus> {
        repair::repair_archive(damaged_path, repaired_path)
    }

    /// Selective extraction into a staging root.
    #[inline]
    pub fn extract_selected_entries(
        archive_path: &Path,
        selection: &EntrySelection,
        staging: &crate::fs::collision::staging::StagingRoot,
        options: &TTZipExtractOptions,
    ) -> Result<usize, TTZipStatus> {
        extract_single::extract_selected_entries(archive_path, selection, staging, options)
    }
}

// ---------------------------------------------------------------------------
// Stage Cleanup & Post-Commit Diagnostics + Test Hooks
// ---------------------------------------------------------------------------

/// Diagnostic recorded when archive commit succeeds on disk but removing a post-commit staging or backup directory fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostCommitCleanupIssue {
    pub path: PathBuf,
    pub role: &'static str,
    pub message: String,
    pub errno: i32,
}

type InnerCleanupRecord = (Option<TTZipStatus>, PathBuf, String);
type BeforeWorkHook = Box<dyn FnOnce(&Path) + Send + 'static>;
type AfterWorkHook = Box<dyn FnOnce(&Path, &Result<(), TTZipStatus>) + Send + 'static>;

thread_local! {
    static LAST_INNER_STAGE_CLEANUP_ERR: std::cell::RefCell<Option<InnerCleanupRecord>> =
        const { std::cell::RefCell::new(None) };
    static LAST_POST_COMMIT_CLEANUP_ISSUES: std::cell::RefCell<Vec<PostCommitCleanupIssue>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static CREATE_STAGE_BEFORE_WORK_HOOK: std::cell::RefCell<Option<BeforeWorkHook>> =
        const { std::cell::RefCell::new(None) };
    static CREATE_STAGE_AFTER_WORK_HOOK: std::cell::RefCell<Option<AfterWorkHook>> =
        const { std::cell::RefCell::new(None) };
}

/// Clears thread-local staging cleanup diagnostics before an archive creation operation begins.
pub fn clear_create_stage_diagnostics() {
    LAST_INNER_STAGE_CLEANUP_ERR.with(|slot| *slot.borrow_mut() = None);
    LAST_POST_COMMIT_CLEANUP_ISSUES.with(|slot| slot.borrow_mut().clear());
}

/// Records an inner staged file cleanup error with the primary operation status.
pub fn record_inner_stage_cleanup_error_with_status(
    path: impl Into<PathBuf>,
    primary_status: TTZipStatus,
    err: &std::io::Error,
) {
    let p = path.into();
    let msg = format!("{err} (kind={:?}, os_err={:?})", err.kind(), err.raw_os_error());
    LAST_INNER_STAGE_CLEANUP_ERR.with(|slot| {
        let mut guard = slot.borrow_mut();
        if guard.is_none() {
            *guard = Some((Some(primary_status), p, msg));
        }
    });
}

/// Records an inner staged file cleanup error.
pub fn record_inner_stage_cleanup_error(path: impl Into<PathBuf>, err: &std::io::Error) {
    let p = path.into();
    let msg = format!("{err} (kind={:?}, os_err={:?})", err.kind(), err.raw_os_error());
    LAST_INNER_STAGE_CLEANUP_ERR.with(|slot| {
        let mut guard = slot.borrow_mut();
        if guard.is_none() {
            *guard = Some((None, p, msg));
        }
    });
}

/// Takes the most recent inner staged file cleanup error recorded on the current thread.
pub fn take_last_inner_stage_cleanup_error() -> Option<(PathBuf, String)> {
    LAST_INNER_STAGE_CLEANUP_ERR
        .with(|slot| slot.borrow_mut().take().map(|(_, p, m)| (p, m)))
}

pub(crate) fn take_last_inner_stage_cleanup_record() -> Option<InnerCleanupRecord> {
    LAST_INNER_STAGE_CLEANUP_ERR.with(|slot| slot.borrow_mut().take())
}

/// Records a post-commit cleanup failure (`.ttzip_create_txn_*` or `.ttzip_create_bak_*`) after target files are committed.
pub fn record_post_commit_cleanup_issue(path: PathBuf, role: &'static str, err: &std::io::Error) {
    let errno = err.raw_os_error().unwrap_or(libc::EIO);
    let message = format!("{role} '{}': {err}", path.display());
    LAST_POST_COMMIT_CLEANUP_ISSUES.with(|slot| {
        slot.borrow_mut().push(PostCommitCleanupIssue {
            path,
            role,
            message,
            errno,
        });
    });
}

/// Takes any post-commit cleanup issues recorded on the current thread.
pub fn take_last_post_commit_cleanup_issues() -> Vec<PostCommitCleanupIssue> {
    LAST_POST_COMMIT_CLEANUP_ISSUES.with(|slot| std::mem::take(&mut *slot.borrow_mut()))
}

/// Registers a one-shot hook invoked with `txn_dir` immediately after `.ttzip_create_txn_*` is created and before `work` runs.
pub fn set_create_stage_before_work_hook<F>(hook: F)
where
    F: FnOnce(&Path) + Send + 'static,
{
    CREATE_STAGE_BEFORE_WORK_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

/// Registers a one-shot hook invoked with `(txn_dir, &work_result)` immediately after `work` returns and before commit/cleanup.
pub fn set_create_stage_after_work_hook<F>(hook: F)
where
    F: FnOnce(&Path, &Result<(), TTZipStatus>) + Send + 'static,
{
    CREATE_STAGE_AFTER_WORK_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

pub(crate) fn run_before_work_hook(txn_dir: &Path) {
    let hook = CREATE_STAGE_BEFORE_WORK_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(cb) = hook {
        cb(txn_dir);
    }
}

pub(crate) fn run_after_work_hook(txn_dir: &Path, res: &Result<(), TTZipStatus>) {
    let hook = CREATE_STAGE_AFTER_WORK_HOOK.with(|slot| slot.borrow_mut().take());
    if let Some(cb) = hook {
        cb(txn_dir, res);
    }
}

// ---------------------------------------------------------------------------
// Bounded Streaming XAR 1.0 Primitives (Temporary Heap File + Chunked Zlib)
// ---------------------------------------------------------------------------

/// RAII wrapper for temporary helper files created inside a creation transaction (`O_RDWR`).
pub struct ScopedTempFile {
    pub path: PathBuf,
    guard: Option<crate::fs::abort_guard::FileAbortGuard>,
    cleanup_status: TTZipStatus,
}

impl ScopedTempFile {
    /// Exclusively creates a read-write temporary helper file inside `destination_path`'s parent directory.
    pub fn create_read_write_in_txn(
        destination_path: &Path,
        prefix: &str,
        txn_ctx: Option<&CreateTransactionContext>,
        role: ResidualPathRole,
    ) -> Result<(Self, std::fs::File), TTZipStatus> {
        let (pid, nanos, seq) = create_staging::next_txn_token();
        let parent = destination_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let path = parent.join(format!(".{prefix}_{pid}_{nanos}_{seq}.tmp"));
        let (file, guard) = crate::fs::abort_guard::FileAbortGuard::create_staged_file_in_txn(
            &path,
            CreateFileAccessMode::ReadWrite,
            txn_ctx,
            role,
        )
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        Ok((
            Self {
                path,
                guard: Some(guard),
                cleanup_status: TTZipStatus::ErrCompressionFailed,
            },
            file,
        ))
    }

    #[inline]
    pub(crate) fn expected_file_id(&self) -> Option<(u64, u64)> {
        self.guard.as_ref().and_then(|g| g.expected_file_id())
    }

    /// Explicitly cleans up the temporary helper file in the transaction (`AC-U1-14a`).
    pub fn cleanup_in_txn(&mut self, primary_status: TTZipStatus) -> Result<(), TTZipStatus> {
        self.cleanup_status = primary_status;
        if let Some(mut guard) = self.guard.take() {
            guard
                .abort_checked_detailed(primary_status)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
        Ok(())
    }

    pub(crate) fn finish_readback_and_cleanup_in_txn(
        &mut self,
        file: std::fs::File,
        prior_write_res: Result<(), (TTZipStatus, String)>,
    ) -> Result<ExplicitFileFinishProof, StageFileFinishError> {
        let Some(guard) = self.guard.take() else {
            let expected_id = (0, 0);
            let scope = CloseScopePrecondition::for_current_target_unique_regular_file();
            return crate::fs::finish_io::finish_and_close_file_for_proof(
                file,
                expected_id,
                prior_write_res,
                None,
                FileFinishSyncPolicy::None,
                scope,
            );
        };
        let mut guard = guard;
        let expected_id = guard.expected_file_id().unwrap_or((0, 0));
        let verified_prior = match (
            prior_write_res,
            crate::fs::abort_guard::fstat_dev_ino(std::os::fd::AsRawFd::as_raw_fd(&file)),
        ) {
            (Err(prior_err), _) => Err(prior_err),
            (Ok(()), Ok(live_id)) if expected_id == (0, 0) || live_id == expected_id => Ok(()),
            (Ok(()), Ok(live_id)) => Err((
                TTZipStatus::ErrCompressionFailed,
                format!("temp file descriptor identity {live_id:?} did not match {expected_id:?}"),
            )),
            (Ok(()), Err(stat_err)) => Err((
                TTZipStatus::ErrCompressionFailed,
                format!("fstat before temp finish failed: {stat_err}"),
            )),
        };
        match create_transaction::run_finish_and_close_file(
            file,
            expected_id,
            verified_prior,
            None,
            FileFinishSyncPolicy::None,
            CloseScopePrecondition::for_current_target_unique_regular_file(),
        ) {
            Ok(proof) => {
                if let Err(cleanup_err) = guard.abort_checked_detailed(TTZipStatus::Ok) {
                    return Err(StageFileFinishError {
                        primary_status: TTZipStatus::ErrCompressionFailed,
                        prior_write_or_encoder_error: Some(format!(
                            "temporary helper cleanup failed for '{}': {cleanup_err}",
                            self.path.display()
                        )),
                        userspace_flush_error: None,
                        sync_error: None,
                        close_error: None,
                        release_status: DescriptorReleaseStatus::ConfirmedReleased,
                    });
                }
                Ok(proof)
            }
            Err(err) => {
                guard.settle_failed_file_finish(&err);
                Err(err)
            }
        }
    }
}

impl Drop for ScopedTempFile {
    fn drop(&mut self) {
        if let Some(mut guard) = self.guard.take() {
            let _ = guard.abort_checked_detailed(self.cleanup_status);
        }
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
    out: &'a mut std::fs::File,
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
    pub(crate) heap_file: &'a mut std::fs::File,
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
    let mut src = std::fs::File::open(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
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

pub(crate) fn validate_create_encryption_options(
    resolved_format: TTZipArchiveFormat,
    options: &crate::types::TTZipCreateOptions,
    encrypt_file_names: Option<bool>,
) -> Result<(), TTZipStatus> {
    let has_non_empty_password = if options.password.is_null() {
        false
    } else {
        let s = unsafe { std::ffi::CStr::from_ptr(options.password) }
            .to_str()
            .map_err(|_| TTZipStatus::ErrInvalidPassword)?;
        if s.is_empty() {
            return Err(TTZipStatus::ErrInvalidPassword);
        }
        true
    };
    let wants_enc = options.encryption != crate::types::TTZipEncryptionMethod::None
        || encrypt_file_names == Some(true);
    if wants_enc && !has_non_empty_password {
        return Err(TTZipStatus::ErrInvalidPassword);
    }
    if encrypt_file_names == Some(true) && resolved_format != TTZipArchiveFormat::SevenZip {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    if (wants_enc || has_non_empty_password)
        && !matches!(
            resolved_format,
            TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto | TTZipArchiveFormat::SevenZip
        )
    {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    Ok(())
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
        kind: XarNodeKind::Directory {
            children: std::collections::BTreeMap::new(),
        },
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
            XarNodeKind::File {
                offset,
                length,
                size,
                ext_sha256,
                arc_sha256,
            } => {
                out.push_str(&format!(
                    "  <file id=\"{id}\"><data><length>{length}</length><offset>{offset}</offset><size>{size}</size><encoding style=\"application/x-gzip\"/><extracted-checksum style=\"sha256\">{ext_sha256}</extracted-checksum><archived-checksum style=\"sha256\">{arc_sha256}</archived-checksum></data><mode>{perm:04o}</mode><type>file</type><name>{esc_name}</name></file>\n"
                ));
            }
        }
    }
}
