// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Malformed XML stream recovery, entity sanitization, and sensitive buffer zeroization.

use std::fmt;
use std::ops::{Deref, DerefMut};
use zeroize::{Zeroize, ZeroizeOnDrop};

// ============================================================================
// 5. Malformed Stream Recovery Guard
// ============================================================================

/// Self-healing and sanitization filter for malformed, broken, or truncated XML streams.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MalformedStreamRecoveryGuard;

impl MalformedStreamRecoveryGuard {
    /// Sanitizes plain text content by escaping XML special characters.
    #[must_use]
    pub fn sanitize_text(text: &str) -> String {
        let mut out = String::with_capacity(text.len() + 16);
        for c in text.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&apos;"),
                _ => out.push(c),
            }
        }
        out
    }

    /// Strips DOCTYPE and DTD subsets from an XML string cleanly.
    #[must_use]
    pub fn strip_dangerous_dtd(xml_input: &str) -> String {
        let lower = xml_input.to_ascii_lowercase();
        if let Some(start) = lower.find("<!doctype") {
            let mut depth = 0;
            let mut in_bracket = false;
            let mut end_pos = None;

            for (idx, b) in xml_input[start..].bytes().enumerate() {
                match b {
                    b'[' => in_bracket = true,
                    b']' => in_bracket = false,
                    b'<' => depth += 1,
                    b'>' => {
                        depth -= 1;
                        if depth == 0 && !in_bracket {
                            end_pos = Some(start + idx + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(end) = end_pos {
                let mut sanitized = String::with_capacity(xml_input.len());
                sanitized.push_str(&xml_input[..start]);
                sanitized.push_str(&xml_input[end..]);
                return sanitized;
            }
        }
        xml_input.to_string()
    }

    /// Heals truncated XML streams by automatically appending matching closing tags for open elements.
    #[must_use]
    pub fn heal_truncated_stream(xml_input: &str) -> String {
        let mut open_tags: Vec<String> = Vec::new();
        let mut in_tag = false;
        let mut is_closing = false;
        let mut is_self_closing = false;
        let mut current_tag = String::new();

        let chars: Vec<char> = xml_input.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '<' {
                in_tag = true;
                current_tag.clear();
                if i + 1 < chars.len() && chars[i + 1] == '/' {
                    is_closing = true;
                    i += 1;
                } else {
                    is_closing = false;
                }
            } else if c == '>' && in_tag {
                in_tag = false;
                let tag_name = current_tag
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('/');
                if !tag_name.is_empty() && !tag_name.starts_with('?') && !tag_name.starts_with('!') {
                    if is_closing {
                        if let Some(pos) = open_tags.iter().rposition(|t| t == tag_name) {
                            open_tags.truncate(pos);
                        }
                    } else if !is_self_closing && !current_tag.ends_with('/') {
                        open_tags.push(tag_name.to_string());
                    }
                }
                is_self_closing = false;
            } else if in_tag {
                if c == '/' && i + 1 < chars.len() && chars[i + 1] == '>' {
                    is_self_closing = true;
                }
                current_tag.push(c);
            }
            i += 1;
        }

        let mut healed = xml_input.to_string();
        if in_tag {
            healed.push('>');
        }
        for tag in open_tags.iter().rev() {
            healed.push_str("</");
            healed.push_str(tag);
            healed.push('>');
        }
        healed
    }

    /// Full recovery and sanitization pipeline for input XML text.
    #[must_use]
    pub fn recover_and_sanitize(xml_input: &str) -> String {
        let stripped = Self::strip_dangerous_dtd(xml_input);
        Self::heal_truncated_stream(&stripped)
    }
}

// ============================================================================
// 6. Sensitive XML Memory Zeroize Guard
// ============================================================================

/// Volatile memory wrapper ensuring zero-on-drop erasure of sensitive XML payloads.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveXmlBuffer {
    data: Vec<u8>,
}

impl SensitiveXmlBuffer {
    /// Creates a new buffer from an existing byte vector.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Creates a sensitive buffer from a byte slice copy.
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
        }
    }

    /// Creates a sensitive buffer from a UTF-8 string.
    #[must_use]
    pub fn from_string(s: String) -> Self {
        Self {
            data: s.into_bytes(),
        }
    }

    /// Creates a sensitive buffer from a string slice.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self {
            data: s.as_bytes().to_vec(),
        }
    }

    /// Returns a borrowed slice of the inner sensitive bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Attempts to interpret inner bytes as a UTF-8 string slice.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }

    /// Returns byte length of the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the buffer contains 0 bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Manually clears and zeroizes the underlying buffer immediately.
    pub fn clear_and_zeroize(&mut self) {
        self.data.zeroize();
        self.data.clear();
    }
}

impl Deref for SensitiveXmlBuffer {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SensitiveXmlBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl AsRef<[u8]> for SensitiveXmlBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl fmt::Debug for SensitiveXmlBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SensitiveXmlBuffer")
            .field("len", &self.data.len())
            .field("data", &"[REDACTED_SENSITIVE_XML_PAYLOAD]")
            .finish()
    }
}
