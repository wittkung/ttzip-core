// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-Precision AST Symbol Outline and Structure Extraction Engine.

use super::highlighter::SourceLayoutIndex;
use super::types::UniFFISymbolNode;
use crate::standards::syntax_highlight::SupportedLanguage;

/// Extracts structural outline symbol tree from source code.
pub fn extract_symbols_internal(code: &str, language_hint: &str) -> Vec<UniFFISymbolNode> {
    if code.trim().is_empty() {
        return Vec::new();
    }

    let layout = SourceLayoutIndex::new(code);

    #[cfg(feature = "syntax")]
    {
        if let Some(lang) = SupportedLanguage::from_extension(language_hint) {
            if let Some(ts_lang) = lang.get_tree_sitter_language() {
                let mut parser = tree_sitter::Parser::new();
                if parser.set_language(&ts_lang).is_ok() {
                    if let Some(tree) = parser.parse(code, None) {
                        let nodes = extract_tree_sitter_symbols(&tree, code, &layout, lang);
                        if !nodes.is_empty() {
                            return nodes;
                        }
                    }
                }
            }
        }
    }

    // Heuristic fallback for all supported formats and general text
    fallback_extract_symbols(code, &layout, language_hint)
}

#[cfg(feature = "syntax")]
fn extract_tree_sitter_symbols(
    tree: &tree_sitter::Tree,
    source: &str,
    layout: &SourceLayoutIndex,
    lang: SupportedLanguage,
) -> Vec<UniFFISymbolNode> {
    let mut results = Vec::new();
    let root = tree.root_node();
    collect_ts_symbols(&root, source, layout, lang, &mut results);

    if results.is_empty() {
        return fallback_extract_symbols(source, layout, &format!("{:?}", lang));
    }

    results
}

#[cfg(feature = "syntax")]
fn collect_ts_symbols(
    node: &tree_sitter::Node,
    source: &str,
    layout: &SourceLayoutIndex,
    lang: SupportedLanguage,
    results: &mut Vec<UniFFISymbolNode>,
) {
    if let Some(sym) = parse_ts_node(node, source, layout, lang) {
        results.push(sym);
        return;
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            collect_ts_symbols(&child, source, layout, lang, results);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

#[cfg(feature = "syntax")]
fn parse_ts_node(
    node: &tree_sitter::Node,
    source: &str,
    layout: &SourceLayoutIndex,
    lang: SupportedLanguage,
) -> Option<UniFFISymbolNode> {
    let kind = node.kind();
    let (sb, eb) = (node.start_byte(), node.end_byte());
    if eb <= sb {
        return None;
    }

    let (location, length, line_number, _) = layout.locate_span(sb, eb);

    let (sym_kind, name, detail) = match lang {
        SupportedLanguage::Rust => match kind {
            "function_item" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "fn".to_string());
                ("function", name, extract_first_line(source, sb, eb))
            }
            "struct_item" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "struct".to_string());
                ("struct", format!("struct {}", name), None)
            }
            "enum_item" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "enum".to_string());
                ("enum", format!("enum {}", name), None)
            }
            "trait_item" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "trait".to_string());
                ("trait", format!("trait {}", name), None)
            }
            "impl_item" => {
                let header = extract_first_line(source, sb, eb).unwrap_or_else(|| "impl".to_string());
                ("impl", header, None)
            }
            "mod_item" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "mod".to_string());
                ("module", format!("mod {}", name), None)
            }
            "type_item" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "type".to_string());
                ("type", format!("type {}", name), None)
            }
            _ => return None,
        },
        SupportedLanguage::Swift => match kind {
            "class_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "protocol_declaration"
            | "extension_declaration" => {
                let slice = &source[sb..eb.min(source.len())];
                let first_line = slice.lines().next().unwrap_or("").trim();
                let stripped = strip_modifiers(first_line);
                let (sym_kind, prefix) = if stripped.starts_with("struct ") {
                    ("struct", "struct")
                } else if stripped.starts_with("enum ") {
                    ("enum", "enum")
                } else if stripped.starts_with("protocol ") {
                    ("protocol", "protocol")
                } else if stripped.starts_with("extension ") {
                    ("extension", "extension")
                } else if stripped.starts_with("actor ") {
                    ("class", "actor")
                } else {
                    ("class", "class")
                };

                let raw_name = get_child_text(node, "name", source).unwrap_or_else(|| {
                    stripped.split('{').next().unwrap_or(stripped).trim().to_string()
                });
                let full_name = if raw_name.starts_with(prefix) {
                    raw_name
                } else {
                    format!("{} {}", prefix, raw_name)
                };
                (sym_kind, full_name, None)
            }
            "function_declaration" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "func".to_string());
                ("function", name, extract_first_line(source, sb, eb))
            }
            _ => return None,
        },
        SupportedLanguage::Python => match kind {
            "function_definition" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "def".to_string());
                ("function", name, extract_first_line(source, sb, eb))
            }
            "class_definition" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "class".to_string());
                ("class", format!("class {}", name), None)
            }
            _ => return None,
        },
        SupportedLanguage::C | SupportedLanguage::Cpp => match kind {
            "function_definition" => {
                let header = extract_first_line(source, sb, eb).unwrap_or_else(|| "function".to_string());
                ("function", header, None)
            }
            "struct_specifier" => ("struct", extract_first_line(source, sb, eb).unwrap_or_else(|| "struct".to_string()), None),
            "class_specifier" => ("class", extract_first_line(source, sb, eb).unwrap_or_else(|| "class".to_string()), None),
            "enum_specifier" => ("enum", extract_first_line(source, sb, eb).unwrap_or_else(|| "enum".to_string()), None),
            _ => return None,
        },
        SupportedLanguage::JavaScript | SupportedLanguage::TypeScript => match kind {
            "function_declaration" | "generator_function_declaration" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "function".to_string());
                ("function", name, extract_first_line(source, sb, eb))
            }
            "class_declaration" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "class".to_string());
                ("class", format!("class {}", name), None)
            }
            "interface_declaration" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "interface".to_string());
                ("interface", format!("interface {}", name), None)
            }
            "type_alias_declaration" => {
                let name = get_child_text(node, "name", source).unwrap_or_else(|| "type".to_string());
                ("type", format!("type {}", name), None)
            }
            _ => return None,
        },
        _ => return None,
    };

    // Extract child items recursively
    let mut children = Vec::new();
    let mut child_cursor = node.walk();
    if child_cursor.goto_first_child() {
        loop {
            let child_node = child_cursor.node();
            if child_node.id() != node.id() {
                collect_ts_symbols(&child_node, source, layout, lang, &mut children);
            }
            if !child_cursor.goto_next_sibling() {
                break;
            }
        }
    }

    Some(UniFFISymbolNode {
        name,
        kind: sym_kind.to_string(),
        location,
        length,
        line_number,
        detail,
        children,
    })
}

#[cfg(feature = "syntax")]
fn get_child_text(node: &tree_sitter::Node, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name).and_then(|c| {
        let (s, e) = (c.start_byte(), c.end_byte());
        if e <= source.len() && s < e {
            Some(source[s..e].trim().to_string())
        } else {
            None
        }
    })
}

fn extract_first_line(source: &str, sb: usize, eb: usize) -> Option<String> {
    if sb >= source.len() {
        return None;
    }
    let slice = &source[sb..eb.min(source.len())];
    let line = slice.lines().next()?.trim();
    let clean = line.trim_end_matches('{').trim_end_matches(':').trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_string())
    }
}

/// Robust fallback outline extractor for Markdown headings, code blocks, and language declarations.
fn fallback_extract_symbols(code: &str, layout: &SourceLayoutIndex, hint: &str) -> Vec<UniFFISymbolNode> {
    let lower_hint = hint.to_ascii_lowercase();
    let is_md = lower_hint.contains("md") || lower_hint.contains("markdown");

    if is_md {
        return extract_markdown_headings(code, layout);
    }

    let mut nodes = Vec::new();
    let mut byte_offset = 0usize;

    for (line_idx, line) in code.lines().enumerate() {
        let trimmed = line.trim();
        let line_len = line.len() + 1; // approximate newline
        let line_number = (line_idx + 1) as u32;

        if !trimmed.is_empty() {
            if let Some((kind, name)) = match_declaration_signature(trimmed) {
                let (loc, len, _, _) = layout.locate_span(byte_offset, byte_offset + line.len());
                nodes.push(UniFFISymbolNode {
                    name,
                    kind: kind.to_string(),
                    location: loc,
                    length: len,
                    line_number,
                    detail: Some(trimmed.to_string()),
                    children: Vec::new(),
                });
            }
        }
        byte_offset += line_len;
    }

    nodes
}

fn strip_modifiers(line: &str) -> &str {
    let mut rest = line.trim();
    loop {
        let mut stripped = false;
        for prefix in &[
            "pub ", "pub(crate) ", "pub(super) ", "public ", "private ", "fileprivate ",
            "internal ", "open ", "final ", "static ", "mutating ", "nonmutating ",
            "override ", "export ", "default ", "async ",
        ] {
            if rest.starts_with(prefix) {
                rest = rest[prefix.len()..].trim_start();
                stripped = true;
            }
        }
        if !stripped {
            break;
        }
    }
    rest
}

fn match_declaration_signature(line: &str) -> Option<(&'static str, String)> {
    let l = line.trim();
    if l.starts_with("//") || l.starts_with("/*") || l.starts_with('*') || l.starts_with('#') {
        return None;
    }

    let stripped = strip_modifiers(l);

    if stripped.starts_with("fn ") || stripped.starts_with("func ") || stripped.starts_with("def ") {
        let name = stripped.split('(').next().unwrap_or(stripped).trim();
        return Some(("function", name.to_string()));
    }
    if stripped.starts_with("struct ") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("struct", name.to_string()));
    }
    if stripped.starts_with("enum ") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("enum", name.to_string()));
    }
    if stripped.starts_with("trait ") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("trait", name.to_string()));
    }
    if stripped.starts_with("impl ") || stripped.starts_with("impl<") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("impl", name.to_string()));
    }
    if stripped.starts_with("class ") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("class", name.to_string()));
    }
    if stripped.starts_with("protocol ") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("protocol", name.to_string()));
    }
    if stripped.starts_with("extension ") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("extension", name.to_string()));
    }
    if stripped.starts_with("interface ") {
        let name = stripped.split('{').next().unwrap_or(stripped).trim();
        return Some(("interface", name.to_string()));
    }
    if stripped.starts_with("type ") {
        let name = stripped.split('=').next().unwrap_or(stripped).trim();
        return Some(("type", name.to_string()));
    }

    None
}

fn extract_markdown_headings(code: &str, layout: &SourceLayoutIndex) -> Vec<UniFFISymbolNode> {
    let mut roots: Vec<UniFFISymbolNode> = Vec::new();
    let mut byte_offset = 0usize;

    for (line_idx, line) in code.lines().enumerate() {
        let trimmed = line.trim_start();
        let line_len = line.len() + 1;
        let line_number = (line_idx + 1) as u32;

        if trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
                let title = trimmed[hashes..].trim();
                let (loc, len, _, _) = layout.locate_span(byte_offset, byte_offset + line.len());
                let node = UniFFISymbolNode {
                    name: title.to_string(),
                    kind: format!("h{}", hashes),
                    location: loc,
                    length: len,
                    line_number,
                    detail: Some(format!("Heading Level {}", hashes)),
                    children: Vec::new(),
                };
                roots.push(node);
            }
        }
        byte_offset += line_len;
    }

    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_symbols() {
        let code = r#"
pub struct ArchiveReader {
    path: String,
}

impl ArchiveReader {
    pub fn open(path: &str) -> Self {
        Self { path: path.to_string() }
    }
}

pub fn standalone_function() -> bool {
    true
}
"#;
        let symbols = extract_symbols_internal(code, "rs");
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.kind == "struct" && s.name.contains("ArchiveReader")));
        assert!(symbols.iter().any(|s| s.kind == "function" || s.kind == "impl"));
    }

    #[test]
    fn test_extract_swift_symbols() {
        let code = r#"
public class EngineManager {
    public struct Config {
        let threads: Int
    }
}

public protocol ArchivingProtocol {
    func compress()
}

extension EngineManager {
    func reset() {}
}
"#;
        let symbols = extract_symbols_internal(code, "swift");
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.kind == "class" && s.name.contains("EngineManager")));
        assert!(symbols.iter().any(|s| s.kind == "protocol" && s.name.contains("ArchivingProtocol")));
        assert!(symbols.iter().any(|s| s.kind == "extension" && s.name.contains("EngineManager")));
    }

    #[test]
    fn test_extract_markdown_headings() {
        let md = r#"
# TTZip Architecture Guide
Introduction text here.

## Core Microkernel
Details about Rust engine.

### Stream Pipeline
Stream processing notes.

## Apple macOS App
Native desktop client.
"#;
        let headings = extract_symbols_internal(md, "md");
        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0].name, "TTZip Architecture Guide");
        assert_eq!(headings[0].kind, "h1");
        assert_eq!(headings[1].name, "Core Microkernel");
        assert_eq!(headings[1].kind, "h2");
        assert_eq!(headings[2].name, "Stream Pipeline");
        assert_eq!(headings[2].kind, "h3");
        assert_eq!(headings[3].name, "Apple macOS App");
        assert_eq!(headings[3].kind, "h2");
    }

    #[test]
    fn test_extract_python_symbols() {
        let code = r#"
class DataProcessor:
    def process(self, data):
        return data

def standalone_task():
    pass
"#;
        let symbols = extract_symbols_internal(code, "py");
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.kind == "class" && s.name.contains("DataProcessor")));
    }
}
