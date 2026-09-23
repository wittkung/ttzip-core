// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Archive non-destructive inspection submodule for Unified Orchestrator.

use std::ffi::{CStr, CString};
use std::path::Path;

use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::types::{TTZipEntryMetadata, TTZipInspectCallback, TTZipStatus};
use libc::{c_void, mode_t};

/// Borrowed, zero-heap archive entry metadata slice.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveEntryRef<'a> {
    pub path: &'a str,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: u16,
    pub detected_encoding: Option<&'a str>,
}

/// Inspects an archive and invokes the callback for every discovered entry metadata item.
///
/// Takes a zero-allocation borrowed `ArchiveEntryRef` slice callback in pure Safe Rust.
pub fn inspect_archive<F>(
    archive_path: &Path,
    password: Option<&str>,
    detect_encoding: bool,
    mut callback: F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
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
        return unsafe { inspect_from_handle(a, detect_encoding, callback) };
    }

    if volume_chain.len() == 1 {
        if let Ok(source) = crate::archive::source::open_archive_source(archive_path) {
            if let Some(mapped) = source.as_slice() {
                if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
                    if let Ok(archive) = crate::sevenz::SevenZArchive::open_slice_with_password(mapped, password) {
                        let mut count = 0;
                        for (idx, file) in archive.files().iter().enumerate() {
                            let mut detected_charset: Option<String> = None;
                            if detect_encoding {
                                let path_bytes = file.rel_path.as_bytes();
                                if !path_bytes.is_ascii() {
                                    detected_charset = crate::codecs::chardet::detect_charset(path_bytes);
                                }
                            }
                            let detected_encoding = detected_charset.as_deref();
                            let loc = archive.seek_index().entries.get(idx);
                            let uncomp_sz = loc.map(|l| l.uncompressed_size).unwrap_or(0);
                            let crc_val = loc.and_then(|l| l.crc).unwrap_or(0);

                            let entry_ref = ArchiveEntryRef {
                                path: file.rel_path.as_str(),
                                uncompressed_size: uncomp_sz,
                                compressed_size: 0,
                                crc32: crc_val,
                                mtime_epoch_secs: file.mtime_epoch_secs.unwrap_or(0),
                                mode: file.mode,
                                is_directory: file.is_directory,
                                is_encrypted: archive.info().is_encrypted,
                                compression_method: archive.info().primary_method_id as u16,
                                detected_encoding,
                            };
                            count += 1;
                            if !callback(&entry_ref) {
                                break;
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
        archive_read_support_format_raw(a);
        archive_read_support_filter_all(a);

        if let Some(ref p) = pwd_c {
            archive_read_add_passphrase(a, p.as_ptr());
        }

        let open_rc = archive_read_open_filename(a, arch_c.as_ptr(), 65536);
        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        inspect_from_handle(a, detect_encoding, callback)
    }
}

unsafe fn inspect_from_handle<F>(
    a: *mut c_void,
    detect_encoding: bool,
    mut cb: F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
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

        let path_c_str = CStr::from_ptr(raw_path);
        let path_bytes = path_c_str.to_bytes();
        if path_bytes.is_empty() {
            archive_read_data_skip(a);
            continue;
        }

        let detected_charset_opt = if detect_encoding && !path_bytes.is_ascii() {
            crate::codecs::chardet::detect_charset(path_bytes)
        } else {
            None
        };
        let detected_encoding = detected_charset_opt.as_deref();

        let uncompressed_size = archive_entry_size(entry).max(0) as u64;
        let mode = archive_entry_mode(entry) as u32;
        let filetype = archive_entry_filetype(entry);
        let is_dir = (filetype & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
            || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
            || path_bytes.ends_with(b"/");
        let mtime = archive_entry_mtime(entry) as i64;
        let is_data_enc = archive_entry_is_data_encrypted(entry) != 0;
        let is_meta_enc = archive_entry_is_metadata_encrypted(entry) != 0;

        let path_cow = path_c_str.to_string_lossy();
        let entry_ref = ArchiveEntryRef {
            path: path_cow.as_ref(),
            uncompressed_size,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: mtime,
            mode,
            is_directory: is_dir,
            is_encrypted: is_data_enc || is_meta_enc,
            compression_method: 0,
            detected_encoding,
        };

        count += 1;
        let should_continue = cb(&entry_ref);
        archive_read_data_skip(a);

        if !should_continue {
            break;
        }
    }

    Ok(count)
}

/// Compatibility wrapper bridging the Safe Rust closure-based `inspect_archive`
/// to the legacy C-ABI function pointer callback and `user_data` pointer.
pub fn inspect_archive_c_compat(
    archive_path: &Path,
    password: Option<&str>,
    detect_encoding: bool,
    callback: TTZipInspectCallback,
    user_data: *mut c_void,
) -> Result<usize, TTZipStatus> {
    let cb = match callback {
        Some(f) => f,
        None => return Err(TTZipStatus::ErrInvalidParam),
    };

    inspect_archive(archive_path, password, detect_encoding, |entry_ref| {
        let path_c = match CString::new(entry_ref.path) {
            Ok(c) => c,
            Err(_) => return true,
        };
        let encoding_c = entry_ref.detected_encoding.and_then(|s| CString::new(s).ok());
        let meta = TTZipEntryMetadata {
            struct_size: std::mem::size_of::<TTZipEntryMetadata>() as u32,
            abi_version: crate::types::TTZIP_ABI_VERSION_2,
            path: path_c.as_ptr(),
            uncompressed_size: entry_ref.uncompressed_size,
            compressed_size: entry_ref.compressed_size,
            crc32: entry_ref.crc32,
            mtime_epoch_secs: entry_ref.mtime_epoch_secs,
            mode: entry_ref.mode,
            is_directory: entry_ref.is_directory,
            is_encrypted: entry_ref.is_encrypted,
            compression_method: entry_ref.compression_method,
            detected_encoding: encoding_c.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
        };
        unsafe { cb(&meta, user_data) }
    })
}
