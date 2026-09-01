// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-language grammar registry, file extension lookup, and Shebang auto-detection.

use std::path::Path;
use serde::{Deserialize, Serialize};
use super::error::{SyntaxError, SyntaxResult};

/// Supported programming, markup, and serialization languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportedLanguage {
    Rust,
    C,
    Cpp,
    Swift,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Json,
    Html,
    Css,
    Markdown,
}

impl SupportedLanguage {
    /// Canonical language identifier name.
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Swift => "swift",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Json => "json",
            Self::Html => "html",
            Self::Css => "css",
            Self::Markdown => "markdown",
        }
    }

    /// User-friendly display title.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Swift => "Swift",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Tsx => "TypeScript (TSX)",
            Self::Json => "JSON",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Markdown => "Markdown",
        }
    }

    /// Primary and secondary file extensions.
    pub const fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::C => &["c", "h"],
            Self::Cpp => &["cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++"],
            Self::Swift => &["swift"],
            Self::Python => &["py", "pyw", "pyi", "gyp"],
            Self::JavaScript => &["js", "mjs", "cjs", "jsx"],
            Self::TypeScript => &["ts", "mts", "cts"],
            Self::Tsx => &["tsx"],
            Self::Json => &["json", "jsonc", "json5"],
            Self::Html => &["html", "htm", "xhtml", "svg"],
            Self::Css => &["css", "scss", "less"],
            Self::Markdown => &["md", "markdown", "mdown", "mkd", "mdx"],
        }
    }

    /// Resolves the corresponding Tree-sitter `Language` object.
    #[cfg(feature = "syntax")]
    pub fn tree_sitter_language(&self) -> Option<tree_sitter::Language> {
        match self {
            Self::Rust => Some(tree_sitter_rust::language()),
            Self::C => Some(tree_sitter_c::language()),
            Self::Cpp => Some(tree_sitter_c::language()),
            Self::Swift => Some(tree_sitter_swift::language()),
            Self::Python => Some(tree_sitter_python::language()),
            Self::JavaScript => Some(tree_sitter_javascript::language()),
            Self::TypeScript => Some(tree_sitter_typescript::language_typescript()),
            Self::Tsx => Some(tree_sitter_typescript::language_tsx()),
            Self::Json => Some(tree_sitter_json::language()),
            Self::Html => Some(tree_sitter_html::language()),
            Self::Css => Some(tree_sitter_css::language()),
            Self::Markdown => Some(tree_sitter_md::language()),
        }
    }

    /// Built-in highlight query S-expression for Tree-sitter query pattern matching.
    pub const fn default_highlight_query(&self) -> &'static str {
        match self {
            Self::Rust => RUST_HIGHLIGHT_QUERY,
            Self::C | Self::Cpp => C_HIGHLIGHT_QUERY,
            Self::Swift => SWIFT_HIGHLIGHT_QUERY,
            Self::Python => PYTHON_HIGHLIGHT_QUERY,
            Self::JavaScript => JS_HIGHLIGHT_QUERY,
            Self::TypeScript | Self::Tsx => TS_HIGHLIGHT_QUERY,
            Self::Json => JSON_HIGHLIGHT_QUERY,
            Self::Html => HTML_HIGHLIGHT_QUERY,
            Self::Css => CSS_HIGHLIGHT_QUERY,
            Self::Markdown => MARKDOWN_HIGHLIGHT_QUERY,
        }
    }
}

/// Global language registry with multi-stage heuristic auto-detection.
pub struct LanguageRegistry;

impl LanguageRegistry {
    /// Array of all supported languages.
    pub const ALL_LANGUAGES: &'static [SupportedLanguage] = &[
        SupportedLanguage::Rust,
        SupportedLanguage::C,
        SupportedLanguage::Cpp,
        SupportedLanguage::Swift,
        SupportedLanguage::Python,
        SupportedLanguage::JavaScript,
        SupportedLanguage::TypeScript,
        SupportedLanguage::Tsx,
        SupportedLanguage::Json,
        SupportedLanguage::Html,
        SupportedLanguage::Css,
        SupportedLanguage::Markdown,
    ];

    /// Look up supported language by identifier string.
    pub fn from_id(id: &str) -> Option<SupportedLanguage> {
        let clean = id.trim().to_ascii_lowercase();
        match clean.as_str() {
            "rust" | "rs" => Some(SupportedLanguage::Rust),
            "c" => Some(SupportedLanguage::C),
            "cpp" | "c++" | "cxx" => Some(SupportedLanguage::Cpp),
            "swift" => Some(SupportedLanguage::Swift),
            "python" | "py" => Some(SupportedLanguage::Python),
            "javascript" | "js" | "jsx" => Some(SupportedLanguage::JavaScript),
            "typescript" | "ts" => Some(SupportedLanguage::TypeScript),
            "tsx" => Some(SupportedLanguage::Tsx),
            "json" | "jsonc" | "json5" => Some(SupportedLanguage::Json),
            "html" | "htm" => Some(SupportedLanguage::Html),
            "css" | "scss" | "less" => Some(SupportedLanguage::Css),
            "markdown" | "md" => Some(SupportedLanguage::Markdown),
            _ => None,
        }
    }

    /// Detect language by file extension.
    pub fn from_extension(ext: &str) -> Option<SupportedLanguage> {
        let clean_ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        match clean_ext.as_str() {
            "rs" => Some(SupportedLanguage::Rust),
            "c" | "h" => Some(SupportedLanguage::C),
            "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" => {
                Some(SupportedLanguage::Cpp)
            }
            "swift" => Some(SupportedLanguage::Swift),
            "py" | "pyw" | "pyi" | "gyp" => Some(SupportedLanguage::Python),
            "js" | "mjs" | "cjs" | "jsx" => Some(SupportedLanguage::JavaScript),
            "ts" | "mts" | "cts" => Some(SupportedLanguage::TypeScript),
            "tsx" => Some(SupportedLanguage::Tsx),
            "json" | "jsonc" | "json5" => Some(SupportedLanguage::Json),
            "html" | "htm" | "xhtml" | "svg" => Some(SupportedLanguage::Html),
            "css" | "scss" | "less" => Some(SupportedLanguage::Css),
            "md" | "markdown" | "mdown" | "mkd" | "mdx" => Some(SupportedLanguage::Markdown),
            _ => None,
        }
    }

    /// Detect language by file path name and filename heuristics.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<SupportedLanguage> {
        let p = path.as_ref();
        if let Some(file_name) = p.file_name().and_then(|s| s.to_str()) {
            let lower_name = file_name.to_ascii_lowercase();
            match lower_name.as_str() {
                "cargo.toml" | "cargo.lock" => return Some(SupportedLanguage::Rust),
                "package.json" | "tsconfig.json" | "jsconfig.json" => {
                    return Some(SupportedLanguage::Json)
                }
                "readme" | "license" | "contributing" | "changelog" => {
                    return Some(SupportedLanguage::Markdown)
                }
                _ => {}
            }
        }

        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            if let Some(lang) = Self::from_extension(ext) {
                return Some(lang);
            }
        }

        None
    }

    /// Detect language from Shebang line or initial source snippet.
    pub fn from_shebang(header: &str) -> Option<SupportedLanguage> {
        let trimmed = header.trim_start();
        if trimmed.starts_with("#!") {
            let first_line = trimmed.lines().next().unwrap_or("");
            let lower = first_line.to_ascii_lowercase();
            // Specific interpreters checked before generic ones
            if lower.contains("ts-node") {
                return Some(SupportedLanguage::TypeScript);
            }
            if lower.contains("python") || lower.contains("pypy") {
                return Some(SupportedLanguage::Python);
            }
            if lower.contains("node") || lower.contains("deno") || lower.contains("bun") {
                return Some(SupportedLanguage::JavaScript);
            }
            if lower.contains("swift") {
                return Some(SupportedLanguage::Swift);
            }
            if lower.contains("cargo") {
                return Some(SupportedLanguage::Rust);
            }
        }

        // XML / HTML heuristics
        if trimmed.starts_with("<!DOCTYPE html")
            || trimmed.starts_with("<html")
            || trimmed.starts_with("<!doctype html")
        {
            return Some(SupportedLanguage::Html);
        }

        // JSON heuristics
        if (trimmed.starts_with('{') || trimmed.starts_with('['))
            && (trimmed.contains(':') || trimmed.contains('"'))
            && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
        {
            return Some(SupportedLanguage::Json);
        }

        None
    }

    /// Unified auto-detection pipeline combining path, extension, and content heuristics.
    pub fn detect(path: Option<&Path>, content: Option<&str>) -> SyntaxResult<SupportedLanguage> {
        if let Some(p) = path {
            if let Some(lang) = Self::from_path(p) {
                return Ok(lang);
            }
        }

        if let Some(c) = content {
            if let Some(lang) = Self::from_shebang(c) {
                return Ok(lang);
            }
        }

        let hint = path
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown source".to_string());
        Err(SyntaxError::UnsupportedLanguage(hint))
    }
}

// ============================================================================
// Validated Tree-sitter S-Expression Highlight Queries
// ============================================================================

pub const RUST_HIGHLIGHT_QUERY: &str = r#"
[
  "as" "async" "await" "break" "const" "continue" "dyn" "else" "enum"
  "extern" "fn" "for" "if" "impl" "in" "let" "loop" "match" "mod" "move"
  "pub" "ref" "return" "static" "struct"
  "trait" "type" "unsafe" "use" "where" "while"
] @keyword

(mutable_specifier) @keyword
(self) @keyword
(super) @keyword
(crate) @keyword

(function_item name: (identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (field_expression field: (field_identifier) @function.call))
(macro_invocation macro: (identifier) @function.macro)

(type_identifier) @type
(primitive_type) @type.builtin

(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(integer_literal) @number
(float_literal) @number
(boolean_literal) @boolean

(line_comment) @comment
(block_comment) @comment

(attribute_item) @attribute
(field_identifier) @property
"#;

pub const PYTHON_HIGHLIGHT_QUERY: &str = r#"
[
  "and" "as" "assert" "async" "await" "break" "class" "continue" "def"
  "del" "elif" "else" "except" "finally" "for" "from" "global" "if"
  "import" "in" "is" "lambda" "nonlocal" "not" "or" "pass" "raise"
  "return" "try" "while" "with" "yield" "match" "case"
] @keyword

(function_definition name: (identifier) @function)
(call function: (identifier) @function.call)
(call function: (attribute attribute: (identifier) @function.call))
(class_definition name: (identifier) @type)

(string) @string
(integer) @number
(float) @number
(true) @boolean
(false) @boolean
(none) @constant.builtin

(comment) @comment
(decorator) @attribute
"#;

pub const C_HIGHLIGHT_QUERY: &str = r#"
[
  "auto" "break" "case" "const" "continue" "default" "do"
  "else" "enum" "extern" "for" "goto" "if" "inline"
  "register" "restrict" "return" "sizeof" "static"
  "struct" "switch" "typedef" "union" "volatile" "while"
] @keyword

(function_declarator declarator: (identifier) @function)
(call_expression function: (identifier) @function.call)

(type_identifier) @type
(primitive_type) @type.builtin

(string_literal) @string
(char_literal) @string
(number_literal) @number

(comment) @comment
(preproc_include) @attribute
(preproc_def) @attribute
(preproc_function_def) @attribute
(preproc_call) @attribute
"#;

pub const JS_HIGHLIGHT_QUERY: &str = r#"
[
  "async" "await" "break" "case" "catch" "class" "const" "continue" "debugger"
  "default" "delete" "do" "else" "export" "extends" "finally" "for" "from"
  "function" "get" "if" "import" "in" "instanceof" "let" "new" "of" "return"
  "set" "static" "switch" "throw" "try" "typeof" "var" "void"
  "while" "with" "yield"
] @keyword

(this) @keyword
(super) @keyword

(function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (member_expression property: (property_identifier) @function.call))
(class_declaration name: (identifier) @type)

(string) @string
(template_string) @string
(number) @number
(true) @boolean
(false) @boolean
(null) @constant.builtin
(undefined) @constant.builtin

(comment) @comment
(property_identifier) @property
"#;

pub const TS_HIGHLIGHT_QUERY: &str = r#"
[
  "async" "await" "break" "case" "catch" "class" "const" "continue" "debugger"
  "default" "delete" "do" "else" "export" "extends" "finally" "for" "from"
  "function" "get" "if" "import" "in" "instanceof" "let" "new" "of" "return"
  "set" "static" "switch" "throw" "try" "typeof" "var" "void"
  "while" "with" "yield" "enum" "interface" "type" "namespace" "implements"
  "declare" "abstract" "readonly" "as" "is" "keyof"
] @keyword

(this) @keyword
(super) @keyword

(function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (member_expression property: (property_identifier) @function.call))

(class_declaration name: (type_identifier) @type)
(type_alias_declaration name: (type_identifier) @type)
(interface_declaration name: (type_identifier) @type)
(type_identifier) @type

(string) @string
(template_string) @string
(number) @number
(true) @boolean
(false) @boolean
(null) @constant.builtin
(undefined) @constant.builtin

(comment) @comment
(property_identifier) @property
"#;

pub const SWIFT_HIGHLIGHT_QUERY: &str = r#"
(function_declaration name: (simple_identifier) @function)
(type_identifier) @type

(line_string_literal) @string
(multi_line_string_literal) @string
(integer_literal) @number
(real_literal) @number
(boolean_literal) @boolean

(comment) @comment
(multiline_comment) @comment
"#;

pub const JSON_HIGHLIGHT_QUERY: &str = r#"
(pair key: (string) @property)
(string) @string
(number) @number
(true) @boolean
(false) @boolean
(null) @constant.builtin
(comment) @comment
"#;

pub const HTML_HIGHLIGHT_QUERY: &str = r#"
(tag_name) @tag
(erroneous_end_tag_name) @tag
(attribute_name) @attribute
(quoted_attribute_value) @string
(attribute_value) @string
(comment) @comment
(text) @variable
"#;

pub const CSS_HIGHLIGHT_QUERY: &str = r#"
(tag_name) @tag
(class_name) @type
(id_name) @type
(property_name) @property
(string_value) @string
(integer_value) @number
(float_value) @number
(color_value) @constant
(comment) @comment
"#;

pub const MARKDOWN_HIGHLIGHT_QUERY: &str = r#"
(atx_heading) @heading
(setext_heading) @heading
(fenced_code_block) @string
(indented_code_block) @string
(block_quote) @comment
(thematic_break) @operator
(list_item) @punctuation
"#;
