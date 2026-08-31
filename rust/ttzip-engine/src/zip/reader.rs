// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ZIP Archive Decompression and Extraction Engine.
//!
//! Features Rayon multi-core parallel decompression, thread-local libdeflate pooling,
//! WinZip AES-256 hardware decryption, Direct I/O + APFS extent preallocation, and ZipSlip-immune safe landing.

use crate::codecs::deflate::with_thread_local_decompressor;
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha1::winzip_aes256_decrypt_and_verify;
use crate::fs::apfs::apfs_preallocate;
use crate::fs::safe_extract::{sanitize_and_validate_path, SafeExtractEngine};
use crate::types::{TTZipExtractOptions, TTZipStatus};
use crate::zip::parser::{parse_all_entries, parse_local_file_header, ZipEntry};
use rayon::prelude::*;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

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

        if entry.is_directory || entry.uncompressed_size == 0 {
            return Ok(Vec::new());
        }

        let lfh_offset = entry.lfh_offset as usize;
        let (payload_offset, _) = parse_local_file_header(self.data, lfh_offset)?;
        let comp_size = entry.compressed_size as usize;

        if payload_offset + comp_size > self.data.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let raw_payload = &self.data[payload_offset..payload_offset + comp_size];
        let mut decrypted_storage = Vec::new();

        let effective_payload = if entry.is_encrypted {
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            if pass.is_empty() {
                return Err(TTZipStatus::ErrInvalidPassword);
            }
            if comp_size < 28 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            let cipher_len = comp_size - 28;
            decrypted_storage.resize(cipher_len, 0);
            let dec_len = winzip_aes256_decrypt_and_verify(pass, raw_payload, &mut decrypted_storage)?;
            &decrypted_storage[..dec_len]
        } else {
            raw_payload
        };

        let uncomp_size = entry.uncompressed_size as usize;
        // Defense-in-depth: limit single-entry in-memory extraction to 256MB to prevent zip-bomb OOM attacks
        if uncomp_size > 256 * 1024 * 1024 {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let mut out_buffer = vec![0u8; uncomp_size];

        match entry.actual_method {
            0 => {
                if effective_payload.len() != uncomp_size {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                out_buffer.copy_from_slice(effective_payload);
            }
            8 => {
                let decomp_size = with_thread_local_decompressor(|dec| {
                    dec.decompress(effective_payload, &mut out_buffer)
                })?;
                if decomp_size != uncomp_size {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            _ => return Err(TTZipStatus::ErrArchiveInitFailed),
        }

        if !entry.is_encrypted || entry.crc32 != 0 {
            let computed_crc = crc32_fast(0, &out_buffer);
            if computed_crc != entry.crc32 {
                return Err(TTZipStatus::ErrCorruptHeader);
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
        let start_time = std::time::Instant::now();
        fs::create_dir_all(dest_dir).map_err(|_| TTZipStatus::ErrOpenFailed)?;

        let mut engine = SafeExtractEngine::new();
        let num_entries = self.entries.len();
        let mut total_uncomp_bytes = 0u64;
        let mut total_comp_bytes = 0u64;

        let password_str = if !options.password.is_null() {
            unsafe { std::ffi::CStr::from_ptr(options.password) }
                .to_str()
                .ok()
        } else {
            None
        };

        for entry in &self.entries {
            total_uncomp_bytes += entry.uncompressed_size;
            total_comp_bytes += entry.compressed_size;

            let safe_path = sanitize_and_validate_path(dest_dir, &entry.rel_path)?;
            let is_symlink = (entry.mode & 0o170000) == 0o120000;
            if !is_symlink {
                engine.register_entry(
                    safe_path.clone(),
                    entry.mode & 0o7777,
                    entry.mtime_epoch_secs,
                    0,
                    entry.is_directory,
                );
            }

            if entry.is_directory {
                fs::create_dir_all(&safe_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            }
        }

        if options.dry_run {
            return Ok(ZipExtractReport {
                processed_entries_count: num_entries,
                total_uncompressed_bytes: total_uncomp_bytes,
                total_compressed_bytes: total_comp_bytes,
                duration_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        let file_indices: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_directory)
            .map(|(i, _)| i)
            .collect();

        let thread_budget = (options.thread_budget as usize).clamp(1, 64);
        let processed_bytes = Arc::new(AtomicU64::new(0));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        if thread_budget <= 1 || file_indices.len() <= 4 {
            for &idx in &file_indices {
                let entry = &self.entries[idx];
                self.extract_single_file_to_disk(dest_dir, idx, entry, password_str)?;
                let current_done = processed_bytes.fetch_add(entry.uncompressed_size, Ordering::Relaxed)
                    + entry.uncompressed_size;

                if let Some(cb) = options.progress_callback {
                    let c_path = CString::new(entry.rel_path.as_str()).unwrap_or_default();
                    let should_continue = unsafe {
                        cb(current_done, total_uncomp_bytes, c_path.as_ptr(), options.user_data)
                    };
                    if !should_continue {
                        return Err(TTZipStatus::Cancelled);
                    }
                }
            }
        } else {
            let pool = crate::platform::cpu::EngineThreadPool::global();

            let pwd_owned = password_str.map(|s| s.to_string());
            let cb_mutex = std::sync::Mutex::new(0u64);
            let progress_cb = options.progress_callback;
            let user_data_raw = options.user_data as usize;

            let par_res = pool.install(|| {
                file_indices.par_iter().try_for_each(|&idx| -> Result<(), TTZipStatus> {
                    if cancel_flag.load(Ordering::Acquire) {
                        return Err(TTZipStatus::Cancelled);
                    }

                    let entry = &self.entries[idx];
                    self.extract_single_file_to_disk(dest_dir, idx, entry, pwd_owned.as_deref())?;
                    let current_done = processed_bytes.fetch_add(entry.uncompressed_size, Ordering::Relaxed)
                        + entry.uncompressed_size;

                    if let Some(cb) = progress_cb {
                        if let Ok(mut last_bytes) = cb_mutex.try_lock() {
                            if current_done.saturating_sub(*last_bytes) >= 1024 * 1024 || current_done >= total_uncomp_bytes {
                                *last_bytes = current_done;
                                let c_path = CString::new(entry.rel_path.as_str()).unwrap_or_default();
                                let should_continue = unsafe {
                                    cb(current_done, total_uncomp_bytes, c_path.as_ptr(), user_data_raw as *mut std::ffi::c_void)
                                };
                                if !should_continue {
                                    cancel_flag.store(true, Ordering::Release);
                                    return Err(TTZipStatus::Cancelled);
                                }
                            }
                        }
                    }
                    Ok(())
                })
            });
            par_res?;
        }

        if options.preserve_permissions {
            engine.apply_all()?;
        }

        Ok(ZipExtractReport {
            processed_entries_count: num_entries,
            total_uncompressed_bytes: total_uncomp_bytes,
            total_compressed_bytes: total_comp_bytes,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    fn extract_single_file_to_disk(
        &self,
        dest_dir: &Path,
        idx: usize,
        entry: &ZipEntry,
        password: Option<&str>,
    ) -> Result<(), TTZipStatus> {
        let safe_path = sanitize_and_validate_path(dest_dir, &entry.rel_path)?;
        if let Some(parent) = safe_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let is_symlink = (entry.mode & 0o170000) == 0o120000;
        if is_symlink {
            let bytes = self.extract_entry_bytes(idx, password)?;
            if let Ok(target_str) = std::str::from_utf8(&bytes) {
                // Strict depth tracking traversal defense
                let mut depth = Path::new(&entry.rel_path).components().count().saturating_sub(1);
                for comp in Path::new(target_str).components() {
                    match comp {
                        std::path::Component::ParentDir => {
                            if depth == 0 {
                                return Err(TTZipStatus::ErrSecurityViolation);
                            }
                            depth -= 1;
                        }
                        std::path::Component::Normal(_) => {
                            depth += 1;
                        }
                        std::path::Component::CurDir => {}
                        std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                            return Err(TTZipStatus::ErrSecurityViolation);
                        }
                    }
                }
                if safe_path.exists() || fs::symlink_metadata(&safe_path).is_ok() {
                    let _ = fs::remove_file(&safe_path);
                }
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(target_str, &safe_path);
                return Ok(());
            }
        }

        if entry.uncompressed_size == 0 {
            File::create(&safe_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
            return Ok(());
        }

        let lfh_offset = entry.lfh_offset as usize;
        let (payload_offset, _) = parse_local_file_header(self.data, lfh_offset)?;
        let comp_size = entry.compressed_size as usize;

        if payload_offset + comp_size > self.data.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let raw_payload = &self.data[payload_offset..payload_offset + comp_size];
        let mut decrypted_storage = Vec::new();

        let effective_payload = if entry.is_encrypted {
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            if pass.is_empty() {
                return Err(TTZipStatus::ErrInvalidPassword);
            }
            if comp_size < 28 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            let cipher_len = comp_size - 28;
            decrypted_storage.resize(cipher_len, 0);
            let dec_len = winzip_aes256_decrypt_and_verify(pass, raw_payload, &mut decrypted_storage)?;
            &decrypted_storage[..dec_len]
        } else {
            raw_payload
        };

        let mut file = File::create(&safe_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
        let fd = file.as_raw_fd();

        let _ = apfs_preallocate(fd, entry.uncompressed_size as i64);

        #[cfg(target_os = "macos")]
        if entry.uncompressed_size >= DIRECT_IO_THRESHOLD {
            unsafe {
                libc::fcntl(fd, libc::F_NOCACHE, 1);
            }
        }

        let mut computed_crc = 0u32;
        let uncomp_size = entry.uncompressed_size;

        match entry.actual_method {
            0 => {
                if (effective_payload.len() as u64) != uncomp_size {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                file.write_all(effective_payload).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                computed_crc = crc32_fast(0, effective_payload);
            }
            8 => {
                use std::io::Read;
                let mut decoder = flate2::read::DeflateDecoder::new(effective_payload);
                let mut chunk = [0u8; 64 * 1024];
                let mut total_written = 0u64;
                loop {
                    let n = decoder.read(&mut chunk).map_err(|_| TTZipStatus::ErrCorruptHeader)?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&chunk[..n]).map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                    computed_crc = crc32_fast(computed_crc, &chunk[..n]);
                    total_written += n as u64;
                }
                if total_written != uncomp_size {
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
            }
            _ => return Err(TTZipStatus::ErrArchiveInitFailed),
        }

        if (!entry.is_encrypted || entry.crc32 != 0) && computed_crc != entry.crc32 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        Ok(())
    }
}
