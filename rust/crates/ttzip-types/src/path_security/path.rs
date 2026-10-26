// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Deterministic, zero-allocation, const-evaluated cross-platform relative path sanitization.
//!
//! Enforces mathematical boundary safety against:
//! - Parent directory traversal (CWE-22 Zip Slip)
//! - Windows trailing dot and whitespace truncation (CWE-41 / MagicDot)
//! - Win32 pre-extension space collisions (`foo .txt` -> `foo.txt`)
//! - NTFS alternate data streams (`:` ADS injections)
//! - NTFS 8.3 short-name aliases (`~1` Git RCE defense)
//! - NTFS system root metadata streams (`$Mft`, `$LogFile`)
//! - Git repository folder and configuration metadata (`.git`, `.gitmodules`, `.gitattributes`)
//! - Apple HFS+ Unicode ignorable code point bypasses (CVE-2014-9390)
//! - Trojan Source bidirectional visual spoofing (CVE-2021-42574)

use std::path::Path;

use super::device::{is_ntfs_reserved_metadata, is_windows_reserved_device_name_bytes};
use super::error::PathSecurityError;

/// Pure-register 64-bit mask for branchless classification of ASCII characters 32..96.
///
/// Set bits correspond to:
/// - '"' (34 - 32 = 2)
/// - '*' (42 - 32 = 10)
/// - ':' (58 - 32 = 26)
/// - '<' (60 - 32 = 28)
/// - '>' (62 - 32 = 30)
/// - '?' (63 - 32 = 31)
/// - '\' (92 - 32 = 60)
const FORBIDDEN_MASK_32_95: u64 = (1u64 << (34 - 32))
    | (1u64 << (42 - 32))
    | (1u64 << (58 - 32))
    | (1u64 << (60 - 32))
    | (1u64 << (62 - 32))
    | (1u64 << (63 - 32))
    | (1u64 << (92 - 32));

/// Pure-register branchless check for characters forbidden in cross-platform portable paths.
///
/// Forbids:
/// - ASCII control characters (0x00..=0x1F)
/// - ASCII DEL (0x7F)
/// - Windows-illegal characters: `< > : " \ | ? *`
#[inline(always)]
pub const fn is_forbidden_path_char(b: u8) -> bool {
    if b < 32 {
        true
    } else if b < 96 {
        ((FORBIDDEN_MASK_32_95 >> (b - 32)) & 1) != 0
    } else {
        b == b'|' || b == 0x7F
    }
}

/// Strictly validates whether a relative path string is safe, returning explicit error diagnostics.
#[inline]
pub const fn validate_safe_relative_path(path_str: &str) -> Result<(), PathSecurityError> {
    let bytes = path_str.as_bytes();
    let total_len = bytes.len();

    if total_len == 0 {
        return Err(PathSecurityError::Empty);
    }
    if total_len > 4096 {
        return Err(PathSecurityError::PathTooLong {
            len: total_len,
            max: 4096,
        });
    }

    if bytes[0] == b'/' {
        return Err(PathSecurityError::LeadingSlash);
    }
    if bytes[total_len - 1] == b'/' {
        return Err(PathSecurityError::TrailingSlash);
    }

    let mut has_normal = false;
    let mut last_is_normal = false;
    let mut remaining = bytes;

    while !remaining.is_empty() {
        let mut seg_len = 0;
        while seg_len < remaining.len() && remaining[seg_len] != b'/' {
            seg_len += 1;
        }

        if seg_len == 0 {
            return Err(PathSecurityError::EmptyComponent);
        }
        if seg_len > 255 {
            return Err(PathSecurityError::ComponentTooLong {
                len: seg_len,
                max: 255,
            });
        }

        let (seg, rest) = remaining.split_at(seg_len);

        if seg_len == 1 && seg[0] == b'.' {
            last_is_normal = false;
        } else if seg_len == 2 && seg[0] == b'.' && seg[1] == b'.' {
            return Err(PathSecurityError::ParentTraversal);
        } else {
            if seg[0] == b' ' {
                return Err(PathSecurityError::LeadingSpace);
            }
            if seg[seg_len - 1] == b' ' || seg[seg_len - 1] == b'.' {
                return Err(PathSecurityError::Win32TrailingDotOrSpace);
            }

            // Check Git repository folder (.git)
            if seg_len == 4 && seg.eq_ignore_ascii_case(b".git") {
                return Err(PathSecurityError::GitDirectory);
            }

            // Check Git repository configuration files (.gitmodules, .gitattributes)
            if seg_len == 11 && seg.eq_ignore_ascii_case(b".gitmodules") {
                return Err(PathSecurityError::GitMetadata);
            }

            if seg_len == 14 && seg.eq_ignore_ascii_case(b".gitattributes") {
                return Err(PathSecurityError::GitMetadata);
            }

            let mut dot_idx = 0;
            let mut has_dot = false;
            let mut prev_b = 0u8;
            let mut i = 0;

            while i < seg_len {
                let b = seg[i];
                if is_forbidden_path_char(b) {
                    return Err(PathSecurityError::ForbiddenChar(b));
                }

                if (b == b'.' && prev_b == b' ') || (b == b' ' && prev_b == b'.') {
                    return Err(PathSecurityError::AdjacentSpaceAndDot);
                }

                // NTFS 8.3 short-name alias defense (~ followed by ASCII digit)
                if b == b'~' && i + 1 < seg_len && seg[i + 1].is_ascii_digit() {
                    return Err(PathSecurityError::ShortNameAlias);
                }

                // Trojan Source, BiDi overrides, and Apple HFS+ / Unicode Ignorable Code Points (CVE-2014-9390, CVE-2021-42574)
                if b == 0xC2 && i + 1 < seg_len && seg[i + 1] == 0xAD {
                    return Err(PathSecurityError::TrojanSource);
                } else if b == 0xE2 && i + 2 < seg_len {
                    let b1 = seg[i + 1];
                    let b2 = seg[i + 2];
                    if (b1 == 0x80 && (matches!(b2, 0x8B..=0x8F | 0xAA..=0xAE)))
                        || (b1 == 0x81 && (b2 == 0xA0 || matches!(b2, 0xA6..=0xA9)))
                    {
                        return Err(PathSecurityError::TrojanSource);
                    }
                } else if b == 0xEF && i + 2 < seg_len && seg[i + 1] == 0xBB && seg[i + 2] == 0xBF {
                    return Err(PathSecurityError::TrojanSource);
                }

                if b == b'.' && !has_dot {
                    dot_idx = i;
                    has_dot = true;
                }

                prev_b = b;
                i += 1;
            }

            let stem_len = if has_dot { dot_idx } else { seg_len };
            let (stem_bytes, _) = seg.split_at(stem_len);

            if is_windows_reserved_device_name_bytes(stem_bytes) {
                return Err(PathSecurityError::WindowsReservedDevice);
            }

            if is_ntfs_reserved_metadata(stem_bytes) {
                return Err(PathSecurityError::NtfsMetadata);
            }

            has_normal = true;
            last_is_normal = true;
        }

        remaining = if !rest.is_empty() {
            rest.split_at(1).1
        } else {
            &[]
        };
    }

    if has_normal && last_is_normal {
        Ok(())
    } else {
        Err(PathSecurityError::NoNormalComponent)
    }
}

/// Strictly validates whether a relative path string is safe against directory traversal,
/// platform quirks, and dangerous metadata overwrites.
#[inline]
#[must_use]
pub const fn is_safe_relative_path(path_str: &str) -> bool {
    validate_safe_relative_path(path_str).is_ok()
}

/// Ergonomic runtime validator accepting any path representation (`AsRef<Path>`).
#[inline]
pub fn validate_safe_path<P: AsRef<Path> + ?Sized>(path: &P) -> Result<(), PathSecurityError> {
    match path.as_ref().to_str() {
        Some(s) => validate_safe_relative_path(s),
        None => Err(PathSecurityError::NonUtf8),
    }
}

/// Ergonomic runtime validator accepting any path representation (`AsRef<Path>`).
#[inline]
#[must_use]
pub fn is_safe_path<P: AsRef<Path> + ?Sized>(path: &P) -> bool {
    validate_safe_path(path).is_ok()
}
