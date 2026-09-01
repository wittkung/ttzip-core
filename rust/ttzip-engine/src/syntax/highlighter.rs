// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tree-sitter Query-based syntax highlighter with viewport range slicing and token normalization.

use std::collections::HashMap;
use std::ops::Range;
use serde::{Deserialize, Serialize};

use super::error::{SyntaxError, SyntaxResult};
use super::parser::TTZipSyntaxParser;
use super::registry::SupportedLanguage;

/// Token classification category for syntax styling and color theming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HighlightTokenKind {
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Operator,
    Punctuation,
    Variable,
    Constant,
    Attribute,
    Property,
    Tag,
    Heading,
    Boolean,
    Error,
}

impl HighlightTokenKind {
    /// String identifier of the token category.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Function => "function",
            Self::Type => "type",
            Self::String => "string",
            Self::Number => "number",
            Self::Comment => "comment",
            Self::Operator => "operator",
            Self::Punctuation => "punctuation",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::Attribute => "attribute",
            Self::Property => "property",
            Self::Tag => "tag",
            Self::Heading => "heading",
            Self::Boolean => "boolean",
            Self::Error => "error",
        }
    }

    /// Maps tree-sitter capture tag string to semantic token category.
    pub fn from_capture_name(capture: &str) -> Self {
        let root = capture.split('.').next().unwrap_or(capture);
        match root {
            "keyword" => Self::Keyword,
            "function" | "method" | "constructor" => Self::Function,
            "type" | "class" | "interface" | "struct" | "enum" => Self::Type,
            "string" | "character" => Self::String,
            "number" | "float" | "integer" => Self::Number,
            "comment" => Self::Comment,
            "operator" => Self::Operator,
            "punctuation" | "delimiter" | "bracket" => Self::Punctuation,
            "variable" | "parameter" => Self::Variable,
            "constant" => Self::Constant,
            "attribute" | "decorator" | "annotation" | "preproc" => Self::Attribute,
            "property" | "field" | "member" => Self::Property,
            "tag" => Self::Tag,
            "heading" | "title" => Self::Heading,
            "boolean" => Self::Boolean,
            "error" => Self::Error,
            _ => {
                if capture.contains("func") || capture.contains("call") {
                    Self::Function
                } else if capture.contains("type") {
                    Self::Type
                } else if capture.contains("comment") {
                    Self::Comment
                } else if capture.contains("string") {
                    Self::String
                } else {
                    Self::Variable
                }
            }
        }
    }
}

/// Highlighted code token with byte range and UTF-16 NSRange metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightToken {
    /// Byte offset range in UTF-8 source.
    pub start_byte: usize,
    pub end_byte: usize,
    /// UTF-16 location and length for Apple TextKit / NSRange.
    pub utf16_location: u32,
    pub utf16_length: u32,
    /// Line index (0-based).
    pub start_line: usize,
    /// Column index (0-based UTF-8 byte offset in line).
    pub start_col: usize,
    /// Semantic token category.
    pub kind: HighlightTokenKind,
    /// Raw Tree-sitter capture name (e.g. `function.call`).
    pub capture_name: String,
}

impl HighlightToken {
    /// Token byte range.
    #[inline]
    pub fn byte_range(&self) -> Range<usize> {
        self.start_byte..self.end_byte
    }
}

/// UTF-16 NSRange lookup index builder.
#[derive(Debug, Clone)]
pub struct Utf16Index {
    byte_to_u16: Vec<u32>,
    total_u16: u32,
}

impl Utf16Index {
    /// Builds byte to UTF-16 offset lookup table.
    pub fn new(text: &str) -> Self {
        let mut byte_to_u16 = Vec::with_capacity(text.len() + 1);
        let mut acc = 0u32;
        for b in text.bytes() {
            byte_to_u16.push(acc);
            if (b as i8) >= -0x40 {
                acc += if b < 0xF0 { 1 } else { 2 };
            }
        }
        byte_to_u16.push(acc);
        Self {
            byte_to_u16,
            total_u16: acc,
        }
    }

    /// Converts byte range `[start, end)` to `(location, length)` in UTF-16 units.
    #[inline]
    pub fn byte_range_to_utf16(&self, start: usize, end: usize) -> (u32, u32) {
        let loc = self
            .byte_to_u16
            .get(start)
            .copied()
            .unwrap_or(self.total_u16);
        let end_loc = self.byte_to_u16.get(end).copied().unwrap_or(self.total_u16);
        (loc, end_loc.saturating_sub(loc))
    }
}

/// AST query-driven syntax highlighter with query caching and viewport slicing.
pub struct SyntaxHighlighter {
    #[cfg(feature = "syntax")]
    query_cache: HashMap<SupportedLanguage, tree_sitter::Query>,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    /// Creates a new syntax highlighter with empty query cache.
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "syntax")]
            query_cache: HashMap::new(),
        }
    }

    /// Pre-compiles and caches highlight queries for specified language.
    #[cfg(feature = "syntax")]
    pub fn register_custom_query(
        &mut self,
        lang: SupportedLanguage,
        query_source: &str,
    ) -> SyntaxResult<()> {
        let ts_lang = lang.tree_sitter_language().ok_or_else(|| {
            SyntaxError::UnsupportedLanguage(format!("{}: missing grammar", lang.id()))
        })?;

        let query = tree_sitter::Query::new(&ts_lang, query_source)
            .map_err(|e| SyntaxError::QueryCompilationError(e.to_string()))?;

        self.query_cache.insert(lang, query);
        Ok(())
    }

    /// Retrieves or compiles the Tree-sitter query for the target language.
    #[cfg(feature = "syntax")]
    fn get_or_compile_query(
        &mut self,
        lang: SupportedLanguage,
    ) -> SyntaxResult<&tree_sitter::Query> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.query_cache.entry(lang) {
            let ts_lang = lang.tree_sitter_language().ok_or_else(|| {
                SyntaxError::UnsupportedLanguage(format!("{}: missing grammar", lang.id()))
            })?;

            let query_str = lang.default_highlight_query();
            let query = tree_sitter::Query::new(&ts_lang, query_str)
                .map_err(|e| SyntaxError::QueryCompilationError(e.to_string()))?;

            e.insert(query);
        }

        Ok(self.query_cache.get(&lang).unwrap())
    }

    /// Highlights syntax for an entire AST tree.
    #[cfg(feature = "syntax")]
    pub fn highlight(
        &mut self,
        tree: &tree_sitter::Tree,
        source: &str,
        lang: SupportedLanguage,
    ) -> SyntaxResult<Vec<HighlightToken>> {
        self.highlight_range(tree, source, lang, None)
    }

    /// Highlights syntax for a given viewport byte range in the source code.
    #[cfg(feature = "syntax")]
    pub fn highlight_range(
        &mut self,
        tree: &tree_sitter::Tree,
        source: &str,
        lang: SupportedLanguage,
        byte_range: Option<Range<usize>>,
    ) -> SyntaxResult<Vec<HighlightToken>> {
        let query = self.get_or_compile_query(lang)?;
        let mut cursor = tree_sitter::QueryCursor::new();

        if let Some(ref range) = byte_range {
            cursor.set_byte_range(range.start..range.end);
        }

        let utf16_index = Utf16Index::new(source);
        let capture_names = query.capture_names();
        let mut raw_tokens = Vec::with_capacity(512);

        let matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        for m in matches {
            for capture in m.captures {
                let node = capture.node;
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();

                if end_byte <= start_byte {
                    continue;
                }

                // If viewport range specified, ensure intersection
                if let Some(ref range) = byte_range {
                    if start_byte >= range.end || end_byte <= range.start {
                        continue;
                    }
                }

                let cap_idx = capture.index as usize;
                let capture_name = capture_names
                    .get(cap_idx)
                    .copied()
                    .unwrap_or("variable");
                let kind = HighlightTokenKind::from_capture_name(capture_name);
                let (utf16_loc, utf16_len) =
                    utf16_index.byte_range_to_utf16(start_byte, end_byte);
                let start_pos = node.start_position();

                raw_tokens.push(HighlightToken {
                    start_byte,
                    end_byte,
                    utf16_location: utf16_loc,
                    utf16_length: utf16_len,
                    start_line: start_pos.row,
                    start_col: start_pos.column,
                    kind,
                    capture_name: capture_name.to_string(),
                });
            }
        }

        // Sort tokens by start_byte ascending, and longer spans first for proper containment
        raw_tokens.sort_by(|a, b| {
            a.start_byte
                .cmp(&b.start_byte)
                .then_with(|| b.end_byte.cmp(&a.end_byte))
        });

        // Deduplicate and disambiguate overlapping tokens
        let normalized = deduplicate_tokens(raw_tokens);
        Ok(normalized)
    }

    /// One-shot helper to parse and highlight source code directly.
    #[cfg(feature = "syntax")]
    pub fn highlight_source(
        &mut self,
        source: &str,
        lang: SupportedLanguage,
    ) -> SyntaxResult<Vec<HighlightToken>> {
        let mut parser = TTZipSyntaxParser::with_language(lang)?;
        let tree = parser.parse_full(source)?;
        self.highlight(tree, source, lang)
    }
}

/// Normalizes and prioritizes overlapping syntax highlight tokens.
fn deduplicate_tokens(tokens: Vec<HighlightToken>) -> Vec<HighlightToken> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut last_end = 0usize;

    for token in tokens {
        if token.start_byte >= last_end {
            last_end = token.end_byte;
            result.push(token);
        } else if token.end_byte > last_end {
            // Nested or slightly overlapping token: only keep if start_byte >= previous start_byte
            if let Some(prev) = result.last_mut() {
                if prev.start_byte == token.start_byte {
                    // Replace with more specific token (smaller end_byte)
                    *prev = token;
                    last_end = prev.end_byte;
                    continue;
                }
            }
            // Retain token
            last_end = token.end_byte;
            result.push(token);
        }
    }

    result
}
