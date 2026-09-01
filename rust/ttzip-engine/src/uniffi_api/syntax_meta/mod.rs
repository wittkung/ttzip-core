// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for Syntax Highlighting and Outline Metaprogramming.
//!
//! Provides zero-disk-landing in-memory AST tokenization, viewport coloring, language
//! heuristic detection, and hierarchical symbol tree extraction for Swift 6 UI inspector
//! and QuickLook preview pipelines.

pub mod detector;
pub mod highlighter;
pub mod outline;
pub mod types;

use std::sync::Arc;

pub use detector::{detect_language_internal, list_supported_languages};
pub use highlighter::{highlight_code_internal, highlight_viewport_internal, SourceLayoutIndex};
pub use outline::extract_symbols_internal;
pub use types::{UniFFIHighlightToken, UniFFILanguageInfo, UniFFISymbolNode, UniFFISyntaxService};

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Tokenizes source code into high-precision UTF-16 NSRange highlight tokens.
#[uniffi::export]
pub fn uniffi_highlight_code(code: String, language_hint: String, max_length: u32) -> Vec<UniFFIHighlightToken> {
    highlight_code_internal(&code, &language_hint, max_length)
}

/// Tokenizes source code restricted to a line viewport [start_line, start_line + line_count).
#[uniffi::export]
pub fn uniffi_highlight_code_viewport(
    code: String,
    language_hint: String,
    start_line: u32,
    line_count: u32,
) -> Vec<UniFFIHighlightToken> {
    highlight_viewport_internal(&code, &language_hint, start_line, line_count)
}

/// Extracts structural outline symbol tree from source code text.
#[uniffi::export]
pub fn uniffi_extract_symbols(code: String, language_hint: String) -> Vec<UniFFISymbolNode> {
    extract_symbols_internal(&code, &language_hint)
}

/// Detects language from filename, extension, and optional first line content hint.
#[uniffi::export]
pub fn uniffi_detect_language(file_path_or_ext: String, first_line_hint: Option<String>) -> UniFFILanguageInfo {
    detect_language_internal(&file_path_or_ext, first_line_hint.as_deref())
}

/// Returns list of all known programming and markup languages supported by the engine.
#[uniffi::export]
pub fn uniffi_get_supported_languages() -> Vec<UniFFILanguageInfo> {
    list_supported_languages()
}

/// Instantiates a new thread-safe syntax metadata service.
#[uniffi::export]
pub fn uniffi_syntax_service_new() -> Arc<UniFFISyntaxService> {
    UniFFISyntaxService::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_free_functions_e2e() {
        let rust_code = "pub fn add(a: i32, b: i32) -> i32 { a + b }";
        let tokens = uniffi_highlight_code(rust_code.to_string(), "rs".to_string(), 0);
        assert!(!tokens.is_empty());
        assert!(tokens.iter().any(|t| t.category == "keyword"));

        let symbols = uniffi_extract_symbols(rust_code.to_string(), "rs".to_string());
        assert!(!symbols.is_empty());

        let lang = uniffi_detect_language("test.rs".to_string(), None);
        assert_eq!(lang.language_id, "rust");
        assert!(lang.is_supported);

        let service = uniffi_syntax_service_new();
        let svc_tokens = service.highlight_code(rust_code.to_string(), "rs".to_string(), 0);
        assert_eq!(tokens.len(), svc_tokens.len());
    }
}
