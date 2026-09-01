// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Virtual File System (VFS) resource router and RFC 3986 path normalization.
//!
//! Maps relative resource links (`<img src="...">`, `<link href="...">`, `<script src="...">`,
//! `<video poster="...">`, `<source srcset="...">`) located in archive HTML documents to
//! canonical `ttzip-vfs://<archive_id>/<canonical_path>` virtual URIs while strictly preventing
//! root traversal (Zip-Slip) attacks.

use serde::{Deserialize, Serialize};

/// Default URI scheme used for TTZip virtual archive resources.
pub const DEFAULT_VFS_SCHEME: &str = "ttzip-vfs";

/// Virtual resource router that transforms relative archive resource links into VFS URLs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlVfsResourceRouter {
    /// Unique identifier of the enclosing archive.
    archive_id: String,
    /// Parent directory of the current HTML document inside the archive.
    base_dir: String,
    /// Virtual URI scheme prefix (defaults to `ttzip-vfs`).
    scheme: String,
}

impl HtmlVfsResourceRouter {
    /// Creates a new VFS resource router for an HTML file at the given in-archive path.
    #[must_use]
    pub fn new(archive_id: impl Into<String>, html_path: &str) -> Self {
        Self::with_scheme(archive_id, html_path, DEFAULT_VFS_SCHEME)
    }

    /// Creates a new VFS resource router with a custom virtual URI scheme.
    #[must_use]
    pub fn with_scheme(
        archive_id: impl Into<String>,
        html_path: &str,
        scheme: impl Into<String>,
    ) -> Self {
        let clean_archive_id = sanitize_archive_id(archive_id.into().as_str());
        let parent_dir = extract_parent_directory(html_path);
        Self {
            archive_id: clean_archive_id,
            base_dir: parent_dir,
            scheme: scheme.into(),
        }
    }

    /// Returns the archive identifier.
    #[must_use]
    pub fn archive_id(&self) -> &str {
        &self.archive_id
    }

    /// Returns the normalized base directory inside the archive.
    #[must_use]
    pub fn base_dir(&self) -> &str {
        &self.base_dir
    }

    /// Returns the active VFS URI scheme.
    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// Rewrites a single URL to a canonical `ttzip-vfs://` URI if it is a relative archive path.
    ///
    /// Returns `Some(rewritten_url)` if the URL was a relative path, or `None` if it was
    /// an external/absolute/special URL (`https://`, `data:`, `#anchor`, etc.).
    #[must_use]
    pub fn route_url(&self, raw_url: &str) -> Option<String> {
        let trimmed = raw_url.trim();
        if is_external_or_special_url(trimmed) {
            return None;
        }

        // Deconstruct URL into path, query, and fragment components
        let (path_part, query_fragment) = split_path_query_fragment(trimmed);
        if path_part.is_empty() {
            return None;
        }

        let canonical_path = normalize_rfc3986_path(&self.base_dir, path_part);
        if canonical_path.is_empty() {
            return None;
        }

        let mut vfs_url = format!("{}://{}/{}", self.scheme, self.archive_id, canonical_path);
        if let Some(suffix) = query_fragment {
            vfs_url.push_str(suffix);
        }

        Some(vfs_url)
    }

    /// Rewrites responsive `srcset` attribute values containing multiple comma-separated image descriptors.
    ///
    /// Example: `img-1x.png 1x, img-2x.png 2x` -> `ttzip-vfs://arc/img-1x.png 1x, ttzip-vfs://arc/img-2x.png 2x`
    #[must_use]
    pub fn route_srcset(&self, srcset: &str) -> String {
        let candidates = split_srcset(srcset);
        let mut rewritten_parts = Vec::with_capacity(candidates.len());

        for (url, descriptor) in candidates {
            let routed_url = self.route_url(url).unwrap_or_else(|| url.to_string());
            if let Some(desc) = descriptor {
                rewritten_parts.push(format!("{} {}", routed_url, desc));
            } else {
                rewritten_parts.push(routed_url);
            }
        }

        rewritten_parts.join(", ")
    }

    /// Evaluates an HTML tag and attribute pair, rewriting the value if it represents a routable resource.
    #[must_use]
    pub fn route_attribute(&self, tag: &str, attr: &str, value: &str) -> Option<String> {
        let tag_lower = tag.to_ascii_lowercase();
        let attr_lower = attr.to_ascii_lowercase();

        if !is_routable_resource_tag_attr(&tag_lower, &attr_lower) {
            return None;
        }

        if attr_lower == "srcset" {
            Some(self.route_srcset(value))
        } else {
            self.route_url(value)
        }
    }
}

/// Normalizes an archive-relative path against a base directory using RFC 3986 reference resolution.
///
/// Strictly eliminates `.` and `..` segments and prevents escaping above the archive root.
#[must_use]
pub fn normalize_rfc3986_path(base_dir: &str, relative_path: &str) -> String {
    let clean_rel = relative_path.replace('\\', "/");
    let clean_base = base_dir.replace('\\', "/");

    let is_root_anchored = clean_rel.starts_with('/');
    let mut segments = Vec::new();

    // If not root-anchored, seed segments with base directory
    if !is_root_anchored && !clean_base.is_empty() {
        for seg in clean_base.split('/') {
            let s = seg.trim();
            if !s.is_empty() && s != "." {
                segments.push(s);
            }
        }
    }

    // Process relative path segments
    for seg in clean_rel.split('/') {
        let s = seg.trim();
        if s.is_empty() || s == "." {
            continue;
        } else if s == ".." {
            // Pop parent directory if available; clamp to root if already at root
            segments.pop();
        } else {
            segments.push(s);
        }
    }

    segments.join("/")
}

/// Extracts the parent directory string from an in-archive file path.
#[must_use]
pub fn extract_parent_directory(path: &str) -> String {
    let clean = path.replace('\\', "/");
    let trimmed = clean.trim().trim_start_matches('/');

    if let Some(pos) = trimmed.rfind('/') {
        trimmed[..pos].to_string()
    } else {
        String::new()
    }
}

/// Checks whether a given URL is external, absolute, or a special browser scheme.
#[must_use]
pub fn is_external_or_special_url(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return true;
    }

    // Protocol-relative URL (`//cdn.example.com/asset.js`)
    if trimmed.starts_with("//") {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();

    // Check for common absolute URI schemes
    let known_schemes = [
        "http://",
        "https://",
        "ftp://",
        "ftps://",
        "data:",
        "blob:",
        "about:",
        "javascript:",
        "vbscript:",
        "mailto:",
        "tel:",
        "file:",
        "ws://",
        "wss://",
        "ttzip-vfs://",
    ];

    for scheme in known_schemes {
        if lower.starts_with(scheme) {
            return true;
        }
    }

    // Check generic URI scheme pattern (e.g. `custom-scheme:...`)
    if let Some(colon_pos) = trimmed.find(':') {
        let scheme_candidate = &trimmed[..colon_pos];
        if !scheme_candidate.is_empty()
            && scheme_candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        {
            return true;
        }
    }

    false
}

/// Checks if an HTML tag name and attribute name pair represents a routable media/resource link.
#[must_use]
pub fn is_routable_resource_tag_attr(tag: &str, attr: &str) -> bool {
    let t = tag.to_ascii_lowercase();
    let a = attr.to_ascii_lowercase();

    match t.as_str() {
        "img" => a == "src" || a == "srcset",
        "link" => a == "href",
        "script" => a == "src",
        "source" => a == "src" || a == "srcset",
        "video" => a == "src" || a == "poster",
        "audio" => a == "src",
        "track" => a == "src",
        "object" => a == "data",
        "embed" => a == "src",
        "iframe" => a == "src",
        "input" => a == "src",
        "image" | "use" => a == "href" || a == "xlink:href",
        _ => false,
    }
}

/// Splits a URL into (path, query_and_fragment_suffix).
fn split_path_query_fragment(url: &str) -> (&str, Option<&str>) {
    let query_pos = url.find('?');
    let fragment_pos = url.find('#');

    let split_pos = match (query_pos, fragment_pos) {
        (Some(q), Some(f)) => Some(q.min(f)),
        (Some(q), None) => Some(q),
        (None, Some(f)) => Some(f),
        (None, None) => None,
    };

    if let Some(pos) = split_pos {
        (&url[..pos], Some(&url[pos..]))
    } else {
        (url, None)
    }
}

/// Parses a `srcset` attribute into `(url, optional_descriptor)` pairs.
fn split_srcset(srcset: &str) -> Vec<(&str, Option<&str>)> {
    let mut results = Vec::new();
    let trimmed = srcset.trim();
    if trimmed.is_empty() {
        return results;
    }

    for item in trimmed.split(',') {
        let entry = item.trim();
        if entry.is_empty() {
            continue;
        }

        let mut parts = entry.split_whitespace();
        if let Some(url) = parts.next() {
            let descriptor = parts.next();
            results.push((url, descriptor));
        }
    }

    results
}

/// Sanitizes archive IDs to ensure safe URL formatting.
fn sanitize_archive_id(id: &str) -> String {
    id.trim().trim_matches('/').replace(['\\', ' ', ':'], "-")
}
