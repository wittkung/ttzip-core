// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Authorized scope path filtering, content tier classification, and UTF-8 text/code chunking
//! (`1,800` characters, `200`-character overlap, 1-indexed line numbers, and code definition strength).

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEFAULT_CHUNK_CHARS: usize = 1_800;
pub const DEFAULT_CHUNK_OVERLAP_CHARS: usize = 200;
pub const MAX_TEXT_FILE_BYTES: u64 = 1024 * 1024; // 1 MiB max per source/text file
pub const MAX_ARCHIVE_ENTRY_TEXT_BYTES: usize = 64 * 1024; // 64 KiB stream cap per archive entry

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentSupportTier {
    /// Excluded path (dotfile, system/credential directory, build artifact). Never indexed.
    Excluded,
    /// Matched by filename/path only (e.g., PDF, Office, image, audio, video, binary).
    Tier1PathOnly,
    /// Plain UTF-8 text file whose body is chunked and embedded.
    Tier2PlainText,
    /// Source code file whose body is chunked with line numbers and definition strength.
    Tier2SourceCode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSegment {
    pub chunk_index: u32,
    pub text: String,
    pub line_start: u32,
    pub line_end: u32,
    pub definition_strength: u8,
}

const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    ".ds_store",
    ".trash",
    ".cache",
    ".cargo",
    ".npm",
    ".venv",
    ".ssh",
    ".aws",
    ".gnupg",
    "node_modules",
    "target",
    "build",
    "dist",
    "deriveddata",
    ".build",
    "vendor",
    "pods",
    "__pycache__",
    "library",
    "system",
];

const EXCLUDED_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "keystore", "der", "crt"];

const CODE_EXTENSIONS: &[&str] = &[
    "rs", "swift", "py", "js", "ts", "tsx", "jsx", "c", "h", "cpp", "hpp", "cc", "go", "java",
    "kt", "sh", "bash", "zsh", "sql", "rb", "php", "cs", "zig", "m", "mm",
];

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "rst", "json", "yaml", "yml", "toml", "csv", "tsv", "log", "xml", "html", "htm",
    "css", "scss", "ini", "conf", "cfg",
];

/// Returns `true` if a relative path inside a scope must be excluded from indexing and search.
pub fn is_excluded_relative_path(rel_path: &str) -> bool {
    let trimmed = rel_path.trim_matches('/');
    if trimmed.is_empty() {
        return true;
    }
    for comp in trimmed.split(['/', '\\']) {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
            return true;
        }
        let lower = comp.to_ascii_lowercase();
        if lower.starts_with('.') {
            return true;
        }
        if lower.starts_with("bazel-") || EXCLUDED_DIRS.contains(&lower.as_str()) {
            return true;
        }
    }
    let file_name = trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    if file_name.starts_with(".env") || file_name == "id_rsa" || file_name == "id_ed25519" {
        return true;
    }
    if let Some(ext) = Path::new(&file_name).extension().and_then(|e| e.to_str()) {
        if EXCLUDED_EXTENSIONS.contains(&ext) {
            return true;
        }
    }
    false
}

/// Classifies a relative file path into its supported semantic indexing tier.
/// PDF, Office, image, audio, and video files are classified as `Tier1PathOnly` in v1.
pub fn classify_content_tier(rel_path: &str, file_size: u64, is_encrypted: bool) -> ContentSupportTier {
    if is_encrypted || is_excluded_relative_path(rel_path) {
        return ContentSupportTier::Excluded;
    }
    let ext = Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if file_size > MAX_TEXT_FILE_BYTES || file_size == 0 {
        return ContentSupportTier::Tier1PathOnly;
    }
    if CODE_EXTENSIONS.contains(&ext.as_str()) {
        ContentSupportTier::Tier2SourceCode
    } else if TEXT_EXTENSIONS.contains(&ext.as_str()) {
        ContentSupportTier::Tier2PlainText
    } else {
        ContentSupportTier::Tier1PathOnly
    }
}

/// Validates UTF-8 text and rejects binary payloads or private key blocks.
pub fn extract_safe_utf8_text(bytes: &[u8]) -> Option<String> {
    let sample_len = bytes.len().min(MAX_ARCHIVE_ENTRY_TEXT_BYTES);
    let slice = &bytes[..sample_len];
    if slice.contains(&0) {
        return None;
    }
    let text = std::str::from_utf8(slice).ok()?;
    if text.contains("-----BEGIN ") && text.contains("PRIVATE KEY-----") {
        return None;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(text.to_string())
}

/// Splits UTF-8 text or source code into overlapping chunks (~`max_chars` with `overlap_chars`),
/// preserving whole lines where possible and recording 1-indexed `(line_start, line_end)`.
pub fn chunk_text_content(
    text: &str,
    is_code: bool,
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<ChunkSegment> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let max_c = max_chars.max(128);
    let overlap_c = overlap_chars.min(max_c / 2);
    let mut segments = Vec::new();
    let mut start_line_idx = 0usize;

    while start_line_idx < lines.len() {
        let mut char_count = 0usize;
        let mut end_line_idx = start_line_idx;
        let mut buf = String::new();

        while end_line_idx < lines.len() {
            let line = lines[end_line_idx];
            let line_chars = line.chars().count() + 1;
            if char_count + line_chars > max_c && end_line_idx > start_line_idx {
                break;
            }
            if !buf.is_empty() {
                buf.push('\n');
            }
            if line_chars > max_c && end_line_idx == start_line_idx {
                buf.extend(line.chars().take(max_c));
                end_line_idx += 1;
                break;
            }
            buf.push_str(line);
            char_count += line_chars;
            end_line_idx += 1;
        }

        let trimmed = buf.trim();
        if !trimmed.is_empty() {
            let def_strength = if is_code {
                detect_code_definition_strength(trimmed)
            } else {
                0
            };
            segments.push(ChunkSegment {
                chunk_index: segments.len() as u32,
                text: trimmed.to_string(),
                line_start: (start_line_idx + 1) as u32,
                line_end: end_line_idx as u32,
                definition_strength: def_strength,
            });
        }

        if end_line_idx >= lines.len() {
            break;
        }

        let mut back_chars = 0usize;
        let mut next_start = end_line_idx;
        while next_start > start_line_idx + 1 {
            let prev_len = lines[next_start - 1].chars().count() + 1;
            if back_chars + prev_len > overlap_c {
                break;
            }
            back_chars += prev_len;
            next_start -= 1;
        }
        start_line_idx = next_start.max(start_line_idx + 1);
    }

    segments
}

fn detect_code_definition_strength(chunk: &str) -> u8 {
    for raw_line in chunk.lines() {
        let line = raw_line.trim_start();
        if line.starts_with("pub fn ")
            || line.starts_with("pub async fn ")
            || line.starts_with("pub unsafe fn ")
            || line.starts_with("fn ")
            || line.starts_with("func ")
            || line.starts_with("public func ")
            || line.starts_with("def ")
            || line.starts_with("async def ")
            || line.starts_with("class ")
            || line.starts_with("struct ")
            || line.starts_with("pub struct ")
            || line.starts_with("enum ")
            || line.starts_with("impl ")
        {
            return 2;
        }
    }
    0
}
