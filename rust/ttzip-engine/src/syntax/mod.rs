// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip High-Performance Tree-sitter AST Syntax Engine.
//!
//! Provides incremental parsing, viewport-sliced syntax highlighting,
//! zero-allocation symbol outline trees, and multi-language Shebang auto-detection.

mod error;
pub mod highlighter;
pub mod outline;
pub mod parser;
pub mod registry;

pub use error::{SyntaxError, SyntaxResult};
pub use highlighter::{HighlightToken, HighlightTokenKind, SyntaxHighlighter};
pub use outline::{SymbolKind, SymbolNode, SymbolOutlineExtractor};
pub use parser::TTZipSyntaxParser;
pub use registry::{LanguageRegistry, SupportedLanguage};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_extension_and_id_detection() {
        assert_eq!(
            LanguageRegistry::from_extension("rs"),
            Some(SupportedLanguage::Rust)
        );
        assert_eq!(
            LanguageRegistry::from_extension("swift"),
            Some(SupportedLanguage::Swift)
        );
        assert_eq!(
            LanguageRegistry::from_extension("cpp"),
            Some(SupportedLanguage::Cpp)
        );
        assert_eq!(
            LanguageRegistry::from_extension("py"),
            Some(SupportedLanguage::Python)
        );
        assert_eq!(
            LanguageRegistry::from_extension("ts"),
            Some(SupportedLanguage::TypeScript)
        );
        assert_eq!(
            LanguageRegistry::from_extension("tsx"),
            Some(SupportedLanguage::Tsx)
        );
        assert_eq!(
            LanguageRegistry::from_extension("json"),
            Some(SupportedLanguage::Json)
        );
        assert_eq!(
            LanguageRegistry::from_extension("html"),
            Some(SupportedLanguage::Html)
        );
        assert_eq!(
            LanguageRegistry::from_extension("css"),
            Some(SupportedLanguage::Css)
        );
        assert_eq!(
            LanguageRegistry::from_extension("md"),
            Some(SupportedLanguage::Markdown)
        );

        assert_eq!(
            LanguageRegistry::from_id("rust"),
            Some(SupportedLanguage::Rust)
        );
        assert_eq!(
            LanguageRegistry::from_id("PYTHON"),
            Some(SupportedLanguage::Python)
        );
        assert_eq!(
            LanguageRegistry::from_id("json5"),
            Some(SupportedLanguage::Json)
        );
    }

    #[test]
    fn test_registry_shebang_and_heuristic_detection() {
        assert_eq!(
            LanguageRegistry::from_shebang("#!/usr/bin/env python3\nimport sys"),
            Some(SupportedLanguage::Python)
        );
        assert_eq!(
            LanguageRegistry::from_shebang("#!/usr/bin/env node\nconsole.log('hi')"),
            Some(SupportedLanguage::JavaScript)
        );
        assert_eq!(
            LanguageRegistry::from_shebang("#!/usr/bin/env ts-node\nconst a: number = 1;"),
            Some(SupportedLanguage::TypeScript)
        );
        assert_eq!(
            LanguageRegistry::from_shebang("#!/usr/bin/env swift\nimport Foundation"),
            Some(SupportedLanguage::Swift)
        );
        assert_eq!(
            LanguageRegistry::from_shebang("<!DOCTYPE html><html><body><h1>Title</h1></body></html>"),
            Some(SupportedLanguage::Html)
        );
        assert_eq!(
            LanguageRegistry::from_shebang(r#"{"name": "ttzip", "version": "1.0.0"}"#),
            Some(SupportedLanguage::Json)
        );
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_all_language_highlight_queries_compile() {
        for lang in LanguageRegistry::ALL_LANGUAGES {
            if let Some(ts_lang) = lang.tree_sitter_language() {
                let query_str = lang.default_highlight_query();
                let query_res = tree_sitter::Query::new(&ts_lang, query_str);
                assert!(
                    query_res.is_ok(),
                    "Failed to compile highlight query for {:?}: {:?}",
                    lang,
                    query_res.err()
                );
            }
        }
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_syntax_parser_full_and_incremental() {
        let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
        let code_v1 = "fn calculate_sum(a: u32, b: u32) -> u32 {\n    a + b\n}";
        let tree_v1 = parser.parse_full(code_v1).unwrap();
        assert!(!tree_v1.root_node().has_error());
        assert_eq!(parser.current_language(), Some(SupportedLanguage::Rust));

        // Perform incremental edit: change "calculate_sum" to "calculate_total"
        let code_v2 = "fn calculate_total(a: u32, b: u32) -> u32 {\n    a + b\n}";
        let edit = tree_sitter::InputEdit {
            start_byte: 3,
            old_end_byte: 16,
            new_end_byte: 18,
            start_position: tree_sitter::Point { row: 0, column: 3 },
            old_end_position: tree_sitter::Point { row: 0, column: 16 },
            new_end_position: tree_sitter::Point { row: 0, column: 18 },
        };

        let start_time = std::time::Instant::now();
        let tree_v2 = parser.parse_incremental(code_v2, &edit).unwrap();
        let elapsed = start_time.elapsed();
        assert!(!tree_v2.root_node().has_error());
        assert!(elapsed.as_millis() < 50);

        let root = tree_v2.root_node();
        let fn_node = root.child(0).unwrap();
        assert_eq!(fn_node.kind(), "function_item");
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_syntax_highlighter_tokenization_and_viewport_slicing() {
        let mut highlighter = SyntaxHighlighter::new();
        let rust_code = r#"
/// Computes archive checksum.
pub fn compute_crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for b in data {
        crc ^= *b as u32;
    }
    crc
}
"#;

        let tokens = highlighter
            .highlight_source(rust_code, SupportedLanguage::Rust)
            .unwrap();

        assert!(!tokens.is_empty());
        assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Keyword));
        assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Function));
        assert!(tokens.iter().any(|t| t.kind == HighlightTokenKind::Comment));

        // Test viewport slicing: highlight only a byte range
        let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
        let tree = parser.parse_full(rust_code).unwrap();
        let range = 35..85;
        let viewport_tokens = highlighter
            .highlight_range(tree, rust_code, SupportedLanguage::Rust, Some(range.clone()))
            .unwrap();

        assert!(!viewport_tokens.is_empty());
        for t in &viewport_tokens {
            assert!(t.start_byte < range.end && t.end_byte > range.start);
        }
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_symbol_outline_extraction_rust() {
        let rust_code = r#"
pub struct ArchiveHeader {
    pub magic: u32,
    pub version: u16,
}

pub enum ArchiveFormat {
    Zip,
    Tar,
    SevenZ,
}

impl ArchiveHeader {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        None
    }
}
"#;

        let outlines =
            SymbolOutlineExtractor::extract_from_source(rust_code, SupportedLanguage::Rust)
                .unwrap();

        assert_eq!(outlines.len(), 3);
        assert_eq!(outlines[0].name, "ArchiveHeader");
        assert_eq!(outlines[0].kind, SymbolKind::Struct);

        assert_eq!(outlines[1].name, "ArchiveFormat");
        assert_eq!(outlines[1].kind, SymbolKind::Enum);

        assert!(outlines[2].name.contains("impl ArchiveHeader"));
        assert_eq!(outlines[2].children.len(), 1);
        assert_eq!(outlines[2].children[0].name, "parse");
        assert_eq!(outlines[2].children[0].kind, SymbolKind::Method);
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_symbol_outline_extraction_python() {
        let py_code = r#"
class TTZipStream:
    def __init__(self, path: str):
        self.path = path

    def read_block(self, size: int) -> bytes:
        return b""

def standalone_helper():
    pass
"#;

        let outlines =
            SymbolOutlineExtractor::extract_from_source(py_code, SupportedLanguage::Python)
                .unwrap();

        assert_eq!(outlines.len(), 2);
        assert_eq!(outlines[0].name, "TTZipStream");
        assert_eq!(outlines[0].kind, SymbolKind::Class);
        assert_eq!(outlines[0].children.len(), 2);
        assert_eq!(outlines[0].children[0].name, "__init__");
        assert_eq!(outlines[0].children[0].kind, SymbolKind::Method);

        assert_eq!(outlines[1].name, "standalone_helper");
        assert_eq!(outlines[1].kind, SymbolKind::Function);
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_symbol_outline_extraction_markdown_hierarchy() {
        let md_content = r#"
# Architecture Overview
Some intro.

## Storage Engine
Details on storage.

### Block Allocator
Allocator internals.

## VFS Layer
Virtual filesystem details.
"#;

        let outlines = SymbolOutlineExtractor::extract_from_source(
            md_content,
            SupportedLanguage::Markdown,
        )
        .unwrap();

        assert_eq!(outlines.len(), 1);
        assert_eq!(outlines[0].name, "Architecture Overview");
        assert_eq!(outlines[0].children.len(), 2);
        assert_eq!(outlines[0].children[0].name, "Storage Engine");
        assert_eq!(outlines[0].children[0].children.len(), 1);
        assert_eq!(outlines[0].children[0].children[0].name, "Block Allocator");
        assert_eq!(outlines[0].children[1].name, "VFS Layer");
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_symbol_find_at_position() {
        let rust_code = "fn alpha() {\n    let x = 1;\n}\n\nfn beta() {\n    let y = 2;\n}";
        let outlines =
            SymbolOutlineExtractor::extract_from_source(rust_code, SupportedLanguage::Rust)
                .unwrap();

        let found_alpha = SymbolOutlineExtractor::find_symbol_at_position(&outlines, 1, 4);
        assert!(found_alpha.is_some());
        assert_eq!(found_alpha.unwrap().name, "alpha");

        let found_beta = SymbolOutlineExtractor::find_symbol_at_position(&outlines, 5, 4);
        assert!(found_beta.is_some());
        assert_eq!(found_beta.unwrap().name, "beta");
    }
}
