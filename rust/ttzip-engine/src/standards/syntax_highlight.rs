// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
// TTZip: High-performance native archiving and compression engine.

//! High-performance Tree-sitter AST syntax tokenization & incremental parsing engine.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HighlightCategory {
    Keyword, String, Number, Type, Function, Comment, Operator,
}

impl HighlightCategory {
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::String => "string",
            Self::Number => "number",
            Self::Type => "type",
            Self::Function => "function",
            Self::Comment => "comment",
            Self::Operator => "operator",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSpan {
    pub start_byte: u32,
    pub end_byte: u32,
    pub utf16_location: u32,
    pub utf16_length: u32,
    pub category: HighlightCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Rust, Swift, C, Cpp, Python, JavaScript, TypeScript, Json, Markdown, Html, Css,
}

impl SupportedLanguage {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "swift" => Some(Self::Swift),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" => Some(Self::Cpp),
            "py" | "pyw" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "json" | "jsonc" => Some(Self::Json),
            "md" | "markdown" => Some(Self::Markdown),
            "html" | "htm" => Some(Self::Html),
            "css" => Some(Self::Css),
            _ => None,
        }
    }

    #[cfg(feature = "syntax")]
    pub fn get_tree_sitter_language(self) -> Option<tree_sitter::Language> {
        match self {
            Self::Rust => Some(tree_sitter_rust::language()),
            Self::Swift => Some(tree_sitter_swift::language()),
            Self::C | Self::Cpp => Some(tree_sitter_c::language()),
            Self::Python => Some(tree_sitter_python::language()),
            Self::JavaScript => Some(tree_sitter_javascript::language()),
            Self::TypeScript => Some(tree_sitter_typescript::language_typescript()),
            Self::Json => Some(tree_sitter_json::language()),
            Self::Markdown => Some(tree_sitter_md::language()),
            Self::Html => Some(tree_sitter_html::language()),
            Self::Css => Some(tree_sitter_css::language()),
        }
    }
}

pub struct Utf16Index {
    byte_to_u16: Vec<u32>,
    total_u16: u32,
}

impl Utf16Index {
    pub fn new(text: &str) -> Self {
        let mut byte_to_u16 = Vec::with_capacity(text.len() + 1);
        let mut acc = 0u32;
        for b in text.bytes() {
            byte_to_u16.push(acc);
            if (b as i8) >= -0x40 { acc += if b < 0xF0 { 1 } else { 2 }; }
        }
        byte_to_u16.push(acc);
        Self { byte_to_u16, total_u16: acc }
    }

    #[inline]
    pub fn byte_range_to_utf16(&self, start: usize, end: usize) -> (u32, u32) {
        let loc = self.byte_to_u16.get(start).copied().unwrap_or(self.total_u16);
        let end_loc = self.byte_to_u16.get(end).copied().unwrap_or(self.total_u16);
        (loc, end_loc.saturating_sub(loc))
    }
}

#[cfg(feature = "syntax")]
pub struct SyntaxEngine {
    parser: tree_sitter::Parser,
    tree: Option<tree_sitter::Tree>,
    lang: Option<SupportedLanguage>,
}

#[cfg(feature = "syntax")]
impl Default for SyntaxEngine {
    fn default() -> Self { Self::new() }
}

#[cfg(feature = "syntax")]
impl SyntaxEngine {
    pub fn new() -> Self {
        Self { parser: tree_sitter::Parser::new(), tree: None, lang: None }
    }

    pub fn set_language(&mut self, lang: SupportedLanguage) -> Result<(), String> {
        if self.lang == Some(lang) && self.tree.is_some() { return Ok(()); }
        let ts = lang.get_tree_sitter_language().ok_or_else(|| format!("Unsupported: {:?}", lang))?;
        self.parser.set_language(&ts).map_err(|e| format!("Lang err: {:?}", e))?;
        self.lang = Some(lang);
        self.tree = None;
        Ok(())
    }

    pub fn parse_full(&mut self, text: &str, lang: SupportedLanguage) -> Result<Vec<TokenSpan>, String> {
        self.set_language(lang)?;
        let tree = self.parser.parse(text, None).ok_or_else(|| "Parse failed".to_string())?;
        let spans = collect_tokens(&tree, text);
        self.tree = Some(tree);
        Ok(spans)
    }

    pub fn parse_incremental(&mut self, new_text: &str, edit: &tree_sitter::InputEdit, lang: SupportedLanguage) -> Result<Vec<TokenSpan>, String> {
        self.set_language(lang)?;
        if let Some(mut old_tree) = self.tree.take() {
            old_tree.edit(edit);
            let tree = self.parser.parse(new_text, Some(&old_tree)).ok_or_else(|| "Inc parse failed".to_string())?;
            let spans = collect_tokens(&tree, new_text);
            self.tree = Some(tree);
            Ok(spans)
        } else {
            self.parse_full(new_text, lang)
        }
    }
}

#[cfg(feature = "syntax")]
fn classify_node(kind: &str, parent: Option<&str>) -> Option<HighlightCategory> {
    if kind.contains("comment") { return Some(HighlightCategory::Comment); }
    if kind.contains("string") || kind == "char_literal" || kind == "character" { return Some(HighlightCategory::String); }
    if kind.contains("number") || kind.contains("integer") || kind.contains("float") || kind == "int" { return Some(HighlightCategory::Number); }
    if kind.contains("type") || kind == "primitive_type" || kind == "class_name" { return Some(HighlightCategory::Type); }
    if kind == "identifier" || kind == "field_identifier" {
        if let Some(pk) = parent {
            if pk.contains("function") || pk.contains("call") || pk.contains("method") { return Some(HighlightCategory::Function); }
            if pk.contains("type") { return Some(HighlightCategory::Type); }
        }
    }
    if is_keyword_kind(kind) { return Some(HighlightCategory::Keyword); }
    if is_operator_kind(kind) { return Some(HighlightCategory::Operator); }
    None
}

#[inline]
fn is_keyword_kind(k: &str) -> bool {
    matches!(k, "fn"|"let"|"mut"|"pub"|"struct"|"enum"|"trait"|"impl"|"use"|"mod"|"const"|"static"|"type"|
        "unsafe"|"async"|"await"|"if"|"else"|"match"|"while"|"loop"|"for"|"in"|"return"|"break"|"continue"|
        "where"|"as"|"dyn"|"ref"|"move"|"self"|"Self"|"super"|"crate"|"def"|"class"|"import"|"from"|"try"|
        "except"|"finally"|"with"|"lambda"|"yield"|"raise"|"global"|"pass"|"function"|"var"|"val"|"interface"|
        "package"|"export"|"default"|"new"|"delete"|"typeof"|"void"|"func"|"guard"|"switch"|"case"|"defer"|
        "init"|"protocol"|"extension"|"true"|"false"|"nil"|"null"|"None")
}

#[inline]
fn is_operator_kind(k: &str) -> bool {
    matches!(k, "+"|"-"|"*"|"/"|"%"|"="|"=="|"!="|"<"|">"|"<="|">="|"&&"|"||"|"!"|"&"|"|"|"^"|"~"|
        "<<"|">>"|"+="|"-="|"*="|"/="|"%="|"=>"|"->"|"::"|".."|"..="|"?"|":"|".")
}

#[cfg(feature = "syntax")]
fn collect_tokens(tree: &tree_sitter::Tree, source: &str) -> Vec<TokenSpan> {
    let index = Utf16Index::new(source);
    let mut spans = Vec::with_capacity(256);
    let mut cursor = tree.walk();

    fn walk(cursor: &mut tree_sitter::TreeCursor, idx: &Utf16Index, spans: &mut Vec<TokenSpan>, parent: Option<&str>) {
        let node = cursor.node();
        let kind = node.kind();
        if node.child_count() == 0 || kind.contains("string") || kind.contains("comment") {
            if let Some(cat) = classify_node(kind, parent) {
                let (sb, eb) = (node.start_byte(), node.end_byte());
                if eb > sb {
                    let (loc, len) = idx.byte_range_to_utf16(sb, eb);
                    spans.push(TokenSpan { start_byte: sb as u32, end_byte: eb as u32, utf16_location: loc, utf16_length: len, category: cat });
                }
            }
            return;
        }
        if cursor.goto_first_child() {
            loop {
                walk(cursor, idx, spans, Some(kind));
                if !cursor.goto_next_sibling() { break; }
            }
            cursor.goto_parent();
        }
    }
    walk(&mut cursor, &index, &mut spans, None);
    spans
}

pub fn tokenize_code(text: &str, extension: &str) -> Vec<TokenSpan> {
    #[cfg(feature = "syntax")]
    {
        if let Some(lang) = SupportedLanguage::from_extension(extension) {
            let mut engine = SyntaxEngine::new();
            if let Ok(spans) = engine.parse_full(text, lang) { return spans; }
        }
    }
    fallback_tokenize(text)
}

pub fn highlight_spans(text: &str, extension: &str) -> Vec<TokenSpan> {
    tokenize_code(text, extension)
}

fn fallback_tokenize(text: &str) -> Vec<TokenSpan> {
    let index = Utf16Index::new(text);
    let (mut spans, bytes, n, mut i) = (Vec::new(), text.as_bytes(), text.len(), 0);
    while i < n {
        let b = bytes[i];
        if b.is_ascii_whitespace() { i += 1; continue; }
        if b == b'/' && i + 1 < n && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
            let s = i;
            if bytes[i + 1] == b'/' {
                i += 2; while i < n && bytes[i] != b'\n' { i += 1; }
            } else {
                i += 2; while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { i += 1; }
                i = (i + 2).min(n);
            }
            let (loc, len) = index.byte_range_to_utf16(s, i);
            spans.push(TokenSpan { start_byte: s as u32, end_byte: i as u32, utf16_location: loc, utf16_length: len, category: HighlightCategory::Comment });
            continue;
        }
        if b == b'"' || b == b'\'' {
            let (q, s) = (b, i);
            i += 1;
            while i < n && bytes[i] != q { if bytes[i] == b'\\' { i += 1; } i += 1; }
            if i < n { i += 1; }
            let (loc, len) = index.byte_range_to_utf16(s, i);
            spans.push(TokenSpan { start_byte: s as u32, end_byte: i as u32, utf16_location: loc, utf16_length: len, category: HighlightCategory::String });
            continue;
        }
        if b.is_ascii_digit() {
            let s = i;
            while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_') { i += 1; }
            let (loc, len) = index.byte_range_to_utf16(s, i);
            spans.push(TokenSpan { start_byte: s as u32, end_byte: i as u32, utf16_location: loc, utf16_length: len, category: HighlightCategory::Number });
            continue;
        }
        if b.is_ascii_alphabetic() || b == b'_' {
            let s = i;
            while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1; }
            let word = &text[s..i];
            let cat = if is_keyword_kind(word) { Some(HighlightCategory::Keyword) }
            else if word.chars().next().map_or(false, |c| c.is_uppercase()) { Some(HighlightCategory::Type) }
            else { None };
            if let Some(category) = cat {
                let (loc, len) = index.byte_range_to_utf16(s, i);
                spans.push(TokenSpan { start_byte: s as u32, end_byte: i as u32, utf16_location: loc, utf16_length: len, category });
            }
            continue;
        }
        i += 1;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_ast_tokenization() {
        let code = "pub fn add(a: u32, b: u32) -> u32 { /* calc */ a + b }";
        let spans = tokenize_code(code, "rs");
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.category == HighlightCategory::Keyword));
        assert!(spans.iter().any(|s| s.category == HighlightCategory::Comment));
    }

    #[test]
    fn test_python_ast_tokenization() {
        let code = "# Process\ndef solve(x: int) -> str:\n    return \"result\"";
        let spans = tokenize_code(code, "py");
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.category == HighlightCategory::Comment));
        assert!(spans.iter().any(|s| s.category == HighlightCategory::Keyword));
    }

    #[cfg(feature = "syntax")]
    #[test]
    fn test_incremental_parsing_microsecond() {
        let mut engine = SyntaxEngine::new();
        let initial_code = "fn compute() -> u32 {\n    let val = 100;\n    val + 1\n}";
        let spans1 = engine.parse_full(initial_code, SupportedLanguage::Rust).unwrap();
        assert!(!spans1.is_empty());

        let new_code = "fn compute() -> u32 {\n    let val = 200;\n    val + 1\n}";
        let edit = tree_sitter::InputEdit {
            start_byte: 38,
            old_end_byte: 41,
            new_end_byte: 41,
            start_position: tree_sitter::Point { row: 1, column: 14 },
            old_end_position: tree_sitter::Point { row: 1, column: 17 },
            new_end_position: tree_sitter::Point { row: 1, column: 17 },
        };
        let start_time = std::time::Instant::now();
        let spans2 = engine.parse_incremental(new_code, &edit, SupportedLanguage::Rust).unwrap();
        let elapsed = start_time.elapsed();
        assert!(!spans2.is_empty());
        assert!(elapsed.as_millis() < 50);
    }
}
