// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Shared test harness infrastructure for 20+ container format matrix verification.

pub mod apple_bsd_matrix;
pub mod cpio_matrix;
pub mod iso_matrix;
pub mod special_matrix;
pub mod tar_matrix;

use std::ffi::{CStr, CString};
use std::fs;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

use ttzip_engine::ffi::archive_ffi::guards::{ArchiveEntryGuard, ArchiveReadGuard, ArchiveWriteGuard};
use ttzip_engine::ffi::archive_ffi::sys::*;
use libc::{c_char, c_void, mode_t, size_t, time_t};

/// Synthetic file/directory/link entry definition for matrix generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticEntry {
    pub path: String,
    pub data: Vec<u8>,
    pub filetype: mode_t,
    pub perm: mode_t,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub symlink: Option<String>,
    pub hardlink: Option<String>,
    pub xattrs: Vec<(String, Vec<u8>)>,
}

impl SyntheticEntry {
    /// Creates a regular file entry with given path and payload bytes.
    #[must_use]
    pub fn file(path: &str, data: Vec<u8>) -> Self {
        Self {
            path: path.to_string(),
            data,
            filetype: libc::S_IFREG as mode_t,
            perm: 0o644,
            mtime: 1_700_000_000,
            mtime_nsec: 0,
            symlink: None,
            hardlink: None,
            xattrs: Vec::new(),
        }
    }

    /// Creates a directory entry.
    #[must_use]
    pub fn dir(path: &str) -> Self {
        Self {
            path: path.to_string(),
            data: Vec::new(),
            filetype: libc::S_IFDIR as mode_t,
            perm: 0o755,
            mtime: 1_700_000_000,
            mtime_nsec: 0,
            symlink: None,
            hardlink: None,
            xattrs: Vec::new(),
        }
    }

    /// Creates a symbolic link entry.
    #[must_use]
    pub fn symlink(path: &str, target: &str) -> Self {
        Self {
            path: path.to_string(),
            data: Vec::new(),
            filetype: libc::S_IFLNK as mode_t,
            perm: 0o777,
            mtime: 1_700_000_000,
            mtime_nsec: 0,
            symlink: Some(target.to_string()),
            hardlink: None,
            xattrs: Vec::new(),
        }
    }

    /// Creates a hard link entry.
    #[must_use]
    pub fn hardlink(path: &str, target: &str) -> Self {
        Self {
            path: path.to_string(),
            data: Vec::new(),
            filetype: libc::S_IFREG as mode_t,
            perm: 0o644,
            mtime: 1_700_000_000,
            mtime_nsec: 0,
            symlink: None,
            hardlink: Some(target.to_string()),
            xattrs: Vec::new(),
        }
    }

    /// Sets the modification timestamp with second and nanosecond precision.
    #[must_use]
    pub fn with_mtime(mut self, sec: i64, nsec: i64) -> Self {
        self.mtime = sec;
        self.mtime_nsec = nsec;
        self
    }

    /// Sets the POSIX permission bitmask.
    #[must_use]
    pub fn with_perm(mut self, perm: mode_t) -> Self {
        self.perm = perm;
        self
    }

    /// Attaches an extended attribute key-value pair.
    #[must_use]
    pub fn with_xattr(mut self, name: &str, value: &[u8]) -> Self {
        self.xattrs.push((name.to_string(), value.to_vec()));
        self
    }

    /// Computes the expected SHA-256 digest of payload data.
    #[must_use]
    pub fn compute_sha256(&self) -> [u8; 32] {
        compute_sha256(&self.data)
    }
}

/// Extracted metadata and payload representation from an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedEntry {
    pub path: String,
    pub data: Vec<u8>,
    pub sha256: [u8; 32],
    pub filetype: mode_t,
    pub perm: mode_t,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub symlink: Option<String>,
    pub hardlink: Option<String>,
    pub xattrs: Vec<(String, Vec<u8>)>,
}

/// Verification policy options for matrix assertions.
#[derive(Debug, Clone, Copy)]
pub struct VerifyPolicy {
    pub check_data_sha256: bool,
    pub check_permissions: bool,
    pub check_mtime_secs: bool,
    pub check_mtime_nanos: bool,
    pub check_symlinks: bool,
    pub check_hardlinks: bool,
    pub check_xattrs: bool,
}

impl Default for VerifyPolicy {
    fn default() -> Self {
        Self {
            check_data_sha256: true,
            check_permissions: true,
            check_mtime_secs: true,
            check_mtime_nanos: false,
            check_symlinks: true,
            check_hardlinks: true,
            check_xattrs: false,
        }
    }
}

impl VerifyPolicy {
    /// Strict policy checking every attribute including nanoseconds and xattrs.
    #[must_use]
    pub const fn strict_all() -> Self {
        Self {
            check_data_sha256: true,
            check_permissions: true,
            check_mtime_secs: true,
            check_mtime_nanos: true,
            check_symlinks: true,
            check_hardlinks: true,
            check_xattrs: true,
        }
    }
}

/// Calculates SHA-256 digest of arbitrary byte slices.
#[must_use]
pub fn compute_sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

/// Formats 32-byte SHA-256 digest into hexadecimal string.
#[must_use]
pub fn format_sha256(hash: &[u8; 32]) -> String {
    hex::encode(hash)
}

/// Writes synthetic entries into an archive byte buffer using a custom libarchive format configurator.
pub fn write_archive_buffer<F>(
    entries: &[SyntheticEntry],
    configure_writer: F,
) -> Result<Vec<u8>, String>
where
    F: FnOnce(*mut c_void) -> Result<(), String>,
{
    let dir = tempdir().map_err(|e| format!("Tempdir creation failed: {}", e))?;
    let temp_archive_path = dir.path().join("archive.bin");
    let path_c = CString::new(
        temp_archive_path
            .to_str()
            .ok_or_else(|| "Invalid UTF-8 path".to_string())?,
    )
    .map_err(|e| format!("CString conversion error: {}", e))?;

    unsafe {
        let a = archive_write_new();
        if a.is_null() {
            return Err("archive_write_new returned null".to_string());
        }
        let _guard = ArchiveWriteGuard(a);

        configure_writer(a)?;

        let open_rc = archive_write_open_filename(a, path_c.as_ptr());
        if open_rc != 0 {
            return Err(format_archive_error(a));
        }

        for item in entries {
            let entry = archive_entry_new();
            if entry.is_null() {
                return Err("archive_entry_new returned null".to_string());
            }
            let _entry_guard = ArchiveEntryGuard(entry);

            let pathname_c = CString::new(item.path.as_str())
                .map_err(|e| format!("Invalid pathname {}: {}", item.path, e))?;
            archive_entry_set_pathname(entry, pathname_c.as_ptr());
            archive_entry_set_filetype(entry, (item.filetype & (libc::S_IFMT as mode_t)) as libc::c_uint);
            archive_entry_set_perm(entry, item.perm & 0o7777);
            archive_entry_set_mtime(
                entry,
                item.mtime as time_t,
                item.mtime_nsec as libc::c_long,
            );

            if let Some(ref sym) = item.symlink {
                let sym_c = CString::new(sym.as_str())
                    .map_err(|e| format!("Invalid symlink target {}: {}", sym, e))?;
                archive_entry_set_symlink(entry, sym_c.as_ptr());
            }

            if let Some(ref hard) = item.hardlink {
                let hard_c = CString::new(hard.as_str())
                    .map_err(|e| format!("Invalid hardlink target {}: {}", hard, e))?;
                archive_entry_set_hardlink(entry, hard_c.as_ptr());
            }

            for (x_name, x_val) in &item.xattrs {
                let x_name_c = CString::new(x_name.as_str())
                    .map_err(|e| format!("Invalid xattr name {}: {}", x_name, e))?;
                archive_entry_xattr_add_entry(
                    entry,
                    x_name_c.as_ptr(),
                    x_val.as_ptr() as *const c_void,
                    x_val.len() as size_t,
                );
            }

            if item.filetype == (libc::S_IFREG as mode_t) && item.hardlink.is_none() {
                archive_entry_set_size(entry, item.data.len() as i64);
            } else {
                archive_entry_set_size(entry, 0);
            }

            let write_hdr_rc = archive_write_header(a, entry);
            if write_hdr_rc != 0 {
                return Err(format!("archive_write_header failed for {}: {}", item.path, format_archive_error(a)));
            }

            if item.filetype == (libc::S_IFREG as mode_t) && item.hardlink.is_none() && !item.data.is_empty() {
                let written = archive_write_data(
                    a,
                    item.data.as_ptr() as *const c_void,
                    item.data.len() as size_t,
                );
                if written < 0 || written as usize != item.data.len() {
                    return Err(format!("archive_write_data short write for {}: {}", item.path, format_archive_error(a)));
                }
            }

            archive_write_finish_entry(a);
        }

        archive_write_close(a);
    }

    fs::read(&temp_archive_path).map_err(|e| format!("Failed to read generated archive: {}", e))
}

/// Reads and extracts all entries and metadata from raw archive bytes using libarchive.
pub fn read_archive_buffer(archive_bytes: &[u8]) -> Result<Vec<ExtractedEntry>, String> {
    if archive_bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut extracted = Vec::new();

    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err("archive_read_new returned null".to_string());
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);

        let open_rc = archive_read_open_memory(
            a,
            archive_bytes.as_ptr() as *const c_void,
            archive_bytes.len() as size_t,
        );
        if open_rc != 0 {
            return Err(format!("archive_read_open_memory failed: {}", format_archive_error(a)));
        }

        let mut entry: *mut c_void = std::ptr::null_mut();
        while archive_read_next_header(a, &mut entry) == 0 {
            if entry.is_null() {
                break;
            }

            let path_ptr = archive_entry_pathname(entry);
            let path_str = if path_ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(path_ptr).to_string_lossy().to_string()
            };

            let filetype = archive_entry_filetype(entry);
            let perm = archive_entry_perm(entry);
            let mtime = archive_entry_mtime(entry) as i64;
            let mtime_nsec = archive_entry_mtime_nsec(entry) as i64;

            let symlink = {
                let sym_ptr = archive_entry_symlink(entry);
                if sym_ptr.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(sym_ptr).to_string_lossy().to_string())
                }
            };

            let hardlink = {
                let hard_ptr = archive_entry_hardlink(entry);
                if hard_ptr.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(hard_ptr).to_string_lossy().to_string())
                }
            };

            let mut xattrs = Vec::new();
            let count = archive_entry_xattr_reset(entry);
            if count > 0 {
                let mut xname_ptr: *const c_char = std::ptr::null();
                let mut xval_ptr: *const c_void = std::ptr::null();
                let mut xval_sz: size_t = 0;

                while archive_entry_xattr_next(entry, &mut xname_ptr, &mut xval_ptr, &mut xval_sz) == 0 {
                    if !xname_ptr.is_null() && !xval_ptr.is_null() {
                        let name_str = CStr::from_ptr(xname_ptr).to_string_lossy().to_string();
                        let val_slice = std::slice::from_raw_parts(xval_ptr as *const u8, xval_sz);
                        xattrs.push((name_str, val_slice.to_vec()));
                    }
                }
            }

            let entry_size = archive_entry_size(entry);
            let mut data = Vec::new();
            let mut buf = [0u8; 65536];

            loop {
                let bytes_read = archive_read_data(a, buf.as_mut_ptr() as *mut c_void, buf.len() as size_t);
                if bytes_read < 0 {
                    return Err(format!("archive_read_data failed on {}: {}", path_str, format_archive_error(a)));
                }
                if bytes_read == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..bytes_read as usize]);
            }

            if entry_size > 0 && data.len() > entry_size as usize {
                data.truncate(entry_size as usize);
            }

            let sha256 = compute_sha256(&data);

            extracted.push(ExtractedEntry {
                path: path_str,
                data,
                sha256,
                filetype,
                perm,
                mtime,
                mtime_nsec,
                symlink,
                hardlink,
                xattrs,
            });
        }

        archive_read_close(a);
    }

    Ok(extracted)
}

/// Asserts exact match between expected synthetic entries and extracted archive entries according to policy.
pub fn assert_roundtrip_match(
    original: &[SyntheticEntry],
    extracted: &[ExtractedEntry],
    policy: &VerifyPolicy,
) {
    for (idx, orig) in original.iter().enumerate() {
        let orig_clean = orig.path.trim_start_matches("./").trim_end_matches('/');
        let found = extracted.iter().find(|e| {
            let ext_clean = e.path.trim_start_matches("./").trim_end_matches('/');
            ext_clean == orig_clean
        });

        assert!(
            found.is_some(),
            "Missing entry [{}] in extracted archive: '{}' (available: {:?})",
            idx,
            orig.path,
            extracted.iter().map(|e| &e.path).collect::<Vec<_>>()
        );
        let ext = found.unwrap();

        if policy.check_data_sha256 && orig.filetype == (libc::S_IFREG as mode_t) && orig.hardlink.is_none() {
            assert_eq!(
                orig.data.len(),
                ext.data.len(),
                "Entry [{}] payload size mismatch for '{}'",
                idx,
                orig.path
            );
            assert_eq!(
                orig.compute_sha256(),
                ext.sha256,
                "Entry [{}] SHA-256 mismatch for '{}': expected {}, got {}",
                idx,
                orig.path,
                format_sha256(&orig.compute_sha256()),
                format_sha256(&ext.sha256)
            );
        }

        if policy.check_permissions {
            let orig_perm = orig.perm & 0o777;
            let ext_perm = ext.perm & 0o777;
            assert_eq!(
                orig_perm, ext_perm,
                "Entry [{}] permissions mismatch for '{}': expected {:o}, got {:o}",
                idx, orig.path, orig_perm, ext_perm
            );
        }

        if policy.check_mtime_secs {
            assert_eq!(
                orig.mtime, ext.mtime,
                "Entry [{}] mtime epoch seconds mismatch for '{}': expected {}, got {}",
                idx, orig.path, orig.mtime, ext.mtime
            );
        }

        if policy.check_mtime_nanos {
            assert_eq!(
                orig.mtime_nsec, ext.mtime_nsec,
                "Entry [{}] mtime nanoseconds mismatch for '{}': expected {}, got {}",
                idx, orig.path, orig.mtime_nsec, ext.mtime_nsec
            );
        }

        if policy.check_symlinks && orig.symlink.is_some() {
            assert_eq!(
                orig.symlink.as_deref(),
                ext.symlink.as_deref(),
                "Entry [{}] symlink target mismatch for '{}'",
                idx,
                orig.path
            );
        }

        if policy.check_hardlinks && orig.hardlink.is_some() {
            let orig_hard_clean = orig.hardlink.as_deref().map(|s| s.trim_start_matches("./"));
            let ext_hard_clean = ext.hardlink.as_deref().map(|s| s.trim_start_matches("./"));
            assert_eq!(
                orig_hard_clean, ext_hard_clean,
                "Entry [{}] hardlink target mismatch for '{}'",
                idx, orig.path
            );
        }

        if policy.check_xattrs && !orig.xattrs.is_empty() {
            for (key, val) in &orig.xattrs {
                let found_attr = ext.xattrs.iter().find(|(k, _)| k == key);
                assert!(
                    found_attr.is_some(),
                    "Entry [{}] missing xattr '{}' on '{}'",
                    idx, key, orig.path
                );
                assert_eq!(
                    &found_attr.unwrap().1,
                    val,
                    "Entry [{}] xattr '{}' value mismatch on '{}'",
                    idx, key, orig.path
                );
            }
        }
    }
}
