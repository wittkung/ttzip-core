// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Memory Allocation Bounded Count & Safe Path Sanitization Defense Layer.
//!
//! Provides proactive defense against memory exhaustion (OOM) via malicious header count declarations
//! and directory traversal escape attacks (Zip-Slip, NTFS ADS, Windows reserved devices).

use std::path::{Component, Path, PathBuf};

use super::dag::SevenZError;
use crate::security::path_sanitizer::normalize_to_nfc;

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
/// 1. Normalizes Unicode strings to NFC form.
/// 2. Converts all backslashes `\` to forward slashes `/`.
/// 3. Validates path safety via `ttzip_types::path_security::path::validate_safe_relative_path`.
/// 4. Rejects traversal attempts (`..`), absolute paths, Windows drive letters,
///    NTFS Alternate Data Streams (ADS), and Windows reserved device names.
pub fn safe_join(dest_root: &Path, entry_path: &str) -> Result<PathBuf, SevenZError> {
    if entry_path.trim().is_empty() {
        return Err(SevenZError::InsecurePath("empty entry path".to_string()));
    }

    // 1. Unicode NFC normalization
    let normalized_nfc = normalize_to_nfc(entry_path);

    // 2. Unify backslashes to forward slashes
    let normalized = if normalized_nfc.contains('\\') {
        std::borrow::Cow::Owned(normalized_nfc.replace('\\', "/"))
    } else {
        normalized_nfc
    };

    // 3. Trim trailing slashes for directory paths and strip leading "./" prefixes
    let mut check_path = normalized.trim_end_matches('/');
    while let Some(stripped) = check_path.strip_prefix("./") {
        check_path = stripped;
    }

    if check_path.is_empty() {
        return Err(SevenZError::InsecurePath(format!(
            "entry path resolved to empty root relative target: '{entry_path}'"
        )));
    }

    // 4. Zero-allocation security validation via ttzip_types::path_security
    ttzip_types::path_security::path::validate_safe_relative_path(check_path).map_err(|e| {
        SevenZError::InsecurePath(format!("{e}: '{entry_path}'"))
    })?;

    // 5. Cleanly join normalized components onto destination root
    let mut target = dest_root.to_path_buf();
    for comp in Path::new(check_path).components() {
        if let Component::Normal(c) = comp {
            target.push(c);
        }
    }

    if target == dest_root {
        return Err(SevenZError::InsecurePath(format!(
            "entry path resolved to empty root relative target: '{entry_path}'"
        )));
    }

    Ok(target)
}
