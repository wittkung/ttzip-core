// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 1: Active HTML & SVG Content Sanitizer Guard.
//!
//! Enforces deterministic, multi-pass sanitization against XSS, DOM-clobbering,
//! active scripts, event handler payloads, and malicious SVG vectors:
//! - Purges forbidden executable and structural hijacking tags (`<script>`, `<iframe>`,
//!   `<object>`, `<embed>`, `<base>`, `<meta>`, `<applet>`, `<foreignObject>`, `<dialog>`, `<template>`)
//! - Strips all inline `on*` event handlers (`onload`, `onerror`, `onclick`, `onmouseover`, etc.)
//! - Intercepts and neutralizes dangerous pseudo-protocols (`javascript:`, `vbscript:`, `data:text/html`, etc.)
//! - Sanitizes SVG elements, preventing embedded script injection, event triggers, and foreign payloads.

use std::collections::HashSet;

/// Metrics and audit report produced by HTML content sanitization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SanitizerReport {
    /// Number of dangerous tags stripped from the document.
    pub stripped_tags_count: usize,
    /// Number of inline DOM `on*` event handler attributes removed.
    pub stripped_events_count: usize,
    /// Number of dangerous pseudo-protocols (`javascript:`, etc.) neutralized.
    pub neutralized_protocols_count: usize,
    /// Number of SVG elements or attributes modified/purged for safety.
    pub sanitized_svg_elements: usize,
}

/// Active content sanitization guard for HTML5 and SVG documents.
#[derive(Debug, Clone)]
pub struct HtmlSanitizerGuard {
    forbidden_tags: HashSet<String>,
    uri_bearing_attributes: HashSet<String>,
    allowed_data_image_types: HashSet<String>,
}

impl Default for HtmlSanitizerGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlSanitizerGuard {
    /// Standard forbidden tags that present active code execution or context hijacking risks.
    const DEFAULT_FORBIDDEN_TAGS: &'static [&'static str] = &[
        "script",
        "iframe",
        "object",
        "embed",
        "base",
        "meta",
        "applet",
        "foreignobject",
        "dialog",
        "template",
        "portal",
        "form",
        "input",
        "button",
        "textarea",
        "select",
        "keygen",
    ];

    /// Attributes known to carry executable or fetchable URI schemes.
    const DEFAULT_URI_ATTRIBUTES: &'static [&'static str] = &[
        "href",
        "src",
        "action",
        "formaction",
        "xlink:href",
        "data",
        "poster",
        "background",
        "codebase",
        "cite",
        "ping",
        "dynsrc",
        "lowsrc",
        "srcset",
    ];

    /// Allowed safe image MIME prefixes for `data:` URIs.
    const DEFAULT_ALLOWED_DATA_IMAGES: &'static [&'static str] = &[
        "data:image/png",
        "data:image/jpeg",
        "data:image/jpg",
        "data:image/gif",
        "data:image/webp",
        "data:image/bmp",
        "data:image/x-icon",
        "data:image/vnd.microsoft.icon",
    ];

    /// Creates a new sanitizer guard with default security policies.
    #[must_use]
    pub fn new() -> Self {
        let forbidden_tags = Self::DEFAULT_FORBIDDEN_TAGS
            .iter()
            .map(|&s| s.to_ascii_lowercase())
            .collect();
        let uri_bearing_attributes = Self::DEFAULT_URI_ATTRIBUTES
            .iter()
            .map(|&s| s.to_ascii_lowercase())
            .collect();
        let allowed_data_image_types = Self::DEFAULT_ALLOWED_DATA_IMAGES
            .iter()
            .map(|&s| s.to_ascii_lowercase())
            .collect();

        Self {
            forbidden_tags,
            uri_bearing_attributes,
            allowed_data_image_types,
        }
    }

    /// Checks if a tag is forbidden and should be stripped.
    #[must_use]
    pub fn is_forbidden_tag(&self, tag_name: &str) -> bool {
        self.forbidden_tags.contains(&tag_name.trim().to_ascii_lowercase())
    }

    /// Checks if an attribute is an inline event handler (`on*` / `ON*`).
    #[must_use]
    pub fn is_event_attribute(attr_name: &str) -> bool {
        let lower = attr_name.trim().to_ascii_lowercase();
        lower.starts_with("on") && lower.len() > 2
    }

    /// Checks if an attribute is a URI-bearing attribute.
    #[must_use]
    pub fn is_uri_attribute(&self, attr_name: &str) -> bool {
        self.uri_bearing_attributes
            .contains(&attr_name.trim().to_ascii_lowercase())
    }

    /// Normalizes and decodes an attribute value to detect hidden pseudo-protocols.
    #[must_use]
    pub fn normalize_uri(val: &str) -> String {
        let mut normalized = String::with_capacity(val.len());
        let mut chars = val.chars().peekable();

        while let Some(c) = chars.next() {
            // Strip ASCII whitespace, null bytes, and non-printable control chars
            if c.is_ascii_whitespace() || c == '\0' || (c.is_ascii_control() && c != '\t' && c != '\n') {
                continue;
            }

            // Simple HTML character entity decoding (&#x6a; / &#106;)
            if c == '&' && chars.peek() == Some(&'#') {
                chars.next(); // consume '#'
                let mut entity_buf = String::new();
                let is_hex = chars.peek() == Some(&'x') || chars.peek() == Some(&'X');
                if is_hex {
                    chars.next(); // consume 'x'
                }
                while let Some(&next_ch) = chars.peek() {
                    if next_ch == ';' {
                        chars.next();
                        break;
                    } else if (is_hex && next_ch.is_ascii_hexdigit())
                        || (!is_hex && next_ch.is_ascii_digit())
                    {
                        entity_buf.push(next_ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                let decoded_char = if is_hex {
                    u32::from_str_radix(&entity_buf, 16)
                        .ok()
                        .and_then(char::from_u32)
                } else {
                    entity_buf.parse::<u32>().ok().and_then(char::from_u32)
                };

                if let Some(dc) = decoded_char {
                    if !dc.is_ascii_whitespace() && dc != '\0' && !dc.is_ascii_control() {
                        normalized.push(dc);
                    }
                }
                continue;
            }

            normalized.push(c);
        }

        normalized.to_ascii_lowercase()
    }

    /// Checks if a URI contains a dangerous executable scheme.
    #[must_use]
    pub fn is_dangerous_uri(&self, uri: &str) -> bool {
        let normalized = Self::normalize_uri(uri);

        // Disallow dangerous script pseudo-protocols
        if normalized.starts_with("javascript:")
            || normalized.starts_with("vbscript:")
            || normalized.starts_with("livescript:")
            || normalized.starts_with("mocha:")
        {
            return true;
        }

        // Check data: URIs
        if normalized.starts_with("data:") {
            // Allow safe image formats, block executable HTML/JS/SVG data URIs
            let is_allowed_image = self
                .allowed_data_image_types
                .iter()
                .any(|prefix| normalized.starts_with(prefix));
            if !is_allowed_image {
                return true;
            }
        }

        false
    }

    /// Sanitizes an attribute key-value pair, returning `None` if the attribute should be removed,
    /// or `Some(sanitized_value)` if preserved/rewritten.
    pub fn sanitize_attribute(
        &self,
        attr_name: &str,
        attr_value: &str,
        report: &mut SanitizerReport,
    ) -> Option<String> {
        let lower_name = attr_name.trim().to_ascii_lowercase();

        // 1. Strip inline event handlers (onclick, onload, etc.)
        if Self::is_event_attribute(&lower_name) {
            report.stripped_events_count = report.stripped_events_count.saturating_add(1);
            return None;
        }

        // 2. Strip srcdoc attribute on frame/iframe
        if lower_name == "srcdoc" {
            report.stripped_events_count = report.stripped_events_count.saturating_add(1);
            return None;
        }

        // 3. Inspect URI attributes for dangerous schemes
        if self.is_uri_attribute(&lower_name) {
            if self.is_dangerous_uri(attr_value) {
                report.neutralized_protocols_count =
                    report.neutralized_protocols_count.saturating_add(1);
                return None;
            }
        }

        Some(attr_value.to_string())
    }

    /// Validates and cleanses an SVG tag or element.
    #[must_use]
    pub fn is_dangerous_svg_tag(&self, tag_name: &str) -> bool {
        let lower = tag_name.trim().to_ascii_lowercase();
        lower == "script"
            || lower == "foreignobject"
            || lower == "handler"
            || lower == "listener"
            || lower == "iframe"
    }
}
