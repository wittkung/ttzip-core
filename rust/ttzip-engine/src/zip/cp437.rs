// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Code Page 437 (DOS Latin US / OEM) decoder with zero-allocation ASCII fast-path.
//!
//! Provides high-throughput decoding for legacy ZIP filenames and comments.
//! Pure ASCII byte slices (`0x00..=0x7F`) return zero-copy `Cow::Borrowed(&str)`.
//! High-bit bytes (`0x80..=0xFF`) map to Unicode characters via static table lookup
//! into `Cow::Owned(String)`.

use std::borrow::Cow;

/// Standard CP437 mapping table for high-byte values `0x80..=0xFF`.
static CP437_HIGH_TABLE: [char; 128] = [
    // 0x80 - 0x8F
    '\u{00C7}', '\u{00FC}', '\u{00E9}', '\u{00E2}', '\u{00E4}', '\u{00E0}', '\u{00E5}', '\u{00E7}',
    '\u{00EA}', '\u{00EB}', '\u{00E8}', '\u{00EF}', '\u{00EE}', '\u{00EC}', '\u{00C4}', '\u{00C5}',
    // 0x90 - 0x9F
    '\u{00C9}', '\u{00E6}', '\u{00C6}', '\u{00F4}', '\u{00F6}', '\u{00F2}', '\u{00FB}', '\u{00F9}',
    '\u{00FF}', '\u{00D6}', '\u{00DC}', '\u{00A2}', '\u{00A3}', '\u{00A5}', '\u{20A7}', '\u{0192}',
    // 0xA0 - 0xAF
    '\u{00E1}', '\u{00ED}', '\u{00F3}', '\u{00FA}', '\u{00F1}', '\u{00D1}', '\u{00AA}', '\u{00BA}',
    '\u{00BF}', '\u{2310}', '\u{00AC}', '\u{00BD}', '\u{00BC}', '\u{00A1}', '\u{00AB}', '\u{00BB}',
    // 0xB0 - 0xBF
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255D}', '\u{255C}', '\u{255B}', '\u{2510}',
    // 0xC0 - 0xCF
    '\u{2514}', '\u{2534}', '\u{252C}', '\u{251C}', '\u{2500}', '\u{253C}', '\u{255E}', '\u{255F}',
    '\u{255A}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256C}', '\u{2567}',
    // 0xD0 - 0xDF
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256B}',
    '\u{256A}', '\u{2518}', '\u{250C}', '\u{2588}', '\u{2584}', '\u{258C}', '\u{2590}', '\u{2580}',
    // 0xE0 - 0xEF
    '\u{03B1}', '\u{00DF}', '\u{0393}', '\u{03C0}', '\u{03A3}', '\u{03C3}', '\u{00B5}', '\u{03C4}',
    '\u{03A6}', '\u{0398}', '\u{03A9}', '\u{03B4}', '\u{221E}', '\u{03C6}', '\u{03B5}', '\u{2229}',
    // 0xF0 - 0xFF
    '\u{2261}', '\u{00B1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00F7}', '\u{2248}',
    '\u{00B0}', '\u{2219}', '\u{00B7}', '\u{221A}', '\u{207F}', '\u{00B2}', '\u{25A0}', '\u{00A0}',
];

/// Decodes CP437 byte slice into a UTF-8 string with zero allocation on ASCII-only inputs.
///
/// If `bytes` contains only standard 7-bit ASCII characters (`0x00..=0x7F`), returns `Cow::Borrowed(&str)`.
/// If high-bit bytes (`>= 0x80`) are encountered, converts each byte according to CP437 table
/// and returns `Cow::Owned(String)`.
pub fn decode_cp437(bytes: &[u8]) -> Cow<'_, str> {
    // Fast path: Check if entire slice is ASCII
    let non_ascii_pos = bytes.iter().position(|&b| b >= 0x80);

    match non_ascii_pos {
        None => {
            // All bytes are < 0x80, guaranteed to be valid UTF-8
            // Safe unwrap because all ASCII bytes are valid UTF-8
            let s = std::str::from_utf8(bytes).unwrap_or("");
            Cow::Borrowed(s)
        }
        Some(first_high) => {
            let mut out = String::with_capacity(bytes.len() + 16);
            // Copy pre-validated ASCII prefix
            if first_high > 0 {
                let ascii_prefix = std::str::from_utf8(&bytes[..first_high]).unwrap_or("");
                out.push_str(ascii_prefix);
            }

            // Decode remaining bytes
            for &b in &bytes[first_high..] {
                if b < 0x80 {
                    out.push(b as char);
                } else {
                    let ch = CP437_HIGH_TABLE[(b - 0x80) as usize];
                    out.push(ch);
                }
            }

            Cow::Owned(out)
        }
    }
}

/// Decodes a ZIP filename according to PKZIP specifications, Language Encoding Flag (Bit 11),
/// and smart heuristic character set detection.
///
/// If `is_utf8_flag_set` is true:
/// - Fast path: attempts strict UTF-8 decoding.
/// - If invalid UTF-8 sequences are encountered, gracefully falls back to smart charset sanitization.
///
/// If `is_utf8_flag_set` is false:
/// - Fast path: if valid ASCII or valid UTF-8, returns UTF-8 string directly.
/// - Fallback: legacy non-UTF8 archive (GB18030, Shift-JIS, Big5, EUC-KR, Windows-1252, CP437).
///   Uses `crate::charset::sanitize_filename` to heuristically detect encoding and transcode cleanly.
pub fn decode_zip_filename(bytes: &[u8], is_utf8_flag_set: bool) -> String {
    if is_utf8_flag_set {
        if let Ok(utf8_str) = std::str::from_utf8(bytes) {
            return utf8_str.to_string();
        }
        return crate::charset::sanitize_filename(bytes);
    }

    // When bit 11 is not set, check if it's already valid UTF-8
    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        return utf8_str.to_string();
    }

    // Heuristically detect encoding (GB18030, Shift-JIS, Big5, EUC-KR, Windows-1252, CP437) and sanitize
    crate::charset::sanitize_filename(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cp437_ascii_zero_allocation() {
        let ascii_bytes = b"standard_english_filename_12345.txt";
        let cow = decode_cp437(ascii_bytes);
        assert!(matches!(cow, Cow::Borrowed(_)));
        assert_eq!(cow, "standard_english_filename_12345.txt");
    }

    #[test]
    fn test_cp437_german_umlauts() {
        let german_bytes = b"\x84\x94\x81 \x8e\x99\x9a \xe1";
        let decoded = decode_cp437(german_bytes);
        assert_eq!(decoded, "äöü ÄÖÜ ß");
    }

    #[test]
    fn test_decode_zip_filename_utf8_bit11_set() {
        let utf8_name = "你好_Dokument_2026.pdf".as_bytes();
        assert_eq!(decode_zip_filename(utf8_name, true), "你好_Dokument_2026.pdf");
    }

    #[test]
    fn test_decode_zip_filename_legacy_gbk_without_bit11() {
        let text = "你好测试文件资料包.txt";
        let (encoded_bytes, _, _) = encoding_rs::GB18030.encode(text);
        assert_eq!(decode_zip_filename(&encoded_bytes, false), text);
    }

    #[test]
    fn test_decode_zip_filename_legacy_shift_jis_without_bit11() {
        let text = "日本語テストファイル作成.zip";
        let (encoded_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(text);
        assert_eq!(decode_zip_filename(&encoded_bytes, false), text);
    }

    #[test]
    fn test_decode_zip_filename_legacy_euc_kr_without_bit11() {
        let text = "한국어테스트문서파일.txt";
        let (encoded_bytes, _, _) = encoding_rs::EUC_KR.encode(text);
        assert_eq!(decode_zip_filename(&encoded_bytes, false), text);
    }

    #[test]
    fn test_decode_zip_filename_legacy_big5_without_bit11() {
        let text = "測試繁體中文檔案說明.txt";
        let (encoded_bytes, _, _) = encoding_rs::BIG5.encode(text);
        assert_eq!(decode_zip_filename(&encoded_bytes, false), text);
    }

    #[test]
    fn test_decode_zip_filename_cp437_fallback() {
        let cp437_name = b"M\x81nchen_Gr\x84tz.log";
        assert_eq!(decode_zip_filename(cp437_name, false), "München_Grätz.log");
    }
}
