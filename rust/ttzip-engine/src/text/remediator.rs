// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Heuristic Mojibake Remediation and Encoding Recovery State Machine.
//!
//! Provides automatic detection and recovery for garbled archive entry filenames,
//! path segments, and archive comments caused by character encoding mismatches
//! (e.g., CP936/GBK/GB18030, Shift_JIS, Big5, EUC-KR, Windows-1252 misinterpretation).

use encoding_rs::Encoding;
use crate::text::detector::{detect_encoding_with_confidence, ConfidenceLevel};
use crate::text::transcoder::TTZipTextTranscoder;

/// Confidence evaluation of the remediation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum RemediationConfidence {
    /// No remediation needed or input was already natural/clean.
    Unchanged,
    /// Plausible recovery based on heuristic text naturalness scores.
    Probable,
    /// Certain recovery with decisive linguistic and structural improvements.
    Certain,
}

/// Detailed result of the mojibake remediation process.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemediationResult {
    /// The original input text before remediation.
    pub original: String,
    /// The remediated (repaired) text, or original if unchanged.
    pub remediated: String,
    /// Guessed original source encoding (e.g., "GB18030", "Shift_JIS", "UTF-8").
    pub detected_source_encoding: Option<&'static str>,
    /// Suspected mistaken decoding scheme (e.g., "windows-1252").
    pub misread_as_encoding: Option<&'static str>,
    /// Confidence level of the remediation.
    pub confidence: RemediationConfidence,
    /// Whether any changes were applied to fix mojibake.
    pub is_remediated: bool,
}

/// Candidate pair for mojibake reverse transcoding.
struct RemediationCandidatePair {
    misread: &'static Encoding,
    target: &'static Encoding,
    misread_name: &'static str,
    target_name: &'static str,
}

/// Heuristic mojibake detector and remediation engine.
pub struct GarbledTextRemediator;

impl GarbledTextRemediator {
    /// List of common mistaken decoding pairs observed in cross-platform archives.
    const HEURISTIC_PAIRS: [RemediationCandidatePair; 8] = [
        // UTF-8 incorrectly decoded as Windows-1252 (Classic Latin-1 mojibake: ä½ å¥½ -> 你好)
        RemediationCandidatePair {
            misread: encoding_rs::WINDOWS_1252,
            target: encoding_rs::UTF_8,
            misread_name: "windows-1252",
            target_name: "UTF-8",
        },
        // GBK/GB18030 incorrectly decoded as Windows-1252 (ÖÐÎÄ -> 中文)
        RemediationCandidatePair {
            misread: encoding_rs::WINDOWS_1252,
            target: encoding_rs::GB18030,
            misread_name: "windows-1252",
            target_name: "GB18030",
        },
        // Shift_JIS incorrectly decoded as Windows-1252 (ƒeƒXƒg -> テスト)
        RemediationCandidatePair {
            misread: encoding_rs::WINDOWS_1252,
            target: encoding_rs::SHIFT_JIS,
            misread_name: "windows-1252",
            target_name: "Shift_JIS",
        },
        // Big5 incorrectly decoded as Windows-1252
        RemediationCandidatePair {
            misread: encoding_rs::WINDOWS_1252,
            target: encoding_rs::BIG5,
            misread_name: "windows-1252",
            target_name: "Big5",
        },
        // EUC-KR incorrectly decoded as Windows-1252 (ÇÑ±¹¾î -> 한국어)
        RemediationCandidatePair {
            misread: encoding_rs::WINDOWS_1252,
            target: encoding_rs::EUC_KR,
            misread_name: "windows-1252",
            target_name: "EUC-KR",
        },
        // UTF-8 incorrectly decoded as ISO-8859-2
        RemediationCandidatePair {
            misread: encoding_rs::ISO_8859_2,
            target: encoding_rs::UTF_8,
            misread_name: "ISO-8859-2",
            target_name: "UTF-8",
        },
        // UTF-8 incorrectly decoded as GB18030
        RemediationCandidatePair {
            misread: encoding_rs::GB18030,
            target: encoding_rs::UTF_8,
            misread_name: "GB18030",
            target_name: "UTF-8",
        },
        // UTF-8 incorrectly decoded as Shift_JIS
        RemediationCandidatePair {
            misread: encoding_rs::SHIFT_JIS,
            target: encoding_rs::UTF_8,
            misread_name: "Shift_JIS",
            target_name: "UTF-8",
        },
    ];

    /// Assesses whether a string is likely to contain mojibake or corrupt character sequences.
    #[must_use]
    pub fn is_likely_garbled(text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        // Check for replacement characters
        if text.contains('\u{FFFD}') {
            return true;
        }

        let mut latin1_supplement_count: usize = 0;
        let mut win1252_artifact_count: usize = 0;
        let mut total_chars: usize = 0;
        let mut ascii_printable_count: usize = 0;
        let mut cjk_count: usize = 0;

        for ch in text.chars() {
            total_chars = total_chars.saturating_add(1);
            match ch {
                ' '..='~' => ascii_printable_count = ascii_printable_count.saturating_add(1),
                '\u{00A0}'..='\u{00FF}' => {
                    latin1_supplement_count = latin1_supplement_count.saturating_add(1);
                }
                // Common Windows-1252 0x80..0x9F mapped artifacts (e.g. ƒ, Š, Œ, Ž, š, œ, ž, Ÿ, …, ‘, ’, “, ”, •, –, —, ™)
                '\u{0192}' | '\u{02C6}' | '\u{02DC}' | '\u{0160}' | '\u{0161}' | '\u{0152}'
                | '\u{0153}' | '\u{017D}' | '\u{017E}' | '\u{0178}' | '\u{2013}'..='\u{2026}'
                | '\u{2030}' | '\u{2039}' | '\u{203A}' | '\u{2122}' => {
                    win1252_artifact_count = win1252_artifact_count.saturating_add(1);
                }
                '\u{4E00}'..='\u{9FFF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{3040}'..='\u{309F}'
                | '\u{30A0}'..='\u{30FF}'
                | '\u{AC00}'..='\u{D7AF}' => {
                    cjk_count = cjk_count.saturating_add(1);
                }
                _ => {}
            }
        }

        if total_chars == 0 || ascii_printable_count == total_chars {
            return false;
        }

        if win1252_artifact_count > 0 && cjk_count == 0 {
            return true;
        }

        // Suspicious high density of Latin-1 supplement symbols without spacing
        let latin1_ratio = (latin1_supplement_count as f32) / (total_chars as f32);
        if latin1_ratio > 0.35 && total_chars >= 2 && cjk_count == 0 {
            return true;
        }

        false
    }

    /// Automatically remediates garbled text by trying candidate reverse-transcoding state transitions.
    #[must_use]
    pub fn remediate_garbled_string(input: &str) -> RemediationResult {
        if input.is_empty() {
            return RemediationResult {
                original: String::new(),
                remediated: String::new(),
                detected_source_encoding: None,
                misread_as_encoding: None,
                confidence: RemediationConfidence::Unchanged,
                is_remediated: false,
            };
        }

        // Fast path: Pure ASCII never needs mojibake remediation
        if input.bytes().all(|b| b < 0x80) {
            return RemediationResult {
                original: input.to_string(),
                remediated: input.to_string(),
                detected_source_encoding: Some("UTF-8"),
                misread_as_encoding: None,
                confidence: RemediationConfidence::Unchanged,
                is_remediated: false,
            };
        }

        let original_score = calculate_naturalness_score(input);
        let mut best_score = original_score;
        let mut best_remediated: Option<String> = None;
        let mut best_misread: Option<&'static str> = None;
        let mut best_source: Option<&'static str> = None;

        for pair in &Self::HEURISTIC_PAIRS {
            if let Some((candidate_str, score)) =
                Self::try_remediation_pair(input, pair.misread, pair.target)
            {
                // Must be strictly better than the original score
                if score > best_score && (score - original_score >= 0.15 || score >= 0.60) {
                    best_score = score;
                    best_remediated = Some(candidate_str);
                    best_misread = Some(pair.misread_name);
                    best_source = Some(pair.target_name);
                }
            }
        }

        if let Some(remediated) = best_remediated {
            let score_gain = best_score - original_score;
            let confidence = if score_gain >= 0.25 || best_score >= 0.80 {
                RemediationConfidence::Certain
            } else {
                RemediationConfidence::Probable
            };

            RemediationResult {
                original: input.to_string(),
                remediated,
                detected_source_encoding: best_source,
                misread_as_encoding: best_misread,
                confidence,
                is_remediated: true,
            }
        } else {
            RemediationResult {
                original: input.to_string(),
                remediated: input.to_string(),
                detected_source_encoding: None,
                misread_as_encoding: None,
                confidence: RemediationConfidence::Unchanged,
                is_remediated: false,
            }
        }
    }

    /// Remediates raw unparsed byte sequences from legacy archive headers.
    #[must_use]
    pub fn remediate_raw_bytes(data: &[u8], tld: Option<&[u8]>) -> RemediationResult {
        if data.is_empty() {
            return RemediationResult {
                original: String::new(),
                remediated: String::new(),
                detected_source_encoding: None,
                misread_as_encoding: None,
                confidence: RemediationConfidence::Unchanged,
                is_remediated: false,
            };
        }

        // Detect encoding
        let detection = detect_encoding_with_confidence(data, tld);
        let (decoded_str, had_errors) = TTZipTextTranscoder::decode_to_utf8(data, detection.encoding);

        let initial_string = decoded_str.into_owned();

        // If detected with High confidence and no errors, check if further remediation is needed
        if detection.confidence == ConfidenceLevel::High && !had_errors && !Self::is_likely_garbled(&initial_string) {
            return RemediationResult {
                original: initial_string.clone(),
                remediated: initial_string,
                detected_source_encoding: Some(detection.encoding.name()),
                misread_as_encoding: None,
                confidence: RemediationConfidence::Certain,
                is_remediated: false,
            };
        }

        // Try heuristic remediation on the decoded string
        let remediation = Self::remediate_garbled_string(&initial_string);
        if remediation.is_remediated {
            remediation
        } else {
            RemediationResult {
                original: initial_string.clone(),
                remediated: initial_string,
                detected_source_encoding: Some(detection.encoding.name()),
                misread_as_encoding: None,
                confidence: match detection.confidence {
                    ConfidenceLevel::High => RemediationConfidence::Certain,
                    ConfidenceLevel::Medium => RemediationConfidence::Probable,
                    ConfidenceLevel::Low => RemediationConfidence::Unchanged,
                },
                is_remediated: false,
            }
        }
    }

    /// Attempts reverse encoding through misread encoding and re-decoding via target encoding.
    fn try_remediation_pair(
        input: &str,
        misread: &'static Encoding,
        target: &'static Encoding,
    ) -> Option<(String, f32)> {
        // Step 1: Re-encode string into bytes using the suspected misread encoding
        let (raw_bytes, _, had_unmappable) = misread.encode(input);
        if had_unmappable || raw_bytes.is_empty() {
            return None;
        }

        // Step 2: Decode raw bytes using the candidate target encoding
        let (decoded_text, _, had_malformed) = target.decode(&raw_bytes);
        if had_malformed {
            return None;
        }

        // Step 3: Reject if string is unchanged or contains replacement character
        if decoded_text == input || decoded_text.contains('\u{FFFD}') {
            return None;
        }

        let score = calculate_naturalness_score(&decoded_text);
        Some((decoded_text.into_owned(), score))
    }
}

/// Evaluates the linguistic naturalness of a candidate string [0.0, 1.0].
fn calculate_naturalness_score(text: &str) -> f32 {
    if text.is_empty() {
        return 1.0;
    }

    let mut total_chars: usize = 0;
    let mut good_weight: usize = 0;
    let mut bad_weight: usize = 0;

    let mut hanzi_count: usize = 0;
    let mut hiragana_count: usize = 0;
    let mut katakana_count: usize = 0;
    let mut hangul_count: usize = 0;
    let mut latin_supp_count: usize = 0;

    for ch in text.chars() {
        total_chars = total_chars.saturating_add(1);
        match ch {
            // Japanese Hiragana
            '\u{3040}'..='\u{309F}' => {
                hiragana_count = hiragana_count.saturating_add(1);
                good_weight = good_weight.saturating_add(6);
            }
            // Japanese Katakana
            '\u{30A0}'..='\u{30FF}' => {
                katakana_count = katakana_count.saturating_add(1);
                good_weight = good_weight.saturating_add(6);
            }
            // Korean Hangul Syllables
            '\u{AC00}'..='\u{D7AF}' => {
                hangul_count = hangul_count.saturating_add(1);
                good_weight = good_weight.saturating_add(6);
            }
            // Common CJK Unified Ideographs
            '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' => {
                hanzi_count = hanzi_count.saturating_add(1);
                good_weight = good_weight.saturating_add(5);
            }
            // Fullwidth punctuation and CJK symbols
            '\u{3000}'..='\u{303F}' | '\u{FF00}'..='\u{FFEF}' => {
                good_weight = good_weight.saturating_add(4);
            }
            // Standard ASCII printable & common path/punctuation characters
            ' '..='~' | '\t' | '\n' | '\r' => {
                good_weight = good_weight.saturating_add(3);
            }
            // Latin-1 Supplement (accented letters)
            '\u{00A0}'..='\u{00FF}' => {
                latin_supp_count = latin_supp_count.saturating_add(1);
                good_weight = good_weight.saturating_add(1);
            }
            // Windows-1252 0x80..0x9F mapped artifacts (often mojibake when single chars in filenames)
            '\u{0192}' | '\u{02C6}' | '\u{02DC}' | '\u{0160}' | '\u{0161}' | '\u{0152}'
            | '\u{0153}' | '\u{017D}' | '\u{017E}' | '\u{0178}' | '\u{2013}'..='\u{2026}'
            | '\u{2030}' | '\u{2039}' | '\u{203A}' | '\u{2122}' => {
                bad_weight = bad_weight.saturating_add(4);
            }
            // Latin Extended-A / B (accented European characters in proper words)
            '\u{0100}'..='\u{024F}' => {
                good_weight = good_weight.saturating_add(1);
            }
            // Control characters, unprintable, or replacement characters
            '\u{FFFD}' | '\0'..='\u{0008}' | '\u{000B}'..='\u{001F}' | '\u{007F}' => {
                bad_weight = bad_weight.saturating_add(8);
            }
            // Unassigned/Private Use Areas
            '\u{E000}'..='\u{F8FF}' => {
                bad_weight = bad_weight.saturating_add(6);
            }
            _ => {}
        }
    }

    if total_chars == 0 {
        return 1.0;
    }

    // Linguistic consistency penalty:
    // 1. Isolated single Katakana character mixed inside predominantly Chinese Hanzi (classic GBK misdecode)
    if hanzi_count >= 2 && katakana_count == 1 && hiragana_count == 0 {
        bad_weight = bad_weight.saturating_add(12);
    }

    // 2. Isolated Hangul character mixed inside predominantly Chinese Hanzi (classic EUC-KR misdecode)
    if hanzi_count >= 2 && hangul_count == 1 {
        bad_weight = bad_weight.saturating_add(12);
    }

    // 3. Dense Latin-1 supplement without ASCII spacing/vowels (classic Latin-1 mojibake)
    if latin_supp_count >= 2 && hanzi_count == 0 && hiragana_count == 0 && katakana_count == 0 && hangul_count == 0 {
        let latin_ratio = (latin_supp_count as f32) / (total_chars as f32);
        if latin_ratio > 0.40 {
            bad_weight = bad_weight.saturating_add(10);
        }
    }

    let max_possible = total_chars.saturating_mul(6);
    let denominator = (max_possible.saturating_add(bad_weight)) as f32;
    let score = (good_weight as f32) / denominator;
    score.clamp(0.0, 1.0)
}

/// Convenience helper to remediate a garbled string.
#[must_use]
pub fn remediate_text(input: &str) -> RemediationResult {
    GarbledTextRemediator::remediate_garbled_string(input)
}

/// Convenience helper to remediate raw filename bytes.
#[must_use]
pub fn remediate_filename_bytes(bytes: &[u8], tld: Option<&[u8]>) -> RemediationResult {
    GarbledTextRemediator::remediate_raw_bytes(bytes, tld)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_ascii_remains_unchanged() {
        let text = "documents/reports/financial_2026.pdf";
        let res = remediate_text(text);
        assert!(!res.is_remediated);
        assert_eq!(res.confidence, RemediationConfidence::Unchanged);
        assert_eq!(res.remediated, text);
    }

    #[test]
    fn test_clean_cjk_remains_unchanged() {
        let text = "归档工程/架构设计说明书.docx";
        let res = remediate_text(text);
        assert!(!res.is_remediated);
        assert_eq!(res.confidence, RemediationConfidence::Unchanged);
        assert_eq!(res.remediated, text);
    }

    #[test]
    fn test_utf8_misread_as_windows1252_remediation() {
        // "你好世界" encoded in UTF-8: E4 BD A0 E5 A5 BD E4 B8 96 E7 95 8C
        // When mistakenly decoded as Windows-1252, it gives "ä½\u{00A0}å¥½ä¸\u{0096}ç\u{0095}\u{008C}" or similar
        let original = "你好世界";
        let (raw_utf8, _, _) = encoding_rs::UTF_8.encode(original);
        let (garbled_str, _, _) = encoding_rs::WINDOWS_1252.decode(&raw_utf8);

        let res = remediate_text(&garbled_str);
        assert!(res.is_remediated);
        assert_eq!(res.remediated, original);
        assert_eq!(res.detected_source_encoding, Some("UTF-8"));
        assert_eq!(res.misread_as_encoding, Some("windows-1252"));
    }

    #[test]
    fn test_gbk_misread_as_windows1252_remediation() {
        // "中文测试" in GBK: D6 D0 CE C4 B2 E2 CA D4
        // Decoded as Windows-1252: "ÖÐÎÄ²âÊÔ"
        let original = "中文测试";
        let (gbk_bytes, _, _) = encoding_rs::GB18030.encode(original);
        let (garbled_str, _, _) = encoding_rs::WINDOWS_1252.decode(&gbk_bytes);

        let res = remediate_text(&garbled_str);
        assert!(res.is_remediated);
        assert_eq!(res.remediated, original);
        assert_eq!(res.detected_source_encoding, Some("GB18030"));
        assert_eq!(res.misread_as_encoding, Some("windows-1252"));
    }

    #[test]
    fn test_shift_jis_misread_as_windows1252_remediation() {
        // "テスト" in Shift_JIS: 83 65 83 58 83 67
        // Decoded as Windows-1252: "ƒeƒXƒg"
        let original = "テスト";
        let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(original);
        let (garbled_str, _, _) = encoding_rs::WINDOWS_1252.decode(&sjis_bytes);

        let res = remediate_text(&garbled_str);
        assert!(res.is_remediated);
        assert_eq!(res.remediated, original);
        assert_eq!(res.detected_source_encoding, Some("Shift_JIS"));
    }

    #[test]
    fn test_raw_gbk_bytes_remediation() {
        let gbk_bytes = [0xD6, 0xD0, 0xCE, 0xC4, 0xB2, 0xE2, 0xCA, 0xD4];
        let res = remediate_filename_bytes(&gbk_bytes, Some(b"cn"));
        assert_eq!(res.remediated, "中文测试");
    }
}
