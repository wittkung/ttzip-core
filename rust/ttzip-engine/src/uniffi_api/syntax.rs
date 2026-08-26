// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI High-Performance Native Syntax Tokenization Engine.
//! Provides zero-regex, memory-bounded, sub-millisecond linear lexical token scanning.

/// Highlight token span exposed across UniFFI boundary with UTF-16 NSRange metrics.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UniFFITokenSpan {
    pub location: u32,
    pub length: u32,
    pub category: String, // "comment", "string", "keyword", "number", "type"
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum LangFamily {
    CStyle, Python, Shell, Sql, Json, HtmlXml, Markdown, General,
}

struct SourceChar {
    ch: char,
    u16_offset: u32,
}

// Strictly sorted static keyword and type tables for zero-allocation binary search
static SWIFT_KEYWORDS: &[&str] = &[
    "Self", "actor", "any", "as", "async", "await", "break", "case", "catch", "class", "continue",
    "convenience", "default", "defer", "deinit", "do", "dynamic", "else", "enum", "extension",
    "fallthrough", "false", "fileprivate", "final", "for", "func", "guard", "if", "import", "in",
    "indirect", "init", "inout", "internal", "is", "lazy", "let", "mutating", "nil", "nonmutating",
    "open", "operator", "override", "private", "protocol", "public", "repeat", "required", "rethrows",
    "return", "self", "some", "static", "struct", "subscript", "super", "switch", "throw", "throws",
    "true", "try", "typealias", "unowned", "var", "weak", "where", "while",
];
static RUST_KEYWORDS: &[&str] = &[
    "Self", "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
    "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];
static CPP_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "class", "const", "const_cast", "constexpr", "continue",
    "default", "delete", "do", "double", "dynamic_cast", "else", "enum", "explicit", "export",
    "extern", "false", "float", "for", "friend", "goto", "if", "inline", "int", "long", "mutable",
    "namespace", "new", "noexcept", "nullptr", "operator", "private", "protected", "public",
    "register", "reinterpret_cast", "restrict", "return", "short", "signed", "sizeof", "static",
    "static_cast", "struct", "switch", "template", "this", "throw", "true", "try", "typedef",
    "typename", "union", "unsigned", "using", "virtual", "void", "volatile", "while",
];
static PY_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "cls",
    "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if",
    "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "self",
    "try", "while", "with", "yield",
];
static JS_KEYWORDS: &[&str] = &[
    "abstract", "any", "as", "async", "await", "break", "case", "catch", "class", "const",
    "continue", "debugger", "declare", "default", "delete", "do", "else", "enum", "export",
    "extends", "false", "finally", "for", "function", "if", "implements", "import", "in",
    "instanceof", "interface", "is", "keyof", "let", "never", "new", "null", "package", "private",
    "protected", "public", "readonly", "return", "static", "super", "switch", "this", "throw",
    "true", "try", "type", "typeof", "unknown", "var", "void", "while", "with", "yield",
];
static GO_KEYWORDS: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough",
    "false", "for", "func", "go", "goto", "if", "import", "interface", "map", "nil", "package",
    "range", "return", "select", "struct", "switch", "true", "type", "var",
];
static SQL_KEYWORDS: &[&str] = &[
    "all", "alter", "and", "as", "asc", "between", "by", "case", "check", "constraint", "create",
    "default", "delete", "desc", "drop", "else", "end", "exists", "foreign", "from", "group",
    "having", "in", "index", "inner", "insert", "into", "is", "join", "key", "left", "like",
    "limit", "not", "null", "offset", "on", "or", "order", "outer", "primary", "references",
    "right", "select", "table", "then", "union", "update", "when", "where",
];
static SHELL_KEYWORDS: &[&str] = &[
    "alias", "awk", "case", "cd", "cp", "do", "done", "echo", "elif", "else", "esac", "exit",
    "export", "fi", "for", "function", "grep", "if", "in", "local", "mkdir", "mv", "return",
    "rm", "sed", "set", "sudo", "then", "unset", "until", "while",
];
static KNOWN_TYPES: &[&str] = &[
    "Any", "Array", "Binding", "Bool", "Boolean", "Box", "Byte", "Char", "Data", "Dictionary",
    "Double", "Err", "Float", "Int", "Int16", "Int32", "Int64", "Int8", "Integer", "List",
    "MainActor", "Map", "None", "Number", "Object", "ObservableObject", "Ok", "Option", "Partial",
    "Promise", "Record", "Result", "Set", "Some", "State", "String", "Task", "Thread", "UInt",
    "UInt16", "UInt32", "UInt64", "UInt8", "URL", "Vec", "View", "Void", "bool", "byte", "char",
    "dict", "error", "f32", "f64", "float", "i128", "i16", "i32", "i64", "i8", "int", "isize",
    "list", "object", "size_t", "str", "tuple", "u128", "u16", "u32", "u64", "u8", "usize",
];

#[inline]
fn is_sorted_member(slice: &[&str], target: &str) -> bool {
    slice.binary_search(&target).is_ok()
}

fn map_extension(ext: &str) -> (&'static str, LangFamily) {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "swift" => ("swift", LangFamily::CStyle),
        "rs" | "rust" => ("rs", LangFamily::CStyle),
        "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" | "m" | "mm" => ("cpp", LangFamily::CStyle),
        "py" | "pyw" | "python" => ("py", LangFamily::Python),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => ("js", LangFamily::CStyle),
        "java" | "kt" | "kts" | "scala" => ("java", LangFamily::CStyle),
        "go" => ("go", LangFamily::CStyle),
        "sh" | "bash" | "zsh" => ("sh", LangFamily::Shell),
        "sql" => ("sql", LangFamily::Sql),
        "json" | "json5" | "jsonc" => ("json", LangFamily::Json),
        "html" | "htm" | "xml" | "plist" | "svg" | "xaml" => ("html", LangFamily::HtmlXml),
        "md" | "markdown" => ("md", LangFamily::Markdown),
        _ => ("general", LangFamily::General),
    }
}

fn classify_identifier(word: &str, lang: &str, fam: LangFamily) -> Option<&'static str> {
    if word.starts_with('@') { return Some("type"); }
    let is_kw = match lang {
        "swift" => is_sorted_member(SWIFT_KEYWORDS, word),
        "rs" => is_sorted_member(RUST_KEYWORDS, word),
        "cpp" => is_sorted_member(CPP_KEYWORDS, word),
        "py" => is_sorted_member(PY_KEYWORDS, word),
        "js" => is_sorted_member(JS_KEYWORDS, word),
        "go" => is_sorted_member(GO_KEYWORDS, word),
        "sh" => is_sorted_member(SHELL_KEYWORDS, word),
        "sql" => is_sorted_member(SQL_KEYWORDS, &word.to_ascii_lowercase()),
        "json" => matches!(word, "true" | "false" | "null"),
        _ => is_sorted_member(SWIFT_KEYWORDS, word) || is_sorted_member(RUST_KEYWORDS, word),
    };
    if is_kw { return Some("keyword"); }
    if is_sorted_member(KNOWN_TYPES, word) { return Some("type"); }
    if let Some(first) = word.chars().next() {
        if first.is_uppercase() && (fam == LangFamily::CStyle || fam == LangFamily::Python) && word.len() > 1 {
            return Some("type");
        }
    }
    None
}

#[inline]
fn push_token(chars: &[SourceChar], start: usize, end: usize, cat: &str, spans: &mut Vec<UniFFITokenSpan>, total_u16: u32) {
    if start >= chars.len() || start >= end { return; }
    let loc = chars[start].u16_offset;
    let len = if end < chars.len() { chars[end].u16_offset.saturating_sub(loc) } else { total_u16.saturating_sub(loc) };
    if len > 0 { spans.push(UniFFITokenSpan { location: loc, length: len, category: cat.to_string() }); }
}

/// Tokenizes source code text into high-precision UTF-16 token spans for syntax highlighting.
#[uniffi::export]
pub fn tokenize_source_code(text: String, file_extension: String, max_length: u32) -> Vec<UniFFITokenSpan> {
    if text.is_empty() { return Vec::new(); }
    let (lang, fam) = map_extension(&file_extension);
    let mut chars = Vec::with_capacity(text.len().min(65536));
    let mut u16_offset = 0u32;
    for ch in text.chars() {
        if max_length > 0 && u16_offset >= max_length { break; }
        chars.push(SourceChar { ch, u16_offset });
        u16_offset += ch.len_utf16() as u32;
    }
    let total_u16 = u16_offset;
    let n = chars.len();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < n {
        let ch = chars[i].ch;
        if ch.is_whitespace() { i += 1; continue; }
        // C-style block comments /* ... */
        if (fam == LangFamily::CStyle || fam == LangFamily::Sql || fam == LangFamily::General) && ch == '/' && i + 1 < n && chars[i + 1].ch == '*' {
            let s = i; i += 2;
            while i < n {
                if chars[i].ch == '*' && i + 1 < n && chars[i + 1].ch == '/' { i += 2; break; }
                i += 1;
            }
            push_token(&chars, s, i, "comment", &mut spans, total_u16);
            continue;
        }
        // C-style line comments // ...
        if (fam == LangFamily::CStyle || fam == LangFamily::General) && ch == '/' && i + 1 < n && chars[i + 1].ch == '/' {
            let s = i; i += 2;
            while i < n && chars[i].ch != '\n' && chars[i].ch != '\r' { i += 1; }
            push_token(&chars, s, i, "comment", &mut spans, total_u16);
            continue;
        }
        // HTML / XML comments <!-- ... -->
        if fam == LangFamily::HtmlXml && ch == '<' && i + 3 < n && chars[i + 1].ch == '!' && chars[i + 2].ch == '-' && chars[i + 3].ch == '-' {
            let s = i; i += 4;
            while i < n {
                if chars[i].ch == '-' && i + 2 < n && chars[i + 1].ch == '-' && chars[i + 2].ch == '>' { i += 3; break; }
                i += 1;
            }
            push_token(&chars, s, i, "comment", &mut spans, total_u16);
            continue;
        }
        // Hash comments # ...
        if (fam == LangFamily::Python || fam == LangFamily::Shell || fam == LangFamily::General) && ch == '#' {
            let s = i; i += 1;
            while i < n && chars[i].ch != '\n' && chars[i].ch != '\r' { i += 1; }
            push_token(&chars, s, i, "comment", &mut spans, total_u16);
            continue;
        }
        // SQL comments -- ...
        if fam == LangFamily::Sql && ch == '-' && i + 1 < n && chars[i + 1].ch == '-' {
            let s = i; i += 2;
            while i < n && chars[i].ch != '\n' && chars[i].ch != '\r' { i += 1; }
            push_token(&chars, s, i, "comment", &mut spans, total_u16);
            continue;
        }
        // Python multiline strings """ or '''
        if fam == LangFamily::Python && (ch == '"' || ch == '\'') && i + 2 < n && chars[i + 1].ch == ch && chars[i + 2].ch == ch {
            let q = ch; let s = i; i += 3;
            while i < n {
                if chars[i].ch == '\\' { i += 2; continue; }
                if chars[i].ch == q && i + 2 < n && chars[i + 1].ch == q && chars[i + 2].ch == q { i += 3; break; }
                i += 1;
            }
            push_token(&chars, s, i, "string", &mut spans, total_u16);
            continue;
        }
        // Strings "...", '...', `...`
        if ch == '"' || ch == '\'' || ch == '`' {
            let q = ch; let s = i; i += 1;
            while i < n {
                if chars[i].ch == '\\' { i += 2; continue; }
                if chars[i].ch == q { i += 1; break; }
                if (q == '"' || q == '\'') && (chars[i].ch == '\n' || chars[i].ch == '\r') { break; }
                i += 1;
            }
            push_token(&chars, s, i, "string", &mut spans, total_u16);
            continue;
        }
        // Markdown headings
        if fam == LangFamily::Markdown && ch == '#' {
            let s = i;
            while i < n && (chars[i].ch == '#' || chars[i].ch == ' ') { i += 1; }
            push_token(&chars, s, i, "keyword", &mut spans, total_u16);
            continue;
        }
        // Numbers
        if ch.is_ascii_digit() {
            let s = i; i += 1;
            if ch == '0' && i < n && matches!(chars[i].ch, 'x' | 'X' | 'b' | 'B') {
                i += 1;
                while i < n && (chars[i].ch.is_ascii_hexdigit() || chars[i].ch == '_') { i += 1; }
            } else {
                while i < n && (chars[i].ch.is_ascii_digit() || chars[i].ch == '_') { i += 1; }
                if i + 1 < n && chars[i].ch == '.' && chars[i + 1].ch.is_ascii_digit() {
                    i += 2;
                    while i < n && (chars[i].ch.is_ascii_digit() || chars[i].ch == '_') { i += 1; }
                }
                if i < n && (chars[i].ch == 'e' || chars[i].ch == 'E') {
                    i += 1;
                    if i < n && (chars[i].ch == '+' || chars[i].ch == '-') { i += 1; }
                    while i < n && (chars[i].ch.is_ascii_digit() || chars[i].ch == '_') { i += 1; }
                }
            }
            push_token(&chars, s, i, "number", &mut spans, total_u16);
            continue;
        }
        // Identifiers / Keywords / Types
        if ch.is_alphabetic() || ch == '_' || ch == '@' || ch == '$' {
            let s = i; i += 1;
            while i < n && (chars[i].ch.is_alphanumeric() || chars[i].ch == '_') { i += 1; }
            let word: String = chars[s..i].iter().map(|c| c.ch).collect();
            if let Some(cat) = classify_identifier(&word, lang, fam) {
                push_token(&chars, s, i, cat, &mut spans, total_u16);
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
    fn test_keyword_tables_are_strictly_sorted() {
        assert!(SWIFT_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(RUST_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(CPP_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(PY_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(JS_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(GO_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(SQL_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(SHELL_KEYWORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(KNOWN_TYPES.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn test_swift_tokenization() {
        let code = "// Comment\nlet count: Int = 42\nlet s = \"hello 🚀\"";
        let spans = tokenize_source_code(code.to_string(), "swift".to_string(), 0);
        assert_eq!(spans.len(), 6);
        assert_eq!(spans[0].category, "comment");
        assert_eq!(spans[1].category, "keyword"); // let
        assert_eq!(spans[2].category, "type");    // Int
        assert_eq!(spans[3].category, "number");  // 42
        assert_eq!(spans[4].category, "keyword"); // let
        assert_eq!(spans[5].category, "string");  // "hello 🚀"
    }

    #[test]
    fn test_rust_tokenization() {
        let code = "pub fn calculate(val: u32) -> Result<String, ()> { /* block */ 0x1F }";
        let spans = tokenize_source_code(code.to_string(), "rs".to_string(), 0);
        let categories: Vec<&str> = spans.iter().map(|s| s.category.as_str()).collect();
        assert!(categories.contains(&"keyword"));
        assert!(categories.contains(&"type"));
        assert!(categories.contains(&"comment"));
        assert!(categories.contains(&"number"));
    }

    #[test]
    fn test_python_tokenization() {
        let code = "# Note\ndef process(data: str):\n    return \"value\"";
        let spans = tokenize_source_code(code.to_string(), "py".to_string(), 0);
        assert_eq!(spans[0].category, "comment");
        assert_eq!(spans[1].category, "keyword"); // def
        assert_eq!(spans[2].category, "type");    // str
        assert_eq!(spans[3].category, "keyword"); // return
        assert_eq!(spans[4].category, "string");  // "value"
    }
}
