// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! ZIP Archive Decompression and Extraction Engine.
//!
//! Features Rayon multi-core parallel decompression, thread-local libdeflate pooling,
//! WinZip AES-256 hardware decryption, Direct I/O + APFS extent preallocation, and ZipSlip-immune safe landing.

use crate::crypto::crc32::crc32_fast;
use crate::fs::apfs::apfs_preallocate;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::security::zip_defense::{detect_overlapping_entries, ExtractionQuotaGuard};
use crate::types::{TTZipExtractOptions, TTZipStatus};
use crate::zip::parser::{parse_all_entries, parse_local_file_header, ZipEntry};
use std::ffi::CString;
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::atomic::Ordering;
use ttzip_types::options::ProgressAtomicSheet;
use ttzip_types::scheduler::LocalWorkStealingPool;

/// Maximum in-memory single-entry decompression threshold (64 MB).
/// Entries exceeding 64 MB must use stream-to-disk or stream-to-sink fallback.
pub const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;

/// Threshold for enabling Direct I/O (F_NOCACHE) on large files (1 MB).
const DIRECT_IO_THRESHOLD: u64 = 1024 * 1024;

/// Detailed report from an archive extraction operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZipExtractReport {
    pub processed_entries_count: usize,
    pub total_uncompressed_bytes: u64,
    pub total_compressed_bytes: u64,
    pub duration_ms: u64,
}

/// Zero-copy memory-mapped ZIP Archive reader.
pub struct ZipArchive<'a> {
    data: &'a [u8],
    entries: Vec<ZipEntry>,
}

impl<'a> ZipArchive<'a> {
    /// Opens and parses a ZIP archive from an in-memory slice.
    pub fn open_slice(data: &'a [u8]) -> Result<Self, TTZipStatus> {
        let entries = parse_all_entries(data)?;
        Ok(Self { data, entries })
    }

    /// Returns reference to all parsed Central Directory entries.
    #[inline]
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// Returns the number of entries in the archive.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the archive contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Decompresses and extracts a single entry into a byte vector.
    pub fn extract_entry_bytes(
        &self,
        entry_idx: usize,
        password: Option<&str>,
    ) -> Result<Vec<u8>, TTZipStatus> {
        let entry = self
            .entries
            .get(entry_idx)
            .ok_or(TTZipStatus::ErrInvalidOffset)?;

        if entry.is_directory || (entry.uncompressed_size == 0 && !entry.is_encrypted) {
            return Ok(Vec::new());
        }
        if entry.uncompressed_size == 0 {
            self.verify_entry_encryption_key(entry_idx, password)?;
            return Ok(Vec::new());
        }

        let lfh_offset = usize::try_from(entry.lfh_offset).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        let (payload_offset, _) = parse_local_file_header(self.data, lfh_offset)?;
        let comp_size = usize::try_from(entry.compressed_size).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

        let payload_end = payload_offset.checked_add(comp_size).ok_or(TTZipStatus::ErrCorruptHeader)?;
        if payload_end > self.data.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let raw_payload = &self.data[payload_offset..payload_end];
        let effective_cow = super::reader_stream::decrypt_slice_entry_payload(
            self.data,
            entry,
            lfh_offset,
            raw_payload,
            password,
        )?;
        let effective_payload = effective_cow.as_ref();

        let uncomp_size = entry.uncompressed_size as usize;
        // Defense-in-depth: limit single-entry in-memory extraction to 64MB to prevent zip-bomb OOM attacks.
        // Entries exceeding 64MB must use stream-to-disk or stream-to-sink fallback with bounded buffer quotas.
        if uncomp_size > MAX_IN_MEMORY_DECOMPRESS_BYTES {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let mut out_buffer = vec![0u8; uncomp_size];

        match entry.actual_method {
            0 => {
                if effective_payload.len() != uncomp_size {
                    return Err(super::reader_stream::map_zipcrypto_decode_err(
                        entry,
                        TTZipStatus::ErrCorruptHeader,
                    ));
                }
                out_buffer.copy_from_slice(effective_payload);
            }
            8 => {
                super::reader_stream::decompress_deflate_in_memory(
                    entry,
                    effective_payload,
                    &mut out_buffer,
                )?;
            }
            93 => {
                let decomp_size = crate::codecs::zstd::zstd_decompress(effective_payload, &mut out_buffer)
                    .map_err(|e| super::reader_stream::map_zipcrypto_decode_err(entry, e))?;
                if decomp_size != uncomp_size {
                    return Err(super::reader_stream::map_zipcrypto_decode_err(
                        entry,
                        TTZipStatus::ErrCorruptHeader,
                    ));
                }
            }
            _ => return Err(TTZipStatus::ErrArchiveInitFailed),
        }

        if super::reader_stream::should_verify_entry_crc32(entry) {
            let computed_crc = crc32_fast(0, &out_buffer);
            if computed_crc != entry.crc32 {
                return Err(super::reader_stream::map_zipcrypto_decode_err(
                    entry,
                    TTZipStatus::ErrCorruptHeader,
                ));
            }
        }

        Ok(out_buffer)
    }

    /// Extracts all entries to the destination directory with Direct I/O and APFS extent preallocation.
    pub fn extract_all(
        &self,
        dest_dir: &Path,
        options: &TTZipExtractOptions,
    ) -> Result<ZipExtractReport, TTZipStatus> {
        self.extract_all_with_sheet(dest_dir, options, None)
    }

    /// Extracts all entries to the destination directory with fine-grained cancellation and atomic progress tracking.
    pub fn extract_all_with_sheet(
        &self,
        dest_dir: &Path,
        options: &TTZipExtractOptions,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<ZipExtractReport, TTZipStatus> {
        let start_time = std::time::Instant::now();
        fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let jail = DirJail::open(dest_dir)?;

        let mut engine = SafeExtractEngine::for_root(dest_dir);
        let num_entries = self.entries.len();
        let mut total_uncomp_bytes = 0u64;
        let mut total_comp_bytes = 0u64;
        let mut has_symlinks = false;

        let password_str = if !options.password.is_null() {
            unsafe { std::ffi::CStr::from_ptr(options.password) }
                .to_str()
                .ok()
        } else {
            None
        };

        let has_encrypted = self.entries.iter().any(|e| e.is_encrypted);
        if has_encrypted {
            if password_str.filter(|p| !p.is_empty()).is_none() {
                return Err(TTZipStatus::ErrInvalidPassword);
            }
            if let Some((enc_idx, _)) = self
                .entries
                .iter()
                .enumerate()
                .find(|(_, e)| {
                    e.is_encrypted
                        && !e.is_directory
                        && ((e.compression_method == 99 && e.compressed_size >= 28)
                            || (e.compression_method != 99 && e.compressed_size >= 12))
                })
            {
                self.verify_entry_encryption_key(enc_idx, password_str)?;
            }
        }

        let mut seen_file_comps = std::collections::HashSet::new();
        for entry in &self.entries {
            total_uncomp_bytes += entry.uncompressed_size;
            total_comp_bytes += entry.compressed_size;

            let safe_path = sanitize_and_validate_path(dest_dir, &entry.rel_path)?;
            let is_symlink = (entry.mode & 0o170000) == 0o120000;
            if is_symlink {
                has_symlinks = true;
            }
            if !entry.is_directory {
                let comps = DirJail::extract_clean_components(&entry.rel_path)?;
                if !seen_file_comps.insert(comps) {
                    has_symlinks = true;
                }
            }
            engine.register_entry_typed(
                safe_path,
                entry.mode & 0o7777,
                entry.mtime_epoch_secs,
                0,
                entry.is_directory,
                is_symlink,
            );

            if entry.is_directory && !options.dry_run {
                jail.create_dir_all_in_jail(&entry.rel_path)?;
            }
        }

        // 42.zip bomb defense: detect overlapping entry payload byte ranges
        let entry_intervals: Vec<(u64, u64)> = self
            .entries
            .iter()
            .map(|e| (e.lfh_offset, e.compressed_size))
            .collect();
        if detect_overlapping_entries(&entry_intervals) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        if options.dry_run {
            return Ok(ZipExtractReport {
                processed_entries_count: num_entries,
                total_uncompressed_bytes: total_uncomp_bytes,
                total_compressed_bytes: total_comp_bytes,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        let mut file_indices: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_directory)
            .map(|(i, _)| i)
            .collect();
        if has_encrypted {
            file_indices.sort_by_key(|&i| {
                let e = &self.entries[i];
                !(e.is_encrypted && e.uncompressed_size > 0)
            });
        }

        let thread_budget = crate::types::resolve_thread_budget(options.thread_budget);
        let local_sheet = ProgressAtomicSheet::with_total(total_uncomp_bytes);
        let active_sheet = sheet.unwrap_or(&local_sheet);
        active_sheet.set_total(total_uncomp_bytes);

        if thread_budget <= 1
            || file_indices.len() <= 4
            || has_symlinks
            || has_encrypted
            || options.progress_callback.is_some()
        {
            let mut quota_guard = ExtractionQuotaGuard::default_limits();
            for &idx in &file_indices {
                if active_sheet.is_cancelled() {
                    return Err(TTZipStatus::Cancelled);
                }
                let entry = &self.entries[idx];
                quota_guard.observe_and_validate(entry.compressed_size, entry.uncompressed_size)?;
                self.extract_single_file_to_disk(
                    &jail,
                    idx,
                    entry,
                    password_str,
                    Some(active_sheet),
                    Some((options, total_uncomp_bytes)),
                )?;
                let current_done = active_sheet.read_snapshot().processed_bytes;

                let elapsed_secs = start_time.elapsed().as_secs_f64();
                let instant_bps = if elapsed_secs > 0.001 {
                    (current_done as f64 / elapsed_secs) as u64
                } else {
                    0
                };
                active_sheet.update_progress(current_done, instant_bps);
            }
        } else {
            let pwd_owned = password_str.map(|s| s.to_string());
            let quota_mutex = std::sync::Mutex::new(ExtractionQuotaGuard::default_limits());

            let num_threads = ttzip_types::recommended_compute_threads().min(file_indices.len()).max(1);
            let next_work_idx = std::sync::atomic::AtomicUsize::new(0);
            let first_error: parking_lot::Mutex<Option<TTZipStatus>> = parking_lot::Mutex::new(None);
            let pool = LocalWorkStealingPool::with_threads(num_threads);

            pool.scope(|s| {
                for _ in 0..num_threads {
                    s.spawn(|| {
                        let res: Result<(), TTZipStatus> = (|| {
                            loop {
                                if active_sheet.is_cancelled() {
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
                                {
                                    let mut guard = quota_mutex.lock().map_err(|_| TTZipStatus::ErrSecurityViolation)?;
                                    guard.observe_and_validate(entry.compressed_size, entry.uncompressed_size)?;
                                }

                                self.extract_single_file_to_disk(
                                    &jail,
                                    idx,
                                    entry,
                                    pwd_owned.as_deref(),
                                    Some(active_sheet),
                                    None,
                                )?;
                            }
                            Ok(())
                        })();
                        if let Err(e) = res {
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

        if has_symlinks {
            jail.verify_all_jail_symlinks()?;
        }

        engine.apply_deferred_metadata(options.preserve_permissions)?;

        active_sheet.mark_finished();


        Ok(ZipExtractReport {
            processed_entries_count: num_entries,
            total_uncompressed_bytes: total_uncomp_bytes,
            total_compressed_bytes: total_comp_bytes,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn verify_entry_encryption_key(
        &self,
        entry_idx: usize,
        password: Option<&str>,
    ) -> Result<(), TTZipStatus> {
        let entry = self
            .entries
            .get(entry_idx)
            .ok_or(TTZipStatus::ErrInvalidOffset)?;
        if !entry.is_encrypted {
            return Ok(());
        }
        let pass = password
            .filter(|p| !p.is_empty())
            .ok_or(TTZipStatus::ErrInvalidPassword)?;
        if entry.uncompressed_size == 0 && entry.compressed_size == 0 {
            return Ok(());
        }
        let lfh_offset =
            usize::try_from(entry.lfh_offset).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        let (payload_offset, _) = parse_local_file_header(self.data, lfh_offset)?;
        let comp_size =
            usize::try_from(entry.compressed_size).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        let payload_end = payload_offset
            .checked_add(comp_size)
            .ok_or(TTZipStatus::ErrCorruptHeader)?;
        if payload_end > self.data.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let raw_payload = &self.data[payload_offset..payload_end];
        super::reader_stream::decrypt_slice_entry_payload(
            self.data,
            entry,
            lfh_offset,
            raw_payload,
            Some(pass),
        )
        .map(|_| ())
    }

    fn extract_single_file_to_disk(
        &self,
        jail: &DirJail,
        idx: usize,
        entry: &ZipEntry,
        password: Option<&str>,
        sheet: Option<&ProgressAtomicSheet>,
        progress_ctx: Option<(&TTZipExtractOptions, u64)>,
    ) -> Result<(), TTZipStatus> {
        self.verify_entry_encryption_key(idx, password)?;

        let is_symlink = (entry.mode & 0o170000) == 0o120000;
        if is_symlink {
            let bytes = self.extract_entry_bytes(idx, password)?;
            let target_str =
                std::str::from_utf8(&bytes).map_err(|_| TTZipStatus::ErrSecurityViolation)?;
            jail.create_symlink_in_jail(&entry.rel_path, target_str)?;
            return Self::check_zip_entry_progress(&entry.rel_path, sheet, progress_ctx);
        }

        let mut inflight =
            crate::fs::abort_guard::InFlightJailFile::open(jail, &entry.rel_path, true)?;
        if entry.uncompressed_size > 0 {
            let fd = inflight.file_mut().as_raw_fd();
            let _ = apfs_preallocate(fd, entry.uncompressed_size as i64);

            #[cfg(target_os = "macos")]
            if entry.uncompressed_size >= DIRECT_IO_THRESHOLD {
                unsafe {
                    libc::fcntl(fd, libc::F_NOCACHE, 1);
                }
            }

            self.extract_entry_to_sink_with_sheet(idx, password, inflight.file_mut(), sheet)?;
        }
        Self::check_zip_entry_progress(&entry.rel_path, sheet, progress_ctx)?;
        inflight.commit(jail)
    }

    fn check_zip_entry_progress(
        rel_path: &str,
        sheet: Option<&ProgressAtomicSheet>,
        progress_ctx: Option<(&TTZipExtractOptions, u64)>,
    ) -> Result<(), TTZipStatus> {
        if let Some((options, total_uncomp_bytes)) = progress_ctx {
            if let Some(cb) = options.progress_callback {
                let current_done = sheet.map_or(0, |s| s.read_snapshot().processed_bytes);
                let c_path = CString::new(rel_path).unwrap_or_default();
                let should_continue = unsafe {
                    cb(
                        current_done,
                        total_uncomp_bytes,
                        c_path.as_ptr(),
                        options.user_data,
                    )
                };
                if !should_continue {
                    if let Some(s) = sheet {
                        s.cancel();
                    }
                    return Err(TTZipStatus::Cancelled);
                }
            }
        }
        Ok(())
    }

    /// Streams decompressed entry bytes directly into `sink` with a bounded 64KB buffer quota,
    /// enforcing strict memory bounds without in-memory allocation limits.
    #[inline]
    pub fn extract_entry_to_sink<W: Write>(
        &self,
        entry_idx: usize,
        password: Option<&str>,
        sink: &mut W,
    ) -> Result<u64, TTZipStatus> {
        self.extract_entry_to_sink_with_sheet(entry_idx, password, sink, None)
    }

    /// Streams decompressed entry bytes directly into `sink` with fine-grained cancellation probe
    /// and atomic progress updating on each 64KB chunk.
    pub fn extract_entry_to_sink_with_sheet<W: Write>(
        &self,
        entry_idx: usize,
        password: Option<&str>,
        sink: &mut W,
        sheet: Option<&ProgressAtomicSheet>,
    ) -> Result<u64, TTZipStatus> {
        let entry = self
            .entries
            .get(entry_idx)
            .ok_or(TTZipStatus::ErrInvalidOffset)?;

        if entry.is_directory || (entry.uncompressed_size == 0 && !entry.is_encrypted) {
            return Ok(0);
        }
        if entry.uncompressed_size == 0 {
            self.verify_entry_encryption_key(entry_idx, password)?;
            return Ok(0);
        }

        let lfh_offset = usize::try_from(entry.lfh_offset).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
        let (payload_offset, _) = parse_local_file_header(self.data, lfh_offset)?;
        let comp_size = usize::try_from(entry.compressed_size).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

        let payload_end = payload_offset.checked_add(comp_size).ok_or(TTZipStatus::ErrCorruptHeader)?;
        if payload_end > self.data.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let raw_payload = &self.data[payload_offset..payload_end];
        let effective_cow = super::reader_stream::decrypt_slice_entry_payload(
            self.data,
            entry,
            lfh_offset,
            raw_payload,
            password,
        )?;
        let effective_payload = effective_cow.as_ref();

        let (total_written, computed_crc) =
            super::reader_stream::decode_decrypted_payload_to_sink(
                entry,
                effective_payload,
                sink,
                sheet,
            )?;

        if super::reader_stream::should_verify_entry_crc32(entry) && computed_crc != entry.crc32 {
            return Err(super::reader_stream::map_zipcrypto_decode_err(
                entry,
                TTZipStatus::ErrCorruptHeader,
            ));
        }

        Ok(total_written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TTZipEncryptionMethod;
    use crate::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

    #[test]
    fn test_extract_entry_to_sink_streaming() {
        let payload = b"Streaming extraction test payload with micro-buffer quota".repeat(100);
        let items = vec![ZipInputItem {
            rel_path: "stream_test.txt".to_string(),
            data: payload.clone(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed = compress_items_parallel(
            items,
            6,
            TTZipEncryptionMethod::None,
            None,
            1,
        ).expect("compression failed");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assembly failed");

        let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
        assert_eq!(archive.entries().len(), 1);

        let mut sink = Vec::new();
        let written = archive.extract_entry_to_sink(0, None, &mut sink).expect("extract to sink");
        assert_eq!(written, payload.len() as u64);
        assert_eq!(sink, payload);
    }

    #[test]
    fn test_extract_entry_bytes_clamps_at_64mb() {
        let items = vec![ZipInputItem {
            rel_path: "mock.txt".to_string(),
            data: b"small payload".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed = compress_items_parallel(
            items,
            0,
            TTZipEncryptionMethod::None,
            None,
            1,
        ).expect("compression failed");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assembly failed");

        let mut archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
        // Artificially simulate an entry whose uncompressed size exceeds 64MB
        archive.entries[0].uncompressed_size = (MAX_IN_MEMORY_DECOMPRESS_BYTES as u64) + 1;

        let res = archive.extract_entry_bytes(0, None);
        assert_eq!(res, Err(TTZipStatus::ErrOutOfMemory));
    }

    #[test]
    fn test_extract_entry_to_sink_circuit_breaker() {
        let payload = vec![b'A'; 10 * 1024];
        let items = vec![ZipInputItem {
            rel_path: "bomb.txt".to_string(),
            data: payload,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed = compress_items_parallel(
            items,
            6,
            TTZipEncryptionMethod::None,
            None,
            1,
        ).expect("compression failed");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assembly failed");

        let mut archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
        // Forged Central Directory uncompressed_size: claiming it's only 10 bytes
        archive.entries[0].uncompressed_size = 10;

        let mut sink = Vec::new();
        let res = archive.extract_entry_to_sink(0, None, &mut sink);
        assert_eq!(res, Err(TTZipStatus::ErrSecurityViolation));
    }

    #[test]
    fn test_zip_intermediate_symlink_traversal_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("extract_root");
        fs::create_dir_all(&dest).unwrap();

        let outside = tmp.path().join("outside_target");
        fs::create_dir_all(&outside).unwrap();

        // Create an intermediate symlink directory inside dest
        let symlink_dir = dest.join("symdir");
        std::os::unix::fs::symlink(&outside, &symlink_dir).unwrap();

        // Create zip with entry pointing inside symdir
        let items = vec![ZipInputItem {
            rel_path: "symdir/evil.txt".to_string(),
            data: b"evil content".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed = compress_items_parallel(
            items,
            0,
            TTZipEncryptionMethod::None,
            None,
            1,
        ).expect("compression failed");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assembly failed");

        let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
        let opts = TTZipExtractOptions::default();
        let res = archive.extract_all(&dest, &opts);
        assert_eq!(res.err(), Some(TTZipStatus::ErrSecurityViolation));
        assert!(!outside.join("evil.txt").exists());
    }

    #[test]
    fn test_extract_entry_to_sink_cancellation_probe() {
        let payload = vec![0x42u8; 256 * 1024];
        let items = vec![ZipInputItem {
            rel_path: "large.bin".to_string(),
            data: payload,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed = compress_items_parallel(
            items,
            1,
            TTZipEncryptionMethod::None,
            None,
            1,
        ).expect("compression failed");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assembly failed");

        let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");
        let sheet = ProgressAtomicSheet::new();
        sheet.cancel();

        let mut sink = Vec::new();
        let res = archive.extract_entry_to_sink_with_sheet(0, None, &mut sink, Some(&sheet));
        assert_eq!(res.err(), Some(TTZipStatus::Cancelled));
    }

    #[test]
    fn test_extract_all_with_sheet_progress_and_cancel() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("extract_cancel");
        fs::create_dir_all(&dest).unwrap();

        let payload = vec![0x99u8; 128 * 1024];
        let items = vec![ZipInputItem {
            rel_path: "item.bin".to_string(),
            data: payload,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed = compress_items_parallel(
            items,
            1,
            TTZipEncryptionMethod::None,
            None,
            1,
        ).expect("compression failed");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assembly failed");

        let archive = ZipArchive::open_slice(&zip_bytes).expect("open zip slice");

        // Test normal extraction with sheet tracking
        let sheet = ProgressAtomicSheet::new();
        let opts = TTZipExtractOptions::default();
        let report = archive.extract_all_with_sheet(&dest, &opts, Some(&sheet)).unwrap();
        assert_eq!(report.processed_entries_count, 1);
        let snap = sheet.read_snapshot();
        assert!(snap.is_finished);
        assert_eq!(snap.processed_bytes, 128 * 1024);

        // Test cancellation abort
        let cancel_sheet = ProgressAtomicSheet::new();
        cancel_sheet.cancel();
        let cancel_dest = tmp.path().join("dest2");
        let res = archive.extract_all_with_sheet(&cancel_dest, &opts, Some(&cancel_sheet));
        assert_eq!(res.err(), Some(TTZipStatus::Cancelled));
    }
}

