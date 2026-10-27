// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-Allocation Cross-Platform Path Sanitizer & ZipSlip Defense Subsystem.
//!
//! Aligned with libarchive `archive_read_disk_posix.c` and `archive_read_disk_windows.c`:
//! - Single-pass stack-based ZipSlip directory traversal neutralization and detection
//! - Windows reserved device name interception (CON, PRN, AUX, NUL, COM0..9, LPT0..9, CLOCK$, PhysicalDrive)
//! - Win32 trailing space and dot normalization
//! - Segment-by-segment NTFS Alternate Data Stream (ADS) identification and stripping
//! - High-throughput Unicode NFC canonical normalization (with zero-allocation fast paths)
//! - Win32 extended-length path formatting (`\\?\` and `\\?\UNC\`)

use std::borrow::Cow;
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};

/// Errors emitted by zero-allocation path slice validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathSecurityError {
    /// Path is empty or consists entirely of whitespace.
    #[error("Path is empty or consists entirely of whitespace")]
    EmptyPath,

    /// Null byte injection detected in path slice.
    #[error("Null-byte injection detected in path slice")]
    NullByteInjection,

    /// Absolute path detected (leading slash, Windows drive letter, or UNC prefix).
    #[error("Absolute path prefix, UNC path, or drive letter is prohibited")]
    AbsolutePath,

    /// Directory traversal or Zip-Slip attack detected.
    #[error("Directory traversal attack detected ('..')")]
    DirectoryTraversal,

    /// Windows DOS reserved device name detected in path segment.
    #[error("Windows reserved device name detected in path segment")]
    ReservedDeviceName,

    /// NTFS Alternate Data Stream (ADS) syntax detected.
    #[error("NTFS alternate data stream (ADS) syntax (':') is prohibited")]
    AlternateDataStream,
}

impl From<PathSecurityError> for ttzip_types::TTZipStatus {
    fn from(_: PathSecurityError) -> Self {
        Self::ErrSecurityViolation
    }
}

/// Result of path security sanitization and canonical normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSanitizationResult {
    pub original_path: String,
    pub normalized_path: String,
    pub has_traversal_attack: bool,
    pub is_absolute: bool,
    pub is_unc: bool,
    pub is_long_path: bool,
    pub is_windows_reserved: bool,
    pub stripped_ads: Option<String>,
    pub win32_formatted_path: String,
}

impl PathSanitizationResult {
    /// Returns true if the path is safe to extract into a sandbox root.
    #[inline]
    #[must_use]
    pub fn is_safe(&self) -> bool {
        !self.has_traversal_attack
            && !self.is_absolute
            && !self.is_unc
            && !self.is_windows_reserved
            && self.stripped_ads.is_none()
            && !self.normalized_path.is_empty()
            && validate_archive_path_slice(self.normalized_path.as_bytes()).is_ok()
    }
}

/// Checks if a path segment or name matches a Windows DOS reserved device.
#[inline]
#[must_use]
pub fn is_windows_reserved_device_name(segment: &str) -> bool {
    is_windows_reserved_device_name_slice(segment.as_bytes())
}

/// Extracts the Windows device stem before the first `.` or `:` and trims
/// leading spaces/tabs and trailing spaces/tabs/dots without allocation.
#[inline]
fn extract_trimmed_windows_device_stem(seg: &[u8]) -> &[u8] {
    let stem = match seg.iter().position(|&b| b == b'.' || b == b':') {
        Some(idx) => &seg[..idx],
        None => seg,
    };
    let start = stem
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(stem.len());
    let end = stem[start..]
        .iter()
        .rposition(|&b| b != b' ' && b != b'\t' && b != b'.')
        .map_or(start, |idx| start + idx + 1);
    &stem[start..end]
}

/// Checks fixed-name DOS/Win32 console and clock/config device stems.
#[inline]
fn is_fixed_windows_device_stem(trimmed: &[u8]) -> bool {
    const FIXED_3: [&[u8]; 4] = [b"CON", b"PRN", b"AUX", b"NUL"];
    const FIXED_6: [&[u8]; 2] = [b"CLOCK$", b"CONIN$"];
    const FIXED_7: [&[u8]; 2] = [b"CONOUT$", b"CONFIG$"];

    match trimmed.len() {
        3 => FIXED_3.iter().any(|d| trimmed.eq_ignore_ascii_case(d)),
        6 => FIXED_6.iter().any(|d| trimmed.eq_ignore_ascii_case(d)),
        7 => FIXED_7.iter().any(|d| trimmed.eq_ignore_ascii_case(d)),
        _ => false,
    }
}

/// Checks numbered `COM` / `LPT` stems (`COM0..9`, `LPT0..9`, and UTF-8 Latin-1 superscripts `¹..³`).
#[inline]
fn is_numbered_com_or_lpt_stem(trimmed: &[u8]) -> bool {
    if trimmed.len() < 4 {
        return false;
    }
    let prefix = &trimmed[..3];
    if !prefix.eq_ignore_ascii_case(b"COM") && !prefix.eq_ignore_ascii_case(b"LPT") {
        return false;
    }
    match &trimmed[3..] {
        [digit] => digit.is_ascii_digit(),
        [0xC2, 0xB9 | 0xB2 | 0xB3] => true,
        _ => false,
    }
}

/// Zero-allocation slice check for Windows DOS reserved device names.
#[inline]
#[must_use]
pub fn is_windows_reserved_device_name_slice(seg: &[u8]) -> bool {
    if seg.is_empty() {
        return false;
    }

    // Windows disallows filenames/directories consisting purely of dots (>= 3 dots)
    if seg.len() >= 3 && seg.iter().all(|&b| b == b'.') {
        return true;
    }

    if seg.len() >= 13 && seg[..13].eq_ignore_ascii_case(b"PHYSICALDRIVE") {
        return true;
    }

    let trimmed = extract_trimmed_windows_device_stem(seg);
    is_fixed_windows_device_stem(trimmed) || is_numbered_com_or_lpt_stem(trimmed)
}

/// Zero-allocation slice check for URL-encoded traversal patterns (%2e%2e).
#[inline]
fn contains_encoded_traversal_slice(seg: &[u8]) -> bool {
    if seg.len() < 6 {
        return false;
    }
    for window in seg.windows(6) {
        if window[0] == b'%'
            && (window[1] == b'2' && (window[2] == b'e' || window[2] == b'E'))
            && window[3] == b'%'
            && (window[4] == b'2' && (window[5] == b'e' || window[5] == b'E'))
        {
            return true;
        }
    }
    false
}

/// Validates an archive path byte slice using zero allocations and zero heap copies.
///
/// Ensures strict resistance against Zip-Slip, path traversal, null-byte injection,
/// whitespace bypasses, absolute paths, drive letters, UNC paths, NTFS ADS streams,
/// and Windows DOS reserved device names.
#[inline]
pub fn validate_archive_path_slice(raw: &[u8]) -> Result<(), PathSecurityError> {
    // 1. Null-byte injection check
    if raw.contains(&0) {
        return Err(PathSecurityError::NullByteInjection);
    }

    // 2. Zero-copy ASCII whitespace trimming (prevents whitespace bypass)
    let mut start = 0;
    while start < raw.len() && matches!(raw[start], b' ' | b'\t' | b'\r' | b'\n') {
        start += 1;
    }

    let mut end = raw.len();
    while end > start && matches!(raw[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }

    let trimmed = &raw[start..end];
    if trimmed.is_empty() {
        return Err(PathSecurityError::EmptyPath);
    }

    // 3. Absolute path, UNC, and drive letter detection
    if trimmed.starts_with(b"/")
        || trimmed.starts_with(b"\\")
        || trimmed.ends_with(b"/")
        || trimmed.ends_with(b"\\")
        || trimmed.starts_with(b"//")
        || trimmed.starts_with(b"\\\\")
        || (trimmed.len() >= 2 && trimmed[0].is_ascii_alphabetic() && trimmed[1] == b':')
        || trimmed.starts_with(b"\\\\?\\")
        || trimmed.starts_with(b"\\\\.\\")
        || trimmed.starts_with(b"\\??\\")
        || trimmed.starts_with(b"//?/")
        || trimmed.starts_with(b"//./")
    {
        return Err(PathSecurityError::AbsolutePath);
    }

    // 4. URL-encoded traversal check (%2e%2e)
    if contains_encoded_traversal_slice(trimmed) {
        return Err(PathSecurityError::DirectoryTraversal);
    }

    // 5. Alternate data stream check (':')
    if trimmed.contains(&b':') {
        return Err(PathSecurityError::AlternateDataStream);
    }

    // 6. Zero-allocation segment-by-segment security validation
    let mut valid_components_seen = false;
    for seg in trimmed.split(|&b| b == b'/' || b == b'\\') {
        if seg.is_empty() {
            return Err(PathSecurityError::DirectoryTraversal);
        }
        if seg == b"." {
            continue;
        }
        if seg == b".." {
            return Err(PathSecurityError::DirectoryTraversal);
        }
        if is_windows_reserved_device_name_slice(seg) {
            return Err(PathSecurityError::ReservedDeviceName);
        }
        if seg
            .iter()
            .any(|&b| b <= 0x1F || matches!(b, b'<' | b'>' | b'"' | b'|' | b'?' | b'*'))
        {
            return Err(PathSecurityError::DirectoryTraversal);
        }
        if seg.ends_with(b" ") || (seg.ends_with(b".") && seg != b"._.") {
            return Err(PathSecurityError::DirectoryTraversal);
        }
        valid_components_seen = true;
    }

    if !valid_components_seen {
        return Err(PathSecurityError::DirectoryTraversal);
    }

    let _ = std::str::from_utf8(trimmed).map_err(|_| PathSecurityError::DirectoryTraversal)?;
    Ok(())
}

/// Normalizes Unicode string to NFC form with zero-allocation fast-paths for ASCII and pre-normalized inputs.
#[inline]
#[must_use]
pub fn normalize_to_nfc(input: &str) -> Cow<'_, str> {
    if input.is_ascii() || is_nfc_quick(input.chars()) == IsNormalized::Yes {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(input.nfc().collect::<String>())
    }
}

/// Sanitizes and canonicalizes a relative or absolute filesystem path.
#[must_use]
pub fn sanitize_path(raw_path: &str) -> PathSanitizationResult {
    let trimmed_path = raw_path.trim();
    if trimmed_path.is_empty() {
        return PathSanitizationResult {
            original_path: raw_path.to_string(),
            normalized_path: String::new(),
            has_traversal_attack: false,
            is_absolute: false,
            is_unc: false,
            is_long_path: false,
            is_windows_reserved: false,
            stripped_ads: None,
            win32_formatted_path: String::new(),
        };
    }

    let mut has_traversal_attack = false;
    if raw_path.contains('\0') {
        has_traversal_attack = true;
    }

    // 1. Boundary & Protocol prefix detection (evaluated on trimmed path to prevent whitespace bypass)
    let mut is_unc = false;
    let mut is_absolute = false;

    if trimmed_path.starts_with(r"\\") || trimmed_path.starts_with("//") {
        is_unc = true;
        is_absolute = true;
    } else if trimmed_path.starts_with('/') || trimmed_path.starts_with('\\') {
        is_absolute = true;
    }

    // 2. Windows drive letter check (e.g. C:, D:/, etc.) on trimmed path
    let trimmed_bytes = trimmed_path.as_bytes();
    let has_drive_letter = trimmed_bytes.len() >= 2
        && trimmed_bytes[0].is_ascii_alphabetic()
        && trimmed_bytes[1] == b':';

    if has_drive_letter {
        is_absolute = true;
    }

    // Global check for PhysicalDrive in trimmed path
    let mut contains_reserved = trimmed_path.to_ascii_uppercase().contains("PHYSICALDRIVE");

    // 3. Fast Unicode NFC normalization
    let nfc_path = normalize_to_nfc(trimmed_path);

    // 4. Single-pass slash/backslash segment splitting, ADS stripping, and ZipSlip stack normalization
    let mut clean_segments: Vec<String> = Vec::with_capacity(8);
    let mut stripped_ads: Option<String> = None;

    // Split segments by '/' or '\\'
    let raw_segments = nfc_path.split(['/', '\\']);

    for (seg_idx, mut seg) in raw_segments.enumerate() {
        if seg.is_empty() {
            continue;
        }

        // NTFS Alternate Data Stream (ADS) identification & stripping
        // If this is the first segment and has a drive letter (e.g. "C:"), don't treat ':' as ADS
        let is_drive_seg = seg_idx == 0 && has_drive_letter && seg.len() == 2 && seg.as_bytes()[1] == b':';
        if !is_drive_seg {
            if let Some(colon_idx) = seg.find(':') {
                let ads_part = &seg[colon_idx..];
                if stripped_ads.is_none() {
                    stripped_ads = Some(ads_part.to_string());
                }
                seg = &seg[..colon_idx];
            }
        }

        if seg.is_empty() {
            continue;
        }

        // ZipSlip stack traversal check
        if seg == "." {
            continue;
        }

        if seg == ".." {
            if !clean_segments.is_empty() {
                // If the top of the stack is a drive letter (e.g. "C:"), popping it would escape the drive root
                let last_is_drive = clean_segments.len() == 1
                    && clean_segments[0].len() == 2
                    && clean_segments[0].as_bytes()[0].is_ascii_alphabetic()
                    && clean_segments[0].as_bytes()[1] == b':';

                if last_is_drive {
                    has_traversal_attack = true;
                } else {
                    clean_segments.pop();
                }
            } else {
                has_traversal_attack = true;
            }
            continue;
        }

        // Check Windows reserved device names
        if !contains_reserved && is_windows_reserved_device_name(seg) {
            contains_reserved = true;
        }

        clean_segments.push(seg.to_string());
    }

    let normalized_path = clean_segments.join("/");
    let is_long_path = normalized_path.chars().map(|c| c.len_utf16()).sum::<usize>() > 260;

    // 5. Format Win32 formatted path
    let win32_base = clean_segments.join("\\");
    let win32_formatted_path = if is_unc {
        format!(r"\\?\UNC\{}", win32_base)
    } else if is_long_path && is_absolute {
        format!(r"\\?\{}", win32_base)
    } else {
        win32_base
    };

    PathSanitizationResult {
        original_path: raw_path.to_string(),
        normalized_path,
        has_traversal_attack,
        is_absolute,
        is_unc,
        is_long_path,
        is_windows_reserved: contains_reserved,
        stripped_ads,
        win32_formatted_path,
    }
}

/// Real-time stream expansion ratio guard to detect and abort decompression bombs (Zip-Bombs).
#[derive(Debug, Clone, Copy)]
pub struct ExpansionRatioGuard {
    pub max_ratio: f64,
    pub threshold_uncompressed_bytes: u64,
}

impl Default for ExpansionRatioGuard {
    fn default() -> Self {
        Self {
            max_ratio: 1000.0,
            threshold_uncompressed_bytes: 100 * 1024 * 1024, // 100MB threshold before enforcing ratio check
        }
    }
}

impl ExpansionRatioGuard {
    pub fn new(max_ratio: f64, threshold_bytes: u64) -> Self {
        Self {
            max_ratio,
            threshold_uncompressed_bytes: threshold_bytes,
        }
    }

    #[inline]
    pub fn check(&self, uncompressed_written: u64, compressed_read: u64) -> Result<(), ttzip_types::TTZipStatus> {
        if uncompressed_written > self.threshold_uncompressed_bytes {
            let ratio = (uncompressed_written as f64) / (compressed_read.max(1) as f64);
            if ratio > self.max_ratio {
                return Err(ttzip_types::TTZipStatus::ErrSecurityViolation);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_bypass_in_sanitize_path() {
        // Leading space before POSIX absolute path
        let res_posix = sanitize_path(" /etc/passwd");
        assert!(res_posix.is_absolute);
        assert!(!res_posix.is_safe());

        // Tab before POSIX absolute path
        let res_tab = sanitize_path("\t/bin/sh");
        assert!(res_tab.is_absolute);
        assert!(!res_tab.is_safe());

        // Leading space before Windows drive letter
        let res_win = sanitize_path(" C:/Windows/System32");
        assert!(res_win.is_absolute);
        assert!(!res_win.is_safe());

        // Leading space before UNC share
        let res_unc = sanitize_path(" \\\\nas_server\\share");
        assert!(res_unc.is_unc);
        assert!(res_unc.is_absolute);
        assert!(!res_unc.is_safe());

        // Harmless leading/trailing whitespace around relative path
        let res_rel = sanitize_path("  my_folder/file.txt  ");
        assert_eq!(res_rel.normalized_path, "my_folder/file.txt");
        assert!(!res_rel.is_absolute);
        assert!(res_rel.is_safe());
    }

    #[test]
    fn test_validate_archive_path_slice_safe() {
        assert_eq!(validate_archive_path_slice(b"foo/bar/test.txt"), Ok(()));
        assert_eq!(validate_archive_path_slice(b"a/b/c/d/e.bin"), Ok(()));
        assert_eq!(validate_archive_path_slice(b"release..notes.txt"), Ok(()));
        assert_eq!(validate_archive_path_slice(b"my folder/data.json"), Ok(()));
    }

    #[test]
    fn test_validate_archive_path_slice_whitespace_bypass() {
        assert_eq!(
            validate_archive_path_slice(b" /etc/passwd"),
            Err(PathSecurityError::AbsolutePath)
        );
        assert_eq!(
            validate_archive_path_slice(b"\t\\Windows\\System32"),
            Err(PathSecurityError::AbsolutePath)
        );
        assert_eq!(
            validate_archive_path_slice(b"   C:/evil.exe"),
            Err(PathSecurityError::AbsolutePath)
        );
        assert_eq!(
            validate_archive_path_slice(b"   //server/share"),
            Err(PathSecurityError::AbsolutePath)
        );
        assert_eq!(
            validate_archive_path_slice(b"   \t  \r\n"),
            Err(PathSecurityError::EmptyPath)
        );
    }

    #[test]
    fn test_validate_archive_path_slice_traversal_attacks() {
        // Direct escapes above root
        assert_eq!(
            validate_archive_path_slice(b"../escape.txt"),
            Err(PathSecurityError::DirectoryTraversal)
        );
        assert_eq!(
            validate_archive_path_slice(b"../../etc/shadow"),
            Err(PathSecurityError::DirectoryTraversal)
        );

        // Internal traversal accompanied by escape attacks
        assert_eq!(
            validate_archive_path_slice(b"foo/../bar"),
            Err(PathSecurityError::DirectoryTraversal)
        );
        assert_eq!(
            validate_archive_path_slice(b"a/b/../c/../d"),
            Err(PathSecurityError::DirectoryTraversal)
        );
        assert_eq!(
            validate_archive_path_slice(b"foo/../../bar"),
            Err(PathSecurityError::DirectoryTraversal)
        );
        assert_eq!(
            validate_archive_path_slice(b"../foo/../bar"),
            Err(PathSecurityError::DirectoryTraversal)
        );
        assert_eq!(
            validate_archive_path_slice(b"foo/../bar/../../outside"),
            Err(PathSecurityError::DirectoryTraversal)
        );
        assert_eq!(
            validate_archive_path_slice(b"foo/.."),
            Err(PathSecurityError::DirectoryTraversal)
        );

        // Encoded traversal attack
        assert_eq!(
            validate_archive_path_slice(b"foo/%2e%2e/bar"),
            Err(PathSecurityError::DirectoryTraversal)
        );
    }

    #[test]
    fn test_validate_archive_path_slice_security_violations() {
        // Null byte injection
        assert_eq!(
            validate_archive_path_slice(b"foo\0bar.txt"),
            Err(PathSecurityError::NullByteInjection)
        );

        // Windows reserved devices
        assert_eq!(
            validate_archive_path_slice(b"CON"),
            Err(PathSecurityError::ReservedDeviceName)
        );
        assert_eq!(
            validate_archive_path_slice(b"docs/PRN.pdf"),
            Err(PathSecurityError::ReservedDeviceName)
        );
        assert_eq!(
            validate_archive_path_slice(b"sub/com1.dat"),
            Err(PathSecurityError::ReservedDeviceName)
        );
        assert_eq!(
            validate_archive_path_slice(b"lpt9"),
            Err(PathSecurityError::ReservedDeviceName)
        );
        assert_eq!(
            validate_archive_path_slice(b"aux..."),
            Err(PathSecurityError::ReservedDeviceName)
        );

        // NTFS Alternate Data Streams
        assert_eq!(
            validate_archive_path_slice(b"file.txt:hidden"),
            Err(PathSecurityError::AlternateDataStream)
        );
        assert_eq!(
            validate_archive_path_slice(b"folder/sub:stream:$DATA"),
            Err(PathSecurityError::AlternateDataStream)
        );
    }

    #[test]
    fn test_windows_reserved_device_superscript_and_config_contract() {
        for reserved in [
            "COM¹",
            "com¹.txt",
            "COM².log",
            "com³.tar.gz",
            "LPT¹",
            "lpt².txt",
            "LPT³.doc",
            "CONFIG$",
            "config$.sys",
            "sub/COM¹.txt",
            "a\\b\\LPT².bin",
            "nested/CONFIG$/item.txt",
        ] {
            assert_eq!(
                validate_archive_path_slice(reserved.as_bytes()),
                Err(PathSecurityError::ReservedDeviceName),
                "expected ReservedDeviceName for {reserved}"
            );
            let sanitized = sanitize_path(reserved);
            assert!(
                sanitized.is_windows_reserved,
                "expected sanitize_path({reserved}).is_windows_reserved == true"
            );
            assert!(
                !sanitized.is_safe(),
                "expected sanitize_path({reserved}).is_safe() == false"
            );
        }

        for benign in [
            "COM10.txt",
            "LPT10.txt",
            "COM¹0.txt",
            "LPT²extra.txt",
            "CONFIG.txt",
            "CONFIG$1.txt",
            "MYCONFIG$.txt",
            "notes/com_1.txt",
        ] {
            assert_eq!(
                validate_archive_path_slice(benign.as_bytes()),
                Ok(()),
                "expected Ok(()) for benign path {benign}"
            );
            let sanitized = sanitize_path(benign);
            assert!(
                !sanitized.is_windows_reserved,
                "expected sanitize_path({benign}).is_windows_reserved == false"
            );
            assert!(
                sanitized.is_safe(),
                "expected sanitize_path({benign}).is_safe() == true"
            );
        }
    }
}
