// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed definitions, format detection, sanitization policies,
//! transformation metrics, and error representations for the TTZip HTML streaming engine.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Format classification of an HTML or XHTML byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HtmlFormat {
    /// Standard modern HTML5 document.
    #[default]
    Html5,
    /// XML-compliant XHTML document (e.g. EPUB 2/3 OPS documents).
    XHtml,
    /// Partial HTML fragment without top-level `<html>` or `<body>` wrappers.
    Fragment,
    /// Non-HTML or unidentifiable byte payload.
    Unknown,
}

impl HtmlFormat {
    /// Detects the HTML format variant from the leading payload slice.
    #[must_use]
    pub fn detect(content: &[u8]) -> Self {
        if content.is_empty() {
            return Self::Unknown;
        }

        // Search leading 2048 bytes for signature markers.
        let scan_len = content.len().min(2048);
        let sample = &content[..scan_len];
        let text = String::from_utf8_lossy(sample).to_ascii_lowercase();

        if text.contains("<!doctype html") {
            Self::Html5
        } else if text.contains("xmlns=\"http://www.w3.org/1999/xhtml\"")
            || text.contains("<!doctype html public \"-//w3c//dtd xhtml")
            || (text.contains("<?xml") && text.contains("<html"))
        {
            Self::XHtml
        } else if text.contains("<html") || text.contains("<body") || text.contains("<head") {
            Self::Html5
        } else if text.contains("<div")
            || text.contains("<p")
            || text.contains("<span")
            || text.contains("<table")
            || text.contains("<h1")
            || text.contains("<section")
            || text.contains("<article")
            || text.contains("<img")
            || text.contains("<a ")
        {
            Self::Fragment
        } else {
            Self::Unknown
        }
    }

    /// Returns a human-readable display label for the format.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Html5 => "HTML5",
            Self::XHtml => "XHTML",
            Self::Fragment => "HTML Fragment",
            Self::Unknown => "Unknown HTML",
        }
    }

    /// Returns whether this format represents a complete document structure.
    #[must_use]
    pub const fn is_document(&self) -> bool {
        matches!(self, Self::Html5 | Self::XHtml)
    }
}

impl fmt::Display for HtmlFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Security sanitization policy applied during HTML streaming transformations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HtmlSanitizationPolicy {
    /// Strict isolation: strips `<script>`, `<iframe>`, `<object>`, `<embed>`,
    /// inline event handlers (`onclick`, etc.), inline `style` tags/attributes, and dangerous URI schemes.
    Strict,
    /// Strips active executable content (`<script>`, `<iframe>`, event handlers),
    /// but allows safe inline CSS `style` attributes and `<style>` blocks.
    #[default]
    AllowInlineStyles,
    /// Permissive mode: preserves all elements and attributes, only performing resource URL rewriting.
    Permissive,
}

impl HtmlSanitizationPolicy {
    /// Returns whether JavaScript `<script>` blocks and attributes are permitted.
    #[must_use]
    pub const fn allows_scripts(&self) -> bool {
        matches!(self, Self::Permissive)
    }

    /// Returns whether inline `style` attributes and `<style>` blocks are permitted.
    #[must_use]
    pub const fn allows_inline_styles(&self) -> bool {
        matches!(self, Self::AllowInlineStyles | Self::Permissive)
    }

    /// Returns whether embedded frames (`<iframe>`, `<object>`, `<embed>`) are permitted.
    #[must_use]
    pub const fn allows_iframes(&self) -> bool {
        matches!(self, Self::Permissive)
    }

    /// Checks if an attribute name is an inline JavaScript event handler (e.g. `onclick`, `onload`).
    #[must_use]
    pub fn is_event_attribute(attr_name: &str) -> bool {
        let lower = attr_name.to_ascii_lowercase();
        lower.starts_with("on") && lower.len() > 2 && lower.chars().nth(2).is_some_and(|c| c.is_ascii_alphabetic())
    }

    /// Checks if a URL scheme is dangerous and should be stripped (e.g. `javascript:`, `vbscript:`, `data:text/html`).
    #[must_use]
    pub fn is_dangerous_url_scheme(url: &str) -> bool {
        let trimmed = url.trim().to_ascii_lowercase();
        trimmed.starts_with("javascript:")
            || trimmed.starts_with("vbscript:")
            || trimmed.starts_with("data:text/html")
            || trimmed.starts_with("data:text/javascript")
            || trimmed.starts_with("data:application/javascript")
    }
}

/// Metadata record for a single rewritten resource link within an HTML document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HtmlResourceLink {
    /// Original URL verbatim as extracted from HTML attribute.
    pub original_url: String,
    /// Rewritten target URL (e.g. pointing to `ttzip-vfs://`).
    pub rewritten_url: String,
    /// HTML tag name containing the resource reference (e.g. `img`, `link`, `script`).
    pub tag_name: String,
    /// Attribute name containing the resource reference (e.g. `src`, `href`, `poster`).
    pub attribute_name: String,
}

impl HtmlResourceLink {
    /// Creates a new resource link mapping.
    #[must_use]
    pub fn new(
        original_url: impl Into<String>,
        rewritten_url: impl Into<String>,
        tag_name: impl Into<String>,
        attribute_name: impl Into<String>,
    ) -> Self {
        Self {
            original_url: original_url.into(),
            rewritten_url: rewritten_url.into(),
            tag_name: tag_name.into(),
            attribute_name: attribute_name.into(),
        }
    }
}

/// Cumulative performance and transformation metrics for an HTML streaming rewrite session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlTransformStats {
    /// Total bytes ingested by the streaming rewriter.
    pub bytes_in: usize,
    /// Total bytes produced and emitted to output sink.
    pub bytes_out: usize,
    /// Total number of HTML tags rewritten or modified.
    pub tags_rewritten: usize,
    /// Total number of relative resource URLs rewritten to VFS scheme.
    pub resources_routed: usize,
    /// Total number of script tags or inline event attributes stripped for security.
    pub scripts_stripped: usize,
    /// Total number of iframe/embed elements stripped.
    pub iframes_stripped: usize,
}

impl HtmlTransformStats {
    /// Resets all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Merges metrics from another stats collector into this instance.
    pub fn merge(&mut self, other: &Self) {
        self.bytes_in += other.bytes_in;
        self.bytes_out += other.bytes_out;
        self.tags_rewritten += other.tags_rewritten;
        self.resources_routed += other.resources_routed;
        self.scripts_stripped += other.scripts_stripped;
        self.iframes_stripped += other.iframes_stripped;
    }
}

/// Domain errors encountered during HTML streaming parsing, rewriting, or VFS routing.
#[derive(Debug, Error)]
pub enum HtmlError {
    /// Low-level HTML stream rewriting or tokenization error.
    #[error("HTML stream rewrite failure: {0}")]
    RewriteError(String),

    /// Syntactic or grammatical failure in CSS selector compilation.
    #[error("CSS selector parse failure: {0}")]
    SelectorParseError(String),

    /// Failure resolving or canonicalizing relative VFS resource route.
    #[error("VFS path routing error: {0}")]
    VfsRoutingError(String),

    /// Standard I/O error encountered while writing chunks to output sink.
    #[error("I/O error during HTML processing: {0}")]
    IoError(#[from] std::io::Error),

    /// Character set encoding or UTF-8 transcode failure.
    #[error("Encoding transcode error: {0}")]
    EncodingError(String),

    /// Security policy violation or illegal unsafe payload.
    #[error("HTML security sanitization violation: {0}")]
    SanitizationError(String),
}

/// Convenience result alias for HTML operations.
pub type HtmlResult<T> = Result<T, HtmlError>;
