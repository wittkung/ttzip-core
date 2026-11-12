// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! High-frequency Traditional Chinese character and bigram probability distribution tables for Big5.

/// Returns a frequency weight (0..=100) for a 2-byte Big5 sequence `[b0, b1]`.
pub fn score_big5_2byte(b0: u8, b1: u8) -> u32 {
    let code = ((b0 as u16) << 8) | (b1 as u16);

    // 1. Top ultra-high frequency Traditional Chinese characters in Big5
    match code {
        0xAAF7 // U+7684
        | 0xA440 // U+4E00
        | 0xAC4F // U+662F
        | 0xA4A3 // U+4E0D
        | 0xA446 // U+4E86
        | 0xA662 // U+5728
        | 0xA448 // U+4EBA
        | 0xA6B3 // U+6709
        | 0xA7DA // U+6211
        | 0xA54C // U+4ED6
        | 0xB36F // U+9019
        | 0xAD47 // U+500B
        | 0xADCC // U+5011
        | 0xA4A4 // U+4E2D
        | 0xA8D3 // U+4F86
        | 0xA457 // U+4E0A
        | 0xA46A // U+5927
        | 0xACAB // U+70BA
        | 0xA94D // U+548C
        | 0xB0EA // U+570B
        | 0xA741 // U+4F60
        | 0xA66E // U+597D
        | 0xB4FA // U+6E2C
        | 0xB8D5 // U+8A66
        | 0xA4E5 // U+6587
        | 0xA5F3 // U+4EF6
        | 0xC0C9 // U+6A94
        | 0xC0A3 // U+58D3
        | 0xC159 // U+7E2E
        | 0xBBA1 // U+8AAA
        | 0xB77C // U+6703
        | 0xB8EA // U+8CC7
        | 0xAEA6 // U+6599
        | 0xB9CF // U+5716
        | 0xA4F9 // U+7247
        | 0xBC76 // U+5F71
        | 0xB773 // U+65B0
        | 0xBC57 // U+589E
        | 0xA9FA // U+660E
        | 0xC163 // U+7E41
        | 0xC5E9 // U+9AD4
        | 0xADBB // U+9999
        | 0xB4E4 // U+6E2F
        | 0xA578 // U+53F0
        | 0xC657 // U+7063
        | 0xB16A // U+5C08
        | 0xAEF7 // U+6848
        | 0xA5D8 // U+76EE
        | 0xBFFD // U+9304
        | 0xB8D1 // U+89E3
        => 100,

        // 2. High frequency Common Big5 Characters (0xA440..=0xC67E: Level 1 Common Hanzi)
        _ if (0xA4..=0xC6).contains(&b0) && ((0x40..=0x7E).contains(&b1) || (0xA1..=0xFE).contains(&b1)) => 70,

        // 3. Secondary Big5 Characters (0xC940..=0xF9D5: Level 2 Hanzi)
        _ if (0xC9..=0xF9).contains(&b0) && ((0x40..=0x7E).contains(&b1) || (0xA1..=0xFE).contains(&b1)) => 10,

        // 4. Other Big5 User-Defined / Extended Zone
        _ if (0x81..=0xFE).contains(&b0) && (0xA1..=0xFE).contains(&b1) => 5,

        _ => 0,
    }
}
