// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Types and Service Definitions for Syntax and Outline Metadata.

use std::sync::Arc;

use super::detector::{detect_language_internal, list_supported_languages};
use super::highlighter::{highlight_code_internal, highlight_viewport_internal};
use super::outline::extract_symbols_internal;

/// High-precision highlight token span exposed across UniFFI boundary with UTF-16 NSRange metrics.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIHighlightToken {
    /// Zero-based character start index in UTF-16 code units (NSRange location).
    pub location: u32,
    /// Length of token span in UTF-16 code units (NSRange length).
    pub length: u32,
    /// Syntactic category ("keyword", "string", "number", "type", "function", "comment", "operator", etc.).
    pub category: String,
    /// 1-based source line number for fast viewport filtering.
    pub line_number: u32,
    /// 0-based character column offset in UTF-16 code units.
    pub column: u32,
}

/// Hierarchical structural symbol node for document and source code outline tree.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFISymbolNode {
    /// Identifier name (e.g. "struct ArchiveReader", "func decompress()").
    pub name: String,
    /// Symbol semantic classification ("function", "struct", "enum", "class", "interface", "trait", "impl", "variable", "constant", "module", "type", "macro", "heading", "property").
    pub kind: String,
    /// Zero-based character start index in UTF-16 code units.
    pub location: u32,
    /// Length of symbol span in UTF-16 code units.
    pub length: u32,
    /// 1-based source line number of the declaration.
    pub line_number: u32,
    /// Optional additional signature, type, or detail description.
    pub detail: Option<String>,
    /// Hierarchical nested child symbols (e.g. methods and properties inside a class or struct).
    pub children: Vec<UniFFISymbolNode>,
}

/// Metadata descriptor of a programming or markup language supported by TTZip.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFILanguageInfo {
    /// Canonical language identifier string (e.g. "rust", "swift", "python", "json").
    pub language_id: String,
    /// Human-readable display name (e.g. "Rust", "Swift", "Python", "JSON").
    pub display_name: String,
    /// List of standard file extensions associated with this language without leading dot.
    pub file_extensions: Vec<String>,
    /// Standard MIME content types.
    pub mime_types: Vec<String>,
    /// Whether high-precision AST / Tree-sitter parsing is natively supported.
    pub is_supported: bool,
}

/// Stateful UniFFI service managing syntax tokenization, language detection, and outline extraction.
#[derive(uniffi::Object, Default)]
pub struct UniFFISyntaxService {}

#[uniffi::export]
impl UniFFISyntaxService {
    /// Constructs a new thread-safe syntax metadata service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Tokenizes source code into UTF-16 NSRange highlight tokens with optional max length truncation.
    pub fn highlight_code(&self, code: String, language_hint: String, max_length: u32) -> Vec<UniFFIHighlightToken> {
        highlight_code_internal(&code, &language_hint, max_length)
    }

    /// Tokenizes source code restricted to a specific line viewport for high-performance virtualized rendering.
    pub fn highlight_code_viewport(
        &self,
        code: String,
        language_hint: String,
        start_line: u32,
        line_count: u32,
    ) -> Vec<UniFFIHighlightToken> {
        highlight_viewport_internal(&code, &language_hint, start_line, line_count)
    }

    /// Extracts hierarchical symbol outline tree (functions, classes, structs, traits, headings).
    pub fn extract_symbols(&self, code: String, language_hint: String) -> Vec<UniFFISymbolNode> {
        extract_symbols_internal(&code, &language_hint)
    }

    /// Detects language from filename, extension, and optional first line content (e.g. shebang).
    pub fn detect_language(&self, file_path_or_ext: String, first_line_hint: Option<String>) -> UniFFILanguageInfo {
        detect_language_internal(&file_path_or_ext, first_line_hint.as_deref())
    }

    /// Returns list of all known programming and markup languages supported by the engine.
    pub fn get_supported_languages(&self) -> Vec<UniFFILanguageInfo> {
        list_supported_languages()
    }
}
