// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records, Enums, and Errors for HTML Transformation and VFS Interception.

use thiserror::Error;

/// Supported HTML-adjacent document and container formats.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, uniffi::Enum)]
pub enum UniFFIHtmlFormat {
    #[default]
    Unknown,
    Html,
    Xhtml,
    Mhtml,
    HtmlFragment,
    Svg,
}

/// Sanitization and security policy governing HTML transformation and resource rewriting.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIHtmlSanitizationPolicy {
    /// Whether JavaScript `<script>` elements and inline event handlers (`onclick`, etc.) are permitted.
    pub allow_scripts: bool,
    /// Whether inline `style="..."` attributes and `<style>` blocks are permitted.
    pub allow_inline_styles: bool,
    /// Whether external network resources (e.g. `http://`, `https://`) are permitted.
    pub allow_external_resources: bool,
    /// Whether interactive `<form>`, `<input>`, and `<button>` elements are permitted.
    pub allow_forms: bool,
    /// Whether embedded `<iframe>`, `<frame>`, `<object>`, or `<embed>` elements are permitted.
    pub allow_iframes: bool,
    /// Optional whitelist of custom HTML tag names. If non-empty, unlisted tags are stripped.
    pub custom_allowed_tags: Vec<String>,
    /// List of explicit tag names to strip or remove from document.
    pub custom_blocked_tags: Vec<String>,
}

impl Default for UniFFIHtmlSanitizationPolicy {
    fn default() -> Self {
        Self {
            allow_scripts: false,
            allow_inline_styles: true,
            allow_external_resources: false,
            allow_forms: false,
            allow_iframes: false,
            custom_allowed_tags: Vec::new(),
            custom_blocked_tags: vec![
                "script".to_string(),
                "iframe".to_string(),
                "frame".to_string(),
                "frameset".to_string(),
                "object".to_string(),
                "embed".to_string(),
                "applet".to_string(),
            ],
        }
    }
}

/// Extracted HTML resource link descriptor for archive assets (images, stylesheets, fonts, media).
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIHtmlResourceLink {
    /// HTML tag name referencing the resource (e.g. "img", "link", "script", "source", "a").
    pub tag_name: String,
    /// Attribute name containing the resource target (e.g. "src", "href", "poster", "data").
    pub attribute_name: String,
    /// Original URI or relative path before transformation.
    pub original_uri: String,
    /// Resolved `ttzip-vfs://` virtual filesystem URI if applicable.
    pub resolved_vfs_uri: Option<String>,
    /// Classification category ("image", "stylesheet", "script", "font", "audio", "video", "link", "media", "other").
    pub resource_type: String,
    /// Whether the URI points to an external remote network endpoint.
    pub is_external: bool,
}

/// Result of HTML VFS rewriting, resource extraction, and structural inspection.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIHtmlTransformResult {
    /// Transformed HTML markup with sanitized elements and rewritten `ttzip-vfs://` URLs.
    pub transformed_html: String,
    /// All discovered and resolved external/internal resource links.
    pub extracted_resources: Vec<UniFFIHtmlResourceLink>,
    /// Document title extracted from `<title>` element if present.
    pub title: Option<String>,
    /// Detected or declared character set encoding from `<meta>` tags.
    pub charset: Option<String>,
    /// Whether the source markup contained active `<script>` tags or inline event handlers.
    pub has_scripts: bool,
    /// Whether the source markup contained styling rules or inline styles.
    pub has_inline_styles: bool,
    /// Total text character count excluding markup tags.
    pub metrics_chars: u32,
    /// Estimated word count excluding markup tags.
    pub metrics_words: u32,
}

/// Strongly-typed HTML transformation error enum mapped directly to Swift `throws UniFFIHtmlError`.
#[derive(Debug, Error, uniffi::Error)]
pub enum UniFFIHtmlError {
    /// Failure during HTML parsing or tokenization.
    #[error("HTML parsing error: {message}")]
    ParseError { message: String },

    /// Failure during stream rewriting or output serialization.
    #[error("HTML rewriting error: {message}")]
    RewriteError { message: String },

    /// Document encoding is unsupported or malformed.
    #[error("Invalid character encoding: {message}")]
    InvalidEncoding { message: String },

    /// Document violates strict security constraints.
    #[error("Security policy violation: {reason}")]
    SecurityViolation { reason: String },

    /// File system or stream I/O failure.
    #[error("I/O error during HTML operation: {message}")]
    IoError { message: String },

    /// Operation was cancelled by caller.
    #[error("HTML operation cancelled")]
    Cancelled,
}

impl UniFFIHtmlError {
    pub fn parse_err(msg: impl std::fmt::Display) -> Self {
        Self::ParseError {
            message: msg.to_string(),
        }
    }

    pub fn rewrite_err(msg: impl std::fmt::Display) -> Self {
        Self::RewriteError {
            message: msg.to_string(),
        }
    }

    pub fn invalid_encoding(msg: impl std::fmt::Display) -> Self {
        Self::InvalidEncoding {
            message: msg.to_string(),
        }
    }

    pub fn security_violation(reason: impl std::fmt::Display) -> Self {
        Self::SecurityViolation {
            reason: reason.to_string(),
        }
    }

    pub fn io_err(msg: impl std::fmt::Display) -> Self {
        Self::IoError {
            message: msg.to_string(),
        }
    }
}
