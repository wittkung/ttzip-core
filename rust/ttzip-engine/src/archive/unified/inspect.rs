// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Archive non-destructive inspection submodule for Unified Orchestrator.

use std::ffi::{CStr, CString};
use std::path::Path;

use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::types::{TTZipEntryMetadata, TTZipInspectCallback, TTZipStatus};
use libc::{c_void, mode_t};

/// Inspects an archive and invokes the callback for every discovered entry metadata item.
pub fn inspect_archive(
    archive_path: &Path,
    password: Option<&str>,
    detect_encoding: bool,
    callback: TTZipInspectCallback,
    user_data: *mut c_void,
) -> Result<usize, TTZipStatus> {
    if callback.is_none() {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    if !archive_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;
    let pwd_c = password.and_then(|p| CString::new(p).ok());

    let volume_chain = crate::archive::split::detect_volume_chain(archive_path)
        .unwrap_or_else(|_| vec![archive_path.to_path_buf()]);

    if volume_chain.len() > 1 {
        let virtual_reader = crate::archive::split::VirtualMultiVolumeReader::from_volumes(volume_chain)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let stream_reader = crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(virtual_reader, 65536)?;
        let a = stream_reader.as_raw_archive();
        if let Some(ref p) = pwd_c {
            unsafe { archive_read_add_passphrase(a, p.as_ptr()); }
        }
        return unsafe { inspect_from_handle(a, detect_encoding, callback, user_data) };
    }

    if volume_chain.len() == 1 {
        if let Ok(source) = crate::archive::source::open_archive_source(archive_path) {
            if let Some(mapped) = source.as_slice() {
                if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
                    if let Ok(archive) = crate::sevenz::SevenZArchive::open_slice_with_password(mapped, password) {
                        let mut count = 0;
                        for (idx, file) in archive.files().iter().enumerate() {
                            let path_c = match CString::new(file.rel_path.as_str()) {
                                Ok(c) => c,
                                Err(_) => continue,
                            };
                            let mut detected_c_str: Option<CString> = None;
                            if detect_encoding {
                                let path_bytes = file.rel_path.as_bytes();
                                if path_bytes.iter().any(|&b| b >= 0x80) {
                                    if let Some(charset) = crate::codecs::chardet::detect_charset(path_bytes) {
                                        detected_c_str = CString::new(charset.as_str()).ok();
                                    }
                                }
                            }
                            let loc = archive.seek_index().entries.get(idx);
                            let uncomp_sz = loc.map(|l| l.uncompressed_size).unwrap_or(0);
                            let crc_val = loc.and_then(|l| l.crc).unwrap_or(0);

                            let meta = TTZipEntryMetadata {
                                struct_size: std::mem::size_of::<TTZipEntryMetadata>() as u32,
                                abi_version: crate::types::TTZIP_ABI_VERSION_2,
                                path: path_c.as_ptr(),
                                uncompressed_size: uncomp_sz,
                                compressed_size: 0,
                                crc32: crc_val,
                                mtime_epoch_secs: file.mtime_epoch_secs.unwrap_or(0),
                                mode: file.mode,
                                is_directory: file.is_directory,
                                is_encrypted: archive.info().is_encrypted,
                                compression_method: archive.info().primary_method_id as u16,
                                detected_encoding: detected_c_str.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                            };
                            count += 1;
                            if let Some(cb) = callback {
                                if unsafe { !cb(&meta, user_data) } {
                                    break;
                                }
                            }
                        }
                        return Ok(count);
                    }
                }
            }
        }
    }

    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);

        if let Some(ref p) = pwd_c {
            archive_read_add_passphrase(a, p.as_ptr());
        }

        let open_rc = archive_read_open_filename(a, arch_c.as_ptr(), 65536);
        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        inspect_from_handle(a, detect_encoding, callback, user_data)
    }
}

unsafe fn inspect_from_handle(
    a: *mut c_void,
    detect_encoding: bool,
    cb: TTZipInspectCallback,
    user_data: *mut c_void,
) -> Result<usize, TTZipStatus> {
    let mut entry: *mut c_void = std::ptr::null_mut();
    let mut count = 0;

    while archive_read_next_header(a, &mut entry) == 0 {
        if entry.is_null() {
            break;
        }
        let raw_path = archive_entry_pathname(entry);
        if raw_path.is_null() {
            archive_read_data_skip(a);
            continue;
        }

        let path_bytes = CStr::from_ptr(raw_path).to_bytes();
        if path_bytes.is_empty() {
            archive_read_data_skip(a);
            continue;
        }

        let mut detected_c_str: Option<CString> = None;
        if detect_encoding {
            let has_non_ascii = path_bytes.iter().any(|&b| b >= 0x80);
            if has_non_ascii {
                if let Some(charset) = crate::codecs::chardet::detect_charset(path_bytes) {
                    detected_c_str = CString::new(charset.as_str()).ok();
                }
            }
        }

        let uncompressed_size = archive_entry_size(entry).max(0) as u64;
        let mode = archive_entry_mode(entry) as u32;
        let filetype = archive_entry_filetype(entry);
        let is_dir = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
            || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
            || path_bytes.ends_with(b"/");
        let mtime = archive_entry_mtime(entry) as i64;
        let is_data_enc = archive_entry_is_data_encrypted(entry) != 0;
        let is_meta_enc = archive_entry_is_metadata_encrypted(entry) != 0;

        let meta = TTZipEntryMetadata {
            struct_size: std::mem::size_of::<TTZipEntryMetadata>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            path: raw_path,
            uncompressed_size,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: mtime,
            mode,
            is_directory: is_dir,
            is_encrypted: is_data_enc || is_meta_enc,
            compression_method: 0,
            detected_encoding: detected_c_str.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
        };

        count += 1;
        let should_continue = if let Some(callback) = cb {
            callback(&meta, user_data)
        } else {
            true
        };
        archive_read_data_skip(a);

        if !should_continue {
            break;
        }
    }

    Ok(count)
}
