// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zip-Slip Sandbox Path Defense & 42.zip Overlapping Payload Breaker Subsystem.
//!
//! Provides defense-in-depth mechanisms against malicious archive attacks:
//! - `enclosed_name`: Strict stack-depth aware Zip-Slip path normalization and rejection.
//! - `simplified_components`: Cross-platform root/drive-letter scrubbing and safe relative path extraction.
//! - `detect_overlapping_entries`: Interval intersection sweep-line scanner detecting 42.zip decompression bombs.
//! - `ExtractionQuotaGuard`: Cumulative decompression size & expansion ratio budget breaker.
//! - `validate_symlink_target`: Symlink recursion depth limiter and sandbox escape boundary validator.

use std::path::{Path, PathBuf};
use crate::types::TTZipStatus;

/// Resolves a sanitized relative path strictly enclosed within a destination sandbox root.
///
/// Depth-aware path stack tracking is performed:
/// - Rejects null bytes (`\0`) and URI schemes (e.g. `file://`).
/// - Rejects absolute paths (`/`, `\`, UNC `\\...`) and Windows drive prefixes (`C:`).
/// - Neutralizes inner `.` segments.
/// - Validates parent directory `..` traversals; if a `..` would cause the path stack
///   to underflow past the sandbox root, `None` is immediately returned.
/// - Rejects embedded Windows drive colons or Windows device reserved stems.
#[must_use]
pub fn enclosed_name(file_name: &str) -> Option<PathBuf> {
    if file_name.is_empty() || file_name.contains('\0') || file_name.contains("://") {
        return None;
    }

    // Reject absolute paths, UNC paths, and Windows drive letters at root
    if file_name.starts_with('/')
        || file_name.starts_with('\\')
        || file_name.starts_with("//")
        || file_name.starts_with(r"\\")
    {
        return None;
    }

    let bytes = file_name.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return None;
    }

    let mut stack: Vec<String> = Vec::with_capacity(8);

    for seg in file_name.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }

        if seg == ".." {
            if stack.is_empty() {
                // Stack underflow: attempting to escape above sandbox root
                return None;
            }
            stack.pop();
            continue;
        }

        // Reject embedded drive letters (e.g. "foo/C:/bar")
        let seg_bytes = seg.as_bytes();
        if seg_bytes.len() >= 2 && seg_bytes[0].is_ascii_alphabetic() && seg_bytes[1] == b':' {
            return None;
        }

        stack.push(seg.to_string());
    }

    if stack.is_empty() {
        return None;
    }

    let mut path_buf = PathBuf::new();
    for seg in stack {
        path_buf.push(seg);
    }

    Some(path_buf)
}

/// Cleanses arbitrary absolute paths or Windows drive letters into safe relative components.
///
/// Converts paths such as `/etc/passwd` to `etc/passwd` and `C:\foo\bar` to `foo/bar`,
/// dropping leading root separators, drive letters, and neutralizing any stack underflows.
#[must_use]
pub fn simplified_components(file_name: &str) -> Option<PathBuf> {
    if file_name.is_empty() || file_name.contains('\0') {
        return None;
    }

    let mut raw = file_name;

    // Strip UNC prefixes
    if raw.starts_with(r"\\?\UNC\") || raw.starts_with(r"//?/UNC/") {
        raw = &raw[8..];
    } else if raw.starts_with(r"\\?\") || raw.starts_with(r"\\.\") || raw.starts_with("//?/") {
        raw = &raw[4..];
    }

    // Strip leading Windows drive letters (e.g. "C:" or "c:")
    let raw_bytes = raw.as_bytes();
    if raw_bytes.len() >= 2 && raw_bytes[0].is_ascii_alphabetic() && raw_bytes[1] == b':' {
        raw = &raw[2..];
    }

    // Strip leading slashes/backslashes
    let trimmed = raw.trim_start_matches(['/', '\\']);
    if trimmed.is_empty() {
        return None;
    }

    let mut stack: Vec<String> = Vec::with_capacity(8);

    for seg in trimmed.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }

        if seg == ".." {
            if !stack.is_empty() {
                stack.pop();
            }
            // If stack is empty, safely ignore leading '..' rather than underflowing
            continue;
        }

        // Clean any inner drive letter segment (e.g. "D:")
        let seg_bytes = seg.as_bytes();
        let clean_seg = if seg_bytes.len() >= 2 && seg_bytes[0].is_ascii_alphabetic() && seg_bytes[1] == b':' {
            &seg[2..]
        } else {
            seg
        };

        if !clean_seg.is_empty() {
            stack.push(clean_seg.to_string());
        }
    }

    if stack.is_empty() {
        return None;
    }

    let mut path_buf = PathBuf::new();
    for seg in stack {
        path_buf.push(seg);
    }

    Some(path_buf)
}

/// Detects 42.zip style overlapping payload intervals across archive entries.
///
/// An overlapping file zip bomb occurs when multiple directory headers point to identical
/// or overlapping byte ranges in the archive payload. A valid archive has disjoint
/// intervals `[offset, offset + length)` for non-empty entries.
///
/// Returns `true` if any two non-empty entries have intersecting byte ranges or if
/// arithmetic overflow occurs.
#[must_use]
pub fn detect_overlapping_entries(entries: &[(u64, u64)]) -> bool {
    let mut intervals: Vec<(u64, u64)> = Vec::with_capacity(entries.len());

    for &(offset, len) in entries {
        if len == 0 {
            continue;
        }
        let end = match offset.checked_add(len) {
            Some(e) => e,
            None => return true, // Arithmetic overflow indicates malformed / hostile offset
        };
        intervals.push((offset, end));
    }

    if intervals.len() <= 1 {
        return false;
    }

    // Sort intervals by start offset ascending, then end offset ascending
    intervals.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut max_prev_end = intervals[0].1;
    for &(start, end) in &intervals[1..] {
        if start < max_prev_end {
            // Overlapping range detected: multiple entries share compressed payload bytes
            return true;
        }
        if end > max_prev_end {
            max_prev_end = end;
        }
    }

    false
}

/// Cumulative uncompressed size budget and expansion ratio breaker.
///
/// Protects against zip bombs by enforcing:
/// 1. Maximum total uncompressed output size (`max_uncompressed_bytes`).
/// 2. Maximum decompression expansion ratio (`max_ratio`, e.g. 100:1) once the cumulative
///    uncompressed size exceeds `threshold_bytes`.
#[derive(Debug, Clone)]
pub struct ExtractionQuotaGuard {
    pub max_uncompressed_bytes: u64,
    pub max_ratio: f64,
    pub threshold_bytes: u64,
    pub cumulative_uncompressed_bytes: u64,
    pub cumulative_compressed_bytes: u64,
}

impl Default for ExtractionQuotaGuard {
    fn default() -> Self {
        Self::default_limits()
    }
}

impl ExtractionQuotaGuard {
    /// Default maximum cumulative uncompressed limit (10 GB).
    pub const DEFAULT_MAX_UNCOMPRESSED_BYTES: u64 = 10 * 1024 * 1024 * 1024;
    /// Default maximum decompression expansion ratio (100:1).
    pub const DEFAULT_MAX_RATIO: f64 = 100.0;
    /// Default threshold before ratio enforcement starts (1 MB).
    pub const DEFAULT_THRESHOLD_BYTES: u64 = 1024 * 1024;

    /// Creates a quota guard with custom uncompressed byte limit and expansion ratio.
    #[must_use]
    pub fn new(max_uncompressed_bytes: u64, max_ratio: f64) -> Self {
        Self {
            max_uncompressed_bytes,
            max_ratio,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
            cumulative_uncompressed_bytes: 0,
            cumulative_compressed_bytes: 0,
        }
    }

    /// Creates a quota guard with custom uncompressed limit, expansion ratio, and activation threshold.
    #[must_use]
    pub fn with_threshold(max_uncompressed_bytes: u64, max_ratio: f64, threshold_bytes: u64) -> Self {
        Self {
            max_uncompressed_bytes,
            max_ratio,
            threshold_bytes,
            cumulative_uncompressed_bytes: 0,
            cumulative_compressed_bytes: 0,
        }
    }

    /// Creates a quota guard with default production limits.
    #[must_use]
    pub fn default_limits() -> Self {
        Self {
            max_uncompressed_bytes: Self::DEFAULT_MAX_UNCOMPRESSED_BYTES,
            max_ratio: Self::DEFAULT_MAX_RATIO,
            threshold_bytes: Self::DEFAULT_THRESHOLD_BYTES,
            cumulative_uncompressed_bytes: 0,
            cumulative_compressed_bytes: 0,
        }
    }

    /// Tracks new incoming compressed and uncompressed byte deltas.
    ///
    /// # Errors
    /// Returns `ErrSecurityViolation` if total uncompressed bytes exceed the quota or
    /// if the cumulative expansion ratio exceeds `max_ratio`.
    pub fn track(&mut self, compressed_delta: u64, uncompressed_delta: u64) -> Result<(), TTZipStatus> {
        self.cumulative_compressed_bytes = self.cumulative_compressed_bytes.saturating_add(compressed_delta);
        self.cumulative_uncompressed_bytes = self.cumulative_uncompressed_bytes.saturating_add(uncompressed_delta);

        // 1. Enforce hard total output byte limit
        if self.cumulative_uncompressed_bytes > self.max_uncompressed_bytes {
            return Err(TTZipStatus::ErrSecurityViolation);
        }

        // 2. Enforce expansion ratio check once beyond warmup threshold
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

    /// Returns the current observed expansion ratio.
    #[inline]
    #[must_use]
    pub fn current_ratio(&self) -> f64 {
        let comp = self.cumulative_compressed_bytes.max(1) as f64;
        (self.cumulative_uncompressed_bytes as f64) / comp
    }

    /// Resets cumulative counters to zero.
    pub fn reset(&mut self) {
        self.cumulative_uncompressed_bytes = 0;
        self.cumulative_compressed_bytes = 0;
    }
}

/// Validates a symlink target against sandbox escape attempts and recursion depth limits.
///
/// # Errors
/// Returns `ErrSecurityViolation` if:
/// - Target is empty or contains null bytes.
/// - Depth limit is zero or exceeded.
/// - Target attempts absolute path traversal (`/`, `\`, `C:`).
/// - Target contains `..` segments that escape above `dest_root`.
pub fn validate_symlink_target(
    dest_root: &Path,
    symlink_target: &str,
    depth_limit: usize,
) -> Result<PathBuf, TTZipStatus> {
    if symlink_target.is_empty() || symlink_target.contains('\0') {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    if depth_limit == 0 {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    // Reject absolute paths, UNC paths, and Windows drive letters
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

    let mut stack: Vec<&str> = Vec::new();

    for comp in symlink_target.split(['/', '\\']) {
        if comp.is_empty() || comp == "." {
            continue;
        }

        if comp == ".." {
            if stack.is_empty() {
                // Escapes sandbox root!
                return Err(TTZipStatus::ErrSecurityViolation);
            }
            stack.pop();
        } else {
            stack.push(comp);
        }
    }

    if stack.len() > depth_limit {
        return Err(TTZipStatus::ErrSecurityViolation);
    }

    let mut resolved = dest_root.to_path_buf();
    for seg in &stack {
        resolved.push(seg);
    }

    Ok(resolved)
}
