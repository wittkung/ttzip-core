// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Heuristic and Extension-based Language Detection Engine for Syntax Analysis.

use std::path::Path;

use super::types::UniFFILanguageInfo;

/// Detects language from filename, extension, or content snippet hint (shebang / header).
pub fn detect_language_internal(file_path_or_ext: &str, first_line_hint: Option<&str>) -> UniFFILanguageInfo {
    let clean_input = file_path_or_ext.trim();

    // 1. Check exact special filename matches
    if let Some(filename) = Path::new(clean_input).file_name().and_then(|n| n.to_str()) {
        let lower = filename.to_ascii_lowercase();
        if let Some(info) = match_special_filename(&lower) {
            return info;
        }
    }

    // 2. Check file extension
    let trimmed_ext = clean_input.trim_start_matches('.');
    let ext = if let Some(e) = Path::new(clean_input).extension().and_then(|e| e.to_str()) {
        e.to_ascii_lowercase()
    } else if !trimmed_ext.contains('/') && !trimmed_ext.contains('\\') {
        trimmed_ext.to_ascii_lowercase()
    } else {
        String::new()
    };

    if !ext.is_empty() {
        if let Some(info) = match_extension(&ext) {
            return info;
        }
    }

    // 3. Check first line / shebang hint if provided
    if let Some(first_line) = first_line_hint {
        let line = first_line.trim();
        if let Some(info) = match_content_hint(line) {
            return info;
        }
    }

    // 4. Default fallback to unknown / plaintext
    UniFFILanguageInfo {
        language_id: "plaintext".to_string(),
        display_name: "Plain Text".to_string(),
        file_extensions: if ext.is_empty() { vec!["txt".to_string()] } else { vec![ext] },
        mime_types: vec!["text/plain".to_string()],
        is_supported: false,
    }
}

/// Returns the complete registry of recognized languages.
pub fn list_supported_languages() -> Vec<UniFFILanguageInfo> {
    vec![
        make_lang("rust", "Rust", &["rs"], &["text/rust", "text/x-rust"], true),
        make_lang("swift", "Swift", &["swift"], &["text/x-swift"], true),
        make_lang("python", "Python", &["py", "pyw", "pyi"], &["text/x-python", "application/x-python-code"], true),
        make_lang("c", "C", &["c", "h"], &["text/x-c", "text/x-chdr"], true),
        make_lang("cpp", "C++", &["cpp", "cc", "cxx", "hpp", "hxx", "hh"], &["text/x-c++src", "text/x-c++hdr"], true),
        make_lang("javascript", "JavaScript", &["js", "mjs", "cjs", "jsx"], &["application/javascript", "text/javascript"], true),
        make_lang("typescript", "TypeScript", &["ts", "mts", "cts", "tsx"], &["application/typescript", "text/typescript"], true),
        make_lang("json", "JSON", &["json", "jsonc", "json5"], &["application/json"], true),
        make_lang("markdown", "Markdown", &["md", "markdown", "mdown", "mkdn"], &["text/markdown", "text/x-markdown"], true),
        make_lang("html", "HTML", &["html", "htm", "xhtml"], &["text/html"], true),
        make_lang("css", "CSS", &["css", "scss", "sass", "less"], &["text/css"], true),
        make_lang("toml", "TOML", &["toml"], &["application/toml", "text/x-toml"], false),
        make_lang("yaml", "YAML", &["yaml", "yml"], &["application/x-yaml", "text/yaml"], false),
        make_lang("xml", "XML", &["xml", "plist", "svg", "xsd", "xsl", "rss", "atom"], &["application/xml", "text/xml"], false),
        make_lang("shell", "Shell Script", &["sh", "bash", "zsh", "ksh", "fish"], &["application/x-sh", "text/x-shellscript"], false),
        make_lang("sql", "SQL", &["sql"], &["application/sql", "text/x-sql"], false),
        make_lang("go", "Go", &["go"], &["text/x-go"], false),
        make_lang("java", "Java", &["java"], &["text/x-java-source"], false),
        make_lang("kotlin", "Kotlin", &["kt", "kts"], &["text/x-kotlin"], false),
        make_lang("ruby", "Ruby", &["rb", "rake", "gemspec"], &["application/x-ruby", "text/x-ruby"], false),
        make_lang("php", "PHP", &["php", "phtml", "php3", "php4", "php5"], &["application/x-httpd-php", "text/x-php"], false),
        make_lang("zig", "Zig", &["zig"], &["text/x-zig"], false),
        make_lang("lua", "Lua", &["lua"], &["text/x-lua"], false),
    ]
}

#[inline]
fn make_lang(id: &str, name: &str, exts: &[&str], mimes: &[&str], supported: bool) -> UniFFILanguageInfo {
    UniFFILanguageInfo {
        language_id: id.to_string(),
        display_name: name.to_string(),
        file_extensions: exts.iter().map(|s| s.to_string()).collect(),
        mime_types: mimes.iter().map(|s| s.to_string()).collect(),
        is_supported: supported,
    }
}

fn match_special_filename(lower: &str) -> Option<UniFFILanguageInfo> {
    match lower {
        "cargo.toml" | "cargo.lock" => Some(make_lang("toml", "TOML", &["toml"], &["application/toml"], false)),
        "package.swift" => Some(make_lang("swift", "Swift", &["swift"], &["text/x-swift"], true)),
        "makefile" | "gnumakefile" => Some(make_lang("makefile", "Makefile", &["mk", "mak"], &["text/x-makefile"], false)),
        "dockerfile" | "containerfile" => Some(make_lang("dockerfile", "Dockerfile", &["dockerfile"], &["text/x-dockerfile"], false)),
        "package.json" | "tsconfig.json" | "jsconfig.json" => Some(make_lang("json", "JSON", &["json"], &["application/json"], true)),
        ".gitignore" | ".gitattributes" | ".editorconfig" => Some(make_lang("config", "Configuration", &["conf", "cfg", "ini"], &["text/plain"], false)),
        ".zshrc" | ".bashrc" | ".bash_profile" | ".profile" => Some(make_lang("shell", "Shell Script", &["sh", "bash", "zsh"], &["text/x-shellscript"], false)),
        _ => None,
    }
}

fn match_extension(ext: &str) -> Option<UniFFILanguageInfo> {
    match ext {
        "rs" => Some(make_lang("rust", "Rust", &["rs"], &["text/rust", "text/x-rust"], true)),
        "swift" => Some(make_lang("swift", "Swift", &["swift"], &["text/x-swift"], true)),
        "py" | "pyw" | "pyi" => Some(make_lang("python", "Python", &["py", "pyw", "pyi"], &["text/x-python"], true)),
        "c" | "h" => Some(make_lang("c", "C", &["c", "h"], &["text/x-c"], true)),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some(make_lang("cpp", "C++", &["cpp", "cc", "cxx", "hpp"], &["text/x-c++src"], true)),
        "js" | "mjs" | "cjs" | "jsx" => Some(make_lang("javascript", "JavaScript", &["js", "mjs", "cjs", "jsx"], &["application/javascript"], true)),
        "ts" | "mts" | "cts" | "tsx" => Some(make_lang("typescript", "TypeScript", &["ts", "mts", "cts", "tsx"], &["application/typescript"], true)),
        "json" | "jsonc" | "json5" => Some(make_lang("json", "JSON", &["json", "jsonc"], &["application/json"], true)),
        "md" | "markdown" | "mdown" | "mkdn" => Some(make_lang("markdown", "Markdown", &["md", "markdown"], &["text/markdown"], true)),
        "html" | "htm" | "xhtml" => Some(make_lang("html", "HTML", &["html", "htm"], &["text/html"], true)),
        "css" | "scss" | "sass" | "less" => Some(make_lang("css", "CSS", &["css", "scss"], &["text/css"], true)),
        "toml" => Some(make_lang("toml", "TOML", &["toml"], &["application/toml"], false)),
        "yaml" | "yml" => Some(make_lang("yaml", "YAML", &["yaml", "yml"], &["application/x-yaml"], false)),
        "xml" | "plist" | "svg" | "xsd" | "xsl" => Some(make_lang("xml", "XML", &["xml", "plist"], &["application/xml"], false)),
        "sh" | "bash" | "zsh" | "fish" => Some(make_lang("shell", "Shell Script", &["sh", "bash", "zsh"], &["text/x-shellscript"], false)),
        "sql" => Some(make_lang("sql", "SQL", &["sql"], &["application/sql"], false)),
        "go" => Some(make_lang("go", "Go", &["go"], &["text/x-go"], false)),
        "java" => Some(make_lang("java", "Java", &["java"], &["text/x-java-source"], false)),
        "kt" | "kts" => Some(make_lang("kotlin", "Kotlin", &["kt", "kts"], &["text/x-kotlin"], false)),
        "rb" | "rake" => Some(make_lang("ruby", "Ruby", &["rb"], &["application/x-ruby"], false)),
        "php" | "phtml" => Some(make_lang("php", "PHP", &["php"], &["application/x-httpd-php"], false)),
        "zig" => Some(make_lang("zig", "Zig", &["zig"], &["text/x-zig"], false)),
        "lua" => Some(make_lang("lua", "Lua", &["lua"], &["text/x-lua"], false)),
        _ => None,
    }
}

fn match_content_hint(line: &str) -> Option<UniFFILanguageInfo> {
    if line.starts_with("#!") {
        if line.contains("python") {
            return Some(make_lang("python", "Python", &["py"], &["text/x-python"], true));
        }
        if line.contains("bash") || line.contains("sh") || line.contains("zsh") {
            return Some(make_lang("shell", "Shell Script", &["sh"], &["text/x-shellscript"], false));
        }
        if line.contains("node") || line.contains("deno") || line.contains("bun") {
            return Some(make_lang("javascript", "JavaScript", &["js"], &["application/javascript"], true));
        }
        if line.contains("ruby") {
            return Some(make_lang("ruby", "Ruby", &["rb"], &["application/x-ruby"], false));
        }
        if line.contains("perl") {
            return Some(make_lang("perl", "Perl", &["pl"], &["text/x-perl"], false));
        }
        if line.contains("php") {
            return Some(make_lang("php", "PHP", &["php"], &["application/x-httpd-php"], false));
        }
    }

    if line.starts_with("<?xml") || line.starts_with("<!DOCTYPE plist") {
        return Some(make_lang("xml", "XML", &["xml", "plist"], &["application/xml"], false));
    }
    if line.to_ascii_lowercase().starts_with("<!doctype html") || line.to_ascii_lowercase().starts_with("<html") {
        return Some(make_lang("html", "HTML", &["html"], &["text/html"], true));
    }
    if line.starts_with("<?php") {
        return Some(make_lang("php", "PHP", &["php"], &["application/x-httpd-php"], false));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_by_extension() {
        let rs = detect_language_internal("src/main.rs", None);
        assert_eq!(rs.language_id, "rust");
        assert!(rs.is_supported);

        let swift = detect_language_internal("Sources/App.swift", None);
        assert_eq!(swift.language_id, "swift");
        assert!(swift.is_supported);

        let py = detect_language_internal(".py", None);
        assert_eq!(py.language_id, "python");
    }

    #[test]
    fn test_detect_by_special_filename() {
        let cargo = detect_language_internal("/workspace/Cargo.toml", None);
        assert_eq!(cargo.language_id, "toml");

        let pkg = detect_language_internal("Package.swift", None);
        assert_eq!(pkg.language_id, "swift");

        let docker = detect_language_internal("Dockerfile", None);
        assert_eq!(docker.language_id, "dockerfile");
    }

    #[test]
    fn test_detect_by_shebang_and_header() {
        let py_script = detect_language_internal("script", Some("#!/usr/bin/env python3"));
        assert_eq!(py_script.language_id, "python");

        let sh_script = detect_language_internal("build", Some("#!/bin/bash -e"));
        assert_eq!(sh_script.language_id, "shell");

        let html_doc = detect_language_internal("document", Some("<!DOCTYPE html><html>"));
        assert_eq!(html_doc.language_id, "html");
    }
}
