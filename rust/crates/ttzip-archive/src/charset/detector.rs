// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Hybrid Charset Sniffing Engine combining SIMD ASCII validation, strict UTF-8 parsing,
//! Coding State Machine (CSM) pruning, and 2-byte CJK Bigram frequency statistics.

use crate::charset::csm::{CharsetKind, CodingStateMachine};
use crate::charset::tables::{
    score_big5_2byte, score_euc_kr_2byte, score_gb18030_2byte, score_shift_jis_2byte,
};
use chardetng::EncodingDetector;

/// Candidate detection score holding metadata and calculated confidence.
#[derive(Debug, Clone)]
struct CandidateScore {
    kind: CharsetKind,
    is_valid_csm: bool,
    final_confidence: f32,
}

struct CjkTracker {
    kind: CharsetKind,
    csm: CodingStateMachine,
    score: u32,
    active: bool,
    scorer: fn(u8, u8) -> u32,
}

impl CjkTracker {
    fn new(kind: CharsetKind, scorer: fn(u8, u8) -> u32) -> Self {
        Self {
            kind,
            csm: CodingStateMachine::new(kind),
            score: 0,
            active: true,
            scorer,
        }
    }

    #[inline]
    fn feed_byte(&mut self, b: u8) {
        if !self.active {
            return;
        }
        if let Some((tok, len)) = self.csm.feed_byte(b) {
            if len == 2 {
                self.score += (self.scorer)(tok[0], tok[1]);
            }
        } else if !self.csm.is_valid() {
            self.active = false;
        }
    }

    fn density(&self) -> f32 {
        let mb = self.csm.multibyte_chars();
        if self.csm.is_valid() && mb > 0 {
            (self.score as f32) / (mb as f32 * 100.0)
        } else {
            0.0
        }
    }

    fn to_candidate(&self, is_chardet: bool) -> CandidateScore {
        CandidateScore {
            kind: self.kind,
            is_valid_csm: self.csm.is_valid(),
            final_confidence: calc_cjk_confidence(
                self.csm.is_valid(),
                self.csm.multibyte_chars(),
                self.score,
                is_chardet,
            ),
        }
    }
}

fn calc_cjk_confidence(is_valid: bool, mb_count: usize, acc_score: u32, is_chardet: bool) -> f32 {
    if !is_valid {
        return 0.0;
    }
    if mb_count == 0 {
        return if is_chardet { 0.5 } else { 0.1 };
    }
    let base_density = (acc_score as f32) / (mb_count as f32 * 100.0);
    // Genuine CJK multi-byte text will have substantial bigram density (>= 0.20).
    // If density is below 0.20, it is random or single-byte CP437 ASCII pair collisions.
    if base_density < 0.20 {
        return 0.0;
    }
    let chardet_boost = if is_chardet { 0.35 } else { 0.0 };
    (base_density * 0.75 + chardet_boost).min(1.0)
}

fn detect_fast_ascii_or_utf8(data: &[u8]) -> Option<(&'static str, f32)> {
    if data.is_empty() || data.is_ascii() {
        return Some(("ASCII", 1.0));
    }
    if data.starts_with(b"\xEF\xBB\xBF") || std::str::from_utf8(data).is_ok() {
        return Some(("UTF-8", 1.0));
    }
    None
}

fn evaluate_csm_trackers(data: &[u8]) -> ([CjkTracker; 4], CodingStateMachine) {
    let mut trackers = [
        CjkTracker::new(CharsetKind::Gb18030, score_gb18030_2byte),
        CjkTracker::new(CharsetKind::ShiftJis, score_shift_jis_2byte),
        CjkTracker::new(CharsetKind::Big5, score_big5_2byte),
        CjkTracker::new(CharsetKind::EucKr, score_euc_kr_2byte),
    ];
    let mut csm_win = CodingStateMachine::new(CharsetKind::Windows1252);

    for &b in data {
        for tracker in &mut trackers {
            tracker.feed_byte(b);
        }
        let _ = csm_win.feed_byte(b);
    }

    (trackers, csm_win)
}

fn find_decisive_cjk_match(trackers: &[CjkTracker; 4]) -> Option<(&'static str, f32)> {
    let mut sorted_cjk = [
        (trackers[0].kind, trackers[0].density()),
        (trackers[1].kind, trackers[1].density()),
        (trackers[2].kind, trackers[2].density()),
        (trackers[3].kind, trackers[3].density()),
    ];
    sorted_cjk.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_wins = sorted_cjk[0].1 > sorted_cjk[1].1 * 1.12 || sorted_cjk[1].1 == 0.0;
    if sorted_cjk[0].1 > 0.15 && top_wins {
        Some((sorted_cjk[0].0.canonical_name(), (sorted_cjk[0].1 * 0.95).min(1.0)))
    } else {
        None
    }
}

fn matches_chardet_kind(kind: CharsetKind, chardet_name: &str) -> bool {
    match kind {
        CharsetKind::Utf8 => chardet_name.eq_ignore_ascii_case("utf-8"),
        CharsetKind::Gb18030 => {
            chardet_name.eq_ignore_ascii_case("gb18030")
                || chardet_name.eq_ignore_ascii_case("gbk")
        }
        CharsetKind::ShiftJis => chardet_name.eq_ignore_ascii_case("shift_jis"),
        CharsetKind::Big5 => chardet_name.eq_ignore_ascii_case("big5"),
        CharsetKind::EucKr => chardet_name.eq_ignore_ascii_case("euc-kr"),
        CharsetKind::Windows1252 => chardet_name.eq_ignore_ascii_case("windows-1252"),
    }
}

/// Detects character set encoding for given raw byte sequence with confidence score [0.0..1.0].
pub fn detect_charset_with_confidence(data: &[u8]) -> (&'static str, f32) {
    // 1. SIMD ASCII & strict UTF-8 fast-path.
    if let Some(fast) = detect_fast_ascii_or_utf8(data) {
        return fast;
    }

    // 2. CSM State Machine Pruning & 2-Byte Bigram Statistical Evaluation
    let (trackers, csm_win) = evaluate_csm_trackers(data);
    if let Some(decisive) = find_decisive_cjk_match(&trackers) {
        return decisive;
    }

    // 3. Chardetng oracle prediction fallback
    let mut chardet = EncodingDetector::new();
    chardet.feed(data, true);
    let chardet_name = chardet.guess(None, true).name();

    let candidates: [CandidateScore; 4] = std::array::from_fn(|i| {
        let is_match = matches_chardet_kind(trackers[i].kind, chardet_name);
        trackers[i].to_candidate(is_match)
    });

    let best_candidate = candidates
        .iter()
        .filter(|c| c.is_valid_csm)
        .max_by(|a, b| a.final_confidence.total_cmp(&b.final_confidence));

    if let Some(best) = best_candidate {
        if best.final_confidence > 0.20 {
            return (best.kind.canonical_name(), best.final_confidence);
        }
    }

    // Fallbacks
    if matches_chardet_kind(CharsetKind::Windows1252, chardet_name) || csm_win.is_valid() {
        ("windows-1252", 0.5)
    } else {
        ("UTF-8", 0.1)
    }
}

/// One-shot charset detection returning canonical charset name string if determined.
pub fn detect_charset(data: &[u8]) -> Option<String> {
    if data.is_empty() {
        return Some("ASCII".to_string());
    }
    let (charset, _conf) = detect_charset_with_confidence(data);
    Some(charset.to_string())
}
