// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust PDF Content Stream Parser, ToUnicode CMap Engine, and Text Searcher.
//!
//! Deconstructs page content streams (`BT`/`ET`, `Tf`, `Tm`, `Td`, `Tj`, `TJ`), resolves `/ToUnicode`
//! font CMaps and standard encodings, reconstructs UTF-8 text flow, and executes high-precision
//! substring search with context snippet highlighting.

use std::collections::HashMap;

use lopdf::content::Content;
use lopdf::Object;

use super::parser::TTZipPdfParser;
use super::PdfError;

/// Span identifying a matched text occurrence within a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfHighlightSpan {
    /// 1-based page number.
    pub page_number: u32,
    /// 1-based line number within the page text.
    pub line_number: u32,
    /// Character offset (start, inclusive) within the line.
    pub start_char: usize,
    /// Character offset (end, exclusive) within the line.
    pub end_char: usize,
    /// Exact matched substring text.
    pub matched_text: String,
    /// Surrounding context snippet with highlighted marker bounds.
    pub context_snippet: String,
}

/// Extracted text content of a single page with line breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfPageText {
    /// 1-based page number.
    pub page_number: u32,
    /// Full aggregated text of the page.
    pub full_text: String,
    /// Line-by-line breakdown for line-addressed searching.
    pub lines: Vec<String>,
}

/// Search configuration parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfTextSearchOptions {
    /// Whether matching is case-sensitive.
    pub case_sensitive: bool,
    /// Match whole words only.
    pub whole_word: bool,
    /// Maximum number of search matches to return across all pages.
    pub max_results: Option<usize>,
    /// Number of context characters before and after the match.
    pub context_padding: usize,
}

impl Default for PdfTextSearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            max_results: None,
            context_padding: 30,
        }
    }
}

/// Aggregated multi-page search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfSearchResult {
    /// Query string searched.
    pub query: String,
    /// Total matches found.
    pub total_matches: usize,
    /// Detailed list of highlighted spans.
    pub matches: Vec<PdfHighlightSpan>,
}

/// Parsed `/ToUnicode` CMap lookup table for decoding character codes into UTF-8.
#[derive(Debug, Clone, Default)]
pub struct ToUnicodeCMap {
    /// Single/multi-byte character code to Unicode string mapping.
    char_map: HashMap<u32, String>,
    /// Byte length of character codes (1 for simple fonts, 2 for CID/Type0 fonts).
    code_byte_len: usize,
}

impl ToUnicodeCMap {
    /// Parses a `/ToUnicode` PostScript CMap stream into an in-memory lookup table.
    pub fn parse_from_bytes(data: &[u8]) -> Self {
        let text = String::from_utf8_lossy(data);
        let mut cmap = Self {
            code_byte_len: 1,
            ..Default::default()
        };

        let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if line.contains("beginbfchar") {
                i += 1;
                while i < lines.len() && !lines[i].contains("endbfchar") {
                    cmap.parse_bfchar_line(lines[i]);
                    i += 1;
                }
            } else if line.contains("beginbfrange") {
                i += 1;
                while i < lines.len() && !lines[i].contains("endbfrange") {
                    cmap.parse_bfrange_line(lines[i]);
                    i += 1;
                }
            }
            i += 1;
        }

        cmap
    }

    /// Decodes a raw byte slice into a UTF-8 string using the CMap.
    pub fn decode_bytes(&self, bytes: &[u8]) -> String {
        if self.char_map.is_empty() {
            return TTZipPdfParser::decode_pdf_string(bytes);
        }

        let mut result = String::new();
        let mut i = 0;

        while i < bytes.len() {
            let mut matched = false;

            // Try 2-byte code if available
            if i + 1 < bytes.len() {
                let code16 = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
                if let Some(s) = self.char_map.get(&code16) {
                    result.push_str(s);
                    i += 2;
                    matched = true;
                }
            }

            // Try 1-byte code
            if !matched {
                let code8 = bytes[i] as u32;
                if let Some(s) = self.char_map.get(&code8) {
                    result.push_str(s);
                } else {
                    // Fallback to raw Latin-1 char
                    result.push(bytes[i] as char);
                }
                i += 1;
            }
        }

        result
    }

    fn parse_bfchar_line(&mut self, line: &str) {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for chunk in tokens.chunks_exact(2) {
            let src = chunk[0].trim_matches(|c| c == '<' || c == '>');
            let dst = chunk[1].trim_matches(|c| c == '<' || c == '>');

            if let Ok(src_code) = u32::from_str_radix(src, 16) {
                if src.len() > 2 {
                    self.code_byte_len = 2;
                }
                if let Some(dst_str) = Self::decode_hex_to_unicode(dst) {
                    self.char_map.insert(src_code, dst_str);
                }
            }
        }
    }

    fn parse_bfrange_line(&mut self, line: &str) {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() >= 3 {
            let src_lo = tokens[0].trim_matches(|c| c == '<' || c == '>');
            let src_hi = tokens[1].trim_matches(|c| c == '<' || c == '>');

            if let (Ok(lo), Ok(hi)) = (u32::from_str_radix(src_lo, 16), u32::from_str_radix(src_hi, 16)) {
                if src_lo.len() > 2 {
                    self.code_byte_len = 2;
                }

                let dst_token = tokens[2];
                if dst_token.starts_with('<') && dst_token.ends_with('>') {
                    let dst_hex = dst_token.trim_matches(|c| c == '<' || c == '>');
                    if let Ok(dst_base) = u32::from_str_radix(dst_hex, 16) {
                        for offset in 0..=(hi.saturating_sub(lo)) {
                            let src_code = lo + offset;
                            let dst_code = dst_base + offset;
                            if let Some(ch) = char::from_u32(dst_code) {
                                self.char_map.insert(src_code, ch.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    fn decode_hex_to_unicode(hex: &str) -> Option<String> {
        if hex.len().is_multiple_of(4) {
            // UTF-16BE hex chunks
            let mut u16_chars = Vec::new();
            for i in (0..hex.len()).step_by(4) {
                let chunk = &hex[i..i + 4];
                let u = u16::from_str_radix(chunk, 16).ok()?;
                u16_chars.push(u);
            }
            Some(String::from_utf16_lossy(&u16_chars))
        } else if hex.len().is_multiple_of(2) {
            // Byte sequence
            let mut bytes = Vec::new();
            for i in (0..hex.len()).step_by(2) {
                let chunk = &hex[i..i + 2];
                let b = u8::from_str_radix(chunk, 16).ok()?;
                bytes.push(b);
            }
            Some(String::from_utf8_lossy(&bytes).to_string())
        } else {
            None
        }
    }
}

/// Pure Safe Rust PDF Text Extraction and Search Engine.
pub struct PdfTextExtractor;

impl PdfTextExtractor {
    /// Extracts full plain text for a specific 1-based page number.
    pub fn extract_page_text(
        parser: &TTZipPdfParser,
        page_number: u32,
    ) -> Result<String, PdfError> {
        if page_number == 0 || page_number > parser.page_count() {
            return Err(PdfError::PageOutOfBounds(page_number, parser.page_count()));
        }

        let content_bytes = parser.get_page_content_bytes(page_number).unwrap_or_default();
        if content_bytes.is_empty() {
            return Ok(String::new());
        }

        let font_cmaps = Self::load_page_font_cmaps(parser, page_number);
        let mut full_text = String::new();
        if let Ok(content) = Content::decode(&content_bytes) {
            full_text = Self::process_content_operations(&content, &font_cmaps);
        }

        if full_text.trim().is_empty() {
            let extract_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                parser.doc().extract_text(&[page_number])
            }));
            if let Ok(Ok(lopdf_text)) = extract_res {
                full_text = lopdf_text;
            }
        }

        Ok(full_text)
    }

    /// Extracts plain text from all pages in sequence.
    pub fn extract_all_text(parser: &TTZipPdfParser) -> Result<String, PdfError> {
        let total_pages = parser.page_count();
        let mut buffer = String::new();

        for p in 1..=total_pages {
            let page_text = Self::extract_page_text(parser, p)?;
            if !page_text.trim().is_empty() {
                if !buffer.is_empty() {
                    buffer.push_str("\n\n");
                }
                buffer.push_str(&page_text);
            }
        }

        Ok(buffer)
    }

    /// Extracts text and line structure for a specific 1-based page number.
    pub fn extract_page_text_with_spans(
        parser: &TTZipPdfParser,
        page_number: u32,
    ) -> Result<PdfPageText, PdfError> {
        let full_text = Self::extract_page_text(parser, page_number)?;
        let lines: Vec<String> = full_text.lines().map(|l| l.to_string()).collect();

        Ok(PdfPageText {
            page_number,
            full_text,
            lines,
        })
    }

    /// Performs search across all pages with customizable options and snippet highlighting.
    pub fn search_text(
        parser: &TTZipPdfParser,
        query: &str,
        options: &PdfTextSearchOptions,
    ) -> Result<PdfSearchResult, PdfError> {
        if query.is_empty() {
            return Ok(PdfSearchResult {
                query: query.to_string(),
                total_matches: 0,
                matches: Vec::new(),
            });
        }

        let query_normalized = if options.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        let mut matches = Vec::new();
        let total_pages = parser.page_count();

        for page_num in 1..=total_pages {
            let full_text = Self::extract_page_text(parser, page_num)?;

            for (line_idx, line) in full_text.lines().enumerate() {
                let mut search_start = 0;
                while search_start < line.len() {
                    let found_opt = if options.case_sensitive {
                        line[search_start..].find(&query_normalized)
                    } else {
                        line[search_start..]
                            .to_ascii_lowercase()
                            .find(&query_normalized)
                    };

                    let found_idx = match found_opt {
                        Some(idx) => idx,
                        None => break,
                    };

                    let match_start = search_start + found_idx;
                    let match_end = match_start + query_normalized.len();

                    // Check whole word requirement
                    let is_valid = if options.whole_word {
                        Self::is_whole_word(line, match_start, match_end)
                    } else {
                        true
                    };

                    if is_valid {
                        let matched_slice = &line[match_start..match_end];
                        let snippet = Self::create_snippet(
                            line,
                            match_start,
                            match_end,
                            options.context_padding,
                        );

                        matches.push(PdfHighlightSpan {
                            page_number: page_num,
                            line_number: (line_idx as u32) + 1,
                            start_char: match_start,
                            end_char: match_end,
                            matched_text: matched_slice.to_string(),
                            context_snippet: snippet,
                        });

                        if let Some(max_m) = options.max_results {
                            if matches.len() >= max_m {
                                return Ok(PdfSearchResult {
                                    query: query.to_string(),
                                    total_matches: matches.len(),
                                    matches,
                                });
                            }
                        }
                    }

                    search_start = match_end;
                }
            }
        }

        let total_matches = matches.len();
        Ok(PdfSearchResult {
            query: query.to_string(),
            total_matches,
            matches,
        })
    }

    /// Loads `/ToUnicode` CMaps from font resources of the page.
    fn load_page_font_cmaps(
        parser: &TTZipPdfParser,
        page_number: u32,
    ) -> HashMap<Vec<u8>, ToUnicodeCMap> {
        let mut cmaps = HashMap::new();
        let resources = match parser.get_page_resources(page_number) {
            Ok(Some(res)) => res,
            _ => return cmaps,
        };

        let font_dict = match resources.get(b"Font") {
            Ok(obj) => match parser.resolve_reference(obj) {
                Ok(Object::Dictionary(dict)) => dict,
                _ => return cmaps,
            },
            _ => return cmaps,
        };

        for (font_name, font_obj) in font_dict.iter() {
            if let Ok(Object::Dictionary(font_entry)) = parser.resolve_reference(font_obj) {
                if let Ok(to_unicode_obj) = font_entry.get(b"ToUnicode") {
                    if let Ok(Object::Stream(stream)) = parser.resolve_reference(to_unicode_obj) {
                        if let Ok(decompressed) = stream.decompressed_content() {
                            let cmap = ToUnicodeCMap::parse_from_bytes(&decompressed);
                            cmaps.insert(font_name.clone(), cmap);
                        }
                    }
                }
            }
        }

        cmaps
    }

    /// Iterates through PDF content stream operations to reconstruct flow text.
    fn process_content_operations(
        content: &Content,
        font_cmaps: &HashMap<Vec<u8>, ToUnicodeCMap>,
    ) -> String {
        let mut out = String::with_capacity(4096);
        let mut current_font: Option<Vec<u8>> = None;
        let mut in_text_object = false;

        for op in &content.operations {
            match op.operator.as_str() {
                "BT" => {
                    in_text_object = true;
                }
                "ET" => {
                    in_text_object = false;
                    if !out.ends_with('\n') && !out.is_empty() {
                        out.push('\n');
                    }
                }
                "Tf" => {
                    if let Some(Object::Name(font_name)) = op.operands.first() {
                        current_font = Some(font_name.clone());
                    }
                }
                "T*" | "'" => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    if op.operator == "'" {
                        if let Some(Object::String(bytes, _)) = op.operands.first() {
                            Self::append_decoded_text(
                                bytes,
                                &current_font,
                                font_cmaps,
                                &mut out,
                            );
                        }
                    }
                }
                "\"" => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    if let Some(Object::String(bytes, _)) = op.operands.get(2) {
                        Self::append_decoded_text(
                            bytes,
                            &current_font,
                            font_cmaps,
                            &mut out,
                        );
                    }
                }
                "Tj" => {
                    if let Some(Object::String(bytes, _)) = op.operands.first() {
                        Self::append_decoded_text(
                            bytes,
                            &current_font,
                            font_cmaps,
                            &mut out,
                        );
                    }
                }
                "TJ" => {
                    if let Some(Object::Array(arr)) = op.operands.first() {
                        for item in arr {
                            match item {
                                Object::String(bytes, _) => {
                                    Self::append_decoded_text(
                                        bytes,
                                        &current_font,
                                        font_cmaps,
                                        &mut out,
                                    );
                                }
                                Object::Integer(spacing)
                                    if *spacing < -100
                                        && !out.ends_with(' ')
                                        && !out.ends_with('\n') =>
                                {
                                    out.push(' ');
                                }
                                Object::Real(spacing)
                                    if *spacing < -100.0
                                        && !out.ends_with(' ')
                                        && !out.ends_with('\n') =>
                                {
                                    out.push(' ');
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "Td" | "TD" => {
                    // Vertical displacement indicates new line if ty != 0
                    if let Some(Object::Real(ty)) = op.operands.get(1) {
                        if ty.abs() > 2.0 && !out.ends_with('\n') && !out.is_empty() {
                            out.push('\n');
                        }
                    } else if let Some(Object::Integer(ty)) = op.operands.get(1) {
                        if ty.abs() > 2 && !out.ends_with('\n') && !out.is_empty() {
                            out.push('\n');
                        }
                    }
                }
                _ => {}
            }
        }

        let _ = in_text_object;
        out.trim().to_string()
    }

    fn append_decoded_text(
        bytes: &[u8],
        current_font: &Option<Vec<u8>>,
        font_cmaps: &HashMap<Vec<u8>, ToUnicodeCMap>,
        out: &mut String,
    ) {
        if !font_cmaps.is_empty() {
            if let Some(font_name) = current_font {
                if let Some(cmap) = font_cmaps.get(font_name) {
                    out.push_str(&cmap.decode_bytes(bytes));
                    return;
                }
            }
        }
        // Fallback: Windows-1252 / Latin-1 or UTF-8 decoding
        if let Ok(utf8) = std::str::from_utf8(bytes) {
            out.push_str(utf8);
        } else {
            for &b in bytes {
                out.push(b as char);
            }
        }
    }

    fn is_whole_word(text: &str, start: usize, end: usize) -> bool {
        let is_left_boundary = if start == 0 {
            true
        } else {
            text[..start]
                .chars()
                .last()
                .map(|c| !c.is_alphanumeric())
                .unwrap_or(true)
        };

        let is_right_boundary = if end >= text.len() {
            true
        } else {
            text[end..]
                .chars()
                .next()
                .map(|c| !c.is_alphanumeric())
                .unwrap_or(true)
        };

        is_left_boundary && is_right_boundary
    }

    fn create_snippet(text: &str, start: usize, end: usize, padding: usize) -> String {
        let snippet_start = start.saturating_sub(padding);
        let snippet_end = (end + padding).min(text.len());

        let prefix = if snippet_start > 0 { "..." } else { "" };
        let suffix = if snippet_end < text.len() { "..." } else { "" };

        let left = &text[snippet_start..start];
        let mid = &text[start..end];
        let right = &text[end..snippet_end];

        format!("{}{}[{}]{}{}", prefix, left, mid, right, suffix)
    }
}
