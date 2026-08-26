// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Concurrent Directory Scanner & POSIX Path Normalization Layer.

use std::path::Path;
use super::types::{DiskItemSummary, PathSuggestionItem, TTZipError, UniFFIParentAndPrefix};

/// Scans a directory and returns lightweight item summaries.
#[uniffi::export]
pub fn scan_directory(path: String, _max_depth: u32) -> Result<Vec<DiskItemSummary>, TTZipError> {
    let root = Path::new(&path);
    if !root.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    let entries = std::fs::read_dir(root).map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mut items = Vec::new();

    for entry in entries.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if name.starts_with('.') {
            continue;
        }

        let is_dir = p.is_dir();
        let meta = p.metadata().ok();
        let size = if is_dir { 0 } else { meta.as_ref().map(|m| m.len()).unwrap_or(0) };
        let mtime = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        items.push(DiskItemSummary {
            path: p.to_string_lossy().to_string(),
            name,
            is_directory: is_dir,
            size,
            mtime_epoch_secs: mtime,
        });
    }

    items.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => crate::fs::natural_sort::natural_cmp(&a.name, &b.name),
    });

    Ok(items)
}

/// Natural string comparator exposed via UniFFI.
#[uniffi::export]
pub fn natural_compare(a: String, b: String) -> i32 {
    match crate::fs::natural_sort::natural_cmp(&a, &b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// Natural sorts a list of paths exposed via UniFFI.
#[uniffi::export]
pub fn natural_sort_paths(mut items: Vec<String>) -> Vec<String> {
    crate::fs::natural_sort::natural_sort(&mut items);
    items
}

/// Normalizes and resolves a raw user input path into a canonical POSIX path.
#[uniffi::export]
pub fn sanitize_posix_path(raw_input: String, base_directory: Option<String>) -> String {
    let trimmed = strip_wrapping_quotes(&raw_input);
    if trimmed.is_empty() {
        return String::new();
    }

    let unescaped = unescape_and_expand(trimmed);
    let resolved = if unescaped.starts_with('/') {
        unescaped
    } else {
        let base = base_directory.unwrap_or_else(get_default_home);
        format!("{}/{}", base.trim_end_matches('/'), unescaped)
    };

    let std = normalize_posix_lexical(&resolved);
    if std.is_empty() { "/".to_string() } else { std }
}

/// Extracts the parent directory to query and the trailing prefix for real-time autocompletion.
#[uniffi::export]
pub fn extract_parent_and_prefix(
    raw_input: String,
    base_directory: Option<String>,
) -> UniFFIParentAndPrefix {
    let trimmed = strip_wrapping_quotes(&raw_input);
    if trimmed.is_empty() {
        let base = base_directory.unwrap_or_else(get_default_home);
        let std_base = normalize_posix_lexical(&base);
        return UniFFIParentAndPrefix {
            parent_directory: if std_base.is_empty() { "/".to_string() } else { std_base },
            prefix: String::new(),
        };
    }

    let raw_unescaped = unescape_url_and_shell(trimmed);
    if raw_unescaped == "~" {
        return UniFFIParentAndPrefix {
            parent_directory: get_default_home(),
            prefix: String::new(),
        };
    }

    let unescaped = expand_tilde(&raw_unescaped);
    let full_path = if unescaped.starts_with('/') {
        unescaped.clone()
    } else {
        let base = base_directory.unwrap_or_else(get_default_home);
        format!("{}/{}", base.trim_end_matches('/'), unescaped)
    };

    let (parent, prefix) = if unescaped.ends_with('/') {
        (normalize_posix_lexical(&full_path), String::new())
    } else if unescaped == "." || unescaped.ends_with("/.") {
        (normalize_posix_lexical(&posix_parent(&full_path)), ".".to_string())
    } else {
        let (p, pre) = posix_parent_and_last(&full_path);
        (normalize_posix_lexical(&p), pre)
    };

    UniFFIParentAndPrefix {
        parent_directory: if parent.is_empty() { "/".to_string() } else { parent },
        prefix,
    }
}

/// Fast path autocompletion query based on directory scanning and prefix matching.
#[uniffi::export]
pub fn autocomplete_disk_path(
    raw_input: String,
    base_directory: String,
    max_results: u32,
) -> Vec<PathSuggestionItem> {
    let input = raw_input.trim();
    if input.is_empty() { return Vec::new(); }

    let parsed = extract_parent_and_prefix(raw_input, Some(base_directory));
    let search_dir = Path::new(&parsed.parent_directory);
    let prefix = parsed.prefix;
    if !search_dir.exists() || !search_dir.is_dir() { return Vec::new(); }

    let archive_exts = [
        "zip", "7z", "tar", "gz", "bz2", "xz", "zst", "rar", "dmg", "iso", "wim", "aar", "cbr", "cab", "lz4", "br"
    ];

    let mut matches = Vec::new();
    if let Ok(entries) = std::fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !prefix.starts_with('.') && name.starts_with('.') { continue; }
            if prefix.is_empty() || name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                let p = entry.path();
                let is_dir = p.is_dir();
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                matches.push(PathSuggestionItem {
                    full_path: p.to_string_lossy().to_string(),
                    display_name: name,
                    is_directory: is_dir,
                    is_archive: archive_exts.contains(&ext.as_str()),
                });
                if matches.len() >= (max_results as usize) * 3 { break; }
            }
        }
    }

    matches.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => crate::fs::natural_sort::natural_cmp(&a.display_name, &b.display_name),
    });
    matches.truncate(max_results as usize);
    matches
}

// MARK: - Internal Helper Functions

fn strip_wrapping_quotes(s: &str) -> &str {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t[1..t.len() - 1].trim()
    } else {
        t
    }
}

fn unescape_url_and_shell(input: &str) -> String {
    let url_stripped = if let Some(s) = input.strip_prefix("file://localhost") {
        percent_decode(s)
    } else if let Some(s) = input.strip_prefix("file://") {
        percent_decode(s)
    } else {
        input.to_string()
    };
    unescape_shell_backslashes(&url_stripped)
}

fn unescape_and_expand(input: &str) -> String {
    expand_tilde(&unescape_url_and_shell(input))
}

fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            if let (Some(h1), Some(h2)) = (chars.next(), chars.next()) {
                if let (Some(v1), Some(v2)) = (hex_val(h1), hex_val(h2)) {
                    bytes.push((v1 << 4) | v2);
                    continue;
                }
                bytes.extend([b'%', h1, h2]);
            } else {
                bytes.push(b'%');
            }
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

#[inline]
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn unescape_shell_backslashes(path: &str) -> String {
    if !path.contains('\\') { return path.to_string(); }
    let mut res = String::with_capacity(path.len());
    let mut esc = false;
    for c in path.chars() {
        if esc { res.push(c); esc = false; }
        else if c == '\\' { esc = true; }
        else { res.push(c); }
    }
    if esc { res.push('\\'); }
    res
}

fn get_default_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") { "/Users".into() } else { "/home".into() }
    })
}

fn expand_tilde(path: &str) -> String {
    if path == "~" {
        get_default_home()
    } else if let Some(s) = path.strip_prefix("~/") {
        format!("{}/{}", get_default_home().trim_end_matches('/'), s)
    } else if let Some(s) = path.strip_prefix('~') {
        let (user, rest) = match s.find('/') {
            Some(i) => (&s[..i], &s[i..]),
            None => (s, ""),
        };
        let pfx = if cfg!(target_os = "macos") { "/Users" } else { "/home" };
        format!("{}/{}{}", pfx, user, rest)
    } else {
        path.to_string()
    }
}

fn normalize_posix_lexical(path: &str) -> String {
    if path.is_empty() { return String::new(); }
    let is_abs = path.starts_with('/');
    let mut comps: Vec<&str> = Vec::new();
    for comp in path.split('/') {
        match comp {
            "" | "." => continue,
            ".." => {
                if let Some(last) = comps.last() {
                    if *last != ".." { comps.pop(); } else if !is_abs { comps.push(".."); }
                } else if !is_abs { comps.push(".."); }
            }
            other => comps.push(other),
        }
    }
    if is_abs {
        if comps.is_empty() { "/".into() } else { format!("/{}", comps.join("/")) }
    } else if comps.is_empty() { ".".into() } else { comps.join("/") }
}

fn posix_parent_and_last(path: &str) -> (String, String) {
    let t = path.trim_end_matches('/');
    if let Some(idx) = t.rfind('/') {
        let parent = if idx == 0 { "/".into() } else { t[..idx].into() };
        (parent, t[idx + 1..].into())
    } else {
        (".".into(), t.into())
    }
}

fn posix_parent(path: &str) -> String {
    posix_parent_and_last(path).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_posix() {
        let home = get_default_home();
        let base = Some("/Users/test/Documents".into());
        let cases: &[(&str, Option<String>, &str)] = &[
            ("~", None, &home), ("~/Downloads", None, &format!("{}/Downloads", home)),
            ("~/Desktop/../Downloads", None, &format!("{}/Downloads", home)),
            ("../folder", base.clone(), "/Users/test/folder"), ("./child", base.clone(), "/Users/test/Documents/child"),
            ("sub/archive.7z", base.clone(), "/Users/test/Documents/sub/archive.7z"),
            (r#"/Users/test/My\ Space/File\ \(1\).zip"#, None, "/Users/test/My Space/File (1).zip"),
            (r#"/var/tmp/Special\[1\]\&\$name.tar"#, None, "/var/tmp/Special[1]&$name.tar"),
            ("file:///Users/test/Downloads/abc%20def", None, "/Users/test/Downloads/abc def"),
            ("file://localhost/Users/test/archive.zip", None, "/Users/test/archive.zip"),
            ("///var///tmp///", None, "/var/tmp"), ("///", None, "/"),
            ("  \"/Users/test/Documents\"  ", None, "/Users/test/Documents"), ("   ", None, ""),
        ];
        for (raw, b, expected) in cases {
            assert_eq!(sanitize_posix_path((*raw).into(), b.clone()), *expected);
        }
    }

    #[test]
    fn test_extract_parent_and_prefix() {
        let home = get_default_home();
        let base = Some("/Users/test/Documents".into());
        let cases = [
            ("/var/lo", "/var", "lo"), ("/var/log/", "/var/log", ""), ("/", "/", ""),
            ("~/Down", home.as_str(), "Down"), ("~/Downloads/", &format!("{}/Downloads", home), ""),
            ("~", home.as_str(), ""), ("sub/fil", "/Users/test/Documents/sub", "fil"),
            (r#"/Users/test/My\ Sp"#, "/Users/test", "My Sp"),
            ("file:///Users/test/Downloads/abc", "/Users/test/Downloads", "abc"),
            ("", "/Users/test/Documents", ""),
        ];
        for (input, exp_parent, exp_pre) in cases {
            let res = extract_parent_and_prefix(input.into(), base.clone());
            assert_eq!(res.parent_directory, exp_parent, "Failed parent for {}", input);
            assert_eq!(res.prefix, exp_pre, "Failed prefix for {}", input);
        }
    }
}

