// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-copy memory-mapped TAR Archive Reader and safe extraction engine.
//!
//! Provides zero-copy payload slicing, Rayon/multi-core parallel disk extraction,
//! ZipSlip-immune path validation, APFS extent preallocation, and two-stage bottom-up metadata restoration.

use super::scanner::{TarEntry, TarSeekScanner};
use crate::fs::apfs::apfs_preallocate_entry;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::types::{TTZipExtractOptions, TTZipStatus};
use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use ttzip_types::ProgressAtomicSheet;

/// Extraction report summarizing processed entries and byte counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TarExtractReport {
    pub processed_entries_count: usize,
    pub total_uncompressed_bytes: u64,
    pub duration_ms: u64,
}

/// Zero-copy memory-mapped TAR Archive parser and extractor.
pub struct TarArchive<'a> {
    data: &'a [u8],
    entries: Vec<TarEntry<'a>>,
}

impl<'a> TarArchive<'a> {
    /// Opens and indexes all entries in a TAR archive from an in-memory slice.
    pub fn open_slice(data: &'a [u8]) -> Result<Self, TTZipStatus> {
        let mut scanner = TarSeekScanner::new(data);
        let entries = scanner.scan_all()?;
        Ok(Self { data, entries })
    }

    /// Returns reference to all parsed TAR entry descriptors.
    #[inline]
    pub fn entries(&self) -> &[TarEntry<'a>] {
        &self.entries
    }

    /// Returns the number of entries in the archive.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the archive contains zero entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Extracts payload data for an entry zero-copy as a subslice of the underlying archive.
    pub fn extract_entry_bytes(&self, entry_idx: usize) -> Result<&'a [u8], TTZipStatus> {
        let entry = self
            .entries
            .get(entry_idx)
            .ok_or(TTZipStatus::ErrInvalidOffset)?;

        if entry.is_directory || entry.size == 0 {
            return Ok(&[]);
        }

        let start = entry.data_offset;
        let end = start + (entry.size as usize);

        if end > self.data.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(&self.data[start..end])
    }

    /// Extracts all archive entries to `dest_dir` with security validation and hardware acceleration.
    pub fn extract_all(
        &self,
        dest_dir: &Path,
        options: &TTZipExtractOptions,
    ) -> Result<TarExtractReport, TTZipStatus> {
        self.extract_all_with_sheet(dest_dir, options, None)
    }

    /// Extracts all archive entries to `dest_dir` while honoring an optional atomic progress/cancellation sheet.
    pub fn extract_all_with_sheet(
        &self,
        dest_dir: &Path,
        options: &TTZipExtractOptions,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<TarExtractReport, TTZipStatus> {
        let start_time = std::time::Instant::now();
        if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
            return Err(TTZipStatus::Cancelled);
        }
        fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let jail = DirJail::open(dest_dir)?;

        let (mut engine, total_uncomp_bytes, has_links) =
            self.register_tar_entries_and_dirs(dest_dir, &jail, options, sheet)?;
        if let Some(s) = sheet {
            s.set_total(total_uncomp_bytes);
        }

        if options.dry_run {
            return Ok(TarExtractReport {
                processed_entries_count: self.entries.len(),
                total_uncompressed_bytes: total_uncomp_bytes,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        let file_indices = self.collect_non_dir_file_indices();
        let thread_budget = ttzip_types::resolve_thread_budget(options.thread_budget);

        if Self::should_extract_tar_sequentially(options, file_indices.len(), has_links) {
            self.extract_files_sequential(
                &jail,
                &file_indices,
                total_uncomp_bytes,
                options,
                sheet,
            )?;
        } else {
            self.extract_files_parallel(&jail, &file_indices, thread_budget, sheet)?;
        }

        if has_links {
            jail.verify_all_jail_symlinks()?;
        }

        engine.apply_deferred_metadata(options.preserve_permissions)?;

        Ok(TarExtractReport {
            processed_entries_count: self.entries.len(),
            total_uncompressed_bytes: total_uncomp_bytes,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    pub(crate) fn should_extract_tar_sequentially(
        options: &TTZipExtractOptions,
        file_count: usize,
        has_links: bool,
    ) -> bool {
        let resolved = ttzip_types::resolve_thread_budget(options.thread_budget);
        resolved <= 1 || file_count <= 4 || has_links || options.progress_callback.is_some()
    }

    fn collect_non_dir_file_indices(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_directory && !is_root_tar_path(&e.path))
            .map(|(i, _)| i)
            .collect()
    }

    fn register_tar_entries_and_dirs(
        &self,
        dest_dir: &Path,
        jail: &DirJail,
        options: &TTZipExtractOptions,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<(SafeExtractEngine, u64, bool), TTZipStatus> {
        let mut engine = SafeExtractEngine::for_root(dest_dir);
        let mut total_uncomp_bytes = 0u64;
        let mut has_links = false;
        let mut seen_file_comps = std::collections::HashSet::new();

        for entry in &self.entries {
            if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
                return Err(TTZipStatus::Cancelled);
            }
            total_uncomp_bytes += entry.size;
            if entry.is_symlink || entry.is_hardlink {
                has_links = true;
            }
            let is_root_dir = is_root_tar_path(&entry.path);
            let safe_path = if is_root_dir {
                dest_dir.to_path_buf()
            } else {
                sanitize_and_validate_path(dest_dir, &entry.path)?
            };
            if !entry.is_directory && !is_root_dir {
                let comps = DirJail::extract_clean_components(&entry.path)?;
                if !seen_file_comps.insert(comps) {
                    has_links = true;
                }
            }

            engine.register_entry_typed(
                safe_path,
                entry.mode,
                entry.mtime_epoch_secs,
                entry.mtime_nanos,
                entry.is_directory,
                entry.is_symlink,
            );

            if entry.is_directory && !is_root_dir && !options.dry_run {
                jail.create_dir_all_in_jail(&entry.path)?;
            }
        }

        Ok((engine, total_uncomp_bytes, has_links))
    }

    fn extract_files_sequential(
        &self,
        jail: &DirJail,
        file_indices: &[usize],
        total_uncomp_bytes: u64,
        options: &TTZipExtractOptions,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<(), TTZipStatus> {
        let mut deferred_hardlinks = crate::fs::abort_guard::DeferredHardlinks::default();
        let mut processed_bytes = 0u64;
        for &idx in file_indices {
            if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
                return Err(TTZipStatus::Cancelled);
            }
            let entry = &self.entries[idx];
            processed_bytes = processed_bytes.saturating_add(entry.size);
            self.extract_single_tar_entry_to_jail(
                jail,
                idx,
                entry,
                Some(&mut deferred_hardlinks),
                Some(TarProgressCheck {
                    options,
                    current_done: processed_bytes,
                    total_uncomp_bytes,
                    sheet,
                }),
            )?;
        }
        deferred_hardlinks.finalize(jail)
    }

    fn extract_files_parallel(
        &self,
        jail: &DirJail,
        file_indices: &[usize],
        thread_budget: usize,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<(), TTZipStatus> {
        let num_threads = thread_budget.min(file_indices.len()).max(1);
        let next_work_idx = std::sync::atomic::AtomicUsize::new(0);
        let cancel_flag = AtomicBool::new(false);
        let first_error: parking_lot::Mutex<Option<TTZipStatus>> = parking_lot::Mutex::new(None);
        let pool = crate::archive::shared_compute_pool(thread_budget);
        let state = TarParallelWorkerState {
            jail,
            file_indices,
            next_work_idx: &next_work_idx,
            cancel_flag: &cancel_flag,
            first_error: &first_error,
            sheet,
        };

        pool.scope(|s| {
            for _ in 0..num_threads {
                s.spawn(|| {
                    if let Err(e) = self.parallel_worker_loop(&state) {
                        if e == TTZipStatus::Cancelled {
                            cancel_flag.store(true, Ordering::Relaxed);
                        }
                        let mut err_guard = first_error.lock();
                        if err_guard.is_none() {
                            *err_guard = Some(e);
                        }
                    }
                });
            }
        });

        if let Some(err) = first_error.into_inner() {
            return Err(err);
        }
        Ok(())
    }

    fn parallel_worker_loop(
        &self,
        state: &TarParallelWorkerState<'_>,
    ) -> Result<(), TTZipStatus> {
        loop {
            if state.cancel_flag.load(Ordering::Relaxed)
                || state.sheet.is_some_and(ProgressAtomicSheet::is_cancelled)
            {
                return Err(TTZipStatus::Cancelled);
            }
            if state.first_error.lock().is_some() {
                break;
            }
            let work_i = state.next_work_idx.fetch_add(1, Ordering::Relaxed);
            if work_i >= state.file_indices.len() {
                break;
            }
            let idx = state.file_indices[work_i];
            let entry = &self.entries[idx];
            let default_opts = TTZipExtractOptions::default();
            self.extract_single_tar_entry_to_jail(
                state.jail,
                idx,
                entry,
                None,
                state.sheet.map(|s| TarProgressCheck {
                    options: &default_opts,
                    current_done: 0,
                    total_uncomp_bytes: 0,
                    sheet: Some(s),
                }),
            )?;
        }
        Ok(())
    }

    fn write_tar_regular_payload_to_jail(
        jail: &DirJail,
        entry: &TarEntry<'a>,
        payload: &[u8],
        progress_ctx: Option<TarProgressCheck<'_>>,
    ) -> Result<(), TTZipStatus> {
        let sheet = progress_ctx.and_then(|ctx| ctx.sheet);
        if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
            return Err(TTZipStatus::Cancelled);
        }
        let mut inflight =
            crate::fs::abort_guard::InFlightJailFile::open(jail, &entry.path, true)?;
        if entry.size > 0 {
            if entry.size >= crate::fs::apfs::MIN_ENTRY_PREALLOCATE_BYTES {
                let _ = apfs_preallocate_entry(inflight.file_mut().as_raw_fd(), entry.size);
            }
            for chunk in payload.chunks(256 * 1024) {
                if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
                    return Err(TTZipStatus::Cancelled);
                }
                inflight
                    .file_mut()
                    .write_all(chunk)
                    .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                if let Some(s) = sheet {
                    s.add_processed(chunk.len() as u64);
                }
            }
        }
        Self::check_tar_progress(&entry.path, progress_ctx)?;
        inflight.commit(jail)
    }

    fn extract_single_tar_entry_to_jail(
        &self,
        jail: &DirJail,
        idx: usize,
        entry: &TarEntry<'a>,
        mut deferred_hardlinks: Option<&mut crate::fs::abort_guard::DeferredHardlinks>,
        progress_ctx: Option<TarProgressCheck<'_>>,
    ) -> Result<(), TTZipStatus> {
        if progress_ctx
            .and_then(|ctx| ctx.sheet)
            .is_some_and(ProgressAtomicSheet::is_cancelled)
        {
            return Err(TTZipStatus::Cancelled);
        }
        let entry_comps = DirJail::extract_clean_components(&entry.path)?;
        if let Some(ref mut hl) = deferred_hardlinks {
            hl.invalidate_destination(&entry_comps);
        }

        if entry.is_symlink || entry.is_hardlink {
            return Self::extract_tar_link_entry_to_jail(
                jail,
                entry,
                &entry_comps,
                deferred_hardlinks,
                progress_ctx,
            );
        }
        let payload = self.extract_entry_bytes(idx)?;
        Self::write_tar_regular_payload_to_jail(jail, entry, payload, progress_ctx)?;
        if let Some(hl) = deferred_hardlinks {
            hl.on_regular_committed(jail, &entry_comps, None)?;
        }
        Ok(())
    }

    fn extract_tar_link_entry_to_jail(
        jail: &DirJail,
        entry: &TarEntry<'a>,
        entry_comps: &[String],
        deferred_hardlinks: Option<&mut crate::fs::abort_guard::DeferredHardlinks>,
        progress_ctx: Option<TarProgressCheck<'_>>,
    ) -> Result<(), TTZipStatus> {
        let target = entry
            .link_target
            .as_deref()
            .ok_or(TTZipStatus::ErrSecurityViolation)?;
        if entry.is_symlink {
            jail.create_symlink_components(entry_comps, target)?;
        } else if let Some(hl) = deferred_hardlinks {
            hl.record_or_apply(jail, &entry.path, target)?;
        } else {
            jail.create_hardlink_in_jail(&entry.path, target)?;
        }
        Self::check_tar_progress(&entry.path, progress_ctx)
    }

    fn check_tar_progress(
        entry_path: &str,
        progress_ctx: Option<TarProgressCheck<'_>>,
    ) -> Result<(), TTZipStatus> {
        if let Some(ctx) = progress_ctx {
            if ctx.sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
                return Err(TTZipStatus::Cancelled);
            }
            if let Some(cb) = ctx.options.progress_callback {
                let c_path = CString::new(entry_path).unwrap_or_default();
                let should_continue = unsafe {
                    cb(
                        ctx.current_done,
                        ctx.total_uncomp_bytes,
                        c_path.as_ptr(),
                        ctx.options.user_data,
                    )
                };
                if !should_continue {
                    if let Some(s) = ctx.sheet {
                        s.cancel();
                    }
                    return Err(TTZipStatus::Cancelled);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct TarProgressCheck<'a> {
    options: &'a TTZipExtractOptions,
    current_done: u64,
    total_uncomp_bytes: u64,
    sheet: Option<&'a ProgressAtomicSheet>,
}

struct TarParallelWorkerState<'a> {
    jail: &'a DirJail,
    file_indices: &'a [usize],
    next_work_idx: &'a std::sync::atomic::AtomicUsize,
    cancel_flag: &'a AtomicBool,
    first_error: &'a parking_lot::Mutex<Option<TTZipStatus>>,
    sheet: Option<&'a ProgressAtomicSheet>,
}

/// Lexically resolves a relative symlink target against the destination root sandbox.
///
/// Resolves `.` and `..` components lexically without performing file I/O.
/// If any `..` component attempts to ascend above `dest_dir`, returns `Err(TTZipStatus::ErrSecurityViolation)`.
/// Legitimate in-sandbox relative symlinks (e.g. `../foo.txt` from `/dest/sub/link` to `/dest/foo.txt`)
/// resolve safely to their normalized in-sandbox target path.
pub fn lexical_normalize_relative(
    dest_dir: &Path,
    parent_dir: &Path,
    target: &str,
) -> Result<PathBuf, TTZipStatus> {
    if target.starts_with('/') || target.starts_with('\\') {
        return Err(TTZipStatus::ErrSecurityViolation);
    }
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let rel_parent = parent_dir
        .strip_prefix(dest_dir)
        .map_err(|_| TTZipStatus::ErrSecurityViolation)?;

    let mut stack = Vec::new();
    push_normalized_components(&mut stack, rel_parent)?;
    push_normalized_components(&mut stack, target_path)?;

    if stack.is_empty() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut resolved = dest_dir.to_path_buf();
    for comp in stack {
        resolved.push(comp);
    }
    Ok(resolved)
}

fn is_root_tar_path(path: &str) -> bool {
    let trimmed = path.trim_matches('/');
    trimmed.is_empty() || trimmed == "."
}

fn push_normalized_components(
    stack: &mut Vec<std::ffi::OsString>,
    path: &Path,
) -> Result<(), TTZipStatus> {
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(c) => stack.push(c.to_os_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if stack.pop().is_none() {
                    return Err(TTZipStatus::ErrSecurityViolation);
                }
            }
            _ => return Err(TTZipStatus::ErrSecurityViolation),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexical_normalize_relative_valid_sibling() {
        let dest = Path::new("/var/sandbox/dest");
        let parent = Path::new("/var/sandbox/dest/sub");
        let res = lexical_normalize_relative(dest, parent, "../foo.txt").expect("should resolve");
        assert_eq!(res, Path::new("/var/sandbox/dest/foo.txt"));
    }

    #[test]
    fn test_lexical_normalize_relative_valid_nested() {
        let dest = Path::new("/var/sandbox/dest");
        let parent = Path::new("/var/sandbox/dest/a/b/c");
        let res = lexical_normalize_relative(dest, parent, "../../d/target.txt").expect("should resolve");
        assert_eq!(res, Path::new("/var/sandbox/dest/a/d/target.txt"));
    }

    #[test]
    fn test_lexical_normalize_relative_escape_rejected() {
        let dest = Path::new("/var/sandbox/dest");
        let parent = Path::new("/var/sandbox/dest/sub");
        let res = lexical_normalize_relative(dest, parent, "../../escape.txt");
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrSecurityViolation);
    }

    #[test]
    fn test_lexical_normalize_relative_root_parent_escape_rejected() {
        let dest = Path::new("/var/sandbox/dest");
        let parent = Path::new("/var/sandbox/dest");
        let res = lexical_normalize_relative(dest, parent, "../escape.txt");
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrSecurityViolation);
    }

    #[test]
    fn test_lexical_normalize_relative_absolute_target_rejected() {
        let dest = Path::new("/var/sandbox/dest");
        let parent = Path::new("/var/sandbox/dest/sub");
        let res = lexical_normalize_relative(dest, parent, "/etc/passwd");
        assert_eq!(res.unwrap_err(), TTZipStatus::ErrSecurityViolation);
    }
}
