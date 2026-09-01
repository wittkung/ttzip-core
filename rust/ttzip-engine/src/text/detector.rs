// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Modern Character Set Detection Engine.
//!
//! Provides hardware-friendly UTF-8/ASCII fast validation paths,
//! tri-gram language model probabilistic scoring via `chardetng`,
//! top candidate ranking, and confidence level evaluation.

use encoding_rs::Encoding;

/// Confidence level of the character set detection result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum ConfidenceLevel {
    /// Ambiguous or insufficient data to make a reliable determination.
    Low,
    /// Plausible match based on language model statistics or partial patterns.
    Medium,
    /// Highly confident match (e.g. valid UTF-8, exact BOM, or decisive statistical dominance).
    High,
}

/// Detailed result of character set detection.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectionResult {
    /// The best guessed encoding.
    pub encoding: &'static Encoding,
    /// Confidence level of the guess.
    pub confidence: ConfidenceLevel,
    /// Whether the input byte stream is valid UTF-8 without loss.
    pub is_utf8_lossless: bool,
    /// Whether the input byte stream consists purely of 7-bit ASCII characters.
    pub is_pure_ascii: bool,
}

/// Candidate encoding evaluation score.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateEncoding {
    /// The candidate encoding reference.
    pub encoding: &'static Encoding,
    /// Canonical encoding name.
    pub name: &'static str,
    /// Heuristic score in range [0.0, 1.0].
    pub score: f32,
    /// Evaluated confidence level for this candidate.
    pub confidence: ConfidenceLevel,
}

/// Modern encoding detector wrapping `chardetng` with fast-path SIMD/state checks.
pub struct TTZipEncodingDetector {
    inner: chardetng::EncodingDetector,
    total_bytes_fed: usize,
    saw_non_ascii: bool,
}

impl Default for TTZipEncodingDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TTZipEncodingDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TTZipEncodingDetector")
            .field("total_bytes_fed", &self.total_bytes_fed)
            .field("saw_non_ascii", &self.saw_non_ascii)
            .finish()
    }
}

impl TTZipEncodingDetector {
    /// Creates a new, empty encoding detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: chardetng::EncodingDetector::new(),
            total_bytes_fed: 0,
            saw_non_ascii: false,
        }
    }

    /// Feeds a chunk of bytes into the detector.
    ///
    /// Returns `true` if the detector has accumulated enough decisive information,
    /// or `false` if more data is welcome.
    pub fn feed(&mut self, data: &[u8], last: bool) -> bool {
        self.total_bytes_fed = self.total_bytes_fed.saturating_add(data.len());
        if !self.saw_non_ascii && !Self::is_pure_ascii(data) {
            self.saw_non_ascii = true;
        }
        self.inner.feed(data, last)
    }

    /// Guesses the best encoding based on accumulated bytes.
    ///
    /// - `tld`: Optional top-level domain hint (e.g. `b"cn"`, `b"jp"`, `b"tw"`).
    /// - `allow_all_utf8`: If true, valid UTF-8 is unconditionally preferred.
    #[must_use]
    pub fn guess(&self, tld: Option<&[u8]>, allow_all_utf8: bool) -> &'static Encoding {
        self.inner.guess(tld, allow_all_utf8)
    }

    /// Guesses the encoding and assesses whether the guess is statistically robust.
    #[must_use]
    pub fn guess_assess(&self, tld: Option<&[u8]>, allow_all_utf8: bool) -> (&'static Encoding, bool) {
        self.inner.guess_assess(tld, allow_all_utf8)
    }

    /// Evaluates the accumulated state into a structured `DetectionResult`.
    #[must_use]
    pub fn evaluate_result(&self, tld: Option<&[u8]>, allow_all_utf8: bool) -> DetectionResult {
        let (encoding, is_confident) = self.inner.guess_assess(tld, allow_all_utf8);
        let confidence = if !self.saw_non_ascii || is_confident {
            ConfidenceLevel::High
        } else if self.total_bytes_fed > 16 {
            ConfidenceLevel::Medium
        } else {
            ConfidenceLevel::Low
        };

        DetectionResult {
            encoding,
            confidence,
            is_utf8_lossless: encoding == encoding_rs::UTF_8,
            is_pure_ascii: !self.saw_non_ascii,
        }
    }

    /// Fast check if slice is strictly 7-bit ASCII.
    #[must_use]
    pub fn is_pure_ascii(data: &[u8]) -> bool {
        // Fast word-at-a-time scanning for non-ASCII high bits
        let mut chunks = data.chunks_exact(8);
        for chunk in &mut chunks {
            let val = u64::from_ne_bytes(match chunk.try_into() {
                Ok(arr) => arr,
                Err(_) => return false,
            });
            if (val & 0x8080_8080_8080_8080) != 0 {
                return false;
            }
        }
        for &b in chunks.remainder() {
            if b >= 0x80 {
                return false;
            }
        }
        true
    }

    /// Fast check if slice is valid UTF-8.
    #[must_use]
    pub fn is_valid_utf8(data: &[u8]) -> bool {
        std::str::from_utf8(data).is_ok()
    }

    /// Detects character encoding of a complete byte slice in a single call.
    #[must_use]
    pub fn detect(data: &[u8]) -> DetectionResult {
        detect_encoding_with_confidence(data, None)
    }

    /// Evaluates and ranks candidate encodings for the given byte slice.
    #[must_use]
    pub fn evaluate_candidates(data: &[u8]) -> Vec<CandidateEncoding> {
        let candidate_list: [(&'static Encoding, &'static str); 7] = [
            (encoding_rs::UTF_8, "UTF-8"),
            (encoding_rs::GB18030, "GB18030"),
            (encoding_rs::SHIFT_JIS, "Shift_JIS"),
            (encoding_rs::BIG5, "Big5"),
            (encoding_rs::EUC_KR, "EUC-KR"),
            (encoding_rs::WINDOWS_1252, "windows-1252"),
            (encoding_rs::ISO_8859_2, "ISO-8859-2"),
        ];

        let mut results = Vec::with_capacity(candidate_list.len());

        for (enc, name) in candidate_list {
            let (cow, had_errors) = enc.decode_without_bom_handling(data);
            let score = if had_errors {
                0.0
            } else {
                calculate_text_naturalness_score(&cow)
            };

            let confidence = if score >= 0.85 {
                ConfidenceLevel::High
            } else if score >= 0.50 {
                ConfidenceLevel::Medium
            } else {
                ConfidenceLevel::Low
            };

            results.push(CandidateEncoding {
                encoding: enc,
                name,
                score,
                confidence,
            });
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

/// Convenience helper to detect character encoding.
#[must_use]
pub fn detect_encoding(data: &[u8]) -> &'static Encoding {
    detect_encoding_with_confidence(data, None).encoding
}

/// Convenience helper to detect character encoding with confidence evaluation.
#[must_use]
pub fn detect_encoding_with_confidence(data: &[u8], tld: Option<&[u8]>) -> DetectionResult {
    // 1. Fast path: Pure ASCII is unconditionally UTF-8 with High confidence
    if TTZipEncodingDetector::is_pure_ascii(data) {
        return DetectionResult {
            encoding: encoding_rs::UTF_8,
            confidence: ConfidenceLevel::High,
            is_utf8_lossless: true,
            is_pure_ascii: true,
        };
    }

    // 2. Fast path: Valid UTF-8 with non-ASCII CJK or Latin characters
    if TTZipEncodingDetector::is_valid_utf8(data) {
        return DetectionResult {
            encoding: encoding_rs::UTF_8,
            confidence: ConfidenceLevel::High,
            is_utf8_lossless: true,
            is_pure_ascii: false,
        };
    }

    // 3. Fallback to chardetng tri-gram language model
    let mut detector = TTZipEncodingDetector::new();
    detector.feed(data, true);
    let (encoding, is_confident) = detector.guess_assess(tld, true);

    let confidence = if is_confident {
        ConfidenceLevel::High
    } else if data.len() >= 16 {
        ConfidenceLevel::Medium
    } else {
        ConfidenceLevel::Low
    };

    DetectionResult {
        encoding,
        confidence,
        is_utf8_lossless: false,
        is_pure_ascii: false,
    }
}

/// Computes a heuristic naturalness score in range [0.0, 1.0].
fn calculate_text_naturalness_score(text: &str) -> f32 {
    if text.is_empty() {
        return 1.0;
    }

    let mut total_chars: usize = 0;
    let mut good_chars: usize = 0;
    let mut bad_chars: usize = 0;

    for ch in text.chars() {
        total_chars = total_chars.saturating_add(1);
        match ch {
            // Standard ASCII printable
            ' '..='~' | '\t' | '\n' | '\r' => {
                good_chars = good_chars.saturating_add(1);
            }
            // CJK Unified Ideographs & Extension A
            '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' => {
                good_chars = good_chars.saturating_add(2);
            }
            // Japanese Hiragana & Katakana
            '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' => {
                good_chars = good_chars.saturating_add(2);
            }
            // Korean Hangul Syllables
            '\u{AC00}'..='\u{D7AF}' => {
                good_chars = good_chars.saturating_add(2);
            }
            // CJK Symbols and Punctuation
            '\u{3000}'..='\u{303F}' | '\u{FF00}'..='\u{FFEF}' => {
                good_chars = good_chars.saturating_add(1);
            }
            // Replacement character U+FFFD or control characters
            '\u{FFFD}' | '\0'..='\u{0008}' | '\u{000B}'..='\u{001F}' | '\u{007F}' => {
                bad_chars = bad_chars.saturating_add(3);
            }
            _ => {
                // Other characters
            }
        }
    }

    let raw_score = (good_chars as f32) / ((total_chars.saturating_add(bad_chars)) as f32);
    raw_score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_fast_path() {
        let ascii = b"Hello, TTZip Archive Architecture!";
        let res = detect_encoding_with_confidence(ascii, None);
        assert_eq!(res.encoding, encoding_rs::UTF_8);
        assert_eq!(res.confidence, ConfidenceLevel::High);
        assert!(res.is_pure_ascii);
        assert!(res.is_utf8_lossless);
    }

    #[test]
    fn test_utf8_cjk_detection() {
        let utf8_cjk = "你好世界，这是一个归档测试。".as_bytes();
        let res = detect_encoding_with_confidence(utf8_cjk, None);
        assert_eq!(res.encoding, encoding_rs::UTF_8);
        assert_eq!(res.confidence, ConfidenceLevel::High);
        assert!(!res.is_pure_ascii);
        assert!(res.is_utf8_lossless);
    }

    #[test]
    fn test_gb18030_detection() {
        // "你好世界" in GBK/GB18030: C4 E3 BA C3 CA C0 BD E7
        let gbk_bytes = [0xC4, 0xE3, 0xBA, 0xC3, 0xCA, 0xC0, 0xBD, 0xE7];
        let res = detect_encoding_with_confidence(&gbk_bytes, Some(b"cn"));
        assert!(res.encoding == encoding_rs::GB18030 || res.encoding == encoding_rs::GBK);
        assert!(!res.is_utf8_lossless);
    }

    #[test]
    fn test_candidate_ranking() {
        let gbk_bytes = [0xC4, 0xE3, 0xBA, 0xC3, 0xCA, 0xC0, 0xBD, 0xE7];
        let candidates = TTZipEncodingDetector::evaluate_candidates(&gbk_bytes);
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].encoding, encoding_rs::GB18030);
        assert!(candidates[0].score > 0.8);
    }
}
