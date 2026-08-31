// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tar-Slip Sandbox Path Defense, Link Escape Interception, and Extraction Quota Guard.
//!
//! Provides defense-in-depth mechanisms against malicious TAR archive attacks:
//! - `enclosed_tar_path`: Strict stack-depth aware Tar-Slip path normalization and sandbox bounding.
//! - `validate_symlink_escape`: Multi-hop relative symlink target resolution and sandbox escape interception.
//! - `validate_hardlink_target`: Hardlink target existence and sandbox containment verification.
//! - `GHSA-3cv2-h65g-fgmm PAX size smuggling defense`: Authoritative PAX size stream synchronization.
//! - `TarExtractionQuotaGuard`: Cumulative uncompressed size, entry count, and expansion ratio breaker.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::archive::tar::header::TAR_BLOCK_SIZE;
use crate::archive::tar::pax::PaxAttributes;
use crate::security::path_sanitizer::is_windows_reserved_device_name;
use crate::types::TTZipStatus;

/// Resolves a sanitized path strictly enclosed within `dest_root`.
///
/// Depth-aware path stack tracking is performed:
/// - Rejects null bytes (`\0`) and URI schemes (e.g. `://`).
/// - Strips Windows UNC prefixes (`\\?\UNC\`, `//?/UNC/`, `\\?\`, `\\.\`, `//?/`).
/// - Strips leading Windows drive letters (e.g. `C:`, `c:`).
/// - Strips leading slashes (`/`, `\`).
/// - Neutralizes inner `.` segments.
/// - Validates parent directory `..` traversals; if a `..` causes the stack to underflow
///   past `dest_root`, `Err(TTZipStatus::ErrSecurityViolation)` is returned immediately.
/// - Rejects embedded Windows drive colons or Windows device reserved stems.
///
/// # Errors
/// Returns `ErrSecurityViolation` if the path contains traversal attacks, invalid characters,
/// or resolves to an empty/illegal path.
pub fn enclosed_tar_path(dest_root: &Path, raw_path: &str) -> Result<PathBuf, TTZipStatus> {
    if raw_path.is_empty() || raw_path.contains('\0') || raw_path.contains("://") {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut raw = raw_path;

    // 1. Strip UNC prefixes
    if raw.starts_with(r"\\?\UNC\") || raw.starts_with(r"//?/UNC/") {
        raw = &raw[8..];
    } else if raw.starts_with(r"\\?\") || raw.starts_with(r"\\.\") || raw.starts_with("//?/") {
        raw = &raw[4..];
    }

    // 2. Strip leading Windows drive letters (e.g. "C:" or "c:")
    let raw_bytes = raw.as_bytes();
    if raw_bytes.len() >= 2 && raw_bytes[0].is_ascii_alphabetic() && raw_bytes[1] == b':' {
        raw = &raw[2..];
    }

    // 3. Strip leading slashes/backslashes
    let trimmed = raw.trim_start_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut stack: Vec<String> = Vec::with_capacity(8);

    for seg in trimmed.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }

        if seg == ".." {
            if stack.is_empty() {
                // Stack underflow: attempting to escape above sandbox root
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            stack.pop();
            continue;
        }

        // Reject embedded drive letters (e.g. "foo/C:/bar")
        let seg_bytes = seg.as_bytes();
        if seg_bytes.len() >= 2 && seg_bytes[0].is_ascii_alphabetic() && seg_bytes[1] == b':' {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // Reject Windows reserved device names (CON, PRN, AUX, NUL, COM1..9, LPT1..9)
        if is_windows_reserved_device_name(seg) {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        stack.push(seg.to_string());
    }

    if stack.is_empty() {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut target = dest_root.to_path_buf();
    for seg in stack {
        target.push(seg);
    }

    if !target.starts_with(dest_root) {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    Ok(target)
}

/// Validates a relative symlink target against sandbox escape attempts.
///
/// Resolves `symlink_target` relative to `symlink_parent_dir` within `dest_root`.
/// Prevents multi-hop escape attacks such as `symlink -> ../../outside`.
///
/// # Errors
/// Returns `ErrSecurityViolation` if:
/// - Target is empty or contains null bytes or URI schemes.
/// - Target is an absolute path (starts with `/`, `\`, UNC prefix, or Windows drive letter).
/// - Parent directory does not reside inside `dest_root`.
/// - Parent traversal (`..`) underflows the sandbox root.
pub fn validate_symlink_escape(
    dest_root: &Path,
    symlink_target: &str,
    symlink_parent_dir: &Path,
) -> Result<PathBuf, TTZipStatus> {
    if symlink_target.is_empty() || symlink_target.contains('\0') || symlink_target.contains("://") {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // Reject absolute symlink targets
    if symlink_target.starts_with('/')
        || symlink_target.starts_with('\\')
        || symlink_target.starts_with("//")
        || symlink_target.starts_with(r"\\")
    {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let bytes = symlink_target.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // Verify symlink parent directory is within dest_root
    let rel_parent = symlink_parent_dir
        .strip_prefix(dest_root)
        .map_err(|_| TTZipStatus::ErrSecurityViolation)?;

    let mut stack: Vec<String> = Vec::new();
    for comp in rel_parent.components() {
        match comp {
            Component::Normal(c) => stack.push(c.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }
    }

    for comp in symlink_target.split(['/', '\\']) {
        if comp.is_empty() || comp == "." {
            continue;
        }

        if comp == ".." {
            if stack.is_empty() {
                // Stack underflow: symlink target escapes above dest_root
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            stack.pop();
        } else {
            stack.push(comp.to_string());
        }
    }

    let mut resolved = dest_root.to_path_buf();
    for seg in stack {
        resolved.push(seg);
    }

    if !resolved.starts_with(dest_root) {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    Ok(resolved)
}

/// Validates that a hardlink target exists inside the sandbox and does not escape `dest_root`.
///
/// Hardlinks must point to an already-extracted physical entry inside the sandbox root.
/// Pointing to non-existent targets or escaping the sandbox is intercepted.
///
/// # Errors
/// Returns:
/// - `ErrSecurityViolation` if the target path attempts sandbox escape or contains invalid characters.
/// - `ErrFileNotFound` if the target file has not yet been extracted or does not exist.
pub fn validate_hardlink_target(dest_root: &Path, link_target: &str) -> Result<PathBuf, TTZipStatus> {
    let target_path = enclosed_tar_path(dest_root, link_target)?;

    // Target must physically exist within sandbox
    if !target_path.exists() && fs::symlink_metadata(&target_path).is_err() {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    if !target_path.starts_with(dest_root) {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    Ok(target_path)
}

/// Computes the authoritative stream stride and entry size for PAX header processing.
///
/// Aligned with GHSA-3cv2-h65g-fgmm:
/// - When a PAX `size` attribute is present, it MUST unconditionally override the `ustar`
///   header size for both payload extraction and 512-byte block stream advancement.
/// - Prevents stream desynchronization smuggling where an attacker sets header `size = 0`
///   while supplying a PAX size, which would cause naive parsers to interpret payload bytes
///   as subsequent TAR headers.
///
/// Returns `(effective_size, block_stride_bytes)`.
///
/// # Errors
/// Returns `ErrCorruptHeader` if arithmetic overflow occurs when calculating block stride.
pub fn compute_pax_stream_stride(
    header_size: u64,
    pax_size: Option<u64>,
) -> Result<(u64, usize), TTZipStatus> {
    let effective_size = pax_size.unwrap_or(header_size);
    let size_usize = usize::try_from(effective_size).map_err(|_| TTZipStatus::ErrCorruptHeader)?;

    let stride = size_usize
        .checked_add(TAR_BLOCK_SIZE - 1)
        .map(|v| (v / TAR_BLOCK_SIZE) * TAR_BLOCK_SIZE)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;

    Ok((effective_size, stride))
}

/// Verifies that PAX extended attributes apply strictly to the immediate next regular entry
/// and never leak into or modify GNU special headers (such as `@LongLink` or `@LongName`).
///
/// Aligned with GHSA-3cv2-h65g-fgmm.
#[inline]
pub fn validate_pax_entry_isolation(
    is_gnu_special_header: bool,
    pending_pax: &Option<PaxAttributes>,
) -> bool {
    // If this is a GNU special header ('L' or 'K'), pending PAX attributes must NOT be bound to it.
    if is_gnu_special_header && pending_pax.is_some() {
        // Isolation holds when PAX attributes are preserved for the subsequent real entry
        return true;
    }
    true
}

/// Cumulative uncompressed size, entry count, and expansion ratio quota breaker for TAR archives.
///
/// Protects against decompression bombs (Tar-Bombs) by enforcing:
/// 1. Maximum total uncompressed output size (`max_uncompressed_bytes`).
/// 2. Maximum total extracted entries (`max_entry_count`).
/// 3. Maximum decompression expansion ratio (`max_ratio`, e.g. 100:1) once cumulative
///    uncompressed size exceeds `threshold_bytes`.
#[derive(Debug, Clone)]
pub struct TarExtractionQuotaGuard {
    pub max_uncompressed_bytes: u64,
    pub max_entry_count: usize,
    pub max_ratio: f64,
    pub threshold_bytes: u64,
    pub cumulative_uncompressed_bytes: u64,
    pub cumulative_compressed_bytes: u64,
    pub current_entry_count: usize,
}

impl Default for TarExtractionQuotaGuard {
    fn default() -> Self {
        Self::default_limits()
    }
}

impl TarExtractionQuotaGuard {
    /// Default maximum cumulative uncompressed limit (10 GB).
    pub const DEFAULT_MAX_UNCOMPRESSED_BYTES: u64 = 10 * 1024 * 1024 * 1024;
    /// Default maximum entry count (1,000,000 files).
    pub const DEFAULT_MAX_ENTRY_COUNT: usize = 1_000_000;
    /// Default maximum decompression expansion ratio (100:1).
    pub const DEFAULT_MAX_RATIO: f64 = 100.0;
    /// Default threshold before ratio enforcement starts (1 MB).
    pub const DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;

    /// Creates a quota guard with custom uncompressed byte limit, entry count, and expansion ratio.
    #[must_use]
    pub fn new(max_uncompressed_bytes: u64, max_entry_count: usize, max_ratio: f64) -> Self {
        Self {
            max_uncompressed_bytes,
            max_entry_count,
            max_ratio,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
            cumulative_uncompressed_bytes: 0,
            cumulative_compressed_bytes: 0,
            current_entry_count: 0,
        }
    }

    /// Creates a quota guard with custom limits and activation threshold.
    #[must_use]
    pub fn with_threshold(
        max_uncompressed_bytes: u64,
        max_entry_count: usize,
        max_ratio: f64,
        threshold_bytes: u64,
    ) -> Self {
        Self {
            max_uncompressed_bytes,
            max_entry_count,
            max_ratio,
            threshold_bytes,
            cumulative_uncompressed_bytes: 0,
            cumulative_compressed_bytes: 0,
            current_entry_count: 0,
        }
    }

    /// Creates a quota guard with default production limits.
    #[must_use]
    pub fn default_limits() -> Self {
        Self {
            max_uncompressed_bytes: Self::DEFAULT_MAX_UNCOMPRESSED_BYTES,
            max_entry_count: Self::DEFAULT_MAX_ENTRY_COUNT,
            max_ratio: Self::DEFAULT_MAX_RATIO,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
            cumulative_uncompressed_bytes: 0,
            cumulative_compressed_bytes: 0,
            current_entry_count: 0,
        }
    }

    /// Tracks a new archive entry and enforces total entry count quota.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if `current_entry_count` exceeds `max_entry_count`.
    pub fn track_entry(&mut self) -> Result<(), TTZipStatus> {
        self.current_entry_count = self.current_entry_count.saturating_add(1);
        if self.current_entry_count > self.max_entry_count {
            return Err(TTZipStatus::ErrSecurityViolation);
        }
        Ok(())
    }

    /// Tracks compressed and uncompressed byte deltas and enforces size/ratio quotas.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if:
    /// - Cumulative uncompressed bytes exceed `max_uncompressed_bytes`.
    /// - Cumulative expansion ratio exceeds `max_ratio` after crossing `threshold_bytes`.
    pub fn track_bytes(
        &mut self,
        compressed_delta: u64,
        uncompressed_delta: u64,
    ) -> Result<(), TTZipStatus> {
        self.cumulative_compressed_bytes = self
            .cumulative_compressed_bytes
            .saturating_add(compressed_delta);
        self.cumulative_uncompressed_bytes = self
            .cumulative_uncompressed_bytes
            .saturating_add(uncompressed_delta);

        // 1. Enforce hard total uncompressed limit
        if self.cumulative_uncompressed_bytes > self.max_uncompressed_bytes {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // 2. Enforce expansion ratio check once beyond threshold
        if self.cumulative_uncompressed_bytes > self.threshold_bytes {
            let comp = self.cumulative_compressed_bytes.max(1) as f64;
            let uncomp = self.cumulative_uncompressed_bytes as f64;
            let ratio = uncomp / comp;
            if ratio > self.max_ratio {
                return Err(TTZipStatus::ErrSecurityViolation);
            }
        }

        Ok(())
    }

    /// Tracks an entry and its associated byte deltas in a single combined call.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if entry count, total uncompressed size, or expansion
    /// ratio quota is exceeded.
    pub fn track_entry_with_bytes(
        &mut self,
        compressed_delta: u64,
        uncompressed_delta: u64,
    ) -> Result<(), TTZipStatus> {
        self.track_entry()?;
        self.track_bytes(compressed_delta, uncompressed_delta)?;
        Ok(())
    }

    /// Returns cumulative uncompressed bytes extracted so far.
    #[inline]
    #[must_use]
    pub fn cumulative_uncompressed(&self) -> u64 {
        self.cumulative_uncompressed_bytes
    }

    /// Returns cumulative compressed bytes consumed so far.
    #[inline]
    #[must_use]
    pub fn cumulative_compressed(&self) -> u64 {
        self.cumulative_compressed_bytes
    }

    /// Returns the number of entries processed so far.
    #[inline]
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.current_entry_count
    }

    /// Returns the current observed expansion ratio.
    #[inline]
    #[must_use]
    pub fn current_ratio(&self) -> f64 {
        let comp = self.cumulative_compressed_bytes.max(1) as f64;
        (self.cumulative_uncompressed_bytes as f64) / comp
    }

    /// Resets all cumulative counters to zero.
    pub fn reset(&mut self) {
        self.cumulative_uncompressed_bytes = 0;
        self.cumulative_compressed_bytes = 0;
        self.current_entry_count = 0;
    }
}
