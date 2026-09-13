// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Stream-First extraction and bidirectional file transfer UniFFI pipeline.
//!
//! Exposes zero-intermediate-file extraction to remote Android devices,
//! ranged streaming downloads, and bulk uploads via registered storage drivers.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use bytes::Bytes;
use ttzip_device_android::models::{TransferDirection, TransferJob, TransferStatus};

use super::registry::{block_on_driver, get_registered_driver};
use super::types::UniFFITransferJob;
use crate::archive::{ARCHIVE_EOF, ARCHIVE_OK};
use crate::ffi::archive_ffi::sys::*;
use crate::pipeline::device_sink::DeviceExtractionSink;
use crate::uniffi_api::types::TTZipError;

/// Directly extracts a local archive to remote Android device directory via streaming pipeline.
///
/// Enforces Stream-First Invariant: reads archive chunks and streams them directly into
/// `DeviceStorageDriver::send_object`, producing 0 bytes intermediate staging in `/tmp`.
#[uniffi::export]
pub fn uniffi_extract_to_device(
    archive_path: String,
    destination_device_id: String,
    destination_dir: String,
) -> Result<UniFFITransferJob, TTZipError> {
    let src_path = Path::new(&archive_path);
    if !src_path.exists() {
        return Err(TTZipError::FileNotFound {
            path: archive_path.clone(),
        });
    }

    let driver = get_registered_driver(&destination_device_id)?;
    let total_archive_size = std::fs::metadata(src_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let job_id = uuid_v4_string();
    let sink = DeviceExtractionSink::new(driver, &destination_dir);

    // Stream-First extraction reading archive headers and piping entry streams
    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipError::EngineError { code: -1 });
        }
        archive_read_support_filter_all(a);
        archive_read_support_format_all(a);

        let path_c = std::ffi::CString::new(archive_path.as_str())
            .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

        let open_res = archive_read_open_filename(a, path_c.as_ptr(), 65536);
        if open_res != 0 {
            archive_read_free(a);
            return Err(TTZipError::IoError {
                message: format!("Failed to open archive: {archive_path}"),
            });
        }

        let mut entry: *mut libc::c_void = std::ptr::null_mut();
        let mut extracted_bytes: u64 = 0;

        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }

            let raw_path = archive_entry_pathname(entry);
            if raw_path.is_null() {
                archive_read_data_skip(a);
                continue;
            }

            let rel_name = match std::ffi::CStr::from_ptr(raw_path).to_str() {
                Ok(s) => s.trim_start_matches(['/', '\\']),
                Err(_) => {
                    archive_read_data_skip(a);
                    continue;
                }
            };

            if rel_name.is_empty() || rel_name.ends_with('/') {
                archive_read_data_skip(a);
                continue;
            }

            let mut writer = match sink.open_entry_writer(rel_name) {
                Ok(w) => w,
                Err(status) => {
                    archive_read_free(a);
                    return Err(TTZipError::EngineError { code: status as i32 });
                }
            };

            let mut block_ptr: *const libc::c_void = std::ptr::null();
            let mut block_size: libc::size_t = 0;
            let mut block_offset: i64 = 0;

            loop {
                let r = archive_read_data_block(
                    a,
                    &mut block_ptr,
                    &mut block_size,
                    &mut block_offset,
                );
                if r == ARCHIVE_EOF {
                    break;
                }
                if r < ARCHIVE_OK {
                    archive_read_free(a);
                    return Err(TTZipError::IoError {
                        message: format!("Failed extracting entry: {rel_name}"),
                    });
                }

                if !block_ptr.is_null() && block_size > 0 {
                    let chunk = std::slice::from_raw_parts(
                        block_ptr as *const u8,
                        block_size as usize,
                    );
                    if let Err(status) = writer.write_chunk(chunk) {
                        archive_read_free(a);
                        return Err(TTZipError::EngineError { code: status as i32 });
                    }
                    extracted_bytes += block_size as u64;
                }
            }

            if let Err(status) = writer.finalize() {
                archive_read_free(a);
                return Err(TTZipError::EngineError { code: status as i32 });
            }
        }

        archive_read_free(a);

        let final_job = TransferJob {
            job_id,
            direction: TransferDirection::DirectPipelineExtract,
            source_path: archive_path,
            destination_path: destination_dir,
            total_bytes: total_archive_size.max(extracted_bytes),
            transferred_bytes: total_archive_size.max(extracted_bytes),
            current_speed_bps: 0,
            status: TransferStatus::Completed,
            error_message: None,
        };

        Ok(final_job.into())
    }
}

/// Downloads a remote file from Android device storage to local macOS path.
#[uniffi::export]
pub fn uniffi_download_file(
    device_id: String,
    remote_path: String,
    local_path: String,
) -> Result<UniFFITransferJob, TTZipError> {
    let driver = get_registered_driver(&device_id)?;
    let parent_path = if let Some(idx) = remote_path.rfind('/') {
        if idx == 0 { "/" } else { &remote_path[..idx] }
    } else {
        "/"
    };

    let nodes = block_on_driver(driver.list_directory(parent_path))
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let node = nodes
        .into_iter()
        .find(|n| n.path == remote_path)
        .ok_or_else(|| TTZipError::FileNotFound {
            path: remote_path.clone(),
        })?;

    let total_size = node.size_bytes;
    let mut out_file = File::create(&local_path).map_err(|e| TTZipError::IoError {
        message: format!("Cannot create local file {local_path}: {e}"),
    })?;

    let mut transferred: u64 = 0;
    let chunk_size = 1024 * 1024; // 1 MB chunk streaming

    while transferred < total_size {
        let to_read = ((total_size - transferred) as usize).min(chunk_size);
        let chunk_data = block_on_driver(driver.get_partial_object(&remote_path, transferred, to_read as u32))
            .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

        out_file.write_all(&chunk_data).map_err(|e| TTZipError::IoError {
            message: format!("Failed writing chunk at {transferred}: {e}"),
        })?;

        transferred += chunk_data.len() as u64;
    }

    out_file.flush().map_err(|e| TTZipError::IoError {
        message: format!("Failed flushing file {local_path}: {e}"),
    })?;

    let job = TransferJob {
        job_id: uuid_v4_string(),
        direction: TransferDirection::AndroidToMac,
        source_path: remote_path,
        destination_path: local_path,
        total_bytes: total_size,
        transferred_bytes: transferred,
        current_speed_bps: 0,
        status: TransferStatus::Completed,
        error_message: None,
    };

    Ok(job.into())
}

/// Uploads a local file from host macOS to remote Android destination directory.
#[uniffi::export]
pub fn uniffi_upload_file(
    device_id: String,
    local_path: String,
    remote_dir: String,
) -> Result<UniFFITransferJob, TTZipError> {
    let driver = get_registered_driver(&device_id)?;
    let mut file = File::open(&local_path).map_err(|e| TTZipError::FileNotFound {
        path: format!("{local_path}: {e}"),
    })?;

    let metadata = file.metadata().map_err(|e| TTZipError::IoError {
        message: e.to_string(),
    })?;
    let total_size = metadata.len();

    let mut buffer = Vec::with_capacity(total_size as usize);
    file.read_to_end(&mut buffer).map_err(|e| TTZipError::IoError {
        message: e.to_string(),
    })?;

    let filename = Path::new(&local_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload_payload");

    let clean_dir = remote_dir.trim_end_matches('/');
    let target_remote_path = format!("{clean_dir}/{filename}");

    block_on_driver(driver.send_object(&target_remote_path, Bytes::from(buffer)))
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;

    let job = TransferJob {
        job_id: uuid_v4_string(),
        direction: TransferDirection::MacToAndroid,
        source_path: local_path,
        destination_path: target_remote_path,
        total_bytes: total_size,
        transferred_bytes: total_size,
        current_speed_bps: 0,
        status: TransferStatus::Completed,
        error_message: None,
    };

    Ok(job.into())
}

fn uuid_v4_string() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(123456789);
    let hi = (nanos >> 64) as u64;
    let lo = nanos as u64;
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (hi >> 32) as u32,
        (hi >> 16) as u16,
        (hi & 0x0FFF) as u16,
        ((lo >> 48) as u16 & 0x0FFF) | 0x8000,
        lo & 0xFFFFFFFFFFFF
    )
}
