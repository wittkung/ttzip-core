// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-frequency Simplified Chinese character and bigram probability distribution tables for GB18030 / GBK.

const TOP_GB18030_CODES: &[u16] = &[
    0xB5C4, // U+7684
    0xD2BB, // U+4E00
    0xCAD7, // U+662F
    0xB2BB, // U+4E0D
    0xC1CB, // U+4E86
    0xD4DA, // U+5728
    0xC8CB, // U+4EBA
    0xD3D0, // U+6709
    0xCED2, // U+6211
    0xCBFB, // U+4ED6
    0xD5E2, // U+8FD9
    0xD6D0, // U+4E2D
    0xB4F3, // U+5927
    0xC0B4, // U+6765
    0xC9CF, // U+4E0A
    0xB9FA, // U+56FD
    0xB8F6, // U+4E2A
    0xB5BD, // U+5230
    0xCB55, // U+8BF4
    0xC3C7, // U+4EEC
    0xCEAA, // U+4E3A
    0xD7D3, // U+5B50
    0xBACD, // U+548C
    0xC4E3, // U+4F60
    0xBAC3, // U+597D
    0xB2E2, // U+6D4B
    0xCAD4, // U+8BD5
    0xCEC4, // U+6587
    0xBCFE, // U+4EF6
    0xB5B5, // U+6863
    0xD1B9, // U+538B
    0xCBF5, // U+7F29
    0xD7CA, // U+8D44
    0xC1CF, // U+6599
    0xB1A4, // U+5305
    0xBCD0, // U+5939
    0xCDC3, // U+56FE
    0xC6AC, // U+7247
    0xD0C2, // U+65B0
    0xBDA8, // U+5EFA
    0xD2F4, // U+97F3
    0xC6B5, // U+9891
    0xCAD3, // U+89C6
    0xB1ED, // U+8868
    0xB8F1, // U+683C
    0xCCE1, // U+63D0
    0xCAA6, // U+793A
    0xCDB3, // U+7EDF
    0xCEBB, // U+4F4D
    0xC0FA, // U+5386
    0xCAB7, // U+53F2
    0xBFEC, // U+5FEB
    0xBDDD, // U+6377
    0xB7BD, // U+65B9
    0xCABD, // U+5F0F
    0xC4BF, // U+76EE
    0xC2BC, // U+5F55
    0xB8B4, // U+590D
    0xD6C6, // U+5236
    0xB7D6, // U+5206
    0xBEED, // U+5377
    0xBAEC, // U+5408
    0xB2A2, // U+5E76
    0xBEB5, // U+955C
    0xCFF1, // U+50CF
    0xC7FD, // U+9A71
    0xB6AF, // U+52A8
    0xC8ED, // U+8F6F
];

/// Returns a frequency weight (0..=100) for a 2-byte GBK/GB18030 sequence `[b0, b1]`.
pub fn score_gb18030_2byte(b0: u8, b1: u8) -> u32 {
    let code = ((b0 as u16) << 8) | (b1 as u16);

    // 1. Top ultra-high frequency Simplified Chinese characters (Score 100)
    if TOP_GB18030_CODES.contains(&code) {
        return 100;
    }

    if !(0xA1..=0xFE).contains(&b1) {
        return 0;
    }

    // 2. High frequency Common Hanzi Zone in GB2312 (Level 1 Hanzi: 0xB0A1..=0xD7F9)
    if (0xB0..=0xD7).contains(&b0) {
        70
    // 3. Level 2 Hanzi in GB2312: (0xD8..=0xF7, 0xA1..=0xFE)
    } else if (0xD8..=0xF7).contains(&b0) {
        10
    // 4. Extended GBK User Defined / Rare Char Zone: 0x81..0xFE with 0xA1..0xFE
    } else if (0x81..=0xFE).contains(&b0) {
        5
    } else {
        0
    }
}
