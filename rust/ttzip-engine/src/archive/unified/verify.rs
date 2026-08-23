// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use libc::c_void;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::crypto::crc32::crc32_fast;
use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::types::{TTZipProgressCallback, TTZipStatus};
use crate::zip::reader::ZipArchive;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CorruptedEntryDetail {
    pub entry_path: String,
    pub error_type: String, // "crc32_mismatch" | "header_damaged" | "block_truncated" | "invalid_dictionary"
    pub expected_checksum: String,
    pub actual_checksum: String,
    pub diagnostic_message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveIntegrityReport {
    pub archive_path: String,
    pub total_entries_count: usize,
    pub verified_entries_count: usize,
    pub corrupted_entries_count: usize,
    pub overall_status: String, // "passed" | "corrupted" | "unreadable" | "encrypted_missing_key"
    pub verification_duration_seconds: f64,
    #[serde(rename = "averageThroughputMBs")]
    pub average_throughput_mbs: f64,
    pub corrupted_entries: Vec<CorruptedEntryDetail>,
}

/// Executes pure in-memory stream-discarding integrity verification across archive entries.
pub fn verify_archive_stream(
    archive_path: &Path,
    password: Option<&str>,
    progress_callback: TTZipProgressCallback,
    user_data: *mut c_void,
) -> Result<ArchiveIntegrityReport, TTZipStatus> {
    if !archive_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let start_time = Instant::now();
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // -----------------------------------------------------------------------
    // PATHWAY A: ZIP Multi-Threaded In-Memory Verification (Rayon)
    // -----------------------------------------------------------------------
    if ext == "zip" || ext == "cbz" || ext == "jar" {
        let mapped = match fs::read(archive_path) {
            Ok(m) => m,
            Err(_) => return Err(TTZipStatus::ErrOpenFailed),
        };
        let zip = match ZipArchive::open_slice(&mapped) {
            Ok(z) => z,
            Err(e) => {
                let duration = start_time.elapsed().as_secs_f64();
                return Ok(ArchiveIntegrityReport {
                    archive_path: archive_path.to_string_lossy().to_string(),
                    total_entries_count: 1,
                    verified_entries_count: 0,
                    corrupted_entries_count: 1,
                    overall_status: "corrupted".to_string(),
                    verification_duration_seconds: duration,
                    average_throughput_mbs: 0.0,
                    corrupted_entries: vec![CorruptedEntryDetail {
                        entry_path: archive_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                        error_type: "header_damaged".to_string(),
                        expected_checksum: "".to_string(),
                        actual_checksum: "".to_string(),
                        diagnostic_message: format!("Archive header corrupted or invalid: {:?}", e),
                    }],
                });
            }
        };
        let total_entries = zip.entries().len();

        let verified_count = Arc::new(AtomicUsize::new(0));
        let total_bytes = Arc::new(AtomicU64::new(0));
        let corrupted_list = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let user_data_addr = user_data as usize;

        zip.entries().par_iter().enumerate().for_each(|(idx, entry)| {
            if entry.is_directory || entry.uncompressed_size == 0 {
                verified_count.fetch_add(1, Ordering::Relaxed);
                return;
            }

            match zip.extract_entry_bytes(idx, password) {
                Ok(bytes) => {
                    let actual_crc = crc32_fast(0, &bytes);
                    if entry.crc32 != 0 && actual_crc != entry.crc32 {
                        corrupted_list.lock().push(CorruptedEntryDetail {
                            entry_path: entry.rel_path.clone(),
                            error_type: "crc32_mismatch".to_string(),
                            expected_checksum: format!("{:08X}", entry.crc32),
                            actual_checksum: format!("{:08X}", actual_crc),
                            diagnostic_message: "Calculated CRC32 does not match ZIP local header checksum".to_string(),
                        });
                    } else {
                        verified_count.fetch_add(1, Ordering::Relaxed);
                        total_bytes.fetch_add(entry.uncompressed_size, Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    let err_type = if e == TTZipStatus::ErrInvalidPassword {
                        "invalid_dictionary"
                    } else {
                        "header_damaged"
                    };
                    corrupted_list.lock().push(CorruptedEntryDetail {
                        entry_path: entry.rel_path.clone(),
                        error_type: err_type.to_string(),
                        expected_checksum: format!("{:08X}", entry.crc32),
                        actual_checksum: "".to_string(),
                        diagnostic_message: format!("Decompression failed with status: {:?}", e),
                    });
                }
            }

            if let Some(cb) = progress_callback {
                let v = verified_count.load(Ordering::Relaxed);
                let c_name = CString::new(entry.rel_path.as_str()).unwrap_or_default();
                unsafe {
                    cb(v as u64, total_entries as u64, c_name.as_ptr(), user_data_addr as *mut c_void);
                }
            }
        });

        let duration = start_time.elapsed().as_secs_f64().max(0.0001);
        let uncompressed = total_bytes.load(Ordering::Relaxed);
        let throughput = (uncompressed as f64 / (1024.0 * 1024.0)) / duration;
        let corrupted = corrupted_list.lock().clone();

        let status = if corrupted.is_empty() {
            "passed"
        } else if password.is_none() && zip.entries().iter().any(|e| e.is_encrypted) {
            "encrypted_missing_key"
        } else {
            "corrupted"
        };

        return Ok(ArchiveIntegrityReport {
            archive_path: archive_path.to_string_lossy().to_string(),
            total_entries_count: total_entries,
            verified_entries_count: verified_count.load(Ordering::Relaxed),
            corrupted_entries_count: corrupted.len(),
            overall_status: status.to_string(),
            verification_duration_seconds: duration,
            average_throughput_mbs: throughput,
            corrupted_entries: corrupted,
        });
    }

    // -----------------------------------------------------------------------
    // PATHWAY B: General Stream-Discarding Verification (Libarchive)
    // -----------------------------------------------------------------------
    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);

        if let Some(pwd) = password {
            if let Ok(c_pwd) = CString::new(pwd) {
                archive_read_add_passphrase(a, c_pwd.as_ptr());
            }
        }

        if archive_read_open_filename(a, arch_c.as_ptr(), 65536) != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        let mut entry: *mut c_void = std::ptr::null_mut();
        let mut total_entries = 0usize;
        let mut verified_count = 0usize;
        let mut total_decomp_bytes = 0u64;
        let mut corrupted_list = Vec::new();
        let mut chunk_buf = vec![0u8; 65536];

        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }
            total_entries += 1;

            let raw_path = archive_entry_pathname(entry);
            let entry_path_str = if !raw_path.is_null() {
                CStr::from_ptr(raw_path).to_string_lossy().to_string()
            } else {
                format!("entry_{}", total_entries)
            };

            let is_dir = archive_entry_size(entry) == 0;
            if is_dir {
                verified_count += 1;
                archive_read_data_skip(a);
                continue;
            }

            let mut running_crc = 0u32;
            let mut entry_has_error = false;

            loop {
                let r = archive_read_data(a, chunk_buf.as_mut_ptr() as *mut c_void, chunk_buf.len());
                if r < 0 {
                    corrupted_list.push(CorruptedEntryDetail {
                        entry_path: entry_path_str.clone(),
                        error_type: "block_truncated".to_string(),
                        expected_checksum: "".to_string(),
                        actual_checksum: format!("{:08X}", running_crc),
                        diagnostic_message: "Decompression stream read error".to_string(),
                    });
                    entry_has_error = true;
                    break;
                }
                if r == 0 {
                    break;
                }
                let read_bytes = r as usize;
                running_crc = crc32_fast(running_crc, &chunk_buf[..read_bytes]);
                total_decomp_bytes += read_bytes as u64;
            }

            if !entry_has_error {
                verified_count += 1;
            }

            if let Some(cb) = progress_callback {
                cb(verified_count as u64, total_entries as u64, raw_path, user_data);
            }
        }

        let duration = start_time.elapsed().as_secs_f64().max(0.0001);
        let throughput = (total_decomp_bytes as f64 / (1024.0 * 1024.0)) / duration;

        let status = if corrupted_list.is_empty() {
            "passed"
        } else {
            "corrupted"
        };

        Ok(ArchiveIntegrityReport {
            archive_path: archive_path.to_string_lossy().to_string(),
            total_entries_count: total_entries,
            verified_entries_count: verified_count,
            corrupted_entries_count: corrupted_list.len(),
            overall_status: status.to_string(),
            verification_duration_seconds: duration,
            average_throughput_mbs: throughput,
            corrupted_entries: corrupted_list,
        })
    }
}
