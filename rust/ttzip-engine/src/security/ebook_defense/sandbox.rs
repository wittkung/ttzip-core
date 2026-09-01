// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 4: Active Content & E-book Presentation Sandbox Guard.
//!
//! Enforces deterministic sanitization on XHTML, HTML5, and SVG content within e-book
//! chapters, purging `<script>`, `<iframe>`, inline `on*` event handlers, and `javascript:` URIs.

use std::collections::HashSet;

/// Report detailing the mutations and sanitizations performed on an e-book document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentSanitizationReport {
    /// Number of dangerous executable tags (e.g. `<script>`, `<iframe>`) purged.
    pub stripped_tags_count: usize,
    /// Number of inline DOM event handlers (e.g. `onload`, `onclick`) neutralized.
    pub neutralized_events_count: usize,
    /// Number of dangerous pseudo-protocols (e.g. `javascript:`) replaced.
    pub neutralized_protocols_count: usize,
}

/// Guard providing tag and attribute sanitization for e-book rendering layers.
#[derive(Debug, Default, Clone, Copy)]
pub struct EbookSandboxGuard;

impl EbookSandboxGuard {
    /// List of dangerous tag names that must be unconditionally stripped from e-book chapters.
    const FORBIDDEN_TAGS: &'static [&'static str] = &[
        "script",
        "iframe",
        "object",
        "embed",
        "applet",
        "foreignobject",
        "meta",
        "form",
        "base",
        "input",
        "button",
        "textarea",
        "select",
        "keygen",
    ];

    /// HTML void / self-closing tags that do not enclose child content.
    const VOID_TAGS: &'static [&'static str] = &[
        "base", "meta", "embed", "img", "input", "link", "br", "hr", "area", "col", "param",
        "source", "track", "wbr", "circle", "rect", "line", "path", "polygon", "polyline",
    ];

    /// Sanitizes an XHTML / HTML5 document string, removing active scripts and event handlers.
    pub fn sanitize_xhtml_content(raw_html: &str) -> (String, ContentSanitizationReport) {
        let mut report = ContentSanitizationReport::default();
        let forbidden_set: HashSet<&'static str> = Self::FORBIDDEN_TAGS.iter().copied().collect();
        let void_set: HashSet<&'static str> = Self::VOID_TAGS.iter().copied().collect();

        let mut sanitized = String::with_capacity(raw_html.len());
        let mut chars = raw_html.chars().peekable();
        let mut forbidden_stack: Vec<String> = Vec::new();

        while let Some(c) = chars.next() {
            if c == '<' {
                // Check if this is an XML comment (<!-- ... -->) or CDATA (<![CDATA[ ... ]]>)
                if chars.peek() == Some(&'!') {
                    let mut lookahead = String::new();
                    while let Some(&ch) = chars.peek() {
                        lookahead.push(ch);
                        chars.next();
                        if lookahead == "!--" {
                            // Skip comment
                            while let Some(comment_ch) = chars.next() {
                                if comment_ch == '-' && chars.peek() == Some(&'-') {
                                    chars.next();
                                    if chars.peek() == Some(&'>') {
                                        chars.next();
                                        break;
                                    }
                                }
                            }
                            break;
                        } else if lookahead == "![cdata[" || lookahead == "![CDATA[" {
                            // Skip or retain CDATA content safely
                            let mut cdata_content = String::new();
                            while let Some(cd_ch) = chars.next() {
                                if cd_ch == ']' && chars.peek() == Some(&']') {
                                    chars.next();
                                    if chars.peek() == Some(&'>') {
                                        chars.next();
                                        break;
                                    }
                                    cdata_content.push(']');
                                    cdata_content.push(']');
                                } else {
                                    cdata_content.push(cd_ch);
                                }
                            }
                            if forbidden_stack.is_empty() {
                                sanitized.push_str(&cdata_content);
                            }
                            break;
                        }
                        if lookahead.len() >= 8 {
                            break;
                        }
                    }
                    continue;
                }

                // Parse tag name
                let is_closing = chars.peek() == Some(&'/');
                if is_closing {
                    chars.next();
                }

                let mut tag_name = String::new();
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_whitespace() || next_ch == '>' || next_ch == '/' {
                        break;
                    }
                    tag_name.push(chars.next().unwrap());
                }

                let tag_lower = tag_name.to_ascii_lowercase();

                // Parse attributes within the tag until '>'
                let mut in_quotes = false;
                let mut quote_char = '"';
                let mut attr_buf = String::new();
                let mut is_self_closing = false;

                for attr_ch in chars.by_ref() {
                    if (attr_ch == '"' || attr_ch == '\'') && !in_quotes {
                        in_quotes = true;
                        quote_char = attr_ch;
                    } else if in_quotes && attr_ch == quote_char {
                        in_quotes = false;
                    }

                    if !in_quotes && attr_ch == '/' {
                        is_self_closing = true;
                    } else if !in_quotes && !attr_ch.is_whitespace() && attr_ch != '>' {
                        is_self_closing = false;
                    }

                    if !in_quotes && attr_ch == '>' {
                        break;
                    }

                    attr_buf.push(attr_ch);
                }

                if forbidden_set.contains(tag_lower.as_str()) {
                    if is_closing {
                        if let Some(pos) = forbidden_stack.iter().rposition(|t| t == &tag_lower) {
                            forbidden_stack.truncate(pos);
                        }
                    } else {
                        report.stripped_tags_count += 1;
                        if !is_self_closing && !void_set.contains(tag_lower.as_str()) {
                            forbidden_stack.push(tag_lower);
                        }
                    }
                    continue;
                }

                if !forbidden_stack.is_empty() {
                    // Suppress all content inside a forbidden element (e.g. <script>...</script>)
                    continue;
                }

                // Emit safe opening/closing tag
                sanitized.push('<');
                if is_closing {
                    sanitized.push('/');
                }
                sanitized.push_str(&tag_name);

                let cleaned_attrs = Self::clean_tag_attributes(
                    &attr_buf,
                    &mut report.neutralized_events_count,
                    &mut report.neutralized_protocols_count,
                );
                sanitized.push_str(&cleaned_attrs);
                sanitized.push('>');
            } else if forbidden_stack.is_empty() {
                sanitized.push(c);
            }
        }

        (sanitized, report)
    }

    /// Strips inline event handlers (`on\w+=`) and dangerous pseudo-protocols from attributes.
    fn clean_tag_attributes(
        attrs: &str,
        event_count: &mut usize,
        protocol_count: &mut usize,
    ) -> String {
        let mut result = String::with_capacity(attrs.len());
        let lower = attrs.to_ascii_lowercase();
        let bytes = attrs.as_bytes();
        let lower_bytes = lower.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            // Check for javascript: / vbscript: pseudo protocol
            if i + 11 <= lower_bytes.len() && &lower_bytes[i..i + 11] == b"javascript:" {
                result.push_str("about:blank");
                *protocol_count += 1;
                i += 11;
                continue;
            }
            if i + 9 <= lower_bytes.len() && &lower_bytes[i..i + 9] == b"vbscript:" {
                result.push_str("about:blank");
                *protocol_count += 1;
                i += 9;
                continue;
            }

            // Check for inline on* events e.g. onclick=, onload=
            if i + 2 < lower_bytes.len()
                && (i == 0 || lower_bytes[i - 1].is_ascii_whitespace() || lower_bytes[i - 1] == b'/')
                && lower_bytes[i] == b'o'
                && lower_bytes[i + 1] == b'n'
                && lower_bytes[i + 2].is_ascii_alphabetic()
            {
                let mut eq_pos = i + 2;
                while eq_pos < lower_bytes.len()
                    && (lower_bytes[eq_pos].is_ascii_alphanumeric() || lower_bytes[eq_pos] == b'_')
                {
                    eq_pos += 1;
                }
                if eq_pos < lower_bytes.len() && lower_bytes[eq_pos] == b'=' {
                    result.push_str("data-disabled-event=");
                    *event_count += 1;
                    i = eq_pos + 1;
                    continue;
                }
            }

            result.push(bytes[i] as char);
            i += 1;
        }

        result
    }

    /// Validates whether a URI target is structurally safe for e-book navigation.
    pub fn is_safe_uri(uri: &str) -> bool {
        let trimmed = uri.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("javascript:")
            || lower.starts_with("vbscript:")
            || lower.starts_with("data:text/html")
        {
            return false;
        }

        true
    }

    /// Sanitizes an embedded SVG vector graphic document or snippet.
    pub fn sanitize_svg(raw_svg: &str) -> (String, ContentSanitizationReport) {
        Self::sanitize_xhtml_content(raw_svg)
    }
}
