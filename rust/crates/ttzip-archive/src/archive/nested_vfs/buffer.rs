// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! In-Memory Buffer Inspection and Parsing for Nested Archive Containers.

use std::ffi::{CStr, CString};
use libc::{c_void, mode_t};

use crate::archive::tar::reader::TarArchive;
use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::sevenz::decoder::archive::SevenZArchive;
use crate::types::TTZipStatus;
use super::{TTZipError, UniFFIEntryMetadata};
use crate::zip::reader::ZipArchive;

pub fn matches_entry_path(entry: &str, target: &str) -> bool {
    let clean_e = entry.replace('\\', "/");
    let clean_t = target.replace('\\', "/");
    let e = clean_e.trim_start_matches("./").trim_start_matches('/');
    let t = clean_t.trim_start_matches("./").trim_start_matches('/');
    e == t || clean_e == clean_t
}

pub fn parse_nested_specifier(drill_path: &[String], target_entry: &str) -> (Vec<String>, String) {
    if !drill_path.is_empty() {
        return (drill_path.to_vec(), target_entry.to_string());
    }
    if target_entry.contains('!') {
        let p: Vec<&str> = target_entry.split('!').collect();
        return (p[..p.len() - 1].iter().map(|s| s.to_string()).collect(), p.last().unwrap_or(&"").to_string());
    }
    if target_entry.contains("::") {
        let p: Vec<&str> = target_entry.split("::").collect();
        return (p[..p.len() - 1].iter().map(|s| s.to_string()).collect(), p.last().unwrap_or(&"").to_string());
    }
    (Vec::new(), target_entry.to_string())
}

pub fn inspect_entries_from_buffer(buf: &[u8], password: Option<&str>) -> Result<Vec<UniFFIEntryMetadata>, TTZipError> {
    if let Some(entries) = try_inspect_fast_container(buf, password) {
        return Ok(entries);
    }
    inspect_entries_via_libarchive(buf, password)
}

fn try_inspect_fast_container(buf: &[u8], password: Option<&str>) -> Option<Vec<UniFFIEntryMetadata>> {
    if buf.starts_with(b"PK\x03\x04") || buf.starts_with(b"PK\x05\x06") {
        if let Ok(zip) = ZipArchive::open_slice(buf) {
            return Some(
                zip.entries()
                    .iter()
                    .map(|e| UniFFIEntryMetadata {
                        path: e.rel_path.clone(),
                        uncompressed_size: e.uncompressed_size,
                        compressed_size: e.compressed_size,
                        crc32: e.crc32,
                        mtime_epoch_secs: e.mtime_epoch_secs,
                        mode: e.mode,
                        is_directory: e.is_directory,
                        is_encrypted: e.is_encrypted,
                        compression_method: "deflate".to_string(),
                        detected_encoding: None,
                    })
                    .collect(),
            );
        }
    }
    if buf.starts_with(b"7z\xBC\xAF\x27\x1C") {
        if let Ok(arch) = SevenZArchive::open_slice_with_password(buf, password) {
            return Some(
                arch.files()
                    .iter()
                    .enumerate()
                    .map(|(idx, f)| {
                        let loc = arch.seek_index().entries.get(idx);
                        UniFFIEntryMetadata {
                            path: f.rel_path.clone(),
                            uncompressed_size: loc.map(|l| l.uncompressed_size).unwrap_or(0),
                            compressed_size: 0,
                            crc32: loc.and_then(|l| l.crc).unwrap_or(0),
                            mtime_epoch_secs: f.mtime_epoch_secs.unwrap_or(0),
                            mode: f.mode,
                            is_directory: f.is_directory,
                            is_encrypted: arch.info().is_encrypted,
                            compression_method: "7z".to_string(),
                            detected_encoding: None,
                        }
                    })
                    .collect(),
            );
        }
    }
    if let Ok(tar) = TarArchive::open_slice(buf) {
        if !tar.is_empty() {
            return Some(
                tar.entries()
                    .iter()
                    .map(|e| UniFFIEntryMetadata {
                        path: e.path.to_string(),
                        uncompressed_size: e.size,
                        compressed_size: e.size,
                        crc32: 0,
                        mtime_epoch_secs: e.mtime_epoch_secs,
                        mode: e.mode,
                        is_directory: e.is_directory,
                        is_encrypted: false,
                        compression_method: "store".to_string(),
                        detected_encoding: None,
                    })
                    .collect(),
            );
        }
    }
    None
}

fn inspect_entries_via_libarchive(buf: &[u8], password: Option<&str>) -> Result<Vec<UniFFIEntryMetadata>, TTZipError> {
    unsafe {
        let a = open_libarchive_memory_reader(buf, password)?;
        let _guard = ArchiveReadGuard(a);
        let mut entries = Vec::new();
        let mut entry: *mut c_void = std::ptr::null_mut();
        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }
            if let Some(meta) = read_libarchive_entry_metadata(entry) {
                entries.push(meta);
            }
            archive_read_data_skip(a);
        }
        Ok(entries)
    }
}

unsafe fn read_libarchive_entry_metadata(entry: *mut c_void) -> Option<UniFFIEntryMetadata> {
    unsafe {
        let rp = archive_entry_pathname(entry);
        if rp.is_null() {
            return None;
        }
        let path = CStr::from_ptr(rp).to_string_lossy().into_owned();
        let sz = archive_entry_size(entry).max(0) as u64;
        let mode = archive_entry_mode(entry) as u32;
        let ft = archive_entry_filetype(entry);
        let is_dir = (ft & (libc::S_IFMT as mode_t)) == (libc::S_IFDIR as mode_t)
            || (mode & (libc::S_IFMT as u32)) == (libc::S_IFDIR as u32)
            || path.ends_with('/');
        let is_enc = archive_entry_is_data_encrypted(entry) != 0 || archive_entry_is_metadata_encrypted(entry) != 0;
        Some(UniFFIEntryMetadata {
            path,
            uncompressed_size: sz,
            compressed_size: 0,
            crc32: 0,
            mtime_epoch_secs: archive_entry_mtime(entry) as i64,
            mode,
            is_directory: is_dir,
            is_encrypted: is_enc,
            compression_method: "libarchive".to_string(),
            detected_encoding: None,
        })
    }
}

pub fn extract_entry_from_buffer(buf: &[u8], target: &str, password: Option<&str>) -> Result<Vec<u8>, TTZipError> {
    if let Some(res) = try_extract_fast_container(buf, target, password) {
        return res;
    }
    extract_entry_via_libarchive(buf, target, password)
}

fn try_extract_fast_container(
    buf: &[u8],
    target: &str,
    password: Option<&str>,
) -> Option<Result<Vec<u8>, TTZipError>> {
    if buf.starts_with(b"PK\x03\x04") || buf.starts_with(b"PK\x05\x06") {
        if let Ok(zip) = ZipArchive::open_slice(buf) {
            if let Some(idx) = zip.entries().iter().position(|e| matches_entry_path(&e.rel_path, target)) {
                return Some(zip.extract_entry_bytes(idx, password).map_err(map_ttzip_status));
            }
        }
    }
    if buf.starts_with(b"7z\xBC\xAF\x27\x1C") {
        if let Ok(sz) = SevenZArchive::open_slice_with_password(buf, password) {
            if let Some(idx) = sz.files().iter().position(|f| matches_entry_path(&f.rel_path, target)) {
                return Some(sz.extract_entry_bytes_stream(idx, password).map_err(map_ttzip_status));
            }
        }
    }
    if let Ok(tar) = TarArchive::open_slice(buf) {
        if !tar.is_empty() {
            if let Some(idx) = tar.entries().iter().position(|e| matches_entry_path(e.path.as_ref(), target)) {
                return Some(tar.extract_entry_bytes(idx).map(|s| s.to_vec()).map_err(map_ttzip_status));
            }
        }
    }
    None
}

fn extract_entry_via_libarchive(buf: &[u8], target: &str, password: Option<&str>) -> Result<Vec<u8>, TTZipError> {
    unsafe {
        let a = open_libarchive_memory_reader(buf, password)?;
        let _guard = ArchiveReadGuard(a);
        let mut entry: *mut c_void = std::ptr::null_mut();
        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }
            let rp = archive_entry_pathname(entry);
            if !rp.is_null() && matches_entry_path(&CStr::from_ptr(rp).to_string_lossy(), target) {
                return read_libarchive_entry_payload(a, entry);
            }
            archive_read_data_skip(a);
        }
        Err(TTZipError::FileNotFound { path: target.to_string() })
    }
}

unsafe fn open_libarchive_memory_reader(buf: &[u8], password: Option<&str>) -> Result<*mut c_void, TTZipError> {
    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipError::EngineError { code: -1 });
        }
        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);
        if let Some(pwd) = password {
            if let Ok(cp) = CString::new(pwd) {
                archive_read_add_passphrase(a, cp.as_ptr());
            }
        }
        if archive_read_open_memory(a, buf.as_ptr() as *const c_void, buf.len()) != 0 {
            archive_read_free(a);
            return Err(TTZipError::CorruptHeader {
                details: "Invalid in-memory archive".to_string(),
                offset: 0,
            });
        }
        Ok(a)
    }
}

unsafe fn read_libarchive_entry_payload(a: *mut c_void, entry: *mut c_void) -> Result<Vec<u8>, TTZipError> {
    unsafe {
        let sz = archive_entry_size(entry).max(0) as usize;
        let mut payload = Vec::with_capacity(sz.min(16 * 1024 * 1024));
        let mut chunk = [0u8; 65536];
        loop {
            let r = archive_read_data(a, chunk.as_mut_ptr() as *mut c_void, chunk.len());
            if r < 0 {
                return Err(TTZipError::EngineError { code: -1 });
            }
            if r == 0 {
                break;
            }
            payload.extend_from_slice(&chunk[..r as usize]);
        }
        Ok(payload)
    }
}

pub fn map_ttzip_status(status: TTZipStatus) -> TTZipError {
    match status {
        TTZipStatus::ErrFileNotFound => TTZipError::FileNotFound { path: "Entry not found in archive".to_string() },
        TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader { details: "Corrupted archive header or CRC".to_string(), offset: 0 },
        TTZipStatus::ErrSecurityViolation => TTZipError::SecurityViolation { reason: "Security check violation".to_string() },
        TTZipStatus::Cancelled => TTZipError::Cancelled,
        _ => TTZipError::EngineError { code: status as i32 },
    }
}
