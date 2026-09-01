// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! E-book Resource Extractor and Path Normalization Engine.
//!
//! Provides targeted on-demand extraction of chapter XHTML/HTML markup, CSS stylesheets,
//! embedded images, and covers with relative path canonicalization and MIME detection.

use crate::ebook::{EbookError, EbookResult};
use crate::zip::ZipArchive;

/// Extracted e-book resource payload with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EbookResource {
    /// Normalized container-relative path.
    pub path: String,
    /// MIME content type (e.g. `application/xhtml+xml`, `image/jpeg`).
    pub media_type: String,
    /// Raw uncompressed byte payload.
    pub data: Vec<u8>,
}

impl EbookResource {
    /// Attempts to view the resource payload as a UTF-8 string slice.
    #[inline]
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.data)
    }

    /// Converts the resource payload to a String using lossy UTF-8 decoding.
    #[inline]
    pub fn as_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }

    /// Returns the length of the binary payload in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the payload is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Extractor for on-demand chapter content, stylesheets, and binary assets.
pub struct EbookResourceExtractor;

impl EbookResourceExtractor {
    /// Extracts a specific resource from the ZIP archive container by container relative path.
    pub fn extract_resource(
        zip: &ZipArchive<'_>,
        path: &str,
        media_type_override: Option<&str>,
    ) -> EbookResult<EbookResource> {
        let clean_path = strip_fragment(path);
        let normalized = clean_container_path(clean_path);

        let entry_idx = find_zip_entry_index(zip, &normalized).ok_or_else(|| {
            EbookError::NotFound(format!("Resource not found in e-book container: {path}"))
        })?;

        let data = zip.extract_entry_bytes(entry_idx, None)?;
        let media_type = media_type_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| guess_media_type(&normalized).to_string());

        Ok(EbookResource {
            path: normalized,
            media_type,
            data,
        })
    }

    /// Extracts a text-based resource (XHTML, HTML, CSS, XML) and decodes it to a UTF-8 `String`.
    pub fn extract_text(zip: &ZipArchive<'_>, path: &str) -> EbookResult<String> {
        let res = Self::extract_resource(zip, path, None)?;
        match std::str::from_utf8(&res.data) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => {
                let (cow, _, had_errors) = encoding_rs::UTF_8.decode(&res.data);
                if !had_errors {
                    Ok(cow.into_owned())
                } else {
                    // Fallback to windows-1252 / auto-detect
                    let (cow1252, _, _) = encoding_rs::WINDOWS_1252.decode(&res.data);
                    Ok(cow1252.into_owned())
                }
            }
        }
    }
}

/// Normalizes a relative href against a base container directory.
///
/// Handles `../`, `./`, redundant slashes, URL percent-encoding, and anchor fragment preservation.
pub fn normalize_path(base_dir: &str, relative_href: &str) -> String {
    let (href_path, fragment) = match relative_href.split_once('#') {
        Some((p, frag)) => (p, Some(frag)),
        None => (relative_href, None),
    };

    let decoded_href = percent_decode(href_path);
    let mut parts: Vec<&str> = Vec::new();

    let clean_base = base_dir.trim_matches(['/', '\\']);
    if !clean_base.is_empty() {
        for seg in clean_base.split(['/', '\\']) {
            if !seg.is_empty() && seg != "." {
                parts.push(seg);
            }
        }
    }

    for seg in decoded_href.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }

    let joined = parts.join("/");
    match fragment {
        Some(frag) if !frag.is_empty() => format!("{joined}#{frag}"),
        _ => joined,
    }
}

/// Strips anchor fragment (`#id`) and query parameters from a resource path.
#[inline]
pub fn strip_fragment(path: &str) -> &str {
    let path = match path.split_once('#') {
        Some((p, _)) => p,
        None => path,
    };
    match path.split_once('?') {
        Some((p, _)) => p,
        None => path,
    }
}

/// Cleans container relative path by removing leading slashes and resolving `.` and `..`.
pub fn clean_container_path(path: &str) -> String {
    let mut parts = Vec::new();
    for seg in path.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    parts.join("/")
}

/// Locates a ZIP entry index matching the given normalized path with fallback heuristics.
fn find_zip_entry_index(zip: &ZipArchive<'_>, target_path: &str) -> Option<usize> {
    let target_clean = clean_container_path(target_path);

    // 1. Exact match
    for (i, entry) in zip.entries().iter().enumerate() {
        if clean_container_path(&entry.rel_path) == target_clean {
            return Some(i);
        }
    }

    // 2. Case-insensitive match
    let lower_target = target_clean.to_lowercase();
    for (i, entry) in zip.entries().iter().enumerate() {
        if clean_container_path(&entry.rel_path).to_lowercase() == lower_target {
            return Some(i);
        }
    }

    // 3. Match suffix (e.g. if path omitted OEBPS prefix)
    for (i, entry) in zip.entries().iter().enumerate() {
        let entry_clean = clean_container_path(&entry.rel_path);
        if entry_clean.ends_with(&target_clean) {
            return Some(i);
        }
    }

    None
}

/// In-place simple percent decoder for URL-encoded paths (e.g., `%20` -> ` `).
fn percent_decode(input: &str) -> String {
    if !input.contains('%') {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h1), Some(h2)) = (from_hex_digit(bytes[i + 1]), from_hex_digit(bytes[i + 2])) {
                out.push((h1 << 4) | h2);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

#[inline]
fn from_hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Guesses MIME media type based on filename extension.
pub fn guess_media_type(path: &str) -> &'static str {
    let clean = strip_fragment(path);
    let ext = clean
        .rsplit_once('.')
        .map(|(_, e)| e)
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "xhtml" | "html" | "htm" => "application/xhtml+xml",
        "css" => "text/css",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ncx" => "application/x-dtbncx+xml",
        "opf" => "application/oebps-package+xml",
        "otf" => "font/otf",
        "ttf" => "font/ttf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "js" => "application/javascript",
        "json" => "application/json",
        "smil" => "application/smil+xml",
        "pls" => "application/pls+xml",
        "mp3" => "audio/mpeg",
        "mp4" | "m4a" => "audio/mp4",
        _ => "application/octet-stream",
    }
}
