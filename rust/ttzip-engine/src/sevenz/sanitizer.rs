// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Memory Allocation Bounded Count & Safe Path Sanitization Defense Layer.
//!
//! Provides proactive defense against memory exhaustion (OOM) via malicious header count declarations
//! and directory traversal escape attacks (Zip-Slip, NTFS ADS, Windows reserved devices).

use std::path::{Path, PathBuf};

use super::dag::SevenZError;
use crate::security::path_sanitizer::{is_windows_reserved_device_name, normalize_to_nfc};

/// Default upper limit for the number of files/entries in a 7z archive.
pub const DEFAULT_MAX_FILES_LIMIT: usize = 2_000_000;

/// Default upper limit for the number of folders in a 7z archive.
pub const DEFAULT_MAX_FOLDERS_LIMIT: usize = 1_000_000;

/// Default upper limit for the number of coders inside a single 7z folder.
pub const DEFAULT_MAX_CODERS_LIMIT: usize = 64;

/// Default upper limit for the number of pack/unpack streams in a 7z archive.
pub const DEFAULT_MAX_STREAMS_LIMIT: usize = 4_000_000;

/// Safely validates that an untrusted 64-bit count from a 7z archive header does not exceed a reasonable memory budget.
///
/// Returns `Ok(usize)` on success, or `Err(SevenZError::CountLimitExceeded)` if `value > limit` or `value > usize::MAX`.
#[inline]
pub fn bounded_count(value: u64, limit: usize, field_name: &'static str) -> Result<usize, SevenZError> {
    if value > limit as u64 {
        return Err(SevenZError::CountLimitExceeded {
            field_name,
            value,
            limit,
        });
    }
    Ok(value as usize)
}

/// Safely converts an untrusted 64-bit size or offset to `usize`, ensuring it does not exceed `max_limit`.
///
/// Returns `Ok(usize)` on success, or `Err(SevenZError::CountLimitExceeded)` if `value > max_limit` or `value > usize::MAX`.
#[inline]
pub fn bounded_usize(value: u64, max_limit: usize, field_name: &'static str) -> Result<usize, SevenZError> {
    bounded_count(value, max_limit, field_name)
}

/// Safely joins a destination root directory and an untrusted archive entry relative path,
/// enforcing strict Zip-Slip defense and cross-platform path normalization.
///
/// # Security Invariants
/// 1. Normalizes all backslashes `\` to forward slashes `/`.
/// 2. Normalizes Unicode strings to NFC form.
/// 3. Rejects empty paths, paths with null bytes (`\0`), root `/` or `\` prefixes,
///    UNC prefixes (`//`, `\\`), and Windows drive letters (e.g. `C:`, `D:`).
/// 4. Stack-evaluates all path segments and strictly intercepts directory traversal attempts
///    (`..`) that escape above the destination root.
/// 5. Intercepts NTFS Alternate Data Streams (ADS) and Windows DOS reserved device names.
pub fn safe_join(dest_root: &Path, entry_path: &str) -> Result<PathBuf, SevenZError> {
    if entry_path.trim().is_empty() {
        return Err(SevenZError::InsecurePath("empty entry path".to_string()));
    }

    if entry_path.contains('\0') {
        return Err(SevenZError::InsecurePath(format!(
            "entry path contains null byte: {:?}",
            entry_path
        )));
    }

    // Check Windows drive letter prefix (e.g. C:, D:)
    let raw_bytes = entry_path.as_bytes();
    if raw_bytes.len() >= 2
        && raw_bytes[0].is_ascii_alphabetic()
        && raw_bytes[1] == b':'
    {
        return Err(SevenZError::InsecurePath(format!(
            "absolute drive letter path is forbidden: {}",
            entry_path
        )));
    }

    // Check absolute path or UNC path prefix
    if entry_path.starts_with('/') || entry_path.starts_with('\\') {
        return Err(SevenZError::InsecurePath(format!(
            "absolute or UNC path is forbidden: {}",
            entry_path
        )));
    }

    // Unicode NFC normalization
    let normalized_nfc = normalize_to_nfc(entry_path);

    // Segment-by-segment stack traversal validation
    let mut segment_stack: Vec<&str> = Vec::with_capacity(8);

    for seg in normalized_nfc.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }

        if seg == ".." {
            if segment_stack.pop().is_none() {
                return Err(SevenZError::InsecurePath(format!(
                    "Zip-Slip directory traversal escape detected in '{}'",
                    entry_path
                )));
            }
            continue;
        }

        // NTFS Alternate Data Stream check
        if seg.contains(':') {
            return Err(SevenZError::InsecurePath(format!(
                "NTFS Alternate Data Stream detected in segment '{}' of '{}'",
                seg, entry_path
            )));
        }

        // Windows reserved device names check
        if is_windows_reserved_device_name(seg) {
            return Err(SevenZError::InsecurePath(format!(
                "Windows reserved device name detected in segment '{}' of '{}'",
                seg, entry_path
            )));
        }

        segment_stack.push(seg);
    }

    if segment_stack.is_empty() {
        return Err(SevenZError::InsecurePath(format!(
            "entry path resolved to empty root relative target: '{}'",
            entry_path
        )));
    }

    let mut target_path = dest_root.to_path_buf();
    for seg in segment_stack {
        target_path.push(seg);
    }

    Ok(target_path)
}
