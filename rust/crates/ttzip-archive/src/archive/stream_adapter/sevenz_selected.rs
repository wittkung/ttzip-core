// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Streaming selective extraction engine for 7-Zip archives directly targeting `DirJail`.
//!
//! Enforces:
//! 1. Memory bound: O(1) resident memory via chunked streaming directly to `InFlightJailFile`.
//! 2. Solid stream windowing: Decodes solid folders with prefix-sum spatial bounds and early termination.
//! 3. Security invariants: Rejects symlink targets > 4KB, verifies CRCs, and prevents uncommitted staging leaks.
//! 4. Decompression quota: every decoded byte (discarded solid-block prefix, inter-entry gaps, and payload
//!    written to staging) is charged to `ExtractionQuotaGuard::cumulative_uncompressed_bytes` before it
//!    reaches the sink, while the denominator is the declared `packed_len` of each decoded folder,
//!    registered exactly once per folder before that folder starts decoding.

use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::{Read, Seek, Write};

use crate::archive::unified::selection::EntrySelection;
use crate::crypto::crc32::crc32_fast;
use crate::fs::abort_guard::InFlightJailFile;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::security::zip_defense::ExtractionQuotaGuard;
use crate::sevenz::decoder::solid_stream::SolidFolderIndex;
use crate::sevenz::decoder::stream::stream_7z_folder_from_reader;
use crate::sevenz::header::SevenZHeaderInfo;
use crate::types::{TTZipExtractOptions, TTZipStatus};

const MAX_SYMLINK_TARGET_BYTES: u64 = 4096;

/// Target specification for an entry selected for extraction within a folder stream.
#[derive(Debug, Clone)]
struct SelectedTarget {
    rel_path: String,
    out_rel: String,
    offset_start: u64,
    offset_end: u64,
    uncompressed_size: u64,
    expected_crc: Option<u32>,
    is_symlink: bool,
}

/// Metadata for an empty or 0-byte entry extracted without running stream decoders.
#[derive(Debug, Clone)]
struct EmptyTarget {
    out_rel: String,
    expected_crc: Option<u32>,
    is_symlink: bool,
}

enum ActiveTargetSink {
    Regular {
        inflight: InFlightJailFile,
        written: u64,
        running_crc: u32,
    },
    Symlink {
        target_bytes: Vec<u8>,
        written: u64,
        running_crc: u32,
    },
}

struct FolderDispatcher<'a> {
    jail: &'a DirJail,
    targets: &'a [SelectedTarget],
    cur_idx: usize,
    current_offset: u64,
    active_sink: Option<ActiveTargetSink>,
    options: &'a TTZipExtractOptions,
    total_processed: &'a mut u64,
    is_encrypted: bool,
    quota_guard: &'a mut ExtractionQuotaGuard,
}

impl<'a> FolderDispatcher<'a> {
    fn new(
        jail: &'a DirJail,
        targets: &'a [SelectedTarget],
        options: &'a TTZipExtractOptions,
        total_processed: &'a mut u64,
        is_encrypted: bool,
        quota_guard: &'a mut ExtractionQuotaGuard,
    ) -> Self {
        Self {
            jail,
            targets,
            cur_idx: 0,
            current_offset: 0,
            active_sink: None,
            options,
            total_processed,
            is_encrypted,
            quota_guard,
        }
    }

    /// Charges decoded bytes to the shared quota guard.
    ///
    /// The numerator is every byte produced by the folder decoder, regardless of whether it is
    /// discarded (solid-block prefix, inter-entry gap, or same-block tail) or written to the target
    /// file. The
    /// denominator is metered per folder by [`meter_folder_compressed_input`], so this function
    /// never passes a compressed delta: passing one here would double-count the folder input.
    ///
    /// Delegates to [`ExtractionQuotaGuard::observe_and_validate`] so the 7z streaming path shares
    /// the exact ratio/limit semantics of the ZIP path, including the `max(1)` denominator floor
    /// that keeps a zero denominator from bypassing the ratio check.
    fn check_quota(&mut self, uncompressed_delta: u64) -> Result<(), TTZipStatus> {
        self.quota_guard.observe_and_validate(0, uncompressed_delta)
    }

    fn check_cancellation(&self, rel_path: Option<&str>) -> Result<(), TTZipStatus> {
        if let Some(cb) = self.options.progress_callback {
            let c_name = rel_path.and_then(|p| CString::new(p).ok());
            let ptr = c_name
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(std::ptr::null());
            let current = *self.total_processed;
            if !unsafe { cb(current, current, ptr, self.options.user_data) } {
                return Err(TTZipStatus::Cancelled);
            }
        }
        Ok(())
    }

    fn open_sink_if_needed(&mut self, target: &SelectedTarget) -> Result<(), TTZipStatus> {
        if self.active_sink.is_some() {
            return Ok(());
        }
        if target.is_symlink {
            if target.uncompressed_size > MAX_SYMLINK_TARGET_BYTES {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            self.active_sink = Some(ActiveTargetSink::Symlink {
                target_bytes: Vec::with_capacity(target.uncompressed_size as usize),
                written: 0,
                running_crc: 0,
            });
        } else {
            let inflight = InFlightJailFile::open(self.jail, &target.out_rel, true)?;
            self.active_sink = Some(ActiveTargetSink::Regular {
                inflight,
                written: 0,
                running_crc: 0,
            });
        }
        Ok(())
    }

    fn write_slice_to_sink(
        &mut self,
        target: &SelectedTarget,
        slice: &[u8],
    ) -> Result<(), TTZipStatus> {
        let sink = self
            .active_sink
            .as_mut()
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        match sink {
            ActiveTargetSink::Regular {
                inflight,
                written,
                running_crc,
            } => {
                inflight
                    .file_mut()
                    .write_all(slice)
                    .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                *running_crc = crc32_fast(*running_crc, slice);
                *written += slice.len() as u64;
            }
            ActiveTargetSink::Symlink {
                target_bytes,
                written,
                running_crc,
            } => {
                target_bytes.extend_from_slice(slice);
                *running_crc = crc32_fast(*running_crc, slice);
                *written += slice.len() as u64;
            }
        }
        *self.total_processed = self.total_processed.saturating_add(slice.len() as u64);
        self.check_cancellation(Some(&target.rel_path))
    }

    fn finalize_current_target(&mut self, target: &SelectedTarget) -> Result<(), TTZipStatus> {
        let sink = self
            .active_sink
            .take()
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        let corrupt_err = if self.is_encrypted {
            TTZipStatus::ErrInvalidPassword
        } else {
            TTZipStatus::ErrCorruptHeader
        };

        match sink {
            ActiveTargetSink::Regular {
                inflight,
                written,
                running_crc,
            } => {
                if written != target.uncompressed_size {
                    return Err(corrupt_err);
                }
                if let Some(exp_crc) = target.expected_crc {
                    if running_crc != exp_crc {
                        return Err(corrupt_err);
                    }
                }
                inflight.commit(self.jail)?;
            }
            ActiveTargetSink::Symlink {
                target_bytes,
                written,
                running_crc,
            } => {
                if written != target.uncompressed_size {
                    return Err(corrupt_err);
                }
                if let Some(exp_crc) = target.expected_crc {
                    if running_crc != exp_crc {
                        return Err(corrupt_err);
                    }
                }
                let target_str = std::str::from_utf8(&target_bytes)
                    .map_err(|_| TTZipStatus::ErrSecurityViolation)?;
                self.jail
                    .create_symlink_in_jail(&target.out_rel, target_str)?;
            }
        }
        Ok(())
    }

    fn consume_chunk(&mut self, mut chunk: &[u8]) -> Result<(), TTZipStatus> {
        // The decoder already produced the entire callback block, including any bytes beyond
        // the final selected entry. Charge it once before dispatching or touching a target.
        self.check_quota(chunk.len() as u64)?;
        while !chunk.is_empty() && self.cur_idx < self.targets.len() {
            let target = &self.targets[self.cur_idx];
            if self.current_offset < target.offset_start {
                let skip_needed = target.offset_start - self.current_offset;
                let skip_len = (chunk.len() as u64).min(skip_needed) as usize;
                self.current_offset += skip_len as u64;
                chunk = &chunk[skip_len..];
                self.check_cancellation(None)?;
                continue;
            }

            self.open_sink_if_needed(target)?;
            let needed = target.offset_end.saturating_sub(self.current_offset);
            let take_len = (chunk.len() as u64).min(needed) as usize;
            let slice = &chunk[..take_len];
            self.write_slice_to_sink(target, slice)?;
            self.current_offset += take_len as u64;
            chunk = &chunk[take_len..];

            if self.current_offset >= target.offset_end {
                self.finalize_current_target(target)?;
                self.cur_idx += 1;
                if self.cur_idx >= self.targets.len() {
                    return Err(TTZipStatus::Eof);
                }
            }
        }

        if self.cur_idx >= self.targets.len() {
            return Err(TTZipStatus::Eof);
        }
        Ok(())
    }

    fn finish(self) -> Result<(), TTZipStatus> {
        if self.cur_idx < self.targets.len() {
            return Err(if self.is_encrypted {
                TTZipStatus::ErrInvalidPassword
            } else {
                TTZipStatus::ErrCorruptHeader
            });
        }
        Ok(())
    }
}

fn decode_header_info<R: Read + Seek>(
    reader: &mut R,
    password: Option<&str>,
) -> Result<SevenZHeaderInfo, TTZipStatus> {
    crate::sevenz::encrypted_header::EncodedHeaderDecoder::default()
        .decode_from_reader(reader, password)
        .map_err(|e| match e {
            crate::sevenz::dag::SevenZError::BadPassword => TTZipStatus::ErrInvalidPassword,
            _ => TTZipStatus::ErrCorruptHeader,
        })
}

fn process_empty_targets(empty_targets: &[EmptyTarget], jail: &DirJail) -> Result<(), TTZipStatus> {
    for empty in empty_targets {
        if empty.is_symlink {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        if let Some(crc) = empty.expected_crc {
            if crc != 0 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
        }
        InFlightJailFile::open(jail, &empty.out_rel, true)?.commit(jail)?;
    }
    Ok(())
}

struct SelectedTargetCollection {
    folder_map: BTreeMap<usize, Vec<SelectedTarget>>,
    empty_targets: Vec<EmptyTarget>,
    pending_dirs: Vec<String>,
    total_selected: usize,
}

fn collect_selected_targets(
    info: &SevenZHeaderInfo,
    selection: &EntrySelection,
    staging_path: &std::path::Path,
    engine: &mut SafeExtractEngine,
) -> Result<SelectedTargetCollection, TTZipStatus> {
    let solid_idx = SolidFolderIndex::build(info);
    let mut folder_map: BTreeMap<usize, Vec<SelectedTarget>> = BTreeMap::new();
    let mut empty_targets = Vec::new();
    let mut pending_dirs = Vec::new();
    let mut total_selected = 0usize;

    for (file_idx, file_item) in info.files.iter().enumerate() {
        let Some(out_rel) = selection.output_path(&file_item.rel_path) else {
            continue;
        };
        let sanitized = sanitize_and_validate_path(staging_path, out_rel)?;
        let is_symlink = (file_item.mode & 0o170000) == 0o120000;
        engine.register_entry_typed(
            sanitized,
            file_item.mode & 0o7777,
            file_item.mtime_epoch_secs.unwrap_or(0),
            0,
            file_item.is_directory,
            is_symlink,
        );
        total_selected += 1;

        if file_item.is_directory {
            pending_dirs.push(out_rel.to_string());
            continue;
        }

        let range = solid_idx
            .lookup(file_idx)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        if range.is_empty_stream || range.uncompressed_size == 0 {
            empty_targets.push(EmptyTarget {
                out_rel: out_rel.to_string(),
                expected_crc: range.crc,
                is_symlink,
            });
            continue;
        }

        let folder_idx = range.folder_index.unwrap_or(0);
        folder_map
            .entry(folder_idx)
            .or_default()
            .push(SelectedTarget {
                rel_path: file_item.rel_path.clone(),
                out_rel: out_rel.to_string(),
                offset_start: range.offset_start,
                offset_end: range.offset_end,
                uncompressed_size: range.uncompressed_size,
                expected_crc: range.crc,
                is_symlink,
            });
    }

    Ok(SelectedTargetCollection {
        folder_map,
        empty_targets,
        pending_dirs,
        total_selected,
    })
}

/// Meters a folder's compressed input into the ratio denominator exactly once per decoded folder.
///
/// The denominator is the folder's **declared** `packed_len`, not the number of compressed bytes
/// actually pulled from the archive. Early termination (a selected entry in the middle of a solid
/// block) therefore leaves the tail's compressed bytes counted, which makes the denominator an
/// upper bound of the real consumption: the computed ratio is a lower bound of the instantaneous
/// ratio. The guard can under-report a window-localized spike, but it never over-reports.
///
/// A folder that carries non-empty selected targets with `packed_len == 0` cannot be metered at
/// all; it is rejected as a corrupt header before decoding, matching the decoder's own check.
fn meter_folder_compressed_input(
    guard: &mut ExtractionQuotaGuard,
    folder_packed_len: usize,
) -> Result<(), TTZipStatus> {
    let packed = u64::try_from(folder_packed_len).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
    if packed == 0 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    guard.cumulative_compressed_bytes = guard.cumulative_compressed_bytes.saturating_add(packed);
    Ok(())
}

fn execute_folder_extractions<R: Read + Seek>(
    reader: &mut R,
    info: &SevenZHeaderInfo,
    folder_map: BTreeMap<usize, Vec<SelectedTarget>>,
    jail: &DirJail,
    options: &TTZipExtractOptions,
    password: Option<&str>,
    total_processed: &mut u64,
    quota_guard: &mut ExtractionQuotaGuard,
) -> Result<(), TTZipStatus> {
    for (folder_idx, mut targets) in folder_map {
        targets.sort_by_key(|t| t.offset_start);
        let folder = info
            .folders
            .get(folder_idx)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;

        // Register the folder's compressed size so the expansion ratio check has a non-zero
        // denominator. Registered once per decoded folder, before the decoder runs, so the very
        // first decoded chunk is already checked against a populated denominator.
        meter_folder_compressed_input(quota_guard, folder.packed_len)?;

        let mut dispatcher = FolderDispatcher::new(
            jail,
            &targets,
            options,
            total_processed,
            info.is_encrypted,
            quota_guard,
        );
        let mut sink =
            |chunk: &[u8]| -> Result<(), TTZipStatus> { dispatcher.consume_chunk(chunk) };

        let stream_res = stream_7z_folder_from_reader(
            reader,
            info,
            folder,
            password,
            options.thread_budget.max(1),
            &mut sink,
        );
        match stream_res {
            Ok(_) | Err(TTZipStatus::Eof) => {}
            Err(e) => return Err(e),
        }
        dispatcher.finish()?;
    }
    Ok(())
}

/// Streams selected entries from a 7z archive (`Read + Seek`) directly into `staging`
/// enforcing the specified `ExtractionQuotaGuard` across all decoded (discarded and written) bytes.
pub fn extract_sevenz_selected_from_reader_with_quota<R: Read + Seek>(
    reader: &mut R,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
    password: Option<&str>,
    quota_guard: Option<ExtractionQuotaGuard>,
) -> Result<usize, TTZipStatus> {
    let info = decode_header_info(reader, password)?;
    if info.is_encrypted && password.map(|s| s.is_empty()).unwrap_or(true) {
        return Err(TTZipStatus::ErrInvalidPassword);
    }

    std::fs::create_dir_all(staging.path()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let jail = DirJail::open(staging.path())?;
    let mut engine = SafeExtractEngine::for_root(staging.path());

    let SelectedTargetCollection {
        folder_map,
        empty_targets,
        pending_dirs,
        total_selected,
    } = collect_selected_targets(&info, selection, staging.path(), &mut engine)?;

    if options.dry_run {
        return Ok(total_selected);
    }

    process_empty_targets(&empty_targets, &jail)?;

    let mut total_processed = 0u64;
    let mut quota = quota_guard.unwrap_or_else(ExtractionQuotaGuard::default_limits);
    execute_folder_extractions(
        reader,
        &info,
        folder_map,
        &jail,
        options,
        password,
        &mut total_processed,
        &mut quota,
    )?;

    for dir_rel in &pending_dirs {
        jail.create_dir_all_in_jail(dir_rel)?;
    }
    jail.verify_all_jail_symlinks()?;
    engine.apply_deferred_metadata(options.preserve_permissions)?;

    Ok(total_selected)
}

/// Streams selected entries from a 7z archive (`Read + Seek`) directly into `staging`
/// with default production extraction quota and memory bound.
pub fn extract_sevenz_selected_from_reader<R: Read + Seek>(
    reader: &mut R,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
    password: Option<&str>,
) -> Result<usize, TTZipStatus> {
    extract_sevenz_selected_from_reader_with_quota(
        reader, selection, staging, options, password, None,
    )
}
