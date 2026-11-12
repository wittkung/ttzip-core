// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-frequency Japanese character distribution and bigram tables for Shift-JIS (CP932).

const TOP_SHIFT_JIS_KANJI: &[u16] = &[
    0x93FA, // U+65E5
    0x967B, // U+672C
    0x8CEA, // U+8A9E
    0x89E6, // U+753B
    0x919C, // U+50CF
    0x8F91, // U+66F8
    0x97DE, // U+985E
    0x8DEC, // U+4F5C
    0x90AC, // U+6210
    0x944E, // U+5E74
    0x8C8E, // U+6708
    0x938C, // U+6771
    0x8B9E, // U+4EAC
    0x8F6F, // U+51FA
    0x97CD, // U+529B
    0x8F88, // U+5904
    0x979D, // U+7406
    0x8E77, // U+6307
    0x92E8, // U+5B9A
    0x95BD, // U+95ED
    0x8A4A, // U+5F00
    0x8D87, // U+518D
    0x90B6, // U+751F
    0x959F, // U+8868
    0x8E8A, // U+793A
];

#[inline]
fn is_shift_jis_trail_byte(b1: u8) -> bool {
    (0x40..=0x7E).contains(&b1) || (0x80..=0xFC).contains(&b1)
}

/// Returns a frequency weight (0..=100) for a 2-byte Shift-JIS sequence `[b0, b1]`.
pub fn score_shift_jis_2byte(b0: u8, b1: u8) -> u32 {
    let code = ((b0 as u16) << 8) | (b1 as u16);

    // 1. Japanese Hiragana range (0x829F..=0x82F1) or Katakana range (0x8340..=0x8396)
    if (0x829F..=0x82F1).contains(&code) || (0x8340..=0x8396).contains(&code) {
        return 100;
    }

    // 2. Distinctive Japanese Punctuation (Choonpu U+30FC 0x815B, Touten U+3001 0x8141, Kuten U+3002 0x8142, Kagikakko U+300C 0x8175, U+300D 0x8176)
    if matches!(code, 0x815B | 0x8141 | 0x8142 | 0x8175 | 0x8176 | 0x8145) {
        return 80;
    }

    // 3. Ultra-high frequency Kanji in Shift-JIS
    if TOP_SHIFT_JIS_KANJI.contains(&code) {
        return 100;
    }

    if is_shift_jis_trail_byte(b1) {
        // 4. Shift-JIS Level 1 Kanji Zone (0x889F..=0x9872)
        if (0x88..=0x98).contains(&b0) {
            return 70;
        }
        // 5. Shift-JIS Level 2 Kanji Zone (0x989F..=0xEA40, 0xE0..=0xFC)
        if (0xE0..=0xFC).contains(&b0) {
            return 10;
        }
    }

    // 6. General Shift-JIS Punctuation/Symbols (0x8140..=0x81AC)
    if (0x8140..=0x81AC).contains(&code) {
        5
    } else {
        0
    }
}
