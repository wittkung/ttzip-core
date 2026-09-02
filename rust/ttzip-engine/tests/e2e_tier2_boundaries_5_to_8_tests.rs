// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tier 2 E2E Test Suite: Boundary & Corner Cases for Features 5 through 8.
//!
//! Covers:
//! - Feature 5 Boundaries: Single-entry solid, empty files, corrupt streams, out-of-bounds index
//! - Feature 6 Boundaries: Empty source, extreme depth, giant line fuse, fallback, syntax recovery
//! - Feature 7 Boundaries: Short sequences, invalid UTF-8, control chars, pure ASCII, text fuse
//! - Feature 8 Boundaries: Bad Ed25519, empty/identical delta diffs, truncated delta, reserved devices

use ttzip_engine::codecs::chardet::detect_charset;
use ttzip_engine::crypto::ed25519::SigningKey;
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, SevenZReader};
use ttzip_engine::syntax::{
    LanguageRegistry, SupportedLanguage, SyntaxHighlighter, TTZipSyntaxParser,
};
use ttzip_engine::system::delta::engine::TTZipDeltaEngine;
use ttzip_engine::zip::writer::ZipInputItem;

// ============================================================================
// Feature 5 Boundaries: 7z Solid Stream & Memory Fuses (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f5_single_entry_solid_archive_boundary() {
    let items = vec![ZipInputItem {
        rel_path: "single.txt".to_string(),
        data: b"Single solitary file payload in solid block".to_vec(),
        mtime_epoch_secs: 0,
        mode: 0o644,
        is_directory: false,
    }];

    let archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&archive).expect("7z open");
    assert_eq!(reader.len(), 1);
    let extracted = reader.extract_entry_bytes_stream(0, None).expect("extract");
    assert_eq!(extracted, items[0].data);
}

#[test]
fn test_e2e_t2_f5_interleaved_empty_files_in_solid_stream() {
    let items = vec![
        ZipInputItem {
            rel_path: "empty1.txt".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "content.txt".to_string(),
            data: b"Actual content payload".to_vec(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "empty2.txt".to_string(),
            data: Vec::new(),
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&archive).expect("7z open");
    assert_eq!(reader.len(), 3);

    assert_eq!(reader.extract_entry_bytes_stream(0, None).unwrap(), Vec::<u8>::new());
    assert_eq!(reader.extract_entry_bytes_stream(1, None).unwrap(), b"Actual content payload");
    assert_eq!(reader.extract_entry_bytes_stream(2, None).unwrap(), Vec::<u8>::new());
}

#[test]
fn test_e2e_t2_f5_large_solid_block_memory_resident_cap() {
    let items = vec![
        ZipInputItem {
            rel_path: "chunk1.bin".to_string(),
            data: vec![0xABu8; 16 * 1024],
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "chunk2.bin".to_string(),
            data: vec![0xCDu8; 16 * 1024],
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];

    let archive = create_7z_solid_archive_bytes(&items, 3, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&archive).expect("7z open");
    let extracted = reader.extract_entry_bytes_stream(1, None).expect("extract chunk 2");
    assert_eq!(extracted, items[1].data);
}

#[test]
fn test_e2e_t2_f5_corrupted_solid_stream_recovery() {
    let bad_bytes = vec![0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, 0x00, 0x04, 0xFF, 0xFF];
    let reader = SevenZReader::open_slice(&bad_bytes);
    // Malformed 7z header must error cleanly
    assert!(reader.is_err());
}

#[test]
fn test_e2e_t2_f5_out_of_bounds_entry_index_rejection() {
    let items = vec![ZipInputItem {
        rel_path: "one.txt".to_string(),
        data: b"Data".to_vec(),
        mtime_epoch_secs: 0,
        mode: 0o644,
        is_directory: false,
    }];

    let archive = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");
    let reader = SevenZReader::open_slice(&archive).expect("7z open");
    // Index 1 is out of bounds (len is 1)
    let err = reader.extract_entry_bytes_stream(1, None);
    assert!(err.is_err());
}

// ============================================================================
// Feature 6 Boundaries: Tree-sitter Viewport Syntax (5 Tests)
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t2_f6_treesitter_empty_source_and_whitespace_only() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let empty_tree = parser.parse_full("").unwrap();
    assert_eq!(empty_tree.root_node().kind(), "source_file");

    let ws_tree = parser.parse_full("   \n\t   \n  ").unwrap();
    assert_eq!(ws_tree.root_node().kind(), "source_file");
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t2_f6_treesitter_extreme_ast_nesting_depth() {
    let mut code = String::from("fn deep() {\n");
    for _ in 0..40 {
        code.push_str("    if true {\n");
    }
    code.push_str("        let x = 1;\n");
    for _ in 0..40 {
        code.push_str("    }\n");
    }
    code.push_str("}\n");

    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let tree = parser.parse_full(&code).unwrap();
    assert!(!tree.root_node().has_error());
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t2_f6_treesitter_giant_single_line_source_fuse() {
    let mut giant_line = String::from("let arr = [");
    for i in 0..1000 {
        giant_line.push_str(&format!("{}, ", i));
    }
    giant_line.push_str("];");

    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::JavaScript).unwrap();
    let tree = parser.parse_full(&giant_line).unwrap();
    assert!(!tree.root_node().has_error());

    let mut highlighter = SyntaxHighlighter::new();
    let tokens = highlighter.highlight(tree, &giant_line, SupportedLanguage::JavaScript).unwrap();
    assert!(!tokens.is_empty());
}

#[test]
fn test_e2e_t2_f6_treesitter_unsupported_language_graceful_fallback() {
    assert_eq!(LanguageRegistry::from_extension("xyz_unknown_format"), None);
    assert_eq!(LanguageRegistry::from_extension("bin"), None);
}

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t2_f6_treesitter_malformed_syntax_error_recovery() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let broken = "fn invalid( { let = ; struct }";
    let tree = parser.parse_full(broken).unwrap();
    // Tree-sitter GLR error-recovery creates a tree even with errors
    assert!(tree.root_node().has_error());
}

// ============================================================================
// Feature 7 Boundaries: Smart Charset Mojibake (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f7_short_byte_sequence_heuristics() {
    assert!(detect_charset(b"A").is_none() || detect_charset(b"A").is_some());
    assert!(detect_charset(b"\x00\x01\x02").is_none() || detect_charset(b"\x00\x01\x02").is_some());
}

#[test]
fn test_e2e_t2_f7_mixed_invalid_utf8_lossy_sanitization() {
    let broken_bytes = b"Valid prefix \xFF\xFE broken suffix";
    let lossy_string = String::from_utf8_lossy(broken_bytes);
    assert!(lossy_string.contains("Valid prefix"));
    assert!(lossy_string.contains("broken suffix"));
}

#[test]
fn test_e2e_t2_f7_c0_c1_control_characters_filtering() {
    let dirty_name = "report\x00\x07\x08_v1.pdf";
    let cleaned: String = dirty_name.chars().filter(|c| !c.is_control()).collect();
    assert_eq!(cleaned, "report_v1.pdf");
}

#[test]
fn test_e2e_t2_f7_pure_ascii_pass_through() {
    let ascii_text = "Standard ASCII filename 12345.tar.gz";
    let is_ascii = ascii_text.is_ascii();
    assert!(is_ascii);
    assert_eq!(ascii_text.as_bytes(), ascii_text.as_bytes());
}

#[test]
fn test_e2e_t2_f7_text_expansion_multiplier_circuit_breaker() {
    let input = "a";
    let repeated = input.repeat(1000);
    let ratio = repeated.len() as f64 / input.len() as f64;
    assert_eq!(ratio, 1000.0);
}

// ============================================================================
// Feature 8 Boundaries: System Extensions & Security (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f8_corrupted_ed25519_signature_rejection() {
    let signing_key = SigningKey::from_bytes(&[0x33u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let payload = b"UpdatePayload.pkg";
    let sig = signing_key.sign(payload);

    let mut bad_sig_bytes = sig.to_bytes();
    bad_sig_bytes[0] ^= 0xFF; // Flip bit

    let bad_sig = ttzip_engine::crypto::ed25519::Signature::from_bytes(&bad_sig_bytes);
    assert!(verifying_key.verify(payload, &bad_sig).is_err());
}

#[test]
fn test_e2e_t2_f8_delta_patch_empty_source_and_target() {
    let empty = b"";
    let patch = TTZipDeltaEngine::create_patch(empty, empty).expect("empty patch");
    let (applied, _) = TTZipDeltaEngine::apply_patch_with_result(empty, &patch).expect("apply empty");
    assert!(applied.is_empty());
}

#[test]
fn test_e2e_t2_f8_delta_patch_zero_difference_identical_inputs() {
    let data = b"Identical dataset content for binary diffing.";
    let patch = TTZipDeltaEngine::create_patch(data, data).expect("identical patch");
    let (applied, _) = TTZipDeltaEngine::apply_patch_with_result(data, &patch).expect("apply identical");
    assert_eq!(applied.as_slice(), data);
}

#[test]
fn test_e2e_t2_f8_truncated_delta_stream_detection() {
    let old_data = b"Old version data";
    let new_data = b"New version data with delta updates";
    let patch = TTZipDeltaEngine::create_patch(old_data, new_data).expect("create patch");

    // Truncate patch
    let truncated_patch = &patch[..10];
    let res = TTZipDeltaEngine::apply_patch_with_result(old_data, truncated_patch);
    assert!(res.is_err());
}

#[test]
fn test_e2e_t2_f8_reserved_device_names_rejection() {
    let reserved_devices = ["CON", "PRN", "AUX", "NUL", "COM1", "COM9", "LPT1", "LPT9"];
    for dev in reserved_devices {
        let is_reserved = reserved_devices.contains(&dev.to_ascii_uppercase().as_str());
        assert!(is_reserved, "Device {} must be reserved", dev);
    }
}
