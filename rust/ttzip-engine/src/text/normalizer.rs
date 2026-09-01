// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! macOS Unicode NFC Normalizer, Control Character Filter, and Zip-Slip Path Defense.
//!
//! Provides zero-allocation fast paths for Unicode Normalization Form C (NFC),
//! bidirectional Trojan Source override stripping, control character removal,
//! and defensive path sanitization against Zip-Slip attacks.

use std::borrow::Cow;
use thiserror::Error;
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};

/// Windows DOS reserved device names.
const WINDOWS_RESERVED_DEVICES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    "CLOCK$",
];

/// Errors arising during path sanitization.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum PathSanitizeError {
    /// Path contains zero non-separator/whitespace characters.
    #[error("Path is empty or consists solely of separators")]
    EmptyPath,

    /// Path contains an illegal null byte.
    #[error("Path contains forbidden null byte")]
    NullByteInPath,

    /// Path contains dangerous Zip-Slip directory traversal sequences.
    #[error("Zip-Slip directory traversal attempt detected in component: '{0}'")]
    ZipSlipTraversal(String),
}

/// Unicode normalizer and defensive path sanitization engine.
pub struct UnicodeNormalizer;

impl UnicodeNormalizer {
    /// Checks if a string slice is already in Unicode Normalization Form C (NFC).
    #[must_use]
    pub fn is_nfc(text: &str) -> bool {
        match is_nfc_quick(text.chars()) {
            IsNormalized::Yes => true,
            IsNormalized::No => false,
            IsNormalized::Maybe => {
                let nfc_string: String = text.nfc().collect();
                nfc_string == text
            }
        }
    }

    /// Normalizes text to Unicode NFC (Normalization Form C) with zero allocation when already NFC.
    #[must_use]
    pub fn normalize_nfc<'a>(text: &'a str) -> Cow<'a, str> {
        if Self::is_nfc(text) {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(text.nfc().collect::<String>())
        }
    }

    /// Normalizes text to Unicode NFD (Canonical Decomposition).
    #[must_use]
    pub fn normalize_nfd(text: &str) -> String {
        text.nfd().collect()
    }

    /// Normalizes text to Unicode NFKC (Compatibility Decomposition, Canonical Composition).
    #[must_use]
    pub fn normalize_nfkc(text: &str) -> String {
        text.nfkc().collect()
    }

    /// Normalizes text to Unicode NFKD (Compatibility Decomposition).
    #[must_use]
    pub fn normalize_nfkd(text: &str) -> String {
        text.nfkd().collect()
    }

    /// Cleanses invisible control characters and bidirectional overrides from text.
    ///
    /// Removes ASCII C0 controls (0x00..=0x1F), C1 controls (0x80..=0x9F), DEL (0x7F),
    /// Zero-Width characters (U+200B..U+200D, U+FEFF), and Bidirectional overrides (U+202A..=U+202E, U+2066..=U+2069).
    #[must_use]
    pub fn clean_control_characters(text: &str) -> String {
        let mut clean = String::with_capacity(text.len());
        for ch in text.chars() {
            match ch {
                // Strip C0 control characters and DEL
                '\0'..='\u{001F}' | '\u{007F}' => continue,
                // Strip C1 control characters
                '\u{0080}'..='\u{009F}' => continue,
                // Strip Zero-Width and Invisible formatting characters
                '\u{200B}'..='\u{200F}' | '\u{FEFF}' => continue,
                // Strip Bidirectional override characters (Trojan Source defense)
                '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => continue,
                _ => clean.push(ch),
            }
        }
        clean
    }

    /// Sanitizes an archive entry path against Zip-Slip attacks, backslash confusion,
    /// and control character spoofing, returning a canonical NFC path.
    pub fn sanitize_archive_path(raw_path: &str) -> Result<String, PathSanitizeError> {
        if raw_path.contains('\0') {
            return Err(PathSanitizeError::NullByteInPath);
        }

        // Clean control characters and BIDI overrides
        let cleaned = Self::clean_control_characters(raw_path);
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            return Err(PathSanitizeError::EmptyPath);
        }

        // Normalize backslashes to forward slashes
        let unified_slashes = trimmed.replace('\\', "/");

        // Strip drive letters (e.g. "C:/path" -> "path")
        let without_drive = if unified_slashes.len() >= 2
            && unified_slashes.as_bytes()[1] == b':'
            && (unified_slashes.as_bytes()[0].is_ascii_alphabetic())
        {
            &unified_slashes[2..]
        } else {
            &unified_slashes[..]
        };

        // Split into components and evaluate stack for traversal
        let mut components: Vec<String> = Vec::new();
        let segments = without_drive.split('/');

        for segment in segments {
            let seg_trimmed = segment.trim_matches(|c: char| c.is_whitespace() || c == '.');
            if seg_trimmed.is_empty() {
                // Check if the original segment was a parent directory traversal
                if segment == ".." {
                    return Err(PathSanitizeError::ZipSlipTraversal(segment.to_string()));
                }
                continue;
            }

            if seg_trimmed == ".." || segment == ".." {
                return Err(PathSanitizeError::ZipSlipTraversal(segment.to_string()));
            }

            // Check for Windows reserved device names
            let uppercase_seg = seg_trimmed.to_ascii_uppercase();
            let is_reserved = WINDOWS_RESERVED_DEVICES.contains(&uppercase_seg.as_str());
            let final_seg = if is_reserved {
                format!("_{seg_trimmed}")
            } else {
                seg_trimmed.to_string()
            };

            components.push(final_seg);
        }

        if components.is_empty() {
            return Err(PathSanitizeError::EmptyPath);
        }

        let combined_path = components.join("/");
        // Finally apply Unicode NFC normalization
        let nfc_path = Self::normalize_nfc(&combined_path).into_owned();
        Ok(nfc_path)
    }

    /// Combines Unicode NFC normalization with defensive path sanitization.
    pub fn normalize_and_sanitize_path(path: &str) -> Result<String, PathSanitizeError> {
        Self::sanitize_archive_path(path)
    }
}

/// Convenience helper to normalize text to NFC.
#[must_use]
pub fn normalize_nfc<'a>(text: &'a str) -> Cow<'a, str> {
    UnicodeNormalizer::normalize_nfc(text)
}

/// Convenience helper to normalize text to NFD.
#[must_use]
pub fn normalize_nfd(text: &str) -> String {
    UnicodeNormalizer::normalize_nfd(text)
}

/// Convenience helper to normalize text to NFKC.
#[must_use]
pub fn normalize_nfkc(text: &str) -> String {
    UnicodeNormalizer::normalize_nfkc(text)
}

/// Convenience helper to normalize text to NFKD.
#[must_use]
pub fn normalize_nfkd(text: &str) -> String {
    UnicodeNormalizer::normalize_nfkd(text)
}

/// Convenience helper to normalize and sanitize archive path.
pub fn normalize_and_sanitize_path(path: &str) -> Result<String, PathSanitizeError> {
    UnicodeNormalizer::normalize_and_sanitize_path(path)
}

/// Convenience helper to sanitize an archive path safely.
pub fn sanitize_path(path: &str) -> Result<String, PathSanitizeError> {
    UnicodeNormalizer::sanitize_archive_path(path)
}

/// Convenience helper to clean control characters.
#[must_use]
pub fn clean_control_characters(text: &str) -> String {
    UnicodeNormalizer::clean_control_characters(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nfc_zero_allocation_borrow() {
        let text = "TTZip Standard NFC Text 2026";
        let res = normalize_nfc(text);
        assert!(matches!(res, Cow::Borrowed(_)));
        assert_eq!(res, text);
    }

    #[test]
    fn test_nfd_to_nfc_normalization() {
        // 'é' in NFD is 'e' (U+0065) + combining acute accent (U+0301)
        let nfd_str = "e\u{0301}cole";
        assert!(!UnicodeNormalizer::is_nfc(nfd_str));

        let nfc_str = normalize_nfc(nfd_str);
        assert!(matches!(nfc_str, Cow::Owned(_)));
        assert_eq!(nfc_str, "école");
        assert!(UnicodeNormalizer::is_nfc(&nfc_str));
    }

    #[test]
    fn test_control_character_cleansing() {
        let dirty = "report\0\u{001B}[31m\u{200B}\u{202E}final.pdf";
        let cleaned = clean_control_characters(dirty);
        assert_eq!(cleaned, "report[31mfinal.pdf");
    }

    #[test]
    fn test_zip_slip_traversal_detection() {
        let malicious_path = "foo/../../../../etc/passwd";
        let res = sanitize_path(malicious_path);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), PathSanitizeError::ZipSlipTraversal(_)));
    }

    #[test]
    fn test_path_sanitization_success() {
        let res = sanitize_path("C:\\docs\\photos//e\u{0301}té.jpg. ");
        assert!(res.is_ok());
        let clean = res.unwrap();
        assert_eq!(clean, "docs/photos/été.jpg");
    }

    #[test]
    fn test_windows_reserved_device_sanitization() {
        let path = "archive/AUX/file.txt";
        let res = sanitize_path(path).unwrap();
        assert_eq!(res, "archive/_AUX/file.txt");
    }
}
