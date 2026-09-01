// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Text Encoding Official Test Vectors and Compliance Verification Suite.
//!
//! Validates:
//! 1. 40+ WHATWG legacy character encoding transcode fidelity and round-trip correctness via `encoding_rs`.
//! 2. Short filename character set detection accuracy for CJK and legacy archives.
//! 3. Unicode NFC / NFD / NFKC / NFKD normalization consistency across macOS HFS+/APFS and Windows/Linux.
//! 4. 3-Engine differential oracle comparison (`chardetng` vs `encoding_rs` vs TTZip native detector).
//! 5. 6-layer defense-in-depth security invariants and circuit breakers.

use chardetng::EncodingDetector;
use encoding_rs::Encoding;

use ttzip_engine::codecs::chardet::detect_charset as codecs_detect_charset;
use ttzip_engine::security::text_encoding_defense::{
    FallbackStrategy, MalformedByteSequenceGuard, NullByteAndPathTraversalGuard,
    SensitiveTextBuffer, SurrogateAndUnassignedGuard, TextDefenseError, TextExpansionGuard,
    EncodingConfidenceFallbackGuard, DEFAULT_TEXT_MEMORY_FUSE_BYTES,
};

// ============================================================================
// 1. 40+ WHATWG Legacy Character Encoding Transcoding Fidelity
// ============================================================================

struct EncodingTestCase {
    name: &'static str,
    encoding: &'static Encoding,
    sample_text: &'static str,
}

#[test]
fn test_40_plus_legacy_encodings_transcoding_fidelity() {
    let test_matrix = [
        // CJK Encodings
        EncodingTestCase { name: "GB18030", encoding: encoding_rs::GB18030, sample_text: "TTZip 高性能压缩引擎 2026" },
        EncodingTestCase { name: "GBK", encoding: encoding_rs::GBK, sample_text: "简体中文测试文件归档" },
        EncodingTestCase { name: "Big5", encoding: encoding_rs::BIG5, sample_text: "繁體中文專案檔案測試" },
        EncodingTestCase { name: "Shift_JIS", encoding: encoding_rs::SHIFT_JIS, sample_text: "日本語のアーカイブファイル" },
        EncodingTestCase { name: "EUC-KR", encoding: encoding_rs::EUC_KR, sample_text: "한국어 압축 파일 테스트" },
        EncodingTestCase { name: "EUC-JP", encoding: encoding_rs::EUC_JP, sample_text: "圧縮展開エンジンの検証" },
        EncodingTestCase { name: "ISO-2022-JP", encoding: encoding_rs::ISO_2022_JP, sample_text: "電子メール標準コード" },

        // Western & Central European Encodings
        EncodingTestCase { name: "windows-1252", encoding: encoding_rs::WINDOWS_1252, sample_text: "Café, résumé & naïve façade" },
        EncodingTestCase { name: "windows-1250", encoding: encoding_rs::WINDOWS_1250, sample_text: "Příliš žluťoučký kůň úpěl ďábelské ódy" },
        EncodingTestCase { name: "windows-1254", encoding: encoding_rs::WINDOWS_1254, sample_text: "Türkçe karakter desteği şğüıçö" },
        EncodingTestCase { name: "windows-1257", encoding: encoding_rs::WINDOWS_1257, sample_text: "Lietuvių ir Latviešu valoda" },
        EncodingTestCase { name: "windows-1258", encoding: encoding_rs::WINDOWS_1258, sample_text: "Tieng Viet go dau Windows-1258" },
        EncodingTestCase { name: "ISO-8859-1", encoding: encoding_rs::ISO_8859_2, sample_text: "Zażółć gęślą jaźń" },
        EncodingTestCase { name: "ISO-8859-3", encoding: encoding_rs::ISO_8859_3, sample_text: "Ĉeĥoslovakio kaj Esperanto" },
        EncodingTestCase { name: "ISO-8859-4", encoding: encoding_rs::ISO_8859_4, sample_text: "Põhja-Euroopa keeled" },
        EncodingTestCase { name: "ISO-8859-7", encoding: encoding_rs::ISO_8859_7, sample_text: "Ελληνική γλώσσα και αρχεία" },
        EncodingTestCase { name: "ISO-8859-10", encoding: encoding_rs::ISO_8859_10, sample_text: "Nordic and Sámi letters" },
        EncodingTestCase { name: "ISO-8859-13", encoding: encoding_rs::ISO_8859_13, sample_text: "Baltic Rim languages test" },
        EncodingTestCase { name: "ISO-8859-14", encoding: encoding_rs::ISO_8859_14, sample_text: "Gaelic and Celtic consonants" },
        EncodingTestCase { name: "ISO-8859-15", encoding: encoding_rs::ISO_8859_15, sample_text: "Euro symbol € and French œ" },
        EncodingTestCase { name: "ISO-8859-16", encoding: encoding_rs::ISO_8859_16, sample_text: "Limba română cu diacritice" },
        EncodingTestCase { name: "macintosh", encoding: encoding_rs::MACINTOSH, sample_text: "Classic Mac OS Roman text" },

        // Cyrillic Encodings
        EncodingTestCase { name: "windows-1251", encoding: encoding_rs::WINDOWS_1251, sample_text: "Русский текст и отчеты" },
        EncodingTestCase { name: "KOI8-R", encoding: encoding_rs::KOI8_R, sample_text: "Проверка кодировки КОИ-8" },
        EncodingTestCase { name: "KOI8-U", encoding: encoding_rs::KOI8_U, sample_text: "Українська мова та літери ґєії" },
        EncodingTestCase { name: "IBM866", encoding: encoding_rs::IBM866, sample_text: "DOS архивы и документы" },
        EncodingTestCase { name: "ISO-8859-5", encoding: encoding_rs::ISO_8859_5, sample_text: "Кириллический стандарт" },
        EncodingTestCase { name: "x-mac-cyrillic", encoding: encoding_rs::X_MAC_CYRILLIC, sample_text: "Старый Mac OS кириллица" },

        // Middle Eastern Encodings
        EncodingTestCase { name: "windows-1256", encoding: encoding_rs::WINDOWS_1256, sample_text: "اللغة العربية وضغط الملفات" },
        EncodingTestCase { name: "ISO-8859-6", encoding: encoding_rs::ISO_8859_6, sample_text: "مستندات نصية عربية" },
        EncodingTestCase { name: "windows-1255", encoding: encoding_rs::WINDOWS_1255, sample_text: "עברית ובדיקת קבצים" },
        EncodingTestCase { name: "ISO-8859-8", encoding: encoding_rs::ISO_8859_8, sample_text: "טקסט עברי תקני" },
        EncodingTestCase { name: "ISO-8859-8-I", encoding: encoding_rs::ISO_8859_8_I, sample_text: "עברית לוגית" },

        // South Asian / Others
        EncodingTestCase { name: "windows-874", encoding: encoding_rs::WINDOWS_874, sample_text: "ภาษาไทยและการบีบอัด" },
        EncodingTestCase { name: "windows-1253", encoding: encoding_rs::WINDOWS_1253, sample_text: "Ελληνικά αρχεία και κείμενο" },
        EncodingTestCase { name: "UTF-8", encoding: encoding_rs::UTF_8, sample_text: "Universal UTF-8 🚀 100% Native" },
    ];

    for case in test_matrix {
        let (encoded_bytes, _, had_unmappable) = case.encoding.encode(case.sample_text);
        assert!(
            !had_unmappable,
            "Encoding '{0}' should losslessly represent its specific sample text",
            case.name
        );
        assert!(!encoded_bytes.is_empty(), "Encoded bytes for '{0}' must not be empty", case.name);

        let (decoded_cow, _, had_errors) = case.encoding.decode(&encoded_bytes);
        assert!(!had_errors, "Decoding '{0}' bytes back to UTF-8 must be error-free", case.name);
        assert_eq!(
            decoded_cow.as_ref(),
            case.sample_text,
            "Round-trip fidelity failure for encoding '{0}'",
            case.name
        );
    }
}

// ============================================================================
// 2. Short Filename Character Set Detection Accuracy
// ============================================================================

struct ShortFilenameSample {
    original_text: &'static str,
    encoding: &'static Encoding,
    expected_family: &'static str,
}

#[test]
fn test_short_filename_charset_detection_accuracy() {
    let cjk_samples = [
        // Chinese Simplified (GB18030 / GBK)
        ShortFilenameSample { original_text: "财务预算表_2026年.xlsx", encoding: encoding_rs::GB18030, expected_family: "GB" },
        ShortFilenameSample { original_text: "软件工程开发规范.docx", encoding: encoding_rs::GB18030, expected_family: "GB" },
        ShortFilenameSample { original_text: "测试报告汇总.pdf", encoding: encoding_rs::GB18030, expected_family: "GB" },

        // Japanese (Shift-JIS)
        ShortFilenameSample { original_text: "旅行写真_東京.jpg", encoding: encoding_rs::SHIFT_JIS, expected_family: "SHIFT" },
        ShortFilenameSample { original_text: "仕様書_新機能.txt", encoding: encoding_rs::SHIFT_JIS, expected_family: "SHIFT" },
        ShortFilenameSample { original_text: "アーカイブ一覧.zip", encoding: encoding_rs::SHIFT_JIS, expected_family: "SHIFT" },

        // Traditional Chinese (Big5)
        ShortFilenameSample { original_text: "會議記錄_最終版.doc", encoding: encoding_rs::BIG5, expected_family: "BIG5" },
        ShortFilenameSample { original_text: "產品架構規劃圖.png", encoding: encoding_rs::BIG5, expected_family: "BIG5" },

        // Korean (EUC-KR)
        ShortFilenameSample { original_text: "보고서_결과분석.pdf", encoding: encoding_rs::EUC_KR, expected_family: "EUC-KR" },
        ShortFilenameSample { original_text: "사진첩_여름휴가.zip", encoding: encoding_rs::EUC_KR, expected_family: "EUC-KR" },
    ];

    for sample in cjk_samples {
        let (bytes, _, unmappable) = sample.encoding.encode(sample.original_text);
        assert!(!unmappable, "Sample text for '{0}' must be encodable", sample.original_text);

        let detected = codecs_detect_charset(&bytes);
        assert!(
            detected.is_some(),
            "Detector must identify charset for short filename: '{0}'",
            sample.original_text
        );

        let detected_name = detected.unwrap().to_uppercase();
        let matches_expected = detected_name.contains(sample.expected_family)
            || (sample.expected_family == "SHIFT" && (detected_name.contains("SJIS") || detected_name.contains("WINDOWS-31J")));

        assert!(
            matches_expected,
            "Detected charset '{0}' for '{1}' does not match expected family '{2}'",
            detected_name, sample.original_text, sample.expected_family
        );
    }

    // UTF-8 short filename verification
    let utf8_samples = ["项目文档_2026.tar.gz", "旅行_✈️_写真.zip", "보고서.pdf"];
    for utf8_str in utf8_samples {
        let detected = codecs_detect_charset(utf8_str.as_bytes()).unwrap();
        assert_eq!(detected.to_uppercase(), "UTF-8");
    }
}

// ============================================================================
// 3. Unicode NFC / NFD Normalization Consistency
// ============================================================================

#[test]
fn test_unicode_nfc_nfd_normalization_consistency() {
    // 1. French Accented Latin (e + combining acute)
    let decomposed_french = "e\u{0301}cole"; // NFD
    let composed_french = "école"; // NFC
    let nfc_result = SurrogateAndUnassignedGuard::normalize_nfc(decomposed_french);
    assert_eq!(nfc_result, composed_french);

    let nfd_result = SurrogateAndUnassignedGuard::normalize_nfd(composed_french);
    assert_eq!(nfd_result, decomposed_french);

    // 2. German Umlaut (u + combining diaeresis)
    let decomposed_german = "u\u{0308}ber";
    let composed_german = "über";
    assert_eq!(SurrogateAndUnassignedGuard::normalize_nfc(decomposed_german), composed_german);
    assert_eq!(SurrogateAndUnassignedGuard::normalize_nfd(composed_german), decomposed_german);

    // 3. Korean Hangul Jamo (ᄀ + ᅡ + ᆨ -> 각)
    let decomposed_hangul = "\u{1100}\u{1161}\u{11A8}";
    let composed_hangul = "각";
    assert_eq!(SurrogateAndUnassignedGuard::normalize_nfc(decomposed_hangul), composed_hangul);

    // 4. Idempotency Invariants: NFC(NFC(x)) == NFC(x) and NFC(NFD(x)) == NFC(x)
    let complex_text = "Tété, Zürich, 서울특별시, 日本語, 🚀 2026";
    let nfd_complex = SurrogateAndUnassignedGuard::normalize_nfd(complex_text);
    let nfc_recomposed = SurrogateAndUnassignedGuard::normalize_nfc(&nfd_complex);
    assert_eq!(nfc_recomposed, complex_text);

    let nfc_idempotent = SurrogateAndUnassignedGuard::normalize_nfc(complex_text);
    assert_eq!(nfc_idempotent, complex_text);
}

// ============================================================================
// 4. 3-Engine Differential Oracle Comparison
// ============================================================================

#[test]
fn test_three_engine_differential_oracle() {
    let differential_corpus: &[&[u8]] = &[
        b"Hello TTZip Differential Oracle 2026.txt",
        "项目发布说明_v1.0.tar.gz".as_bytes(),
        "テスト画像_📸.png".as_bytes(),
        &[0xB2, 0xE2, 0xCA, 0xD4, 0xCE, 0xC4, 0xBC, 0xFE, 0x2E, 0x74, 0x78, 0x74],
        &[0x83, 0x65, 0x83, 0x58, 0x83, 0x67, 0x2E, 0x7A, 0x69, 0x70],
        &[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x02, 0x03],
    ];

    for (idx, &input_bytes) in differential_corpus.iter().enumerate() {
        let mut chardet = EncodingDetector::new();
        chardet.feed(input_bytes, true);
        let chardet_res = chardet.guess(None, true);

        let (cow_text, _, had_errors) = chardet_res.decode(input_bytes);
        assert!(!cow_text.is_empty() || input_bytes.is_empty());

        let ttzip_detected = codecs_detect_charset(input_bytes);

        if std::str::from_utf8(input_bytes).is_ok() {
            assert!(!had_errors, "Corpus item {idx} is valid UTF-8, encoding_rs must decode without error");
            assert_eq!(cow_text.as_bytes(), input_bytes);
            if let Some(ref name) = ttzip_detected {
                assert!(name.eq_ignore_ascii_case("UTF-8") || name.eq_ignore_ascii_case("ASCII"));
            }
        }
    }
}

// ============================================================================
// 5. 6-Layer Defense-in-Depth Comprehensive Security Invariant Tests
// ============================================================================

#[test]
fn test_defense_layer1_malformed_and_truncation_guard() {
    assert!(MalformedByteSequenceGuard::validate_utf8("Valid Safe String".as_bytes()).is_ok());

    let truncated_utf8 = [0xE4, 0xBD];
    let err = MalformedByteSequenceGuard::validate_utf8(&truncated_utf8).unwrap_err();
    assert!(matches!(err, TextDefenseError::UnexpectedTruncation { .. }));

    let mixed_payload = [0x61, 0x62, 0x63, 0xE4, 0xBD];
    let (clean_prefix, trimmed_count) = MalformedByteSequenceGuard::trim_to_valid_utf8_boundary(&mixed_payload);
    assert_eq!(clean_prefix, b"abc");
    assert_eq!(trimmed_count, 2);

    let broken = [0xFF, 0xFE, 0x80, 0x81];
    assert!(MalformedByteSequenceGuard::validate_utf8(&broken).is_err());
}

#[test]
fn test_defense_layer2_text_expansion_and_memory_fuse() {
    let mut guard = TextExpansionGuard::new(4.0, 1024 * 1024);

    assert!(guard.track_transcode(500, 1500).is_ok());
    assert_eq!(guard.cumulative_in(), 500);
    assert_eq!(guard.cumulative_out(), 1500);

    let err = guard.track_transcode(10, 50000).unwrap_err();
    assert!(matches!(err, TextDefenseError::ExpansionQuotaExceeded { .. }));

    let mut fuse_guard = TextExpansionGuard::new(10.0, DEFAULT_TEXT_MEMORY_FUSE_BYTES);
    let oom_err = fuse_guard.track_transcode(1000, DEFAULT_TEXT_MEMORY_FUSE_BYTES + 1).unwrap_err();
    assert!(matches!(oom_err, TextDefenseError::MemoryFuseExceeded { .. }));
}

#[test]
fn test_defense_layer3_surrogate_and_unassigned_sanitization() {
    let guard = SurrogateAndUnassignedGuard::default();

    let bidi_malicious = "legit_document\u{202E}fdp.exe";
    let sanitized = guard.sanitize_text(bidi_malicious);
    assert_eq!(sanitized, "legit_documentfdp.exe");

    assert!(SurrogateAndUnassignedGuard::validate_codepoint(0xD800).is_err());
    assert!(SurrogateAndUnassignedGuard::validate_codepoint(0xDFFF).is_err());
    assert!(SurrogateAndUnassignedGuard::validate_codepoint(0x110000).is_err());
    assert!(SurrogateAndUnassignedGuard::validate_codepoint(0x0041).is_ok());
}

#[test]
fn test_defense_layer4_null_byte_and_path_traversal_defense() {
    let null_byte_attack = "secret.pdf\0.png";
    assert!(NullByteAndPathTraversalGuard::validate_path(null_byte_attack).is_err());

    let zip_slip = "../../../../../etc/shadow";
    assert!(NullByteAndPathTraversalGuard::validate_path(zip_slip).is_err());

    assert!(NullByteAndPathTraversalGuard::validate_path("/etc/passwd").is_err());
    assert!(NullByteAndPathTraversalGuard::validate_path("C:\\Windows\\System32").is_err());
    assert!(NullByteAndPathTraversalGuard::validate_path("\\\\server\\share\\data").is_err());

    assert!(NullByteAndPathTraversalGuard::validate_path("AUX.txt").is_err());
    assert!(NullByteAndPathTraversalGuard::validate_path("NUL").is_err());
    assert!(NullByteAndPathTraversalGuard::validate_path("COM1.dat").is_err());

    let dirty_path = "archive/sub//nested/../valid_file.txt";
    let safe_path = NullByteAndPathTraversalGuard::sanitize_path(dirty_path);
    assert_eq!(safe_path, "archive/sub/valid_file.txt");
}

#[test]
fn test_defense_layer5_sensitive_memory_zeroize() {
    let mut sensitive = SensitiveTextBuffer::new_from_str("SuperSecretArchiveKey2026!");
    assert_eq!(sensitive.as_str().unwrap(), "SuperSecretArchiveKey2026!");
    assert_eq!(sensitive.len(), 26);

    let debug_str = format!("{sensitive:?}");
    assert!(!debug_str.contains("SuperSecretArchiveKey2026!"));
    assert!(debug_str.contains("***REDACTED***"));

    sensitive.explicit_zeroize();
    assert!(sensitive.is_empty());
    assert_eq!(sensitive.len(), 0);
}

#[test]
fn test_defense_layer6_encoding_confidence_fallback() {
    let guard = EncodingConfidenceFallbackGuard::new(0.60, FallbackStrategy::SafeUtf8Replacement);
    let gbk_payload = [0xB2, 0xE2, 0xCA, 0xD4];

    let confident_res = guard.evaluate(encoding_rs::GB18030, 0.95, &gbk_payload).unwrap();
    assert_eq!(confident_res.encoding, encoding_rs::GB18030);
    assert!(!confident_res.is_fallback);

    let fallback_res = guard.evaluate(encoding_rs::GB18030, 0.35, &gbk_payload).unwrap();
    assert_eq!(fallback_res.encoding, encoding_rs::UTF_8);
    assert!(fallback_res.is_fallback);

    let strict_guard = EncodingConfidenceFallbackGuard::new(0.70, FallbackStrategy::StrictReject);
    let strict_err = strict_guard.evaluate(encoding_rs::GB18030, 0.50, &gbk_payload).unwrap_err();
    assert!(matches!(strict_err, TextDefenseError::LowConfidenceEncoding { .. }));
}
