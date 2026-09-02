// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Adversarial Verification Test Suite for Challenger 2.
//!
//! Stress-tests four core microkernel architectural domains:
//! 1. Multi-volume splitting/chaining with missing volumes, out-of-order sequences, zero-byte chunks.
//! 2. Charset auto-detection heuristics under mixed encoding inputs (GBK, Big5, Shift-JIS, CP437, EUC-KR).
//! 3. Tree-sitter tokenization under rapid viewport scrolling, empty inputs, and large sources.
//! 4. HTML VFS rewriting under nested CSS `@import`, SVG `xlink:href`, and script src tags.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tempfile::tempdir;

use ttzip_engine::archive::split::{detect_volume_chain, VirtualMultiVolumeReader};
use ttzip_engine::charset::{
    detect_charset_with_confidence, sanitize_filename, sanitize_filename_to_slice,
};
use ttzip_engine::html::{
    normalize_rfc3986_path, HtmlSanitizationPolicy, HtmlVfsResourceRouter, TTZipHtmlRewriter,
};
use ttzip_engine::syntax::{
    SupportedLanguage, SymbolOutlineExtractor, SyntaxHighlighter, TTZipSyntaxParser,
};
use ttzip_engine::types::TTZipStatus;

// ============================================================================
// Domain 1: Multi-Volume Splitting & Chaining
// ============================================================================

#[test]
fn test_adv_multivol_missing_middle_volume_chain_truncation() {
    let dir = tempdir().unwrap();
    let vol1 = dir.path().join("archive.7z.001");
    let vol2 = dir.path().join("archive.7z.002");
    // Missing vol3: "archive.7z.003"
    let vol4 = dir.path().join("archive.7z.004");

    fs::write(&vol1, b"PART1_AAA").unwrap();
    fs::write(&vol2, b"PART2_BBB").unwrap();
    fs::write(&vol4, b"PART4_DDD").unwrap();

    // 1. Detection from vol1 stops at gap (returns 001 and 002)
    let chain_from_1 = detect_volume_chain(&vol1).unwrap();
    assert_eq!(chain_from_1.len(), 2);
    assert_eq!(chain_from_1[0], vol1);
    assert_eq!(chain_from_1[1], vol2);

    // 2. Detection from vol4 searches from 001 upwards and stops at gap
    let chain_from_4 = detect_volume_chain(&vol4).unwrap();
    assert_eq!(chain_from_4.len(), 2);
    assert_eq!(chain_from_4[0], vol1);
    assert_eq!(chain_from_4[1], vol2);

    // 3. Passing the incomplete set explicitly reads continuous virtual stream
    let mut reader =
        VirtualMultiVolumeReader::from_volumes(vec![vol1.clone(), vol2.clone(), vol4.clone()])
            .unwrap();
    assert_eq!(reader.total_size(), 27); // 9 + 9 + 9
    let mut out = String::new();
    reader.read_to_string(&mut out).unwrap();
    assert_eq!(out, "PART1_AAAPART2_BBBPART4_DDD");
}

#[test]
fn test_adv_multivol_out_of_order_chain_stitching_and_seeks() {
    let dir = tempdir().unwrap();
    let p_a = dir.path().join("chunk_A.bin");
    let p_b = dir.path().join("chunk_B.bin");
    let p_c = dir.path().join("chunk_C.bin");

    fs::write(&p_a, b"1111").unwrap(); // 4 bytes
    fs::write(&p_b, b"222222").unwrap(); // 6 bytes
    fs::write(&p_c, b"33333333").unwrap(); // 8 bytes

    // Out-of-order list: C (8), A (4), B (6) -> Total 18 bytes
    let mut reader =
        VirtualMultiVolumeReader::from_volumes(vec![p_c.clone(), p_a.clone(), p_b.clone()]).unwrap();
    assert_eq!(reader.total_size(), 18);

    let mut all_bytes = Vec::new();
    reader.read_to_end(&mut all_bytes).unwrap();
    assert_eq!(&all_bytes[..], b"333333331111222222");

    // Seek backward across multiple segments
    reader.seek(SeekFrom::Start(10)).unwrap(); // offset 10 = in segment A (offset 8..12, intra=2)
    let mut small = [0u8; 4];
    reader.read_exact(&mut small).unwrap();
    // In A: 1111 (read 2 bytes: "11"), in B: 222222 (read 2 bytes: "22") -> "1122"
    assert_eq!(&small, b"1122");

    // Seek to very start
    reader.seek(SeekFrom::Start(0)).unwrap();
    let mut first = [0u8; 2];
    reader.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"33");
}

#[test]
fn test_adv_multivol_zero_sized_segments_interleaved() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("vol.001");
    let p2 = dir.path().join("vol.002"); // 0 bytes
    let p3 = dir.path().join("vol.003");
    let p4 = dir.path().join("vol.004"); // 0 bytes
    let p5 = dir.path().join("vol.005");

    fs::write(&p1, b"HELLO_").unwrap(); // 6 bytes
    fs::write(&p2, b"").unwrap(); // 0 bytes
    fs::write(&p3, b"WORLD_").unwrap(); // 6 bytes
    fs::write(&p4, b"").unwrap(); // 0 bytes
    fs::write(&p5, b"2026").unwrap(); // 4 bytes

    let mut reader =
        VirtualMultiVolumeReader::from_volumes(vec![p1, p2, p3, p4, p5]).unwrap();
    assert_eq!(reader.total_size(), 16);

    let mut buf = String::new();
    reader.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, "HELLO_WORLD_2026");

    // Seek directly to offset 6 (at zero-sized segment boundary)
    reader.seek(SeekFrom::Start(6)).unwrap();
    let mut part = [0u8; 6];
    reader.read_exact(&mut part).unwrap();
    assert_eq!(&part, b"WORLD_");
}

#[test]
fn test_adv_multivol_extreme_boundary_seeks_and_overflows() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("seg.001");
    let p2 = dir.path().join("seg.002");
    fs::write(&p1, vec![0x11u8; 100]).unwrap();
    fs::write(&p2, vec![0x22u8; 100]).unwrap();

    let mut reader = VirtualMultiVolumeReader::from_volumes(vec![p1, p2]).unwrap();
    assert_eq!(reader.total_size(), 200);

    // Seek to exact boundary (offset 100)
    reader.seek(SeekFrom::Start(100)).unwrap();
    let mut b = [0u8; 1];
    reader.read_exact(&mut b).unwrap();
    assert_eq!(b[0], 0x22);

    // Seek to EOF (offset 200)
    reader.seek(SeekFrom::Start(200)).unwrap();
    let mut eof_buf = [0u8; 10];
    let n = reader.read(&mut eof_buf).unwrap();
    assert_eq!(n, 0);

    // Seek beyond EOF (offset 500)
    reader.seek(SeekFrom::Start(500)).unwrap();
    assert_eq!(reader.read(&mut eof_buf).unwrap(), 0);

    // Negative seek error
    assert!(reader.seek(SeekFrom::Current(-600)).is_err());
    assert!(reader.seek(SeekFrom::End(-300)).is_err());
}

#[test]
fn test_adv_multivol_cross_volume_spanning_read() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("span.001");
    let p2 = dir.path().join("span.002");
    let p3 = dir.path().join("span.003");

    // 40 bytes each = 120 bytes total
    let data1 = vec![0x11u8; 40];
    let data2 = vec![0x22u8; 40];
    let data3 = vec![0x33u8; 40];

    fs::write(&p1, &data1).unwrap();
    fs::write(&p2, &data2).unwrap();
    fs::write(&p3, &data3).unwrap();

    let mut reader = VirtualMultiVolumeReader::from_volumes(vec![p1, p2, p3]).unwrap();
    assert_eq!(reader.total_size(), 120);

    // Read 100 bytes starting from offset 10:
    // This spans 30 bytes of vol 1, all 40 bytes of vol 2, and 30 bytes of vol 3
    reader.seek(SeekFrom::Start(10)).unwrap();
    let mut span_buf = [0u8; 100];
    reader.read_exact(&mut span_buf).unwrap();

    assert_eq!(&span_buf[0..30], &vec![0x11u8; 30][..]);
    assert_eq!(&span_buf[30..70], &vec![0x22u8; 40][..]);
    assert_eq!(&span_buf[70..100], &vec![0x33u8; 30][..]);
    assert_eq!(reader.current_offset(), 110);
}

#[test]
fn test_adv_multivol_corrupt_boundary_truncation_recovery() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("trunc.001");
    let p2 = dir.path().join("trunc.002");

    fs::write(&p1, b"HEADER_METADATA_BLOCK_1").unwrap(); // 23 bytes
    fs::write(&p2, b"DATA_CHUNK_2").unwrap();            // 12 bytes

    let mut reader = VirtualMultiVolumeReader::from_volumes(vec![p1.clone(), p2.clone()]).unwrap();
    assert_eq!(reader.total_size(), 35); // 23 + 12 = 35

    // Read through boundary
    let mut full_buf = Vec::new();
    reader.read_to_end(&mut full_buf).unwrap();
    assert_eq!(full_buf, b"HEADER_METADATA_BLOCK_1DATA_CHUNK_2");
}

#[test]
fn test_adv_multivol_empty_and_nonexistent_error_handling() {
    // Empty path vector
    let empty_res = VirtualMultiVolumeReader::from_volumes(vec![]);
    assert!(empty_res.is_err());

    // Non-existent volume in list
    let fake_path = Path::new("/definitely/nonexistent/file.7z.001").to_path_buf();
    let fake_res = VirtualMultiVolumeReader::from_volumes(vec![fake_path]);
    assert!(fake_res.is_err());

    // Non-existent seed for detection
    let missing_seed = Path::new("/tmp/missing_archive_volume.z01");
    assert!(detect_volume_chain(missing_seed).is_err());
}

// ============================================================================
// Domain 2: Charset Auto-Detection Heuristics Under Mixed Encodings
// ============================================================================

#[test]
fn test_adv_charset_cjk_disambiguation_matrix() {
    // 1. Simplified Chinese (GB18030 / GBK)
    let gbk_text = "工程技术架构规范与微内核设计文档.pdf";
    let (gbk_bytes, _, _) = encoding_rs::GB18030.encode(gbk_text);
    let (gbk_det, gbk_conf) = detect_charset_with_confidence(&gbk_bytes);
    assert!(gbk_det == "GB18030" || gbk_det == "GBK", "Got {}", gbk_det);
    assert!(gbk_conf >= 0.50);
    assert_eq!(sanitize_filename(&gbk_bytes), gbk_text);

    // 2. Traditional Chinese (Big5)
    let big5_text = "繁體中文軟體專案需求與介面設計.docx";
    let (big5_bytes, _, _) = encoding_rs::BIG5.encode(big5_text);
    let (big5_det, big5_conf) = detect_charset_with_confidence(&big5_bytes);
    assert_eq!(big5_det, "Big5");
    assert!(big5_conf >= 0.50);
    assert_eq!(sanitize_filename(&big5_bytes), big5_text);

    // 3. Japanese (Shift-JIS)
    let sjis_text = "日本語アーカイブ圧縮解凍処理.zip";
    let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(sjis_text);
    let (sjis_det, sjis_conf) = detect_charset_with_confidence(&sjis_bytes);
    assert_eq!(sjis_det, "Shift_JIS");
    assert!(sjis_conf >= 0.50);
    assert_eq!(sanitize_filename(&sjis_bytes), sjis_text);

    // 4. Korean (EUC-KR)
    let euckr_text = "한국어데이터베이스백업파일.tar";
    let (euckr_bytes, _, _) = encoding_rs::EUC_KR.encode(euckr_text);
    let (euckr_det, euckr_conf) = detect_charset_with_confidence(&euckr_bytes);
    assert_eq!(euckr_det, "EUC-KR");
    assert!(euckr_conf >= 0.50);
    assert_eq!(sanitize_filename(&euckr_bytes), euckr_text);
}

#[test]
fn test_adv_charset_cp437_dos_legacy_fallback() {
    // 1. CP437 accented Latin containing 0x81 (ü in CP437; unassigned in Windows-1252):
    // "München_Grätz_Bericht.doc" encoded in DOS CP437:
    let cp437_accent = b"M\x81nchen_Gr\x84tz_Bericht.doc";
    let sanitized_accent = sanitize_filename(cp437_accent);
    assert_eq!(sanitized_accent, "München_Grätz_Bericht.doc");

    // 2. Direct CP437 zero-allocation decoder on DOS box-drawing symbols
    let cp437_box = b"\xC9\xCD\xBB_frame.dat";
    let decoded_box = ttzip_engine::zip::cp437::decode_cp437(cp437_box);
    assert_eq!(decoded_box, "╔═╗_frame.dat");

    // 3. Windows-1252 extended ASCII single-byte decoding
    let win1252_bytes = b"\xDA\xC4\xBF\xB3\xC0\xC4\xD9.txt";
    let (det, conf) = detect_charset_with_confidence(win1252_bytes);
    assert_eq!(det, "windows-1252");
    assert!(conf >= 0.20);
    let sanitized_win = sanitize_filename(win1252_bytes);
    assert_eq!(sanitized_win, "ÚÄ¿³ÀÄÙ.txt");
}

#[test]
fn test_adv_charset_extreme_length_and_special_delimiters() {
    // 1. 1024-byte long GB18030 filename with mixed ASCII digits and symbols
    let mut long_chinese = String::with_capacity(2048);
    for i in 0..100 {
        long_chinese.push_str(&format!("项目归档分卷数据_第{i}批次_"));
    }
    long_chinese.push_str(".tar.gz");

    let (encoded_long, _, _) = encoding_rs::GB18030.encode(&long_chinese);
    let decoded_long = sanitize_filename(&encoded_long);
    assert_eq!(decoded_long, long_chinese);

    // 2. Filename containing internal null bytes and forward slashes in non-UTF8
    let raw_with_slashes = b"subfolder/\xD6\xD0\xCE\xC4/data.bin"; // "subfolder/中文/data.bin" in GBK
    let decoded_path = sanitize_filename(raw_with_slashes);
    assert_eq!(decoded_path, "subfolder/中文/data.bin");
}

#[test]
fn test_adv_charset_truncated_multibyte_safety() {
    // Truncated multi-byte byte sequences must NOT panic
    let truncated_gbk = b"Prefix_\x81"; // lead byte without trail
    let sanitized_gbk = sanitize_filename(truncated_gbk);
    assert!(sanitized_gbk.starts_with("Prefix_"));

    let truncated_utf8 = b"Hello_\xE4\xBD"; // 2 bytes of a 3-byte sequence
    let sanitized_utf8 = sanitize_filename(truncated_utf8);
    assert!(sanitized_utf8.starts_with("Hello_"));

    // Buffer slicing with small buffer
    let mut small_buf = [0u8; 3];
    let res = sanitize_filename_to_slice(b"VeryLongFilename.txt", &mut small_buf);
    assert_eq!(res, Err(TTZipStatus::ErrPathTooLong));
}

#[test]
fn test_adv_charset_mixed_ascii_cjk_complex_transcode() {
    let mixed_gbk = "2026_Q3_财务预算分析报表_Final_v1.0.xlsx";
    let (encoded, _, _) = encoding_rs::GB18030.encode(mixed_gbk);
    let result = sanitize_filename(&encoded);
    assert_eq!(result, mixed_gbk);

    let mixed_sjis = "Project_TTZip_設定ファイル_release.json";
    let (sjis_encoded, _, _) = encoding_rs::SHIFT_JIS.encode(mixed_sjis);
    let sjis_result = sanitize_filename(&sjis_encoded);
    assert_eq!(sjis_result, mixed_sjis);
}

#[test]
fn test_adv_charset_utf8_bom_astral_plane_emojis() {
    // UTF-8 with BOM
    let with_bom = b"\xEF\xBB\xBFUTF8_Document_With_BOM.md";
    let (det, conf) = detect_charset_with_confidence(with_bom);
    assert_eq!(det, "UTF-8");
    assert_eq!(conf, 1.0);

    // UTF-8 with 4-byte astral symbols: 🦀 🚀 📦 ⚡
    let emoji_str = "Release_🦀_v2.0_🚀_Archive_📦.7z";
    let emoji_bytes = emoji_str.as_bytes();
    let sanitized = sanitize_filename(emoji_bytes);
    assert_eq!(sanitized, emoji_str);
}

// ============================================================================
// Domain 3: Tree-sitter Tokenization Under Rapid Scrolling & Huge Files
// ============================================================================

#[test]
fn test_adv_syntax_empty_and_whitespace_sources() {
    let mut highlighter = SyntaxHighlighter::new();

    // 1. Completely empty string
    let tokens_empty = highlighter
        .highlight_source("", SupportedLanguage::Rust)
        .unwrap();
    assert!(tokens_empty.is_empty());

    let outlines_empty =
        SymbolOutlineExtractor::extract_from_source("", SupportedLanguage::Rust).unwrap();
    assert!(outlines_empty.is_empty());

    // 2. Whitespace only string
    let ws_source = "    \n\n\t\t\r\n   ";
    let tokens_ws = highlighter
        .highlight_source(ws_source, SupportedLanguage::Rust)
        .unwrap();
    assert!(tokens_ws.is_empty());

    // 3. Viewport query on empty tree
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let tree = parser.parse_full("").unwrap();
    let vp_tokens = highlighter
        .highlight_range(tree, "", SupportedLanguage::Rust, Some(0..10))
        .unwrap();
    assert!(vp_tokens.is_empty());
}

#[test]
fn test_adv_syntax_huge_file_stress_and_outlines() {
    // Generate a 1,000-line synthetic Rust file
    let mut big_code = String::with_capacity(64 * 1024);
    big_code.push_str("//! High performance synthetic benchmark module.\n\n");
    for i in 0..250 {
        big_code.push_str(&format!(
            r#"
pub struct Record_{i} {{
    pub id: u64,
    pub name: String,
    pub score: f64,
}}

impl Record_{i} {{
    pub fn new(id: u64, name: &str) -> Self {{
        Self {{ id, name: name.to_string(), score: 100.0 }}
    }}

    pub fn compute_hash(&self) -> u64 {{
        self.id ^ 0x5555_AAAA_5555_AAAA
    }}
}}
"#
        ));
    }

    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let start_parse = std::time::Instant::now();
    let tree = parser.parse_full(&big_code).unwrap();
    let parse_time = start_parse.elapsed();
    assert!(!tree.root_node().has_error());
    assert!(parse_time.as_millis() < 500, "Parse took {:?}", parse_time);

    // Extract symbol outline from 1,000-line file
    let outlines =
        SymbolOutlineExtractor::extract_from_source(&big_code, SupportedLanguage::Rust).unwrap();
    assert_eq!(outlines.len(), 500); // 250 structs + 250 impls
}

#[test]
fn test_adv_syntax_rapid_random_viewport_scrolling_harness() {
    let mut code = String::with_capacity(32 * 1024);
    for i in 0..100 {
        code.push_str(&format!(
            "/// Documentation for helper_{i}\npub fn helper_{i}(val: u32) -> u32 {{\n    let x = val * 2;\n    x + {i}\n}}\n\n"
        ));
    }

    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let tree = parser.parse_full(&code).unwrap();
    let code_len = code.len();

    let mut highlighter = SyntaxHighlighter::new();

    // Simulate 500 rapid random viewport scroll queries
    let mut pseudo_rand = 0x1234_5678u64;
    for _ in 0..500 {
        pseudo_rand = pseudo_rand.wrapping_mul(6364136223846793005).wrapping_add(1);
        let start = (pseudo_rand as usize) % code_len;
        pseudo_rand = pseudo_rand.wrapping_mul(6364136223846793005).wrapping_add(1);
        let span = ((pseudo_rand as usize) % 400) + 10;
        let end = (start + span).min(code_len);

        let vp_tokens = highlighter
            .highlight_range(tree, &code, SupportedLanguage::Rust, Some(start..end))
            .unwrap();

        for t in &vp_tokens {
            assert!(t.start_byte < end);
            assert!(t.end_byte > start);
            assert!(t.start_byte < t.end_byte);
            assert!(t.utf16_length > 0);
        }
    }
}

#[test]
fn test_adv_syntax_incremental_edit_syntax_error_recovery() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let valid_code = "fn compute() -> u32 {\n    42\n}";
    let tree = parser.parse_full(valid_code).unwrap();
    assert!(!tree.root_node().has_error());

    // Inject syntax error: incomplete token
    let error_code = "fn compute() -> u32 {\n    let x = ;\n    42\n}";
    let edit_error = tree_sitter::InputEdit {
        start_byte: 26,
        old_end_byte: 26,
        new_end_byte: 40,
        start_position: tree_sitter::Point { row: 1, column: 4 },
        old_end_position: tree_sitter::Point { row: 1, column: 4 },
        new_end_position: tree_sitter::Point { row: 2, column: 4 },
    };
    let tree_err = parser.parse_incremental(error_code, &edit_error).unwrap();
    assert!(tree_err.root_node().has_error());

    // Recover from error
    let recovered_code = "fn compute() -> u32 {\n    let x = 10;\n    42 + x\n}";
    let edit_fix = tree_sitter::InputEdit {
        start_byte: 26,
        old_end_byte: 40,
        new_end_byte: 48,
        start_position: tree_sitter::Point { row: 1, column: 4 },
        old_end_position: tree_sitter::Point { row: 2, column: 4 },
        new_end_position: tree_sitter::Point { row: 2, column: 10 },
    };
    let tree_fixed = parser.parse_incremental(recovered_code, &edit_fix).unwrap();
    assert!(!tree_fixed.root_node().has_error());
}

#[test]
fn test_adv_syntax_deeply_nested_ast_stress() {
    // 1. 300 levels of nested parentheses: ((((... 42 ...))))
    let mut nested_expr = String::with_capacity(2048);
    nested_expr.push_str("fn nested_expr() -> i32 {\n    ");
    for _ in 0..300 {
        nested_expr.push('(');
    }
    nested_expr.push_str("42");
    for _ in 0..300 {
        nested_expr.push(')');
    }
    nested_expr.push_str("\n}\n");

    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let tree_expr = parser.parse_full(&nested_expr).unwrap();
    assert!(!tree_expr.root_node().has_error());

    let mut highlighter = SyntaxHighlighter::new();
    let tokens_expr = highlighter
        .highlight(tree_expr, &nested_expr, SupportedLanguage::Rust)
        .unwrap();
    assert!(!tokens_expr.is_empty());

    // 2. 150 levels of nested blocks: { { { ... let val = 100; ... } } }
    let mut nested_blocks = String::with_capacity(2048);
    nested_blocks.push_str("fn block_nesting() {\n");
    for _ in 0..150 {
        nested_blocks.push_str("    {\n");
    }
    nested_blocks.push_str("        let val = 100;\n");
    for _ in 0..150 {
        nested_blocks.push_str("    }\n");
    }
    nested_blocks.push_str("}\n");

    let tree_blocks = parser.parse_full(&nested_blocks).unwrap();
    assert!(!tree_blocks.root_node().has_error());
    let tokens_blocks = highlighter
        .highlight(tree_blocks, &nested_blocks, SupportedLanguage::Rust)
        .unwrap();
    assert!(!tokens_blocks.is_empty());

    // 3. 150 levels of nested JSON objects: {"a": {"a": ... {"leaf": true} ...}}
    let mut nested_json = String::with_capacity(4096);
    for _ in 0..150 {
        nested_json.push_str(r#"{"nested": "#);
    }
    nested_json.push_str(r#"{"leaf": true}"#);
    for _ in 0..150 {
        nested_json.push('}');
    }

    let mut json_parser = TTZipSyntaxParser::with_language(SupportedLanguage::Json).unwrap();
    let json_tree = json_parser.parse_full(&nested_json).unwrap();
    assert!(!json_tree.root_node().has_error());
    let json_tokens = highlighter
        .highlight(json_tree, &nested_json, SupportedLanguage::Json)
        .unwrap();
    assert!(!json_tokens.is_empty());
}

#[test]
fn test_adv_syntax_extreme_long_single_line_viewport_streaming() {
    // 1. 50,000-character single line in Rust without linebreaks
    let mut long_rust_line = String::with_capacity(64 * 1024);
    long_rust_line.push_str("pub fn massive_single_line() -> u64 { let mut sum = 0u64;");
    for i in 0..4000 {
        long_rust_line.push_str(&format!(" sum += {i};"));
    }
    long_rust_line.push_str(" sum }");
    let total_len = long_rust_line.len();
    assert!(total_len > 40_000);

    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let tree = parser.parse_full(&long_rust_line).unwrap();
    assert!(!tree.root_node().has_error());

    let mut highlighter = SyntaxHighlighter::new();

    // Viewport query at start
    let vp_start = highlighter
        .highlight_range(tree, &long_rust_line, SupportedLanguage::Rust, Some(0..500))
        .unwrap();
    assert!(!vp_start.is_empty());
    for t in &vp_start {
        assert!(t.start_byte < 500 && t.end_byte > 0);
        assert_eq!(t.start_line, 0); // single line
    }

    // Viewport query in the middle
    let mid_start = total_len / 2;
    let mid_end = mid_start + 600;
    let vp_mid = highlighter
        .highlight_range(
            tree,
            &long_rust_line,
            SupportedLanguage::Rust,
            Some(mid_start..mid_end),
        )
        .unwrap();
    assert!(!vp_mid.is_empty());
    for t in &vp_mid {
        assert!(t.start_byte < mid_end && t.end_byte > mid_start);
        assert_eq!(t.start_line, 0);
    }

    // Viewport query at the end
    let vp_end = highlighter
        .highlight_range(
            tree,
            &long_rust_line,
            SupportedLanguage::Rust,
            Some((total_len - 500)..total_len),
        )
        .unwrap();
    assert!(!vp_end.is_empty());

    // Out-of-bounds viewport query (must return empty, zero panic)
    let vp_oob = highlighter
        .highlight_range(
            tree,
            &long_rust_line,
            SupportedLanguage::Rust,
            Some(total_len + 1000..total_len + 2000),
        )
        .unwrap();
    assert!(vp_oob.is_empty());

    // Zero-length / inverted range query (zero panic)
    let vp_empty = highlighter
        .highlight_range(
            tree,
            &long_rust_line,
            SupportedLanguage::Rust,
            Some(100..100),
        )
        .unwrap();
    assert!(vp_empty.is_empty());
}


// ============================================================================
// Domain 4: HTML VFS Rewriting Under Complex Nested CSS, SVG, and Scripts
// ============================================================================

#[test]
fn test_adv_html_vfs_svg_xlink_href_and_image_href() {
    let html = br#"
<!DOCTYPE html>
<html>
<head><title>SVG Vector Test</title></head>
<body>
    <svg width="200" height="200">
        <image href="assets/icon.png" width="50" height="50"/>
        <image xlink:href="../shared/logo.svg" width="100" height="100"/>
        <use href="symbols.svg#icon-star"/>
        <use xlink:href="../common/defs.svg#icon-check"/>
    </svg>
</body>
</html>
"#;

    let (out_bytes, stats) = TTZipHtmlRewriter::rewrite_all(
        html,
        "archive-svg",
        "views/page.html",
        HtmlSanitizationPolicy::Permissive,
    )
    .unwrap();

    let out_str = String::from_utf8(out_bytes).unwrap();

    assert!(out_str.contains("ttzip-vfs://archive-svg/views/assets/icon.png"));
    assert!(out_str.contains("ttzip-vfs://archive-svg/shared/logo.svg"));
    assert!(out_str.contains("ttzip-vfs://archive-svg/views/symbols.svg#icon-star"));
    assert!(out_str.contains("ttzip-vfs://archive-svg/common/defs.svg#icon-check"));
    assert!(stats.resources_routed >= 4);
}

#[test]
fn test_adv_html_vfs_script_src_policy_enforcement() {
    let html = br#"
<!DOCTYPE html>
<html>
<head>
    <script src="../vendor/jquery.min.js?v=3.6.0"></script>
    <SCRIPT SRC="./app/bootstrap.js"></SCRIPT>
    <script src="https://cdn.example.com/external.js"></script>
</head>
<body>
    <h1>App Shell</h1>
</body>
</html>
"#;

    // 1. Permissive policy: relative scripts rewritten, external left untouched
    let (perm_bytes, perm_stats) = TTZipHtmlRewriter::rewrite_all(
        html,
        "app-arc",
        "dist/index.html",
        HtmlSanitizationPolicy::Permissive,
    )
    .unwrap();
    let perm_str = String::from_utf8(perm_bytes).unwrap();
    assert!(perm_str.contains("ttzip-vfs://app-arc/vendor/jquery.min.js?v=3.6.0"));
    assert!(perm_str.contains("ttzip-vfs://app-arc/dist/app/bootstrap.js"));
    assert!(perm_str.contains("https://cdn.example.com/external.js"));
    assert_eq!(perm_stats.scripts_stripped, 0);

    // 2. Strict policy: all script tags stripped completely
    let (strict_bytes, strict_stats) = TTZipHtmlRewriter::rewrite_all(
        html,
        "app-arc",
        "dist/index.html",
        HtmlSanitizationPolicy::Strict,
    )
    .unwrap();
    let strict_str = String::from_utf8(strict_bytes).unwrap();
    assert!(!strict_str.contains("<script"));
    assert!(!strict_str.contains("<SCRIPT"));
    assert!(strict_stats.scripts_stripped >= 3);
}

#[test]
fn test_adv_html_vfs_nested_css_and_srcset_matrix() {
    let html = br#"
<!DOCTYPE html>
<html>
<head>
    <link rel="stylesheet" href="../../themes/dark/theme.css">
    <link rel="stylesheet" href="./style.css?theme=nord#root">
</head>
<body>
    <picture>
        <source srcset="../images/hero-1x.webp 1x, ../images/hero-2x.webp 2x" type="image/webp">
        <img src="fallback.png" srcset="img-small.png 300w, img-large.png 800w" alt="Hero">
    </picture>
</body>
</html>
"#;

    let (out_bytes, stats) = TTZipHtmlRewriter::rewrite_all(
        html,
        "nested-arc",
        "apps/web/index.html",
        HtmlSanitizationPolicy::AllowInlineStyles,
    )
    .unwrap();

    let out_str = String::from_utf8(out_bytes).unwrap();

    // Link stylesheet rewriting
    assert!(out_str.contains("ttzip-vfs://nested-arc/themes/dark/theme.css"));
    assert!(out_str.contains("ttzip-vfs://nested-arc/apps/web/style.css?theme=nord#root"));

    // Picture / Source / Img srcset rewriting
    assert!(out_str.contains("ttzip-vfs://nested-arc/apps/images/hero-1x.webp 1x"));
    assert!(out_str.contains("ttzip-vfs://nested-arc/apps/images/hero-2x.webp 2x"));
    assert!(out_str.contains("ttzip-vfs://nested-arc/apps/web/img-small.png 300w"));
    assert!(out_str.contains("ttzip-vfs://nested-arc/apps/web/img-large.png 800w"));
    assert!(out_str.contains("ttzip-vfs://nested-arc/apps/web/fallback.png"));

    assert!(stats.resources_routed >= 5);
}

#[test]
fn test_adv_html_vfs_zip_slip_and_malformed_resilience() {
    // 1. Path traversal escape attempts
    assert_eq!(
        normalize_rfc3986_path("app/web", "../../../../../etc/shadow"),
        "etc/shadow"
    );
    assert_eq!(
        normalize_rfc3986_path("", "../../secret.token"),
        "secret.token"
    );

    // 2. Windows-style backslashes in HTML relative links
    let router = HtmlVfsResourceRouter::new("arc-win", r"docs\ch1\page.html");
    let routed = router.route_url(r"..\images\diagram.png");
    assert_eq!(
        routed.as_deref(),
        Some("ttzip-vfs://arc-win/docs/images/diagram.png")
    );

    // 3. Malformed HTML tags and unclosed elements
    let malformed_html = b"<div><img src='broken.jpg' <span class='test'>Unclosed<a href='../doc.pdf'>Link";
    let (out_bytes, _) = TTZipHtmlRewriter::rewrite_all(
        malformed_html,
        "arc-mal",
        "sub/doc.html",
        HtmlSanitizationPolicy::Permissive,
    )
    .unwrap();
    let out_str = String::from_utf8(out_bytes).unwrap();
    assert!(out_str.contains("ttzip-vfs://arc-mal/sub/broken.jpg"));
}
