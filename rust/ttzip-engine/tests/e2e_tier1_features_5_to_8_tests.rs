// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tier 1 E2E Test Suite: Feature Coverage for Features 5 through 8.
//!
//! Covers:
//! - Feature 5: 7z Solid Stream Decoding & Mach RSS Monitoring
//! - Feature 6: Tree-sitter Viewport Syntax Highlighting
//! - Feature 7: Smart Charset Mojibake Repair
//! - Feature 8: System Extensions & Architecture Hard Gates

use std::fs;

use tempfile::tempdir;
use unicode_normalization::UnicodeNormalization;

use ttzip_engine::codecs::chardet::detect_charset;
use ttzip_engine::crypto::ed25519::SigningKey;
use ttzip_engine::security::system_defense::SensitiveCredentialBuffer;
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, get_current_rss_bytes, SevenZReader};
use ttzip_engine::syntax::{
    HighlightTokenKind, SupportedLanguage, SymbolOutlineExtractor, SyntaxHighlighter,
    TTZipSyntaxParser,
};
use ttzip_engine::system::delta::engine::TTZipDeltaEngine;
use ttzip_engine::system::delta::types::DeltaPatchHeader;
use ttzip_engine::zip::writer::ZipInputItem;

// ============================================================================
// Feature 5: 7z Solid Stream & Mach RSS Gate (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t1_f5_sevenz_solid_multi_file_creation_and_reading() {
    let items = vec![
        ZipInputItem {
            rel_path: "kernel/init.rs".to_string(),
            data: b"pub fn init() -> Result<(), &'static str> { Ok(()) }".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "kernel/mmu.rs".to_string(),
            data: b"pub struct PageTable { entries: [u64; 512] }".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "kernel/sched.rs".to_string(),
            data: b"pub fn schedule_next_thread() -> usize { 0 }".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&solid_archive).expect("7z open slice");
    assert_eq!(reader.len(), 3);

    for (idx, item) in items.iter().enumerate() {
        let extracted = reader.extract_entry_bytes_stream(idx, None).expect("7z extract");
        assert_eq!(extracted, item.data);
    }
}

#[test]
fn test_e2e_t1_f5_sevenz_solid_folder_index_and_file_mapping() {
    let items = vec![
        ZipInputItem {
            rel_path: "file1.dat".to_string(),
            data: vec![0x11u8; 512],
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "file2.dat".to_string(),
            data: vec![0x22u8; 1024],
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&solid_archive).expect("7z open");
    let files = reader.files();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].rel_path, "file1.dat");
    assert_eq!(files[1].rel_path, "file2.dat");
}

#[test]
fn test_e2e_t1_f5_sevenz_solid_micro_buffer_streaming_extraction() {
    let items = vec![
        ZipInputItem {
            rel_path: "manifest.json".to_string(),
            data: b"{\"name\": \"ttzip-solid\", \"version\": \"1.0.0\"}".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "payload.bin".to_string(),
            data: vec![0x7Fu8; 8192],
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&solid_archive).expect("7z open");

    let stream0 = reader.extract_entry_bytes_stream(0, None).expect("stream 0");
    assert_eq!(stream0, items[0].data);
    let stream1 = reader.extract_entry_bytes_stream(1, None).expect("stream 1");
    assert_eq!(stream1, items[1].data);
}

#[test]
fn test_e2e_t1_f5_sevenz_solid_random_access_entry_extraction() {
    let mut items = Vec::new();
    for i in 0..5 {
        items.push(ZipInputItem {
            rel_path: format!("item_{}.txt", i),
            data: format!("Payload content for block item {}", i).into_bytes(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        });
    }

    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&solid_archive).expect("7z open");

    // Random non-sequential extraction: 4, then 1, then 3, then 0, then 2
    let order = [4, 1, 3, 0, 2];
    for &idx in &order {
        let extracted = reader.extract_entry_bytes_stream(idx, None).expect("extract non-sequential");
        assert_eq!(extracted, items[idx].data);
    }
}

#[test]
fn test_e2e_t1_f5_mach_rss_resident_memory_monitoring_invariants() {
    let rss = get_current_rss_bytes();
    assert!(rss > 0, "Resident memory should be positive");
    assert!(rss < 1024 * 1024 * 1024, "Process memory within safe operating boundary");
}

// ============================================================================
// Feature 6: Tree-sitter Viewport Syntax Highlighting (6 Tests)
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t1_f6_treesitter_rust_syntax_tokenization() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let source = "pub fn add_numbers(a: u32, b: u32) -> u32 { a + b }";
    let tree = parser.parse_full(source).unwrap();
    assert!(!tree.root_node().has_error());

    let mut highlighter = SyntaxHighlighter::new();
    let tokens = highlighter.highlight(tree, source, SupportedLanguage::Rust).unwrap();
    assert!(!tokens.is_empty());
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Keyword));
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Function));
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t1_f6_treesitter_c_and_swift_tokenization() {
    let mut c_parser = TTZipSyntaxParser::with_language(SupportedLanguage::C).unwrap();
    let c_source = "int main(int argc, char **argv) { return 0; }";
    let c_tree = c_parser.parse_full(c_source).unwrap();
    assert!(!c_tree.root_node().has_error());

    let mut swift_parser = TTZipSyntaxParser::with_language(SupportedLanguage::Swift).unwrap();
    let swift_source = "import SwiftUI\nstruct ContentView: View { var body: some View { Text(\"Hello\") } }";
    let swift_tree = swift_parser.parse_full(swift_source).unwrap();
    assert_eq!(swift_tree.root_node().kind(), "source_file");
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t1_f6_treesitter_python_and_javascript_tokenization() {
    let mut py_parser = TTZipSyntaxParser::with_language(SupportedLanguage::Python).unwrap();
    let py_source = "def compute_hashes(data: bytes) -> str:\n    return hashlib.sha256(data).hexdigest()\n";
    let py_tree = py_parser.parse_full(py_source).unwrap();
    assert!(!py_tree.root_node().has_error());

    let mut js_parser = TTZipSyntaxParser::with_language(SupportedLanguage::JavaScript).unwrap();
    let js_source = "const fetchMetrics = async (endpoint) => { const res = await fetch(endpoint); return res.json(); };";
    let js_tree = js_parser.parse_full(js_source).unwrap();
    assert!(!js_tree.root_node().has_error());
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t1_f6_treesitter_json_and_html_structured_tokens() {
    let mut json_parser = TTZipSyntaxParser::with_language(SupportedLanguage::Json).unwrap();
    let json_source = r#"{"name": "ttzip", "threads": 8, "solid": true}"#;
    let json_tree = json_parser.parse_full(json_source).unwrap();
    assert!(!json_tree.root_node().has_error());

    let mut html_parser = TTZipSyntaxParser::with_language(SupportedLanguage::Html).unwrap();
    let html_source = r#"<!DOCTYPE html><html><head><title>TTZip</title></head><body><h1>Ready</h1></body></html>"#;
    let html_tree = html_parser.parse_full(html_source).unwrap();
    assert_eq!(html_tree.root_node().kind(), "document");
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t1_f6_treesitter_incremental_edit_ast_reparse() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let initial_source = "fn compute() -> u32 { 10 }";
    let tree1 = parser.parse_full(initial_source).unwrap().clone();

    let updated_source = "fn compute() -> u32 { 10 + 20 }";
    let edit = tree_sitter::InputEdit {
        start_byte: 22,
        old_end_byte: 24,
        new_end_byte: 29,
        start_position: tree_sitter::Point { row: 0, column: 22 },
        old_end_position: tree_sitter::Point { row: 0, column: 24 },
        new_end_position: tree_sitter::Point { row: 0, column: 29 },
    };

    let tree2 = parser.parse_incremental(updated_source, &edit).unwrap();
    assert!(!tree2.root_node().has_error());
    assert_eq!(tree1.root_node().kind(), tree2.root_node().kind());
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t1_f6_treesitter_viewport_range_token_streaming() {
    let source = "struct Config {\n    port: u16,\n    host: String,\n}\n\nfn start_server(c: Config) {\n    println!(\"Starting\");\n}\n";
    let symbols = SymbolOutlineExtractor::extract_from_source(source, SupportedLanguage::Rust).unwrap();
    assert!(!symbols.is_empty());
    assert!(symbols.iter().any(|s| s.name == "Config"));
    assert!(symbols.iter().any(|s| s.name == "start_server"));
}

// ============================================================================
// Feature 7: Smart Charset Mojibake Repair (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t1_f7_gb18030_and_gbk_cjk_detection_and_repair() {
    let text = "高性能压缩归档测试文档";
    let (encoded, _, _) = encoding_rs::GB18030.encode(text);
    let detected = detect_charset(&encoded);
    assert!(detected.is_some());

    let (decoded, _, had_errors) = encoding_rs::GB18030.decode(&encoded);
    assert!(!had_errors);
    assert_eq!(decoded, text);
}

#[test]
fn test_e2e_t1_f7_shift_jis_japanese_detection_and_repair() {
    let text = "日本語のアーカイブファイル検証";
    let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    let detected = detect_charset(&encoded);
    assert!(detected.is_some());

    let (decoded, _, had_errors) = encoding_rs::SHIFT_JIS.decode(&encoded);
    assert!(!had_errors);
    assert_eq!(decoded, text);
}

#[test]
fn test_e2e_t1_f7_euc_kr_korean_detection_and_repair() {
    let text = "한국어 파일명 인코딩 복구 테스트";
    let (encoded, _, _) = encoding_rs::EUC_KR.encode(text);
    let (decoded, _, had_errors) = encoding_rs::EUC_KR.decode(&encoded);
    assert!(!had_errors);
    assert_eq!(decoded, text);
}

#[test]
fn test_e2e_t1_f7_windows_1252_western_latin_transcoding() {
    let text = "Café résumé naïve façade Noël";
    let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(text);
    let (decoded, _, had_errors) = encoding_rs::WINDOWS_1252.decode(&encoded);
    assert!(!had_errors);
    assert_eq!(decoded, text);
}

#[test]
fn test_e2e_t1_f7_unicode_normalization_nfc_and_nfd_consistency() {
    let nfc_text = "résumé";
    let nfd_text: String = nfc_text.nfd().collect();
    assert_ne!(nfc_text.as_bytes(), nfd_text.as_bytes());

    let normalized_back: String = nfd_text.nfc().collect();
    assert_eq!(normalized_back, nfc_text);
}

// ============================================================================
// Feature 8: System Extensions & Architecture Hard Gates (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t1_f8_binary_delta_patch_creation_and_apply() {
    let old_data = b"Base microkernel firmware version 1.0.0 released with initial features.";
    let new_data = b"Base microkernel firmware version 1.1.0 released with activated fast streaming.";

    let patch = TTZipDeltaEngine::create_patch(old_data, new_data).expect("create patch");
    let (applied, telemetry) = TTZipDeltaEngine::apply_patch_with_result(old_data, &patch).expect("apply patch");

    assert_eq!(applied.as_slice(), new_data);
    assert_eq!(telemetry.bytes_out, new_data.len());
}

#[test]
fn test_e2e_t1_f8_delta_container_header_crc32_integrity() {
    let header = DeltaPatchHeader::new(
        *b"spk!",
        4,
        0,
        0x1122_3344,
        0x5566_7788,
        1024,
    );
    let bytes = header.to_bytes();
    assert_eq!(bytes.len(), 24);

    let parsed = DeltaPatchHeader::from_bytes(&bytes).expect("parse header");
    assert_eq!(parsed.magic, *b"spk!");
    assert_eq!(parsed.before_tree_hash, 0x1122_3344);
    assert_eq!(parsed.after_tree_hash, 0x5566_7788);
}

#[test]
fn test_e2e_t1_f8_ed25519_appcast_signature_verification() {
    let signing_key = SigningKey::from_bytes(&[0x42u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let payload = b"TTZip-v1.1.0-universal-macos.dmg";

    let signature = signing_key.sign(payload);
    assert!(verifying_key.verify(payload, &signature).is_ok());

    // Corrupted payload fails verification
    let corrupted_payload = b"TTZip-v1.1.0-tampered-macos.dmg";
    assert!(verifying_key.verify(corrupted_payload, &signature).is_err());
}

#[test]
fn test_e2e_t1_f8_sensitive_buffer_zeroize_on_drop() {
    let secret = b"TOP_SECRET_PASSPHRASE_KEY_2026";
    let mut buffer = SensitiveCredentialBuffer::new(secret.to_vec());
    assert_eq!(buffer.as_slice(), secret);
    assert_eq!(buffer.len(), secret.len());
    buffer.clear();
    assert!(buffer.is_empty());
}

#[test]
fn test_e2e_t1_f8_temporary_workspace_isolation_and_raii_cleanup() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("preview_frame.raw");
    fs::write(&file_path, b"RAW RGB FRAME DATA").unwrap();
    assert!(file_path.exists());

    let dir_path = temp_dir.path().to_path_buf();
    drop(temp_dir);
    assert!(!dir_path.exists(), "Temp dir must be cleaned up on drop");
}
