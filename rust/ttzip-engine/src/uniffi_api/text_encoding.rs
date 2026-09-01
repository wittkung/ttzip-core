// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Character Encoding & Filename Remediation Scaffolding.
//!
//! Provides typed, memory-safe bindings for universal character set detection,
//! zero-allocation transcoding, legacy CJK codepage remediation (GB18030, Shift-JIS,
//! Big5, EUC-KR, Windows-1252), and batch VFS filename sanitization.

use std::sync::Arc;
use super::types::TTZipError;
use crate::charset::{
    detect_charset_with_confidence, lookup_encoding,
};

/// Strongly-typed character encoding metadata record exposed via UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, Hash, uniffi::Record)]
pub struct UniFFIEncodingInfo {
    /// Canonical identifier (e.g. "UTF-8", "GB18030", "Shift_JIS", "Big5").
    pub name: String,
    /// User-facing descriptive display title with region / script info.
    pub display_name: String,
    /// Standard IANA encoding label string.
    pub iana_name: String,
    /// Whether the encoding belongs to the Unicode standard family.
    pub is_unicode: bool,
    /// Whether the encoding is a CJK (Chinese, Japanese, Korean) multibyte codepage.
    pub is_cjk: bool,
    /// Whether the encoding represents a single-byte (8-bit) legacy codepage.
    pub is_single_byte: bool,
}

/// Result of automated character set sniffing and confidence scoring.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIDetectedEncoding {
    /// Canonical detected character set name.
    pub encoding_name: String,
    /// Statistical confidence score bounded in [0.0..1.0].
    pub confidence: f32,
    /// Whether the payload transcoded into valid UTF-8 without replacement characters.
    pub is_lossless: bool,
    /// UTF-8 decoded text sample preview (capped to 128 characters/bytes).
    pub sample_preview: String,
}

/// Remediation outcome for a filename or string entry.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIRemediationResult {
    /// Original raw filename string (lossy UTF-8 representation).
    pub original_name: String,
    /// Remediated, clean UTF-8 string output.
    pub remediated_name: String,
    /// Encoding applied during transcoding or remediation.
    pub encoding_used: String,
    /// Confidence score of the applied encoding [0.0..1.0].
    pub confidence: f32,
    /// Whether any byte translation or transformation was performed.
    pub was_remediated: bool,
    /// Whether unmapped bytes or replacement characters (U+FFFD) were produced.
    pub has_unmapped_chars: bool,
}

/// Character encoding and filename remediation service exposed via UniFFI.
#[derive(uniffi::Object)]
pub struct UniFFITextEncodingService;

#[uniffi::export]
impl UniFFITextEncodingService {
    /// Creates a new instance of the text encoding service.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    /// Returns a list of all pre-configured standard and legacy encodings supported by TTZip.
    pub fn supported_encodings(&self) -> Vec<UniFFIEncodingInfo> {
        get_all_supported_encodings()
    }

    /// Detects character set encoding for given raw byte sequence with confidence scoring.
    pub fn detect_encoding(&self, data: Vec<u8>) -> UniFFIDetectedEncoding {
        detect_encoding_internal(&data)
    }

    /// Transcodes raw byte sequence to valid UTF-8 String using the specified encoding.
    pub fn transcode_to_utf8(&self, data: Vec<u8>, encoding_name: String) -> Result<String, TTZipError> {
        transcode_to_utf8_internal(&data, &encoding_name)
    }

    /// Transcodes a UTF-8 string into target legacy or Unicode byte sequence.
    pub fn transcode_from_utf8(&self, text: String, encoding_name: String) -> Result<Vec<u8>, TTZipError> {
        transcode_from_utf8_internal(&text, &encoding_name)
    }

    /// Remediates raw filename bytes into clean UTF-8 with automatic sniffing or fallback.
    pub fn remediate_filename(&self, raw_bytes: Vec<u8>, fallback_encoding: Option<String>) -> UniFFIRemediationResult {
        remediate_filename_internal(&raw_bytes, fallback_encoding.as_deref())
    }

    /// Batch remediates a collection of raw filename byte sequences.
    pub fn remediate_filenames_batch(
        &self,
        items: Vec<Vec<u8>>,
        fallback_encoding: Option<String>,
    ) -> Vec<UniFFIRemediationResult> {
        items
            .into_iter()
            .map(|bytes| remediate_filename_internal(&bytes, fallback_encoding.as_deref()))
            .collect()
    }

    /// Attempts to repair mojibake in a UTF-8 string caused by misinterpreting legacy bytes as Windows-1252/Latin-1.
    pub fn remediate_mojibake_utf8(&self, text: String, source_encoding: Option<String>) -> UniFFIRemediationResult {
        remediate_mojibake_internal(&text, source_encoding.as_deref())
    }
}

/// Standalone convenience function for character encoding sniffing.
#[uniffi::export]
pub fn uniffi_detect_encoding(data: Vec<u8>) -> UniFFIDetectedEncoding {
    detect_encoding_internal(&data)
}

/// Standalone convenience function for transcoding raw bytes to UTF-8.
#[uniffi::export]
pub fn uniffi_transcode_to_utf8(data: Vec<u8>, encoding_name: String) -> Result<String, TTZipError> {
    transcode_to_utf8_internal(&data, &encoding_name)
}

/// Standalone convenience function for single filename remediation.
#[uniffi::export]
pub fn uniffi_remediate_filename(raw_bytes: Vec<u8>, fallback_encoding: Option<String>) -> UniFFIRemediationResult {
    remediate_filename_internal(&raw_bytes, fallback_encoding.as_deref())
}

// -----------------------------------------------------------------------------
// Internal Helpers & Implementation Details
// -----------------------------------------------------------------------------

fn get_all_supported_encodings() -> Vec<UniFFIEncodingInfo> {
    vec![
        UniFFIEncodingInfo {
            name: "UTF-8".to_string(),
            display_name: "UTF-8 (Unicode)".to_string(),
            iana_name: "utf-8".to_string(),
            is_unicode: true,
            is_cjk: false,
            is_single_byte: false,
        },
        UniFFIEncodingInfo {
            name: "GB18030".to_string(),
            display_name: "GB18030 / GBK / GB2312 (Simplified Chinese)".to_string(),
            iana_name: "gb18030".to_string(),
            is_unicode: false,
            is_cjk: true,
            is_single_byte: false,
        },
        UniFFIEncodingInfo {
            name: "Big5".to_string(),
            display_name: "Big5 / CP950 (Traditional Chinese)".to_string(),
            iana_name: "big5".to_string(),
            is_unicode: false,
            is_cjk: true,
            is_single_byte: false,
        },
        UniFFIEncodingInfo {
            name: "Shift_JIS".to_string(),
            display_name: "Shift-JIS / CP932 (Japanese)".to_string(),
            iana_name: "shift_jis".to_string(),
            is_unicode: false,
            is_cjk: true,
            is_single_byte: false,
        },
        UniFFIEncodingInfo {
            name: "EUC-KR".to_string(),
            display_name: "EUC-KR / CP949 (Korean)".to_string(),
            iana_name: "euc-kr".to_string(),
            is_unicode: false,
            is_cjk: true,
            is_single_byte: false,
        },
        UniFFIEncodingInfo {
            name: "EUC-JP".to_string(),
            display_name: "EUC-JP (Japanese Unix)".to_string(),
            iana_name: "euc-jp".to_string(),
            is_unicode: false,
            is_cjk: true,
            is_single_byte: false,
        },
        UniFFIEncodingInfo {
            name: "windows-1252".to_string(),
            display_name: "Windows-1252 (Western European)".to_string(),
            iana_name: "windows-1252".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "windows-1251".to_string(),
            display_name: "Windows-1251 (Cyrillic)".to_string(),
            iana_name: "windows-1251".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "windows-1250".to_string(),
            display_name: "Windows-1250 (Central European)".to_string(),
            iana_name: "windows-1250".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "windows-1256".to_string(),
            display_name: "Windows-1256 (Arabic)".to_string(),
            iana_name: "windows-1256".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "windows-1254".to_string(),
            display_name: "Windows-1254 (Turkish)".to_string(),
            iana_name: "windows-1254".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "ISO-8859-1".to_string(),
            display_name: "ISO-8859-1 (Latin-1)".to_string(),
            iana_name: "iso-8859-1".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "ISO-8859-2".to_string(),
            display_name: "ISO-8859-2 (Latin-2 Central European)".to_string(),
            iana_name: "iso-8859-2".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "IBM866".to_string(),
            display_name: "CP866 / IBM866 (DOS Cyrillic)".to_string(),
            iana_name: "ibm866".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "IBM437".to_string(),
            display_name: "CP437 / IBM437 (DOS Latin US)".to_string(),
            iana_name: "ibm437".to_string(),
            is_unicode: false,
            is_cjk: false,
            is_single_byte: true,
        },
        UniFFIEncodingInfo {
            name: "UTF-16LE".to_string(),
            display_name: "UTF-16LE (Unicode Little Endian)".to_string(),
            iana_name: "utf-16le".to_string(),
            is_unicode: true,
            is_cjk: false,
            is_single_byte: false,
        },
        UniFFIEncodingInfo {
            name: "UTF-16BE".to_string(),
            display_name: "UTF-16BE (Unicode Big Endian)".to_string(),
            iana_name: "utf-16be".to_string(),
            is_unicode: true,
            is_cjk: false,
            is_single_byte: false,
        },
    ]
}

fn detect_encoding_internal(data: &[u8]) -> UniFFIDetectedEncoding {
    if data.is_empty() {
        return UniFFIDetectedEncoding {
            encoding_name: "ASCII".to_string(),
            confidence: 1.0,
            is_lossless: true,
            sample_preview: String::new(),
        };
    }

    let (detected_name, conf) = detect_charset_with_confidence(data);
    let sample_slice = if data.len() > 128 { &data[..128] } else { data };
    let encoding = lookup_encoding(&detected_name);
    let (cow, _, had_errors) = encoding.decode(sample_slice);

    UniFFIDetectedEncoding {
        encoding_name: detected_name,
        confidence: conf,
        is_lossless: !had_errors,
        sample_preview: cow.into_owned(),
    }
}

fn transcode_to_utf8_internal(data: &[u8], encoding_name: &str) -> Result<String, TTZipError> {
    if data.is_empty() {
        return Ok(String::new());
    }
    let encoding = lookup_encoding(encoding_name);
    let (cow, _, _) = encoding.decode(data);
    Ok(cow.into_owned())
}

fn transcode_from_utf8_internal(text: &str, encoding_name: &str) -> Result<Vec<u8>, TTZipError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let encoding = lookup_encoding(encoding_name);
    let (cow, _, _) = encoding.encode(text);
    Ok(cow.into_owned())
}

fn remediate_filename_internal(raw_bytes: &[u8], fallback_encoding: Option<&str>) -> UniFFIRemediationResult {
    if raw_bytes.is_empty() {
        return UniFFIRemediationResult {
            original_name: String::new(),
            remediated_name: String::new(),
            encoding_used: "ASCII".to_string(),
            confidence: 1.0,
            was_remediated: false,
            has_unmapped_chars: false,
        };
    }

    let original_lossy = String::from_utf8_lossy(raw_bytes).into_owned();

    // Fast path: Pure ASCII
    if raw_bytes.is_ascii() {
        return UniFFIRemediationResult {
            original_name: original_lossy.clone(),
            remediated_name: original_lossy,
            encoding_used: "ASCII".to_string(),
            confidence: 1.0,
            was_remediated: false,
            has_unmapped_chars: false,
        };
    }

    // Fast path: Strict UTF-8 without explicit legacy override
    if std::str::from_utf8(raw_bytes).is_ok() {
        let is_explicit_legacy = fallback_encoding
            .map(|f| !f.trim().is_empty() && !f.eq_ignore_ascii_case("auto") && !f.eq_ignore_ascii_case("utf-8") && !f.eq_ignore_ascii_case("utf8"))
            .unwrap_or(false);

        if !is_explicit_legacy {
            return UniFFIRemediationResult {
                original_name: original_lossy.clone(),
                remediated_name: original_lossy,
                encoding_used: "UTF-8".to_string(),
                confidence: 1.0,
                was_remediated: false,
                has_unmapped_chars: false,
            };
        }
    }

    // Determine target encoding
    let (encoding_name, conf) = match fallback_encoding {
        Some(fb) if !fb.trim().is_empty() && !fb.eq_ignore_ascii_case("auto") => {
            (fb.to_string(), 1.0)
        }
        _ => detect_charset_with_confidence(raw_bytes),
    };

    let encoding = lookup_encoding(&encoding_name);
    let (cow, _, had_errors) = encoding.decode(raw_bytes);
    let remediated = cow.into_owned();
    let was_remediated = remediated != original_lossy;

    UniFFIRemediationResult {
        original_name: original_lossy,
        remediated_name: remediated,
        encoding_used: encoding_name,
        confidence: conf,
        was_remediated,
        has_unmapped_chars: had_errors,
    }
}

fn remediate_mojibake_internal(text: &str, source_encoding: Option<&str>) -> UniFFIRemediationResult {
    if text.is_empty() {
        return UniFFIRemediationResult {
            original_name: String::new(),
            remediated_name: String::new(),
            encoding_used: "UTF-8".to_string(),
            confidence: 1.0,
            was_remediated: false,
            has_unmapped_chars: false,
        };
    }

    // Re-encode UTF-8 string through Windows-1252 (the typical culprit when legacy 8-bit bytes are misdecoded)
    let (raw_bytes, _, _) = encoding_rs::WINDOWS_1252.encode(text);

    let (encoding_name, conf) = match source_encoding {
        Some(src) if !src.trim().is_empty() && !src.eq_ignore_ascii_case("auto") => {
            (src.to_string(), 1.0)
        }
        _ => detect_charset_with_confidence(&raw_bytes),
    };

    let encoding = lookup_encoding(&encoding_name);
    let (cow, _, had_errors) = encoding.decode(&raw_bytes);
    let remediated = cow.into_owned();
    let was_remediated = remediated != text;

    UniFFIRemediationResult {
        original_name: text.to_string(),
        remediated_name: remediated,
        encoding_used: encoding_name,
        confidence: conf,
        was_remediated,
        has_unmapped_chars: had_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_supported_encodings() {
        let service = UniFFITextEncodingService::new();
        let encodings = service.supported_encodings();
        assert!(!encodings.is_empty());
        assert!(encodings.iter().any(|e| e.name == "UTF-8" && e.is_unicode));
        assert!(encodings.iter().any(|e| e.name == "GB18030" && e.is_cjk));
        assert!(encodings.iter().any(|e| e.name == "Shift_JIS" && e.is_cjk));
        assert!(encodings.iter().any(|e| e.name == "Big5" && e.is_cjk));
        assert!(encodings.iter().any(|e| e.name == "windows-1252" && e.is_single_byte));
    }

    #[test]
    fn test_uniffi_detect_and_transcode_gb18030() {
        let text = "你好，世界！TTZip 字符编码测试资料.txt";
        let (gb_bytes, _, _) = encoding_rs::GB18030.encode(text);

        let detected = uniffi_detect_encoding(gb_bytes.to_vec());
        assert!(
            detected.encoding_name == "GB18030" || detected.encoding_name == "GBK",
            "Expected GB18030/GBK, got {}",
            detected.encoding_name
        );
        assert!(detected.confidence > 0.5);
        assert!(detected.is_lossless);

        let transcoded = uniffi_transcode_to_utf8(gb_bytes.to_vec(), "GB18030".to_string()).unwrap();
        assert_eq!(transcoded, text);
    }

    #[test]
    fn test_uniffi_detect_and_transcode_shift_jis() {
        let text = "日本語のファイル名テスト.zip";
        let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(text);

        let detected = uniffi_detect_encoding(sjis_bytes.to_vec());
        assert_eq!(detected.encoding_name, "Shift_JIS");
        assert!(detected.confidence > 0.5);

        let remediated = uniffi_remediate_filename(sjis_bytes.to_vec(), None);
        assert_eq!(remediated.remediated_name, text);
        assert_eq!(remediated.encoding_used, "Shift_JIS");
        assert!(remediated.was_remediated);
        assert!(!remediated.has_unmapped_chars);
    }

    #[test]
    fn test_uniffi_remediate_filenames_batch() {
        let service = UniFFITextEncodingService::new();
        let (gb_bytes, _, _) = encoding_rs::GB18030.encode("文档1.pdf");
        let (big5_bytes, _, _) = encoding_rs::BIG5.encode("說明2.docx");

        let batch = vec![gb_bytes.to_vec(), big5_bytes.to_vec(), b"plain_ascii.txt".to_vec()];
        let results = service.remediate_filenames_batch(batch, None);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].remediated_name, "文档1.pdf");
        assert_eq!(results[1].remediated_name, "說明2.docx");
        assert_eq!(results[2].remediated_name, "plain_ascii.txt");
        assert!(!results[2].was_remediated);
    }

    #[test]
    fn test_uniffi_mojibake_remediation() {
        let service = UniFFITextEncodingService::new();
        // Simulate a typical mojibake: Chinese GBK text encoded, but decoded as Windows-1252
        let original_text = "中国开源归档引擎";
        let (gb_bytes, _, _) = encoding_rs::GB18030.encode(original_text);
        let (mojibake_utf8, _, _) = encoding_rs::WINDOWS_1252.decode(&gb_bytes);

        let fixed = service.remediate_mojibake_utf8(mojibake_utf8.to_string(), Some("GB18030".to_string()));
        assert_eq!(fixed.remediated_name, original_text);
        assert!(fixed.was_remediated);
    }
}
