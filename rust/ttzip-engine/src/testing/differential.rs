// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Rayon-accelerated multi-core concurrent directory tree scanner, 5-dimension manifest
//! comparison engine, and libarchive/system-tool Golden Oracle differential verifier.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString};
use std::fs;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};
use std::slice;

use libc::{c_char, mode_t, readlink, stat, PATH_MAX, S_IFDIR, S_IFLNK, S_IFMT};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::crypto::sha256::FastSha256;
use crate::testing::hex_diff::generate_hex_diff;
use crate::types::TTZipStatus;

// MARK: - Entry Types and Manifest Models

/// File system entry type for manifest modeling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryType {
    #[serde(rename = "regular", alias = "regularFile")]
    RegularFile,
    #[serde(rename = "directory")]
    Directory,
    #[serde(rename = "symlink", alias = "symbolicLink")]
    SymbolicLink,
    #[serde(rename = "hardlink", alias = "hardLink")]
    HardLink,
}

impl EntryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryType::RegularFile => "regular",
            EntryType::Directory => "directory",
            EntryType::SymbolicLink => "symlink",
            EntryType::HardLink => "hardlink",
        }
    }
}

impl std::str::FromStr for EntryType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "directory" => EntryType::Directory,
            "symlink" | "symbolicLink" => EntryType::SymbolicLink,
            "hardlink" | "hardLink" => EntryType::HardLink,
            _ => EntryType::RegularFile,
        })
    }
}

/// Single record in a file tree manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub relative_path: String,
    pub entry_type: EntryType,
    pub byte_size: i64,
    pub sha256_checksum: String,
    pub posix_mode: u16,
    pub symlink_target: Option<String>,
}

impl ManifestEntry {
    pub fn new(
        relative_path: String,
        entry_type: EntryType,
        byte_size: i64,
        sha256_checksum: String,
        posix_mode: u16,
        symlink_target: Option<String>,
    ) -> Self {
        Self { relative_path, entry_type, byte_size, sha256_checksum, posix_mode, symlink_target }
    }
}

/// Complete manifest snapshot of an extracted directory tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTreeManifest {
    pub root_directory: String,
    pub entries: BTreeMap<String, ManifestEntry>,
    pub total_byte_size: i64,
    pub total_file_count: usize,
    pub total_directory_count: usize,
    pub total_symlink_count: usize,
}

impl FileTreeManifest {
    pub fn empty(root_directory: String) -> Self {
        Self {
            root_directory,
            entries: BTreeMap::new(),
            total_byte_size: 0,
            total_file_count: 0,
            total_directory_count: 0,
            total_symlink_count: 0,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn from_json(json_str: &str) -> Option<Self> {
        serde_json::from_str(json_str).ok()
    }
}

/// Bidirectional differential test report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferentialReport {
    #[serde(rename = "format")]
    pub format_name: String,
    pub target_oracle: String,
    pub is_passed: bool,
    pub ttzip_manifest: FileTreeManifest,
    pub oracle_manifest: FileTreeManifest,
    pub divergence_errors: Vec<String>,
    pub hex_diff_output: Option<String>,
}

impl DifferentialReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// MARK: - SHA-256 Hashing Utilities

/// Computes SHA-256 hex string of a file at the given path.
pub fn compute_file_sha256(path: &Path, file_size: u64) -> String {
    if file_size == 0 {
        return "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string();
    }
    if file_size <= 64 * 1024 * 1024 {
        if let Ok(bytes) = fs::read(path) {
            return hex_string(&FastSha256::digest(&bytes));
        }
    }
    use std::io::Read;
    if let Ok(mut file) = fs::File::open(path) {
        let mut hasher = FastSha256::new();
        let mut buffer = [0u8; 65536];
        while let Ok(n) = file.read(&mut buffer) {
            if n == 0 { break; }
            hasher.update(&buffer[..n]);
        }
        return hex_string(&hasher.finalize());
    }
    String::new()
}

#[inline]
fn hex_string(bytes: &[u8; 32]) -> String {
    let hex_chars = b"0123456789abcdef";
    let mut out = Vec::with_capacity(64);
    for &b in bytes {
        out.push(hex_chars[(b >> 4) as usize]);
        out.push(hex_chars[(b & 0x0F) as usize]);
    }
    unsafe { String::from_utf8_unchecked(out) }
}

// MARK: - Rayon-Accelerated Directory Scanner

struct RawFsItem {
    full_path: PathBuf,
    normalized_rel_path: String,
    entry_type: EntryType,
    byte_size: i64,
    posix_mode: u16,
    symlink_target: Option<String>,
}

/// Recursively traverses a directory and returns a normalized `FileTreeManifest`.
pub fn scan_directory_tree(root_path: &Path) -> Result<FileTreeManifest, TTZipStatus> {
    if !root_path.exists() || !root_path.is_dir() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let root_str = root_path.to_string_lossy().to_string();
    let mut raw_items: Vec<RawFsItem> = Vec::new();
    let mut dirs_to_visit: Vec<(PathBuf, PathBuf)> = vec![(root_path.to_path_buf(), PathBuf::new())];

    while let Some((current_dir, rel_prefix)) = dirs_to_visit.pop() {
        let read_dir = match fs::read_dir(&current_dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        let mut dir_entries: Vec<fs::DirEntry> = read_dir.filter_map(|e| e.ok()).collect();
        dir_entries.sort_by_key(|e| e.file_name());

        for entry in dir_entries {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.starts_with("._") || name_str == ".noindex" || name_str == ".DS_Store" || name_str == "__MACOSX" {
                continue;
            }

            let full_path = entry.path();
            let entry_rel = if rel_prefix.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                rel_prefix.join(&file_name)
            };
            let rel_str = entry_rel.to_string_lossy().replace('\\', "/");
            let normalized_rel: String = rel_str.nfc().collect();
            if normalized_rel.is_empty() {
                continue;
            }

            let c_path = match CString::new(full_path.to_string_lossy().as_bytes()) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut st: stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::lstat(c_path.as_ptr(), &mut st) } != 0 {
                continue;
            }

            let mode = (st.st_mode & 0o777) as u16;
            let s_mode = st.st_mode as mode_t;

            if (s_mode & S_IFMT) == S_IFLNK {
                let mut link_buf = [0 as c_char; PATH_MAX as usize];
                let len = unsafe { readlink(c_path.as_ptr(), link_buf.as_mut_ptr(), link_buf.len() - 1) };
                let target = if len > 0 {
                    let u8_slice = unsafe { slice::from_raw_parts(link_buf.as_ptr() as *const u8, len as usize) };
                    Some(String::from_utf8_lossy(u8_slice).to_string())
                } else {
                    None
                };
                raw_items.push(RawFsItem {
                    full_path,
                    normalized_rel_path: normalized_rel,
                    entry_type: EntryType::SymbolicLink,
                    byte_size: st.st_size as i64,
                    posix_mode: mode,
                    symlink_target: target,
                });
            } else if (s_mode & S_IFMT) == S_IFDIR {
                raw_items.push(RawFsItem {
                    full_path: full_path.clone(),
                    normalized_rel_path: normalized_rel.clone(),
                    entry_type: EntryType::Directory,
                    byte_size: 0,
                    posix_mode: mode,
                    symlink_target: None,
                });
                dirs_to_visit.push((full_path, entry_rel));
            } else {
                raw_items.push(RawFsItem {
                    full_path,
                    normalized_rel_path: normalized_rel,
                    entry_type: EntryType::RegularFile,
                    byte_size: st.st_size as i64,
                    posix_mode: mode,
                    symlink_target: None,
                });
            }
        }
    }

    let manifest_entries: Vec<ManifestEntry> = raw_items
        .into_par_iter()
        .map(|item| {
            let sha256_checksum = if item.entry_type == EntryType::RegularFile {
                compute_file_sha256(&item.full_path, item.byte_size.max(0) as u64)
            } else {
                String::new()
            };
            ManifestEntry::new(
                item.normalized_rel_path,
                item.entry_type,
                item.byte_size,
                sha256_checksum,
                item.posix_mode,
                item.symlink_target,
            )
        })
        .collect();

    let mut entries = BTreeMap::new();
    let mut total_byte_size: i64 = 0;
    let mut total_file_count: usize = 0;
    let mut total_directory_count: usize = 0;
    let mut total_symlink_count: usize = 0;

    for entry in manifest_entries {
        match entry.entry_type {
            EntryType::RegularFile | EntryType::HardLink => {
                total_byte_size += entry.byte_size;
                total_file_count += 1;
            }
            EntryType::Directory => total_directory_count += 1,
            EntryType::SymbolicLink => total_symlink_count += 1,
        }
        entries.insert(entry.relative_path.clone(), entry);
    }

    Ok(FileTreeManifest {
        root_directory: root_str,
        entries,
        total_byte_size,
        total_file_count,
        total_directory_count,
        total_symlink_count,
    })
}

// MARK: - 5-Dimension Manifest Verifier

/// Compares two `FileTreeManifest` instances across 5 dimensions.
pub fn compare_manifests(
    ttzip: &FileTreeManifest,
    oracle: &FileTreeManifest,
    is_tar_format: bool,
    oracle_name: &str,
    format_name: &str,
) -> DifferentialReport {
    let mut divergence_errors: Vec<String> = Vec::new();
    let mut hex_diff_output: Option<String> = None;

    let ttzip_keys: BTreeSet<&String> = ttzip.entries.keys().collect();
    let oracle_keys: BTreeSet<&String> = oracle.entries.keys().collect();

    for missing_key in oracle_keys.difference(&ttzip_keys) {
        if let Some(oracle_entry) = oracle.entries.get(*missing_key) {
            divergence_errors.push(format!(
                "Missing entry in TTZip output: '{}' (oracle type: {}, size: {}B)",
                missing_key, oracle_entry.entry_type.as_str(), oracle_entry.byte_size
            ));
        }
    }

    for extra_key in ttzip_keys.difference(&oracle_keys) {
        if let Some(ttzip_entry) = ttzip.entries.get(*extra_key) {
            divergence_errors.push(format!(
                "Unexpected extra entry in TTZip output: '{}' (ttzip type: {}, size: {}B)",
                extra_key, ttzip_entry.entry_type.as_str(), ttzip_entry.byte_size
            ));
        }
    }

    for common_key in ttzip_keys.intersection(&oracle_keys) {
        let ttzip_entry = &ttzip.entries[*common_key];
        let oracle_entry = &oracle.entries[*common_key];

        if ttzip_entry.entry_type != oracle_entry.entry_type {
            divergence_errors.push(format!(
                "Entry '{}' type mismatch: TTZip is {}, Oracle is {}",
                common_key, ttzip_entry.entry_type.as_str(), oracle_entry.entry_type.as_str()
            ));
            continue;
        }

        if ttzip_entry.entry_type == EntryType::RegularFile {
            if ttzip_entry.byte_size != oracle_entry.byte_size {
                divergence_errors.push(format!(
                    "Entry '{}' byte size mismatch: TTZip={}B, Oracle={}B",
                    common_key, ttzip_entry.byte_size, oracle_entry.byte_size
                ));
            }
            if ttzip_entry.sha256_checksum != oracle_entry.sha256_checksum {
                divergence_errors.push(format!(
                    "Entry '{}' SHA-256 checksum mismatch: TTZip={}, Oracle={}",
                    common_key, ttzip_entry.sha256_checksum, oracle_entry.sha256_checksum
                ));
                if hex_diff_output.is_none() {
                    let ttzip_path = Path::new(&ttzip.root_directory).join(common_key);
                    let oracle_path = Path::new(&oracle.root_directory).join(common_key);
                    if let (Ok(tb), Ok(ob)) = (fs::read(&ttzip_path), fs::read(&oracle_path)) {
                        hex_diff_output = generate_hex_diff(&ob, &tb, 256, true);
                    }
                }
            }
        }

        if ttzip_entry.entry_type == EntryType::SymbolicLink && ttzip_entry.symlink_target != oracle_entry.symlink_target {
            divergence_errors.push(format!(
                "Entry '{}' symlink target mismatch: TTZip='{}', Oracle='{}'",
                common_key, ttzip_entry.symlink_target.as_deref().unwrap_or("nil"),
                oracle_entry.symlink_target.as_deref().unwrap_or("nil")
            ));
        }

        if is_tar_format {
            if ttzip_entry.posix_mode != oracle_entry.posix_mode {
                divergence_errors.push(format!(
                    "Entry '{}' POSIX permission mismatch: TTZip=0o{:o}, Oracle=0o{:o}",
                    common_key, ttzip_entry.posix_mode, oracle_entry.posix_mode
                ));
            }
        } else if (ttzip_entry.posix_mode & 0o111) != (oracle_entry.posix_mode & 0o111) {
            divergence_errors.push(format!(
                "Entry '{}' executable bit mismatch: TTZip=0o{:o}, Oracle=0o{:o}",
                common_key, ttzip_entry.posix_mode, oracle_entry.posix_mode
            ));
        }
    }

    DifferentialReport {
        format_name: format_name.to_string(),
        target_oracle: oracle_name.to_string(),
        is_passed: divergence_errors.is_empty(),
        ttzip_manifest: ttzip.clone(),
        oracle_manifest: oracle.clone(),
        divergence_errors,
        hex_diff_output,
    }
}

// MARK: - C-ABI FFI Exports

/// C-ABI: Scans directory recursively using Rayon and generates JSON FileTreeManifest.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_rust_differential_scan_directory(
    path: *const c_char,
    out_manifest_json: *mut *mut c_char,
) -> TTZipStatus {
    unsafe {
        let result = catch_unwind(|| {
            if path.is_null() || out_manifest_json.is_null() {
                return TTZipStatus::ErrInvalidParam;
            }
            *out_manifest_json = std::ptr::null_mut();
            let path_str = match CStr::from_ptr(path).to_str() {
                Ok(s) => s,
                Err(_) => return TTZipStatus::ErrInvalidParam,
            };
            match scan_directory_tree(Path::new(path_str)) {
                Ok(manifest) => {
                    let json = manifest.to_json();
                    if let Ok(c_json) = CString::new(json) {
                        *out_manifest_json = c_json.into_raw();
                        TTZipStatus::Ok
                    } else {
                        TTZipStatus::ErrOutOfMemory
                    }
                }
                Err(status) => status,
            }
        });
        result.unwrap_or(TTZipStatus::ErrPanicCaught)
    }
}

/// C-ABI: Compares two FileTreeManifest JSON representations across 5 dimensions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_rust_differential_compare_manifests(
    ttzip_json: *const c_char,
    oracle_json: *const c_char,
    is_tar_format: bool,
    oracle_name: *const c_char,
    format_name: *const c_char,
    out_report_json: *mut *mut c_char,
    out_is_passed: *mut bool,
) -> TTZipStatus {
    unsafe {
        let result = catch_unwind(|| {
            if ttzip_json.is_null() || oracle_json.is_null() || out_report_json.is_null() {
                return TTZipStatus::ErrInvalidParam;
            }
            *out_report_json = std::ptr::null_mut();
            let ttzip_str = match CStr::from_ptr(ttzip_json).to_str() {
                Ok(s) => s,
                Err(_) => return TTZipStatus::ErrInvalidParam,
            };
            let oracle_str = match CStr::from_ptr(oracle_json).to_str() {
                Ok(s) => s,
                Err(_) => return TTZipStatus::ErrInvalidParam,
            };
            let or_name = if !oracle_name.is_null() { CStr::from_ptr(oracle_name).to_str().unwrap_or("oracle") } else { "oracle" };
            let fmt_name = if !format_name.is_null() { CStr::from_ptr(format_name).to_str().unwrap_or("zip") } else { "zip" };

            let ttzip_manifest = match FileTreeManifest::from_json(ttzip_str) {
                Some(m) => m,
                None => return TTZipStatus::ErrCorruptHeader,
            };
            let oracle_manifest = match FileTreeManifest::from_json(oracle_str) {
                Some(m) => m,
                None => return TTZipStatus::ErrCorruptHeader,
            };

            let report = compare_manifests(&ttzip_manifest, &oracle_manifest, is_tar_format, or_name, fmt_name);
            if !out_is_passed.is_null() {
                *out_is_passed = report.is_passed;
            }
            if let Ok(c_json) = CString::new(report.to_json()) {
                *out_report_json = c_json.into_raw();
                TTZipStatus::Ok
            } else {
                TTZipStatus::ErrOutOfMemory
            }
        });
        result.unwrap_or(TTZipStatus::ErrPanicCaught)
    }
}

/// C-ABI: Frees string memory allocated by differential scanner or verifier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttzip_rust_free_differential_string(ptr: *mut c_char) {
    unsafe {
        if !ptr.is_null() {
            let _ = CString::from_raw(ptr);
        }
    }
}

pub use super::differential_runner::{AutoCleanTempDir, DifferentialOracleTestRunner};

