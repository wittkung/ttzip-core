// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-copy memory-mapped TAR Archive Reader and safe extraction engine.
//!
//! Provides zero-copy payload slicing, Rayon/multi-core parallel disk extraction,
//! ZipSlip-immune path validation, APFS extent preallocation, and two-stage bottom-up metadata restoration.

use super::scanner::{TarEntry, TarSeekScanner};
use crate::fs::apfs::apfs_preallocate;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::types::{TTZipExtractOptions, TTZipStatus};
use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use ttzip_types::scheduler::LocalWorkStealingPool;

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
        let start_time = std::time::Instant::now();
        fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let jail = DirJail::open(dest_dir)?;

        let mut engine = SafeExtractEngine::for_root(dest_dir);
        let mut total_uncomp_bytes = 0u64;
        let mut has_links = false;
        let mut seen_file_comps = std::collections::HashSet::new();

        // Stage 1: Register all entries with SafeExtractEngine & create directories
        for entry in &self.entries {
            total_uncomp_bytes += entry.size;
            if entry.is_symlink || entry.is_hardlink {
                has_links = true;
            }
            let trimmed = entry.path.trim_matches('/');
            let is_root_dir = trimmed.is_empty() || trimmed == ".";
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

        if options.dry_run {
            return Ok(TarExtractReport {
                processed_entries_count: self.entries.len(),
                total_uncompressed_bytes: total_uncomp_bytes,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        // Collect non-directory file indices
        let file_indices: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_directory && !e.path.trim_matches('/').is_empty() && e.path.trim_matches('/') != ".")
            .map(|(i, _)| i)
            .collect();

        let thread_budget = (options.thread_budget as usize).clamp(1, 64);
        let processed_bytes = Arc::new(AtomicU64::new(0));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        if thread_budget <= 1
            || file_indices.len() <= 4
            || has_links
            || options.progress_callback.is_some()
        {
            // Single-threaded path
            let mut deferred_hardlinks = crate::fs::abort_guard::DeferredHardlinks::default();
            for &idx in &file_indices {
                let entry = &self.entries[idx];
                let current_done =
                    processed_bytes.fetch_add(entry.size, Ordering::Relaxed) + entry.size;
                self.extract_single_tar_entry_to_jail(
                    &jail,
                    idx,
                    entry,
                    Some(&mut deferred_hardlinks),
                    Some((options, current_done, total_uncomp_bytes)),
                )?;
            }
            deferred_hardlinks.finalize(&jail)?;
        } else {
            let num_threads = ttzip_types::recommended_compute_threads().min(file_indices.len()).max(1);
            let next_work_idx = std::sync::atomic::AtomicUsize::new(0);
            let first_error: parking_lot::Mutex<Option<TTZipStatus>> = parking_lot::Mutex::new(None);
            let pool = LocalWorkStealingPool::with_threads(num_threads);

            pool.scope(|s| {
                for _ in 0..num_threads {
                    s.spawn(|| {
                        let task_res: Result<(), TTZipStatus> = (|| {
                            loop {
                                if cancel_flag.load(Ordering::Relaxed) {
                                    return Err(TTZipStatus::Cancelled);
                                }
                                if first_error.lock().is_some() {
                                    break;
                                }
                                let work_i = next_work_idx.fetch_add(1, Ordering::Relaxed);
                                if work_i >= file_indices.len() {
                                    break;
                                }
                                let idx = file_indices[work_i];
                                let entry = &self.entries[idx];
                                self.extract_single_tar_entry_to_jail(
                                    &jail,
                                    idx,
                                    entry,
                                    None,
                                    None,
                                )?;

                                processed_bytes.fetch_add(entry.size, Ordering::Relaxed);
                            }
                            Ok(())
                        })();

                        if let Err(e) = task_res {
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
        }

        if has_links {
            jail.verify_all_jail_symlinks()?;
        }

        // Stage 2: Apply deferred metadata bottom-up (or record staged plan if in StagingArea)
        engine.apply_deferred_metadata(options.preserve_permissions)?;

        Ok(TarExtractReport {

            processed_entries_count: self.entries.len(),
            total_uncompressed_bytes: total_uncomp_bytes,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn write_tar_regular_payload_to_jail(
        jail: &DirJail,
        entry: &TarEntry<'a>,
        payload: &[u8],
        progress_ctx: Option<(&TTZipExtractOptions, u64, u64)>,
    ) -> Result<(), TTZipStatus> {
        let mut inflight =
            crate::fs::abort_guard::InFlightJailFile::open(jail, &entry.path, true)?;
        if entry.size > 0 {
            let _ = apfs_preallocate(inflight.file_mut().as_raw_fd(), entry.size as i64);
            inflight
                .file_mut()
                .write_all(payload)
                .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
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
        progress_ctx: Option<(&TTZipExtractOptions, u64, u64)>,
    ) -> Result<(), TTZipStatus> {
        let entry_comps = DirJail::extract_clean_components(&entry.path)?;
        if let Some(ref mut hl) = deferred_hardlinks {
            hl.invalidate_destination(&entry_comps);
        }

        if entry.is_symlink {
            let target = entry
                .link_target
                .as_deref()
                .ok_or(TTZipStatus::ErrSecurityViolation)?;
            jail.create_symlink_components(&entry_comps, target)?;
            return Self::check_tar_progress(&entry.path, progress_ctx);
        }
        if entry.is_hardlink {
            let target = entry
                .link_target
                .as_deref()
                .ok_or(TTZipStatus::ErrSecurityViolation)?;
            if let Some(hl) = deferred_hardlinks {
                hl.record_or_apply(jail, &entry.path, target)?;
            } else {
                jail.create_hardlink_in_jail(&entry.path, target)?;
            }
            return Self::check_tar_progress(&entry.path, progress_ctx);
        }
        let payload = self.extract_entry_bytes(idx)?;
        Self::write_tar_regular_payload_to_jail(jail, entry, payload, progress_ctx)?;
        if let Some(hl) = deferred_hardlinks {
            hl.on_regular_committed(jail, &entry_comps, None)?;
        }
        Ok(())
    }

    fn check_tar_progress(
        entry_path: &str,
        progress_ctx: Option<(&TTZipExtractOptions, u64, u64)>,
    ) -> Result<(), TTZipStatus> {
        if let Some((options, current_done, total_uncomp_bytes)) = progress_ctx {
            if let Some(cb) = options.progress_callback {
                let c_path = CString::new(entry_path).unwrap_or_default();
                let should_continue = unsafe {
                    cb(
                        current_done,
                        total_uncomp_bytes,
                        c_path.as_ptr(),
                        options.user_data,
                    )
                };
                if !should_continue {
                    return Err(TTZipStatus::Cancelled);
                }
            }
        }
        Ok(())
    }
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
    for comp in rel_parent.components() {
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

    for comp in target_path.components() {
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

    if stack.is_empty() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut resolved = dest_dir.to_path_buf();
    for comp in stack {
        resolved.push(comp);
    }
    Ok(resolved)
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
