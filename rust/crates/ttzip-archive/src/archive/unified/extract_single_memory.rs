// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::ffi::CString;
use std::path::Path;
use libc::c_void;

use super::open_libarchive_filename_guard;
use super::super::fallback_policy::{
    has_7z_signature, has_zip_signature, should_abort_without_fallback,
};
use crate::archive::tar::reader::TarArchive;
use crate::ffi::archive_ffi::guards::archive_entry_pathname_safe;
use crate::ffi::archive_ffi::sys::*;
use crate::sevenz::decoder::archive::SevenZArchive;
use crate::types::TTZipStatus;
use crate::zip::reader::ZipArchive;

fn extract_memory_special_or_compressed_stream(
    archive_path: &Path,
    volume_chain: &[std::path::PathBuf],
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Option<Vec<u8>>, TTZipStatus> {
    if let Some(transcoded) =
        crate::archive::stream_adapter::try_transcode_special_container(archive_path, volume_chain)?
    {
        if transcoded.entry_count == 0 {
            return Err(TTZipStatus::ErrFileNotFound);
        }
        let stream_reader =
            crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                transcoded.cursor,
                65536,
            )?;
        let bytes = unsafe {
            extract_single_entry_from_libarchive_handle(
                stream_reader.as_raw_archive(),
                entry_path,
                entry_index,
            )
        }?;
        return Ok(Some(bytes));
    }

    match crate::archive::stream_adapter::try_open_resolved_stream(archive_path, volume_chain)? {
        Some(crate::archive::stream_adapter::ResolvedStreamKind::TarContainer(pre)) => {
            let pwd_c = password.and_then(|p| CString::new(p).ok());
            let stream_reader =
                crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
                    pre, 65536,
                )?;
            let a = stream_reader.as_raw_archive();
            if let Some(ref p) = pwd_c {
                unsafe {
                    archive_read_add_passphrase(a, p.as_ptr());
                }
            }
            let bytes =
                unsafe { extract_single_entry_from_libarchive_handle(a, entry_path, entry_index) }?;
            Ok(Some(bytes))
        }
        Some(crate::archive::stream_adapter::ResolvedStreamKind::SingleFileStream {
            stream,
            entry_name,
            compressed_size,
            ..
        }) => {
            let bytes = crate::archive::stream_adapter::extract_memory_single_file_stream(
                stream,
                &entry_name,
                compressed_size,
                entry_path,
                entry_index,
            )?;
            Ok(Some(bytes))
        }
        None => Ok(None),
    }
}

fn extract_multivolume_single_entry_memory(
    archive_path: &Path,
    volume_chain: Vec<std::path::PathBuf>,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    use std::io::{Read, Seek, SeekFrom};
    let mut virtual_reader =
        crate::archive::split::VirtualMultiVolumeReader::from_volumes(volume_chain.clone())
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut magic = [0u8; 6];
    let has_magic = virtual_reader.read_exact(&mut magic).is_ok();
    let _ = virtual_reader.seek(SeekFrom::Start(0));
    if has_magic && has_7z_signature(&magic) {
        return crate::archive::stream_adapter::extract_7z_single_entry_from_seekable(
            &mut virtual_reader,
            entry_path,
            entry_index,
            password,
        );
    }

    if has_magic && has_zip_signature(&magic) {
        let total_len = virtual_reader.total_size();
        match crate::zip::reader_stream::extract_zip_single_entry_from_reader(
            &mut virtual_reader,
            total_len,
            entry_path,
            entry_index,
            password,
        ) {
            Ok(bytes) => return Ok(bytes),
            Err(err)
                if should_abort_without_fallback(err) || err == TTZipStatus::ErrFileNotFound =>
            {
                return Err(err);
            }
            Err(_) => {
                let _ = virtual_reader.seek(SeekFrom::Start(0));
            }
        }
    }

    if let Some(bytes) = extract_memory_special_or_compressed_stream(
        archive_path,
        &volume_chain,
        entry_path,
        entry_index,
        password,
    )? {
        return Ok(bytes);
    }

    let pwd_c = password.and_then(|p| CString::new(p).ok());
    let stream_reader =
        crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(
            virtual_reader,
            65536,
        )?;
    let a = stream_reader.as_raw_archive();
    if let Some(ref p) = pwd_c {
        unsafe {
            archive_read_add_passphrase(a, p.as_ptr());
        }
    }
    unsafe { extract_single_entry_from_libarchive_handle(a, entry_path, entry_index) }
}

unsafe fn read_matched_libarchive_entry_bytes(
    a: *mut c_void,
    entry: *mut c_void,
) -> Result<Vec<u8>, TTZipStatus> {
    unsafe {
        const MAX_IN_MEMORY_DECOMPRESS_BYTES: usize = 64 * 1024 * 1024;
        let size = archive_entry_size(entry).max(0) as usize;
        if size > MAX_IN_MEMORY_DECOMPRESS_BYTES {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let mut payload = Vec::with_capacity(size.min(10 * 1024 * 1024));
        let mut chunk = [0u8; 65536];
        loop {
            let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
            if r < 0 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            if r == 0 {
                break;
            }
            if payload.len() + (r as usize) > MAX_IN_MEMORY_DECOMPRESS_BYTES {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            payload.extend_from_slice(&chunk[..r as usize]);
        }
        Ok(payload)
    }
}

unsafe fn extract_single_entry_from_libarchive_handle(
    a: *mut c_void,
    entry_path: Option<&str>,
    entry_index: i64,
) -> Result<Vec<u8>, TTZipStatus> {
    unsafe {
        let mut entry: *mut c_void = std::ptr::null_mut();
        let mut curr_idx: i64 = 0;

        loop {
            let hdr_rc = archive_read_next_header(a, &mut entry);
            if hdr_rc == 1 {
                break;
            }
            if hdr_rc < -20 || (hdr_rc < 0 && entry.is_null()) {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            if entry.is_null() {
                break;
            }
            let Some(path_c_str) = archive_entry_pathname_safe(entry) else {
                archive_read_data_skip(a);
                curr_idx += 1;
                continue;
            };
            let Ok(entry_rel_str) = path_c_str.to_str() else {
                return Err(TTZipStatus::ErrSecurityViolation);
            };
            let is_match = if entry_index >= 0 {
                curr_idx == entry_index
            } else if let Some(target_p) = entry_path {
                entry_rel_str == target_p || entry_rel_str.trim_start_matches("./") == target_p
            } else {
                false
            };
            if is_match {
                return read_matched_libarchive_entry_bytes(a, entry);
            }
            archive_read_data_skip(a);
            curr_idx += 1;
        }
        Err(TTZipStatus::ErrFileNotFound)
    }
}

fn extract_mapped_zip_single_entry(
    mapped: &[u8],
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let zip = ZipArchive::open_slice(mapped)?;
    let target_idx = if entry_index >= 0 {
        entry_index as usize
    } else if let Some(target_p) = entry_path {
        zip.entries()
            .iter()
            .position(|e| e.rel_path == target_p || e.rel_path.trim_start_matches("./") == target_p)
            .ok_or(TTZipStatus::ErrFileNotFound)?
    } else {
        return Err(TTZipStatus::ErrInvalidParam);
    };
    zip.extract_entry_bytes(target_idx, password)
}

fn extract_mapped_7z_single_entry(
    mapped: &[u8],
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let sevenz = SevenZArchive::open_slice_with_password(mapped, password)?;
    let target_idx = if entry_index >= 0 {
        entry_index as usize
    } else if let Some(target_p) = entry_path {
        sevenz
            .seek_index()
            .get_by_path(target_p)
            .map(|loc| loc.file_index)
            .ok_or(TTZipStatus::ErrFileNotFound)?
    } else {
        return Err(TTZipStatus::ErrInvalidParam);
    };
    sevenz.extract_entry_bytes_stream(target_idx, password)
}

fn extract_mapped_tar_single_entry(
    mapped: &[u8],
    entry_path: Option<&str>,
    entry_index: i64,
) -> Result<Vec<u8>, TTZipStatus> {
    let tar = TarArchive::open_slice(mapped)?;
    let target_idx = if entry_index >= 0 {
        entry_index as usize
    } else if let Some(target_p) = entry_path {
        tar.entries()
            .iter()
            .position(|e| {
                e.path.as_ref() == target_p || e.path.as_ref().trim_start_matches("./") == target_p
            })
            .ok_or(TTZipStatus::ErrFileNotFound)?
    } else {
        return Err(TTZipStatus::ErrInvalidParam);
    };
    tar.extract_entry_bytes(target_idx).map(|s| s.to_vec())
}

fn extract_mapped_single_entry_memory(
    mapped: &[u8],
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Option<Vec<u8>>, TTZipStatus> {
    if is_zip_magic(mapped) {
        return extract_mapped_zip_single_entry(mapped, entry_path, entry_index, password)
            .map(Some);
    }
    if is_sevenz_magic(mapped) {
        return extract_mapped_7z_single_entry(mapped, entry_path, entry_index, password).map(Some);
    }
    if is_tar_magic(mapped) {
        return extract_mapped_tar_single_entry(mapped, entry_path, entry_index).map(Some);
    }
    Ok(None)
}

/// Extracts a single entry directly into an in-memory buffer without full archive extraction.
pub fn extract_single_entry_memory(
    archive_path: &Path,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    let volume_chain = crate::archive::split::detect_volume_chain(archive_path)
        .map_err(|_| TTZipStatus::ErrFileNotFound)?;
    if volume_chain.len() > 1 {
        return extract_multivolume_single_entry_memory(
            archive_path,
            volume_chain,
            entry_path,
            entry_index,
            password,
        );
    }

    let single_path = volume_chain.first().map(|p| p.as_path()).unwrap_or(archive_path);
    if !single_path.exists() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let source = crate::archive::source::open_archive_source(single_path)?;
    if let Some(mapped) = source.as_slice() {
        if let Some(bytes) =
            extract_mapped_single_entry_memory(mapped, entry_path, entry_index, password)?
        {
            return Ok(bytes);
        }
    }

    if let Some(bytes) = extract_memory_special_or_compressed_stream(
        single_path,
        &volume_chain,
        entry_path,
        entry_index,
        password,
    )? {
        return Ok(bytes);
    }

    let guard = open_libarchive_filename_guard(single_path, password)?;
    unsafe { extract_single_entry_from_libarchive_handle(guard.0, entry_path, entry_index) }
}

#[inline]
fn is_zip_magic(mapped: &[u8]) -> bool {
    if mapped.len() < 22 {
        return false;
    }
    if mapped.starts_with(b"PK\x03\x04") || mapped.starts_with(b"PK\x05\x06") {
        return true;
    }
    let search_window = mapped.len().min(65535 + 22);
    let tail = &mapped[mapped.len() - search_window..];
    tail.windows(4).rev().any(|w| w == b"PK\x05\x06")
}

#[inline]
fn is_sevenz_magic(mapped: &[u8]) -> bool {
    mapped.len() >= 6 && &mapped[0..6] == b"7z\xBC\xAF\x27\x1C"
}

#[inline]
fn is_tar_magic(mapped: &[u8]) -> bool {
    mapped.len() >= 262 && &mapped[257..262] == b"ustar"
}
