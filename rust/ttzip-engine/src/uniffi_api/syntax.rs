// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI High-Performance Native Tree-sitter Syntax Tokenization Engine.
//! Exposes AST streaming tokenization and syntax highlighting to Swift/Kotlin/Python.

use crate::standards::syntax_highlight::tokenize_code;

/// Highlight token span exposed across UniFFI boundary with UTF-16 NSRange metrics.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFITokenSpan {
    pub location: u32,
    pub length: u32,
    pub category: String, // "keyword", "string", "number", "type", "function", "comment", "operator"
}

/// Tokenizes source code text into high-precision UTF-16 token spans for syntax highlighting.
#[uniffi::export]
pub fn tokenize_source_code(text: String, file_extension: String, max_length: u32) -> Vec<UniFFITokenSpan> {
    if text.is_empty() {
        return Vec::new();
    }
    let input = if max_length > 0 && (text.len() as u32) > max_length {
        let mut u16_len = 0u32;
        let mut byte_idx = 0usize;
        for ch in text.chars() {
            u16_len += ch.len_utf16() as u32;
            if u16_len > max_length {
                break;
            }
            byte_idx += ch.len_utf8();
        }
        &text[..byte_idx]
    } else {
        &text
    };

    tokenize_code(input, &file_extension)
        .into_iter()
        .map(|span| UniFFITokenSpan {
            location: span.utf16_location,
            length: span.utf16_length,
            category: span.category.as_str().to_string(),
        })
        .collect()
}

/// Full-file syntax highlight spans using Tree-sitter AST and UTF-16 NSRange offsets.
#[uniffi::export]
pub fn highlight_code_spans(text: String, file_extension: String) -> Vec<UniFFITokenSpan> {
    tokenize_source_code(text, file_extension, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_tokenization_rust() {
        let code = "pub fn add(x: i32) -> i32 { /* comment */ x + 10 }";
        let spans = tokenize_source_code(code.to_string(), "rs".to_string(), 0);
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.category == "keyword"));
        assert!(spans.iter().any(|s| s.category == "comment"));
        assert!(spans.iter().any(|s| s.category == "number"));
    }

    #[test]
    fn test_uniffi_highlight_code_spans_python() {
        let code = "def hello():\n    return \"world\" # greeting";
        let spans = highlight_code_spans(code.to_string(), "py".to_string());
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.category == "keyword"));
        assert!(spans.iter().any(|s| s.category == "string"));
        assert!(spans.iter().any(|s| s.category == "comment"));
    }

    #[test]
    fn test_uniffi_max_length_truncation() {
        let code = "let x = 12345; let y = 67890; let z = 99999;";
        let spans_all = highlight_code_spans(code.to_string(), "rs".to_string());
        let spans_limited = tokenize_source_code(code.to_string(), "rs".to_string(), 15);
        assert!(!spans_limited.is_empty());
        assert!(spans_limited.len() <= spans_all.len());
    }
}
