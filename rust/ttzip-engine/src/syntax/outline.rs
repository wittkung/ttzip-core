// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-allocation AST traversal and multi-language symbol outline tree extractor.

use serde::{Deserialize, Serialize};

use super::error::SyntaxResult;
use super::parser::TTZipSyntaxParser;
use super::registry::SupportedLanguage;

/// Canonical symbol categories for IDE code navigation and symbol outlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Trait,
    Macro,
    TypeAlias,
    Heading,
    Tag,
}

impl SymbolKind {
    /// String identifier for the symbol kind.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Namespace => "namespace",
            Self::Package => "package",
            Self::Class => "class",
            Self::Method => "method",
            Self::Property => "property",
            Self::Field => "field",
            Self::Constructor => "constructor",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Function => "function",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Array => "array",
            Self::Object => "object",
            Self::Key => "key",
            Self::Null => "null",
            Self::EnumMember => "enum_member",
            Self::Struct => "struct",
            Self::Event => "event",
            Self::Operator => "operator",
            Self::TypeParameter => "type_parameter",
            Self::Trait => "trait",
            Self::Macro => "macro",
            Self::TypeAlias => "type_alias",
            Self::Heading => "heading",
            Self::Tag => "tag",
        }
    }
}

/// Outline node representing a code symbol in the hierarchical navigation tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolNode {
    /// Symbol name identifier (e.g. `parse_full`, `TTZipSyntaxParser`).
    pub name: String,
    /// Semantic category of the symbol.
    pub kind: SymbolKind,
    /// Optional supplementary details (e.g. type signature or heading level).
    pub detail: Option<String>,
    /// Full node byte range `[start_byte, end_byte)`.
    pub start_byte: usize,
    pub end_byte: usize,
    /// 0-based line and column coordinates.
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
    /// Selection byte range focused specifically on the name identifier.
    pub selection_start_byte: usize,
    pub selection_end_byte: usize,
    /// Nested child symbols.
    pub children: Vec<SymbolNode>,
}

impl SymbolNode {
    /// Recursively flattens this symbol and its children into a linear vector.
    pub fn flatten(&self) -> Vec<&SymbolNode> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.flatten());
        }
        result
    }

    /// Checks whether the symbol span contains the specified byte offset.
    #[inline]
    pub fn contains_byte(&self, byte: usize) -> bool {
        byte >= self.start_byte && byte < self.end_byte
    }

    /// Checks whether the symbol span contains the given 0-based row and column.
    pub fn contains_position(&self, row: usize, col: usize) -> bool {
        if row < self.start_row || row > self.end_row {
            return false;
        }
        if row == self.start_row && col < self.start_col {
            return false;
        }
        if row == self.end_row && col > self.end_col {
            return false;
        }
        true
    }
}

/// High-throughput symbol outline extractor leveraging zero-allocation `TreeCursor` traversal.
pub struct SymbolOutlineExtractor;

impl SymbolOutlineExtractor {
    /// Extracts hierarchical symbol outline tree from a pre-parsed AST.
    #[cfg(feature = "syntax")]
    pub fn extract(
        tree: &tree_sitter::Tree,
        source: &str,
        lang: SupportedLanguage,
    ) -> Vec<SymbolNode> {
        let root = tree.root_node();
        let mut symbols = Vec::new();
        Self::collect_symbols_from_node(root, source, lang, false, &mut symbols);

        if lang == SupportedLanguage::Markdown {
            nest_markdown_headings(symbols)
        } else {
            symbols
        }
    }

    /// One-shot parse and outline extraction helper.
    #[cfg(feature = "syntax")]
    pub fn extract_from_source(
        source: &str,
        lang: SupportedLanguage,
    ) -> SyntaxResult<Vec<SymbolNode>> {
        let mut parser = TTZipSyntaxParser::with_language(lang)?;
        let tree = parser.parse_full(source)?;
        Ok(Self::extract(tree, source, lang))
    }

    /// Recursively traverses and extracts symbols from AST nodes.
    #[cfg(feature = "syntax")]
    fn collect_symbols_from_node(
        node: tree_sitter::Node<'_>,
        source: &str,
        lang: SupportedLanguage,
        is_inside_container: bool,
        output: &mut Vec<SymbolNode>,
    ) {
        let kind_str = node.kind();
        if let Some((name, kind, detail, sel_start, sel_end)) =
            extract_symbol_info(node, kind_str, source, lang, is_inside_container)
        {
            let start_pos = node.start_position();
            let end_pos = node.end_position();
            let mut children = Vec::new();

            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    Self::collect_symbols_from_node(
                        cursor.node(),
                        source,
                        lang,
                        true,
                        &mut children,
                    );
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }

            output.push(SymbolNode {
                name,
                kind,
                detail,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_row: start_pos.row,
                start_col: start_pos.column,
                end_row: end_pos.row,
                end_col: end_pos.column,
                selection_start_byte: sel_start,
                selection_end_byte: sel_end,
                children,
            });
        } else {
            let mut cursor = node.walk();
            if cursor.goto_first_child() {
                loop {
                    Self::collect_symbols_from_node(
                        cursor.node(),
                        source,
                        lang,
                        is_inside_container,
                        output,
                    );
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    /// Finds the most specific symbol node enclosing the given cursor position.
    pub fn find_symbol_at_position(
        roots: &[SymbolNode],
        row: usize,
        col: usize,
    ) -> Option<&SymbolNode> {
        for node in roots {
            if node.contains_position(row, col) {
                if let Some(child) = Self::find_symbol_at_position(&node.children, row, col) {
                    return Some(child);
                }
                return Some(node);
            }
        }
        None
    }

    /// Flattens a slice of symbol root nodes.
    pub fn flatten_symbols(roots: &[SymbolNode]) -> Vec<&SymbolNode> {
        let mut out = Vec::new();
        for r in roots {
            out.extend(r.flatten());
        }
        out
    }
}

#[cfg(feature = "syntax")]
fn extract_symbol_info<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
    lang: SupportedLanguage,
    is_inside_container: bool,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    match lang {
        SupportedLanguage::Rust => extract_rust_symbol(node, kind_str, source, is_inside_container),
        SupportedLanguage::Python => extract_python_symbol(node, kind_str, source, is_inside_container),
        SupportedLanguage::JavaScript
        | SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx => extract_js_ts_symbol(node, kind_str, source),
        SupportedLanguage::C | SupportedLanguage::Cpp => extract_c_cpp_symbol(node, kind_str, source),
        SupportedLanguage::Swift => extract_swift_symbol(node, kind_str, source),
        SupportedLanguage::Markdown => extract_markdown_symbol(node, kind_str, source),
        SupportedLanguage::Json => extract_json_symbol(node, kind_str, source),
        SupportedLanguage::Html => extract_html_symbol(node, kind_str, source),
        SupportedLanguage::Css => extract_css_symbol(node, kind_str, source),
    }
}

// ----------------------------------------------------------------------------
// Rust Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_rust_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
    is_inside_container: bool,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    match kind_str {
        "function_item" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let kind = if is_inside_container {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, kind, None, s, e))
        }
        "struct_item" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Struct, None, s, e))
        }
        "enum_item" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Enum, None, s, e))
        }
        "enum_variant" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::EnumMember, None, s, e))
        }
        "trait_item" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Trait, None, s, e))
        }
        "impl_item" => {
            let type_node = node.child_by_field_name("type")?;
            let trait_node = node.child_by_field_name("trait");
            let name = if let Some(tr) = trait_node {
                format!(
                    "impl {} for {}",
                    node_text(tr, source),
                    node_text(type_node, source)
                )
            } else {
                format!("impl {}", node_text(type_node, source))
            };
            let (s, e) = (type_node.start_byte(), type_node.end_byte());
            Some((name, SymbolKind::Class, None, s, e))
        }
        "mod_item" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Module, None, s, e))
        }
        "macro_definition" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Macro, None, s, e))
        }
        "type_item" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::TypeAlias, None, s, e))
        }
        "const_item" | "static_item" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Constant, None, s, e))
        }
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// Python Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_python_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
    is_inside_container: bool,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    match kind_str {
        "function_definition" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let kind = if is_inside_container {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, kind, None, s, e))
        }
        "class_definition" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Class, None, s, e))
        }
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// JS / TS Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_js_ts_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    match kind_str {
        "function_declaration" | "function" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Function, None, s, e))
        }
        "method_definition" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Method, None, s, e))
        }
        "class_declaration" | "class" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Class, None, s, e))
        }
        "interface_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Interface, None, s, e))
        }
        "type_alias_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::TypeAlias, None, s, e))
        }
        "enum_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Enum, None, s, e))
        }
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// C / C++ Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_c_cpp_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    match kind_str {
        "function_definition" => {
            let decl = node.child_by_field_name("declarator")?;
            let name_node = find_nested_identifier(decl)?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Function, None, s, e))
        }
        "struct_specifier" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Struct, None, s, e))
        }
        "class_specifier" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Class, None, s, e))
        }
        "enum_specifier" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Enum, None, s, e))
        }
        "type_definition" => {
            let name_node = node.child_by_field_name("declarator")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::TypeAlias, None, s, e))
        }
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// Swift Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_swift_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    match kind_str {
        "function_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Function, None, s, e))
        }
        "class_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Class, None, s, e))
        }
        "struct_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Struct, None, s, e))
        }
        "enum_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Enum, None, s, e))
        }
        "protocol_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = node_text(name_node, source).to_string();
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Interface, None, s, e))
        }
        "extension_declaration" => {
            let name_node = node.child_by_field_name("type")?;
            let name = format!("extension {}", node_text(name_node, source));
            let (s, e) = (name_node.start_byte(), name_node.end_byte());
            Some((name, SymbolKind::Module, None, s, e))
        }
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// Markdown Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_markdown_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    if kind_str == "atx_heading" || kind_str == "setext_heading" || kind_str == "heading" {
        let text = node_text(node, source).trim();
        let hashes = text.chars().take_while(|&c| c == '#').count();
        let level = if hashes > 0 { hashes } else { 1 };
        let title = text.trim_start_matches('#').trim();
        let (s, e) = (node.start_byte(), node.end_byte());
        Some((
            title.to_string(),
            SymbolKind::Heading,
            Some(format!("H{}", level)),
            s,
            e,
        ))
    } else {
        None
    }
}

// ----------------------------------------------------------------------------
// JSON Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_json_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    if kind_str == "pair" {
        let key_node = node.child_by_field_name("key")?;
        let key_text = node_text(key_node, source)
            .trim_matches('"')
            .to_string();
        let (s, e) = (key_node.start_byte(), key_node.end_byte());
        Some((key_text, SymbolKind::Field, None, s, e))
    } else {
        None
    }
}

// ----------------------------------------------------------------------------
// HTML Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_html_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    if kind_str == "element" {
        let start_tag = node.child(0)?;
        let tag_name_node = start_tag.child_by_field_name("name")?;
        let tag_name = node_text(tag_name_node, source).to_string();
        let (s, e) = (tag_name_node.start_byte(), tag_name_node.end_byte());
        Some((tag_name, SymbolKind::Tag, None, s, e))
    } else {
        None
    }
}

// ----------------------------------------------------------------------------
// CSS Symbol Extractor
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
fn extract_css_symbol<'a>(
    node: tree_sitter::Node<'a>,
    kind_str: &str,
    source: &str,
) -> Option<(String, SymbolKind, Option<String>, usize, usize)> {
    if kind_str == "rule_set" {
        let selectors_node = node.child_by_field_name("selectors")?;
        let name = node_text(selectors_node, source).trim().to_string();
        let (s, e) = (selectors_node.start_byte(), selectors_node.end_byte());
        Some((name, SymbolKind::Class, None, s, e))
    } else {
        None
    }
}

// ----------------------------------------------------------------------------
// Utilities
// ----------------------------------------------------------------------------
#[cfg(feature = "syntax")]
#[inline]
fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> &'a str {
    let (s, e) = (node.start_byte(), node.end_byte());
    if e <= source.len() && s <= e {
        &source[s..e]
    } else {
        ""
    }
}

#[cfg(feature = "syntax")]
fn find_nested_identifier<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == "identifier" || node.kind() == "field_identifier" {
        return Some(node);
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if let Some(found) = find_nested_identifier(child) {
                return Some(found);
            }
        }
    }
    None
}

/// Nests linear markdown headings into a hierarchical tree based on H1 > H2 > H3.
fn nest_markdown_headings(headings: Vec<SymbolNode>) -> Vec<SymbolNode> {
    let mut root_headings: Vec<SymbolNode> = Vec::new();
    let mut stack: Vec<(usize, SymbolNode)> = Vec::new();

    for heading in headings {
        let level = heading
            .detail
            .as_ref()
            .and_then(|d| d.strip_prefix('H'))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1);

        while let Some((parent_level, _)) = stack.last() {
            if *parent_level >= level {
                let (_, completed) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push(completed);
                } else {
                    root_headings.push(completed);
                }
            } else {
                break;
            }
        }

        stack.push((level, heading));
    }

    while let Some((_, completed)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(completed);
        } else {
            root_headings.push(completed);
        }
    }

    root_headings
}
