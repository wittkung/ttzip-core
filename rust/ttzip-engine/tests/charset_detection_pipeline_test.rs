// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use ttzip_engine::codecs::chardet::detect_charset;

#[test]
fn test_charset_detection_cjk_pipeline() {
    // 1. UTF-8
    let utf8_bytes = "测试文件.txt".as_bytes();
    assert_eq!(detect_charset(utf8_bytes).as_deref(), Some("UTF-8"));

    // 2. GB18030 / GBK bytes for "测试文件" (B2 E2 CA D4 CE C4 BC FE)
    let gbk_bytes: [u8; 8] = [0xB2, 0xE2, 0xCA, 0xD4, 0xCE, 0xC4, 0xBC, 0xFE];
    let detected_gbk = detect_charset(&gbk_bytes).expect("Failed to detect GBK");
    assert!(detected_gbk.to_uppercase().contains("GB"));

    // 3. Shift-JIS bytes for "テスト" (83 65 83 58 83 67)
    let sjis_bytes: [u8; 6] = [0x83, 0x65, 0x83, 0x58, 0x83, 0x67];
    let detected_sjis = detect_charset(&sjis_bytes).expect("Failed to detect Shift-JIS");
    let upper = detected_sjis.to_uppercase();
    assert!(upper.contains("SHIFT") || upper.contains("SJIS") || upper.contains("WINDOWS-31J"));
}
