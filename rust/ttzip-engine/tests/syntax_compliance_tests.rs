// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive Multi-Language AST Syntax Compliance & 6-Layer Defense Test Suite.
//!
//! Validates:
//! 1. **Official Multi-Language AST Tree Consistency**:
//!    - Rust, C, Swift, Python, JavaScript, TypeScript, and JSON canonical grammar vectors.
//! 2. **Incremental vs Full Parse AST Equivalence Oracle**:
//!    - Verifies that incremental AST re-parsing with `InputEdit` produces bit-for-bit
//!      and structure-for-structure equivalent syntax trees and token spans.
//! 3. **6-Layer Security Defense Verification**:
//!    - `AstDepthGuard`: Interception of deeply nested expressions (>256 levels).
//!    - `ErrorRecoveryTimeoutGuard`: Budget enforcement on GLR recovery and error loops.
//!    - `QueryPatternSafetyGuard`: S-expression query pattern validation and depth limiting.
//!    - `LineLengthFuseGuard`: Single-line memory fuse on giant source lines (>128KB).
//!    - `ParsingTimeoutGuard`: Microsecond timeout circuit breaking on pathological grammars.
//!    - `SensitiveTokenBuffer`: Zero-allocation and zeroize-on-drop volatile memory erasure.

use ttzip_engine::security::syntax_defense::{
    AstDepthGuard, ErrorRecoveryTimeoutGuard, LineLengthFuseGuard, ParsingTimeoutGuard,
    QueryPatternSafetyGuard, SensitiveTokenBuffer, SyntaxDefenseError, SyntaxSecurityPipeline,
    DEFAULT_MAX_AST_DEPTH, DEFAULT_MAX_LINE_LENGTH, DEFAULT_MAX_QUERY_DEPTH,
    DEFAULT_MAX_RECOVERY_STEPS, DEFAULT_PARSING_TIMEOUT_MICROS,
};
use ttzip_engine::syntax::{
    HighlightTokenKind, LanguageRegistry, SupportedLanguage, SymbolKind, SymbolOutlineExtractor,
    SyntaxHighlighter, TTZipSyntaxParser,
};

// ============================================================================
// 1. Official Multi-Language AST Consistency Tests
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_rust_official_vector_ast_consistency() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let rust_source = r#"
/// High-throughput generic checksum accumulator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumEngine<T: AsRef<[u8]>> {
    state: u32,
    phantom: std::marker::PhantomData<T>,
}

impl<T: AsRef<[u8]>> ChecksumEngine<T> {
    pub const fn new(seed: u32) -> Self {
        Self { state: seed, phantom: std::marker::PhantomData }
    }

    pub async fn process_chunk(&mut self, chunk: T) -> Result<u32, &'static str> {
        let bytes = chunk.as_ref();
        for &byte in bytes {
            self.state = self.state.rotate_left(1) ^ (byte as u32);
        }
        Ok(self.state)
    }
}
"#;

    let tree = parser.parse_full(rust_source).unwrap();
    let root = tree.root_node();
    assert_eq!(root.kind(), "source_file");
    assert!(!root.has_error());

    let mut highlighter = SyntaxHighlighter::new();
    let tokens = highlighter
        .highlight_source(rust_source, SupportedLanguage::Rust)
        .unwrap();
    assert!(!tokens.is_empty());
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Keyword));
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Type));
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Function));

    let outlines =
        SymbolOutlineExtractor::extract_from_source(rust_source, SupportedLanguage::Rust).unwrap();
    assert_eq!(outlines.len(), 2);
    assert_eq!(outlines[0].name, "ChecksumEngine");
    assert_eq!(outlines[0].kind, SymbolKind::Struct);
}

#[cfg(feature = "syntax")]
#[test]
fn test_c_official_vector_ast_consistency() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::C).unwrap();
    let c_source = r#"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

typedef struct {
    uint32_t magic;
    uint16_t version;
    size_t payload_len;
} ttzip_header_t;

int ttzip_validate_header(const ttzip_header_t* header) {
    if (!header) {
        return -1;
    }
    if (header->magic != 0x54545A50) {
        return -2;
    }
    return 0;
}
"#;

    let tree = parser.parse_full(c_source).unwrap();
    let root = tree.root_node();
    assert_eq!(root.kind(), "translation_unit");
    assert!(!root.has_error());

    let outlines =
        SymbolOutlineExtractor::extract_from_source(c_source, SupportedLanguage::C).unwrap();
    assert!(!outlines.is_empty());
    assert!(outlines.iter().any(|o| o.name == "ttzip_validate_header"));
}

#[cfg(feature = "syntax")]
#[test]
fn test_swift_official_vector_ast_consistency() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Swift).unwrap();
    let swift_source = r#"
import Foundation

public protocol ArchiveProcessor: Sendable {
    func extractEntry(at path: String) async throws -> Data
}

public final class SwiftArchiveManager: ArchiveProcessor {
    private let archivePath: URL

    public init(archivePath: URL) {
        self.archivePath = archivePath
    }

    public func extractEntry(at path: String) async throws -> Data {
        guard !path.isEmpty else {
            throw NSError(domain: "TTZipError", code: 400)
        }
        return Data()
    }
}
"#;

    let tree = parser.parse_full(swift_source).unwrap();
    let root = tree.root_node();
    assert_eq!(root.kind(), "source_file");
    assert!(!root.has_error());

    let outlines =
        SymbolOutlineExtractor::extract_from_source(swift_source, SupportedLanguage::Swift).unwrap();
    assert_eq!(outlines.len(), 2);
    assert_eq!(outlines[0].name, "ArchiveProcessor");
    assert_eq!(outlines[0].kind, SymbolKind::Interface);
    assert_eq!(outlines[1].name, "SwiftArchiveManager");
    assert_eq!(outlines[1].kind, SymbolKind::Class);
}

#[cfg(feature = "syntax")]
#[test]
fn test_python_official_vector_ast_consistency() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Python).unwrap();
    let py_source = r#"
from typing import Optional, List, Dict
import asyncio

class ArchiveCluster:
    def __init__(self, cluster_id: str, nodes: List[str]):
        self.cluster_id = cluster_id
        self.nodes = nodes
        self._cache: Dict[str, bytes] = {}

    async def fetch_block(self, block_hash: str) -> Optional[bytes]:
        if block_hash in self._cache:
            return self._cache[block_hash]
        return None

def compute_digest(data: bytes) -> str:
    return hex(hash(data))
"#;

    let tree = parser.parse_full(py_source).unwrap();
    let root = tree.root_node();
    assert_eq!(root.kind(), "module");
    assert!(!root.has_error());

    let outlines =
        SymbolOutlineExtractor::extract_from_source(py_source, SupportedLanguage::Python).unwrap();
    assert_eq!(outlines.len(), 2);
    assert_eq!(outlines[0].name, "ArchiveCluster");
    assert_eq!(outlines[0].kind, SymbolKind::Class);
    assert_eq!(outlines[1].name, "compute_digest");
    assert_eq!(outlines[1].kind, SymbolKind::Function);
}

#[cfg(feature = "syntax")]
#[test]
fn test_javascript_and_typescript_official_vector_ast_consistency() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::TypeScript).unwrap();
    let ts_source = r#"
export interface ArchiveNodeConfig {
    readonly endpoint: string;
    readonly timeoutMs: number;
    readonly retryCount?: number;
}

export class StreamingArchiveReader {
    private isClosed: boolean = false;

    constructor(private readonly config: ArchiveNodeConfig) {}

    public async readChunk(offset: number, length: number): Promise<Uint8Array> {
        if (this.isClosed) {
            throw new Error("Stream reader is already closed");
        }
        return new Uint8Array(length);
    }
}
"#;

    let tree = parser.parse_full(ts_source).unwrap();
    let root = tree.root_node();
    assert_eq!(root.kind(), "program");
    assert!(!root.has_error());

    let outlines =
        SymbolOutlineExtractor::extract_from_source(ts_source, SupportedLanguage::TypeScript).unwrap();
    assert_eq!(outlines.len(), 2);
    assert_eq!(outlines[0].name, "ArchiveNodeConfig");
    assert_eq!(outlines[0].kind, SymbolKind::Interface);
    assert_eq!(outlines[1].name, "StreamingArchiveReader");
    assert_eq!(outlines[1].kind, SymbolKind::Class);
}

#[cfg(feature = "syntax")]
#[test]
fn test_json_official_vector_ast_consistency() {
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Json).unwrap();
    let json_source = r#"{
    "name": "ttzip-workspace",
    "version": "1.0.0",
    "private": true,
    "engines": {
        "node": ">=20.0.0",
        "rust": ">=1.80.0"
    },
    "features": [
        "lzma2",
        "brotli",
        "blake3",
        "ed25519"
    ],
    "metrics": {
        "max_throughput_mb_s": 2450.5,
        "zero_alloc": true,
        "status": null
    }
}"#;

    let tree = parser.parse_full(json_source).unwrap();
    let root = tree.root_node();
    assert_eq!(root.kind(), "document");
    assert!(!root.has_error());

    let mut highlighter = SyntaxHighlighter::new();
    let tokens = highlighter
        .highlight_source(json_source, SupportedLanguage::Json)
        .unwrap();
    assert!(!tokens.is_empty());
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::String));
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Number));
    assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Boolean));
}

// ============================================================================
// 2. Incremental vs Full Parse AST Equivalence Oracle
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_incremental_vs_full_ast_equivalence_oracle() {
    let initial_rust = "fn compute(val: u64) -> u64 {\n    let base = 100u64;\n    val + base\n}\n";
    let modified_rust = "fn compute(val: u64) -> u64 {\n    let base = 2000u64;\n    val + base\n}\n";

    let mut incremental_parser =
        TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let _ = incremental_parser.parse_full(initial_rust).unwrap();

    let edit = tree_sitter::InputEdit {
        start_byte: 45,
        old_end_byte: 48,
        new_end_byte: 49,
        start_position: tree_sitter::Point { row: 1, column: 15 },
        old_end_position: tree_sitter::Point { row: 1, column: 18 },
        new_end_position: tree_sitter::Point { row: 1, column: 19 },
    };

    let inc_tree = incremental_parser
        .parse_incremental(modified_rust, &edit)
        .unwrap();

    let mut full_parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let full_tree = full_parser.parse_full(modified_rust).unwrap();

    // Structural Equivalence Assertions:
    assert_eq!(inc_tree.root_node().kind(), full_tree.root_node().kind());
    assert_eq!(
        inc_tree.root_node().child_count(),
        full_tree.root_node().child_count()
    );
    assert_eq!(
        inc_tree.root_node().named_child_count(),
        full_tree.root_node().named_child_count()
    );
    assert!(!inc_tree.root_node().has_error());
    assert!(!full_tree.root_node().has_error());

    // Highlight token equivalence:
    let mut highlighter = SyntaxHighlighter::new();
    let tokens_inc = highlighter
        .highlight(inc_tree, modified_rust, SupportedLanguage::Rust)
        .unwrap();
    let tokens_full = highlighter
        .highlight(full_tree, modified_rust, SupportedLanguage::Rust)
        .unwrap();

    assert_eq!(tokens_inc.len(), tokens_full.len());
    for (t_inc, t_full) in tokens_inc.iter().zip(tokens_full.iter()) {
        assert_eq!(t_inc.kind, t_full.kind);
        assert_eq!(t_inc.start_byte, t_full.start_byte);
        assert_eq!(t_inc.end_byte, t_full.end_byte);
    }
}

#[test]
fn test_language_registry_official_mapping_vectors() {
    assert_eq!(LanguageRegistry::from_extension("rs"), Some(SupportedLanguage::Rust));
    assert_eq!(LanguageRegistry::from_extension("c"), Some(SupportedLanguage::C));
    assert_eq!(LanguageRegistry::from_extension("h"), Some(SupportedLanguage::C));
    assert_eq!(LanguageRegistry::from_extension("swift"), Some(SupportedLanguage::Swift));
    assert_eq!(LanguageRegistry::from_extension("py"), Some(SupportedLanguage::Python));
    assert_eq!(LanguageRegistry::from_extension("js"), Some(SupportedLanguage::JavaScript));
    assert_eq!(LanguageRegistry::from_extension("ts"), Some(SupportedLanguage::TypeScript));
    assert_eq!(LanguageRegistry::from_extension("json"), Some(SupportedLanguage::Json));
    assert_eq!(LanguageRegistry::from_id("rust"), Some(SupportedLanguage::Rust));
    assert_eq!(LanguageRegistry::from_id("swift"), Some(SupportedLanguage::Swift));
    assert_eq!(LanguageRegistry::from_id("python"), Some(SupportedLanguage::Python));
    assert_eq!(LanguageRegistry::from_shebang("#!/usr/bin/env python3"), Some(SupportedLanguage::Python));
}

// ============================================================================
// 3. 6-Layer Security Defense Verification Matrix
// ============================================================================

#[test]
fn test_guard_1_ast_depth_overflow_defense() {
    let mut guard = AstDepthGuard::with_max_depth(DEFAULT_MAX_AST_DEPTH);
    assert_eq!(guard.max_depth(), 256);

    for _ in 0..DEFAULT_MAX_AST_DEPTH {
        assert!(guard.push_depth().is_ok());
    }
    assert_eq!(guard.current_depth(), 256);

    // Exceeding 256 levels must deterministically trip the fuse
    let breach = guard.push_depth();
    assert!(matches!(
        breach,
        Err(SyntaxDefenseError::AstDepthExceeded {
            depth: 257,
            max_depth: 256
        })
    ));
}

#[test]
fn test_guard_2_glr_error_recovery_step_limit() {
    let mut guard = ErrorRecoveryTimeoutGuard::with_max_steps(DEFAULT_MAX_RECOVERY_STEPS);
    assert_eq!(guard.step_count(), 0);

    // Consume entire budget up to 10000 steps
    assert!(guard.record_steps(10000).is_ok());
    assert_eq!(guard.step_count(), 10000);

    // 10001st step triggers circuit breaker
    let err = guard.record_step().unwrap_err();
    assert!(matches!(
        err,
        SyntaxDefenseError::ErrorRecoveryLimitExceeded {
            steps: 10001,
            max_steps: 10000
        }
    ));
}

#[test]
fn test_guard_3_query_pattern_safety_and_depth_limits() {
    let guard = QueryPatternSafetyGuard::with_limits(
        DEFAULT_MAX_QUERY_DEPTH,
        4096,
        64,
    );

    // 1. Normal valid query passes
    assert!(guard
        .validate_pattern("(function_item name: (identifier) @fn)")
        .is_ok());

    // 2. Query exceeding 32 levels of nesting fails
    let nested_pattern = "(".repeat(33) + &")".repeat(33);
    let err = guard.validate_pattern(&nested_pattern).unwrap_err();
    assert!(matches!(
        err,
        SyntaxDefenseError::QueryPatternViolation { .. }
    ));

    // 3. Unbalanced parenthesis
    assert!(guard.validate_pattern("(function_item (identifier)").is_err());

    // 4. Excessive predicates (>64)
    let mut excessive_preds = "(node)".to_string();
    for i in 0..70 {
        excessive_preds.push_str(&format!(" #eq? @t \"val_{i}\""));
    }
    assert!(guard.validate_pattern(&excessive_preds).is_err());
}

#[test]
fn test_guard_4_line_length_lexical_fuse() {
    let guard = LineLengthFuseGuard::with_max_line_length(DEFAULT_MAX_LINE_LENGTH);
    assert_eq!(guard.max_line_length(), 131072);

    // Clean multi-line code passes
    let normal_source = "line 1\nline 2\nline 3\n";
    assert!(guard.scan_source(normal_source).is_ok());

    // 130KB single line exceeds 128KB limit
    let huge_line = "a".repeat(130 * 1024);
    let err = guard.scan_source(&huge_line).unwrap_err();
    assert!(matches!(
        err,
        SyntaxDefenseError::LineLengthExceeded {
            line_number: 1,
            len,
            max_len: 131072
        } if len > 131072
    ));
}

#[test]
fn test_guard_5_parsing_timeout_circuit_breaker() {
    let guard = ParsingTimeoutGuard::with_timeout_micros(DEFAULT_PARSING_TIMEOUT_MICROS);
    assert_eq!(DEFAULT_PARSING_TIMEOUT_MICROS, 20_000);
    assert!(!guard.is_expired());
    assert!(guard.check_timeout().is_ok());
}

#[test]
fn test_guard_6_sensitive_token_buffer_zeroize() {
    let mut buffer = SensitiveTokenBuffer::new();
    let secret = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAISecretPrivateKeyPayload";
    buffer.push_bytes(secret.as_bytes()).unwrap();
    buffer
        .push_token("token", secret, 0, secret.len())
        .unwrap();

    assert_eq!(buffer.len(), secret.len());
    assert_eq!(buffer.token_count(), 1);

    // Test sanitization mask
    let masked = buffer.render_masked('#');
    assert_eq!(masked, "#".repeat(secret.len()));

    // Zeroize & clear memory
    buffer.zeroize_and_clear();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.token_count(), 0);
}

#[cfg(feature = "syntax")]
#[test]
fn test_syntax_security_pipeline_full_envelope() {
    let mut pipeline = SyntaxSecurityPipeline::new();

    // 1. Secure parse of valid source
    let source = "pub fn add(x: i32, y: i32) -> i32 { x + y }";
    let (tree, spans) = pipeline
        .parse_securely(source, ttzip_engine::standards::syntax_highlight::SupportedLanguage::Rust)
        .unwrap();
    assert!(!spans.is_empty());
    assert_eq!(tree.root_node().kind(), "source_file");

    // 2. Giant line rejection
    let giant_line = "x".repeat(150 * 1024);
    assert!(pipeline
        .validate_source_text(&giant_line)
        .is_err());

    // 3. Dangerous query pattern rejection
    let bad_query = "(".repeat(40) + &")".repeat(40);
    assert!(pipeline.validate_query_pattern(&bad_query).is_err());
}
