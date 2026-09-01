// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-Performance AST Syntax Tokenization & Viewport Coloring Engine.

use super::types::UniFFIHighlightToken;
use crate::standards::syntax_highlight::{tokenize_code, Utf16Index};

/// Source text layout index tracking UTF-16 NSRange mappings, line boundaries, and column metrics.
pub struct SourceLayoutIndex {
    utf16_index: Utf16Index,
    line_starts: Vec<(usize, u32)>, // (byte_offset, utf16_offset)
}

impl SourceLayoutIndex {
    pub fn new(text: &str) -> Self {
        let utf16_index = Utf16Index::new(text);
        let mut line_starts = Vec::with_capacity(text.lines().count() + 2);
        line_starts.push((0, 0));

        let mut byte_idx = 0usize;
        let mut u16_idx = 0u32;

        for ch in text.chars() {
            let ch_u16 = ch.len_utf16() as u32;
            let ch_u8 = ch.len_utf8();
            byte_idx += ch_u8;
            u16_idx += ch_u16;

            if ch == '\n' {
                line_starts.push((byte_idx, u16_idx));
            }
        }

        Self {
            utf16_index,
            line_starts,
        }
    }

    /// Converts byte start and end offsets into NSRange location, length, 1-based line, and 0-based column.
    #[inline]
    pub fn locate_span(&self, start_byte: usize, end_byte: usize) -> (u32, u32, u32, u32) {
        let (loc, len) = self.utf16_index.byte_range_to_utf16(start_byte, end_byte);

        // Binary search for line index
        let line_idx = match self.line_starts.binary_search_by(|(b, _)| b.cmp(&start_byte)) {
            Ok(exact) => exact,
            Err(next) => next.saturating_sub(1),
        };

        let line_number = (line_idx + 1) as u32;
        let line_u16_start = self.line_starts.get(line_idx).map(|(_, u)| *u).unwrap_or(0);
        let column = loc.saturating_sub(line_u16_start);

        (loc, len, line_number, column)
    }
}

/// Tokenizes source code into high-precision highlight tokens with optional max length truncation.
pub fn highlight_code_internal(code: &str, language_hint: &str, max_length: u32) -> Vec<UniFFIHighlightToken> {
    if code.is_empty() {
        return Vec::new();
    }

    let input = if max_length > 0 && (code.len() as u32) > max_length {
        let mut u16_len = 0u32;
        let mut byte_idx = 0usize;
        for ch in code.chars() {
            u16_len += ch.len_utf16() as u32;
            if u16_len > max_length {
                break;
            }
            byte_idx += ch.len_utf8();
        }
        &code[..byte_idx]
    } else {
        code
    };

    let layout = SourceLayoutIndex::new(input);
    let spans = tokenize_code(input, language_hint);

    spans
        .into_iter()
        .map(|span| {
            let (_, _, line_number, column) = layout.locate_span(span.start_byte as usize, span.end_byte as usize);
            UniFFIHighlightToken {
                location: span.utf16_location,
                length: span.utf16_length,
                category: span.category.as_str().to_string(),
                line_number,
                column,
            }
        })
        .collect()
}

/// Tokenizes source code filtered to a specific viewport line range [start_line, start_line + line_count).
pub fn highlight_viewport_internal(
    code: &str,
    language_hint: &str,
    start_line: u32,
    line_count: u32,
) -> Vec<UniFFIHighlightToken> {
    if code.is_empty() {
        return Vec::new();
    }

    let all_tokens = highlight_code_internal(code, language_hint, 0);
    if start_line == 0 && line_count == 0 {
        return all_tokens;
    }

    let end_line = start_line.saturating_add(line_count);
    all_tokens
        .into_iter()
        .filter(|token| {
            if line_count == 0 {
                token.line_number >= start_line
            } else {
                token.line_number >= start_line && token.line_number < end_line
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust_code_lines_and_columns() {
        let code = "fn main() {\n    let x: i32 = 42;\n    println!(\"{}\", x);\n}";
        let tokens = highlight_code_internal(code, "rs", 0);
        assert!(!tokens.is_empty());

        let fn_token = tokens.iter().find(|t| t.category == "keyword" && t.location == 0).unwrap();
        assert_eq!(fn_token.line_number, 1);
        assert_eq!(fn_token.column, 0);

        let let_token = tokens.iter().find(|t| t.category == "keyword" && t.line_number == 2).unwrap();
        assert_eq!(let_token.line_number, 2);
        assert_eq!(let_token.column, 4);
    }

    #[test]
    fn test_highlight_viewport_filtering() {
        let code = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8";
        let tokens = highlight_viewport_internal(code, "txt", 3, 3);
        for t in &tokens {
            assert!(t.line_number >= 3 && t.line_number < 6);
        }
    }

    #[test]
    fn test_highlight_python_spans() {
        let code = "def greet(name: str) -> str:\n    return f\"Hello {name}\" # greeting";
        let tokens = highlight_code_internal(code, "py", 0);
        assert!(tokens.iter().any(|t| t.category == "keyword"));
        assert!(tokens.iter().any(|t| t.category == "comment"));
        assert!(tokens.iter().any(|t| t.category == "string"));
    }
}
