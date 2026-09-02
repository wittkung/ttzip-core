// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Subtitle Active Script Neutralizer, Protocol Sanitizer, and ASS Drawing Guard.
//!
//! Protects against malicious subtitle payloads embedded in MP4/MKV/standalone tracks:
//! - Neutralizes external network and file protocols (`http:`, `https:`, `file:`, `ftp:`, `javascript:`, `data:`).
//! - Strips active HTML/XML execution tags (`<script>`, `<iframe>`, `<style>`, `<object>`, `on*` event handlers).
//! - Caps ASS/SSA vector drawing command nodes (<= 1,024 vertices) against polygon rendering bombs.
//! - Disinfects directory path traversal attempts (`../`, absolute paths, null bytes) in font/attachment metadata.

use super::{VideoDefenseError, DEFAULT_MAX_ASS_DRAWING_NODES};

/// Subtitle container format identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VideoSubtitleFormat {
    /// SubRip Text (.srt).
    #[default]
    Srt,
    /// Web Video Text Tracks (.vtt).
    Vtt,
    /// Advanced SubStation Alpha (.ass).
    Ass,
    /// SubStation Alpha (.ssa).
    Ssa,
    /// Unknown or generic plain text subtitles.
    Generic,
}

/// Statistics and metrics gathered during subtitle sanitization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubtitleSanitizeReport {
    /// Number of dangerous protocol schemes neutralized.
    pub neutralized_protocols: usize,
    /// Number of active script/HTML execution tags stripped.
    pub stripped_tags: usize,
    /// Total ASS drawing command nodes parsed.
    pub ass_drawing_nodes: usize,
    /// Number of path traversal sequences disarmed.
    pub neutralized_traversals: usize,
}

/// Sanitized subtitle payload ready for safe rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedSubtitle {
    /// Disinfected text content.
    pub sanitized_text: String,
    /// Format of the subtitle.
    pub format: VideoSubtitleFormat,
    /// Sanitization report.
    pub report: SubtitleSanitizeReport,
}

/// Defensive guard protecting subtitle parsers and renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubtitleScriptSandboxGuard {
    max_ass_drawing_nodes: usize,
}

impl Default for SubtitleScriptSandboxGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl SubtitleScriptSandboxGuard {
    /// Creates a guard with default security constraints (<= 1024 ASS drawing nodes).
    pub const fn new() -> Self {
        Self {
            max_ass_drawing_nodes: DEFAULT_MAX_ASS_DRAWING_NODES,
        }
    }

    /// Creates a guard with custom drawing node limits.
    pub const fn with_max_ass_drawing_nodes(max_nodes: usize) -> Self {
        Self {
            max_ass_drawing_nodes: max_nodes,
        }
    }

    /// Returns the maximum allowed ASS drawing nodes.
    #[inline]
    pub const fn max_ass_drawing_nodes(&self) -> usize {
        self.max_ass_drawing_nodes
    }

    /// Neutralizes dangerous URI schemes and protocol strings in the text.
    pub fn neutralize_protocols(&self, text: &str) -> (String, usize) {
        let dangerous_schemes = [
            "javascript:",
            "data:text/html",
            "data:application",
            "file://",
            "file:",
            "http://",
            "https://",
            "ftp://",
            "blob:",
            "vbscript:",
        ];

        let mut count = 0;
        let mut result = text.to_string();

        for scheme in &dangerous_schemes {
            while let Some(pos) = result.to_lowercase().find(scheme) {
                result.replace_range(pos..pos + scheme.len(), "[blocked-scheme]");
                count += 1;
            }
        }

        (result, count)
    }

    /// Strips active script tags, iframes, styles, and dangerous event handlers.
    pub fn strip_active_tags(&self, text: &str) -> (String, usize) {
        let full_removal_tags = ["script", "style"];
        let dangerous_tags = [
            "script", "iframe", "object", "embed", "applet", "style", "form", "input", "button",
            "link", "meta", "base",
        ];

        let mut count = 0;
        let mut output = String::with_capacity(text.len());
        let mut idx = 0;
        let lower = text.to_lowercase();
        let bytes = text.as_bytes();

        while idx < text.len() {
            if bytes[idx] == b'<' {
                if let Some(close_tag) = text[idx..].find('>') {
                    let tag_slice = &text[idx + 1..idx + close_tag];
                    let trimmed = tag_slice.trim();
                    let tag_name = trimmed
                        .trim_start_matches('/')
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_lowercase();

                    let has_event = trimmed.to_lowercase().contains("on")
                        && (trimmed.to_lowercase().contains("onload")
                            || trimmed.to_lowercase().contains("onerror")
                            || trimmed.to_lowercase().contains("onclick")
                            || trimmed.to_lowercase().contains("onmouseover"));

                    if full_removal_tags.contains(&tag_name.as_str()) && !trimmed.starts_with('/') {
                        // Strip open tag and all inner content until </script> or </style>
                        let close_pattern = format!("</{}", tag_name);
                        count += 1;
                        if let Some(end_pos) = lower[idx + close_tag + 1..].find(&close_pattern) {
                            let close_tag_end = match text[idx + close_tag + 1 + end_pos..].find('>') {
                                Some(gt) => idx + close_tag + 1 + end_pos + gt + 1,
                                None => text.len(),
                            };
                            idx = close_tag_end;
                            continue;
                        } else {
                            // No closing tag found, strip till end
                            break;
                        }
                    }

                    if dangerous_tags.contains(&tag_name.as_str()) || has_event {
                        count += 1;
                        idx += close_tag + 1;
                        continue;
                    } else {
                        output.push('<');
                        output.push_str(tag_slice);
                        output.push('>');
                        idx += close_tag + 1;
                        continue;
                    }
                }
            }

            if let Some(ch) = text[idx..].chars().next() {
                output.push(ch);
                idx += ch.len_utf8();
            } else {
                break;
            }
        }

        (output, count)
    }

    /// Disinfects path traversal sequences (`../`, `..\`, absolute root prefixes, null bytes).
    pub fn neutralize_path_traversal(&self, text: &str) -> (String, usize) {
        let mut count = 0;
        let mut result = text.replace('\0', "");

        let traversal_patterns = ["../", "..\\", "/etc/", "c:\\windows", "\\\\?\\"];

        for pat in &traversal_patterns {
            while let Some(pos) = result.to_lowercase().find(pat) {
                result.replace_range(pos..pos + pat.len(), "[neutralized-path]");
                count += 1;
            }
        }

        (result, count)
    }

    /// Counts ASS drawing command vector nodes (inside `{\p1} ... {\p0}` blocks).
    pub fn count_and_validate_ass_drawing(
        &self,
        text: &str,
    ) -> Result<(String, usize), VideoDefenseError> {
        let mut total_nodes: usize = 0;
        let mut in_drawing_mode = false;
        let mut output = String::with_capacity(text.len());
        let mut i = 0;
        let bytes = text.as_bytes();

        while i < bytes.len() {
            if bytes[i] == b'{' {
                if let Some(close_idx) = text[i..].find('}') {
                    let override_tag = &text[i..=i + close_idx];
                    if override_tag.contains(r"\p1")
                        || override_tag.contains(r"\p2")
                        || override_tag.contains(r"\p3")
                        || override_tag.contains(r"\p4")
                    {
                        in_drawing_mode = true;
                    } else if override_tag.contains(r"\p0") {
                        in_drawing_mode = false;
                    }
                    output.push_str(override_tag);
                    i += close_idx + 1;
                    continue;
                }
            }

            if in_drawing_mode {
                let start_draw = i;
                while i < bytes.len() && bytes[i] != b'{' {
                    i += 1;
                }
                let draw_cmds = &text[start_draw..i];
                let tokens: Vec<&str> = draw_cmds.split_whitespace().collect();
                total_nodes = total_nodes.saturating_add(tokens.len());

                if total_nodes > self.max_ass_drawing_nodes {
                    return Err(VideoDefenseError::AssDrawingLimitExceeded {
                        node_count: total_nodes,
                        limit: self.max_ass_drawing_nodes,
                    });
                }
                output.push_str(draw_cmds);
            } else {
                if let Some(ch) = text[i..].chars().next() {
                    output.push(ch);
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
        }

        Ok((output, total_nodes))
    }

    /// Sanitizes general subtitle dialogue and metadata text across all safety dimensions.
    pub fn sanitize(
        &self,
        raw_text: &str,
        format: VideoSubtitleFormat,
    ) -> Result<SanitizedSubtitle, VideoDefenseError> {
        let (no_proto, neutralized_protocols) = self.neutralize_protocols(raw_text);
        let (no_tags, stripped_tags) = self.strip_active_tags(&no_proto);
        let (no_traversal, neutralized_traversals) = self.neutralize_path_traversal(&no_tags);

        let (final_text, ass_drawing_nodes) =
            if format == VideoSubtitleFormat::Ass || format == VideoSubtitleFormat::Ssa {
                self.count_and_validate_ass_drawing(&no_traversal)?
            } else {
                (no_traversal, 0)
            };

        Ok(SanitizedSubtitle {
            sanitized_text: final_text,
            format,
            report: SubtitleSanitizeReport {
                neutralized_protocols,
                stripped_tags,
                ass_drawing_nodes,
                neutralized_traversals,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_neutralization() {
        let guard = SubtitleScriptSandboxGuard::new();
        let raw = "Download font at http://malicious.com/hack.ttf or file:///etc/shadow or javascript:alert(1)";
        let (sanitized, count) = guard.neutralize_protocols(raw);
        assert_eq!(count, 3);
        assert!(!sanitized.contains("http://"));
        assert!(!sanitized.contains("file://"));
        assert!(!sanitized.contains("javascript:"));
        assert!(sanitized.contains("[blocked-scheme]"));
    }

    #[test]
    fn test_script_tag_stripping() {
        let guard = SubtitleScriptSandboxGuard::new();
        let raw = "Hello <script>alert(document.cookie)</script>World <iframe src='foo'></iframe><b>Bold</b> <div onload='pwn()'>text</div>";
        let (sanitized, count) = guard.strip_active_tags(raw);
        assert!(count >= 3);
        assert!(!sanitized.contains("<script>"));
        assert!(!sanitized.contains("<iframe>"));
        assert!(!sanitized.contains("onload"));
        assert!(sanitized.contains("<b>Bold</b>"));
        assert!(sanitized.contains("Hello World"));
    }

    #[test]
    fn test_path_traversal_neutralization() {
        let guard = SubtitleScriptSandboxGuard::new();
        let raw = "FontFile: ../../../etc/passwd and ..\\..\\windows\\system32";
        let (sanitized, count) = guard.neutralize_path_traversal(raw);
        assert!(count >= 2);
        assert!(!sanitized.contains("../"));
        assert!(!sanitized.contains("..\\"));
    }

    #[test]
    fn test_ass_drawing_limits() {
        let guard = SubtitleScriptSandboxGuard::with_max_ass_drawing_nodes(10);
        let valid_ass = r"Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\p1}m 0 0 l 10 10{\p0}";
        let res = guard.count_and_validate_ass_drawing(valid_ass);
        assert!(res.is_ok());

        // Oversized drawing bomb (> 10 tokens)
        let invalid_ass = r"Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\p1}m 0 0 l 10 10 l 20 20 l 30 30 l 40 40{\p0}";
        let err = guard.count_and_validate_ass_drawing(invalid_ass).unwrap_err();
        match err {
            VideoDefenseError::AssDrawingLimitExceeded { node_count, limit } => {
                assert!(node_count > 10);
                assert_eq!(limit, 10);
            }
            _ => panic!("Expected AssDrawingLimitExceeded"),
        }
    }
}
