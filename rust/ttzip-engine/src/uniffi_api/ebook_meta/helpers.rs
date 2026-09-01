// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Shared Helper Functions for Ebook Parsing, Path Resolution, MIME Detection, and Markup Cleaning.

use std::fs::File;
use std::path::Path;

use super::types::{UniFFIEbookError, UniFFIEbookResource};
use crate::zip::reader::ZipArchive;

/// Reads full bytes of a local file via memory mapping.
pub(crate) fn read_file_bytes(path_str: &str) -> Result<Vec<u8>, UniFFIEbookError> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(UniFFIEbookError::not_found(path_str));
    }
    let file = File::open(path).map_err(UniFFIEbookError::io_err)?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(UniFFIEbookError::io_err)?;
    Ok(mmap.to_vec())
}

/// Finds and extracts raw decompressed entry bytes from an in-memory ZIP archive.
pub(crate) fn find_and_extract_entry(zip: &ZipArchive<'_>, target: &str) -> Option<Vec<u8>> {
    let norm = target.trim_start_matches('/').replace('\\', "/");
    for (idx, entry) in zip.entries().iter().enumerate() {
        let entry_norm = entry.rel_path.trim_start_matches('/').replace('\\', "/");
        if entry_norm == norm {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    let norm_lower = norm.to_lowercase();
    for (idx, entry) in zip.entries().iter().enumerate() {
        let entry_norm = entry
            .rel_path
            .trim_start_matches('/')
            .replace('\\', "/")
            .to_lowercase();
        if entry_norm == norm_lower {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    None
}

/// Extracts an embedded resource by href from an in-memory ZIP archive.
pub(crate) fn extract_resource_from_zip(
    data: &[u8],
    href: &str,
) -> Result<UniFFIEbookResource, UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;
    let clean = href.split('#').next().unwrap_or(href);
    let bytes = find_and_extract_entry(&zip, clean)
        .ok_or_else(|| UniFFIEbookError::not_found(clean))?;

    let media_type = guess_mime_type(clean);
    let size_bytes = bytes.len() as u64;

    Ok(UniFFIEbookResource {
        href: clean.to_string(),
        media_type,
        data: bytes,
        size_bytes,
    })
}

/// Resolves a relative path against a base directory while preserving fragment anchors.
pub(crate) fn resolve_path(base_dir: &str, rel_path: &str) -> String {
    let (path_part, anchor_part) = match rel_path.find('#') {
        Some(idx) => (&rel_path[..idx], &rel_path[idx..]),
        None => (rel_path, ""),
    };
    let clean_rel = path_part.split('?').next().unwrap_or(path_part);
    let resolved = if base_dir.is_empty() || clean_rel.starts_with('/') {
        clean_rel.trim_start_matches('/').to_string()
    } else {
        let combined = format!("{}/{}", base_dir.trim_matches('/'), clean_rel.trim_matches('/'));
        let mut segments = Vec::new();
        for seg in combined.split('/') {
            if seg == "." || seg.is_empty() {
                continue;
            } else if seg == ".." {
                segments.pop();
            } else {
                segments.push(seg);
            }
        }
        segments.join("/")
    };
    if anchor_part.is_empty() {
        resolved
    } else {
        format!("{resolved}{anchor_part}")
    }
}

/// Returns true if the relative path ends with a standard comic / graphic image extension.
pub(crate) fn is_image_path(p: &str) -> bool {
    let l = p.to_lowercase();
    l.ends_with(".jpg")
        || l.ends_with(".jpeg")
        || l.ends_with(".png")
        || l.ends_with(".webp")
        || l.ends_with(".gif")
}

/// Heuristically determines the MIME type from a file path extension.
pub(crate) fn guess_mime_type(path: &str) -> String {
    let l = path.to_lowercase();
    if l.ends_with(".jpg") || l.ends_with(".jpeg") {
        "image/jpeg".to_string()
    } else if l.ends_with(".png") {
        "image/png".to_string()
    } else if l.ends_with(".webp") {
        "image/webp".to_string()
    } else if l.ends_with(".gif") {
        "image/gif".to_string()
    } else if l.ends_with(".svg") {
        "image/svg+xml".to_string()
    } else if l.ends_with(".css") {
        "text/css".to_string()
    } else if l.ends_with(".xhtml") || l.ends_with(".html") || l.ends_with(".htm") {
        "application/xhtml+xml".to_string()
    } else if l.ends_with(".ncx") {
        "application/x-dtbncx+xml".to_string()
    } else if l.ends_with(".opf") {
        "application/oebps-package+xml".to_string()
    } else if l.ends_with(".woff2") {
        "font/woff2".to_string()
    } else if l.ends_with(".woff") {
        "font/woff".to_string()
    } else if l.ends_with(".ttf") {
        "font/ttf".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

/// Extracts primary heading or title string from raw HTML/XHTML content.
pub(crate) fn extract_html_heading(html: &str) -> Option<String> {
    for tag in ["h1", "h2", "title"] {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        if let Some(start) = html.to_lowercase().find(&open) {
            if let Some(gt) = html[start..].find('>') {
                let content_start = start + gt + 1;
                if let Some(end) = html[content_start..].to_lowercase().find(&close) {
                    let text = strip_html_tags(&html[content_start..content_start + end]);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
    }
    None
}

/// Strips markup tags and unescapes standard XML/HTML entities.
pub(crate) fn strip_html_tags(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            res.push(c);
        }
    }
    res.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
