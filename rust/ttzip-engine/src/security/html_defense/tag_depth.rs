// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 3: Tag Nesting Depth & Unclosed Tag Quota Guard.
//!
//! Enforces deterministic DOM hierarchy limits to prevent deep recursion,
//! call-stack exhaustion, and CPU spinning from unclosed tag cascades:
//! - Tag nesting depth ceiling <= 64 levels
//! - Unclosed tag quota <= 256 tags
//!
//! Recognizes HTML5 void elements that do not push to the hierarchy stack.

use super::{HtmlDefenseError, DEFAULT_MAX_HTML_DEPTH, DEFAULT_MAX_UNCLOSED_TAGS};

/// Inspection metrics and report from tag depth and recursion validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagDepthReport {
    /// Maximum nesting depth reached during document parsing.
    pub max_depth_reached: usize,
    /// Total number of start tags processed.
    pub total_start_tags: usize,
    /// Total number of end tags processed.
    pub total_end_tags: usize,
    /// Total unclosed tags detected at end of stream or stack prune.
    pub unclosed_tags_count: usize,
}

/// Guard enforcing DOM element nesting recursion bounds and unclosed tag limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagNestingDepthGuard {
    max_depth: usize,
    max_unclosed_tags: usize,
    stack: Vec<String>,
    max_depth_seen: usize,
    unclosed_tags_count: usize,
    total_start_tags: usize,
    total_end_tags: usize,
}

impl Default for TagNestingDepthGuard {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_HTML_DEPTH, DEFAULT_MAX_UNCLOSED_TAGS)
    }
}

impl TagNestingDepthGuard {
    /// HTML5 void tags that do not require closing tags and do not increase nesting depth.
    const VOID_TAGS: &'static [&'static str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];

    /// Creates a new tag nesting depth guard with custom depth and unclosed quotas.
    #[must_use]
    pub fn new(max_depth: usize, max_unclosed_tags: usize) -> Self {
        Self {
            max_depth,
            max_unclosed_tags,
            stack: Vec::with_capacity(max_depth.min(128)),
            max_depth_seen: 0,
            unclosed_tags_count: 0,
            total_start_tags: 0,
            total_end_tags: 0,
        }
    }

    /// Returns `true` if the tag name is a standard HTML5 void element.
    #[must_use]
    pub fn is_void_tag(tag_name: &str) -> bool {
        Self::VOID_TAGS
            .iter()
            .any(|v| v.eq_ignore_ascii_case(tag_name.trim()))
    }

    /// Handles opening of a tag, tracking hierarchy depth and unclosed tag count.
    pub fn on_element_start(
        &mut self,
        tag_name: &str,
        is_self_closing: bool,
    ) -> Result<(), HtmlDefenseError> {
        self.total_start_tags = self.total_start_tags.saturating_add(1);
        let trimmed = tag_name.trim();

        if is_self_closing || Self::is_void_tag(trimmed) {
            return Ok(());
        }

        let new_depth = self.stack.len().saturating_add(1);
        if new_depth > self.max_depth {
            return Err(HtmlDefenseError::TagDepthLimitExceeded {
                depth: new_depth,
                max_depth: self.max_depth,
            });
        }

        self.stack.push(trimmed.to_ascii_lowercase());
        if new_depth > self.max_depth_seen {
            self.max_depth_seen = new_depth;
        }

        Ok(())
    }

    /// Handles closing of a tag, matching and unwinding the nesting stack.
    pub fn on_element_end(&mut self, tag_name: &str) -> Result<(), HtmlDefenseError> {
        self.total_end_tags = self.total_end_tags.saturating_add(1);
        let trimmed = tag_name.trim().to_ascii_lowercase();

        if Self::is_void_tag(&trimmed) {
            return Ok(());
        }

        if let Some(pos) = self.stack.iter().rposition(|t| t == &trimmed) {
            // Unclosed tags between top of stack and matching pos
            let unclosed_in_scope = self.stack.len().saturating_sub(pos + 1);
            if unclosed_in_scope > 0 {
                self.unclosed_tags_count = self
                    .unclosed_tags_count
                    .saturating_add(unclosed_in_scope);
                if self.unclosed_tags_count > self.max_unclosed_tags {
                    return Err(HtmlDefenseError::UnclosedTagQuotaExceeded {
                        count: self.unclosed_tags_count,
                        max_quota: self.max_unclosed_tags,
                    });
                }
            }
            self.stack.truncate(pos);
        } else {
            // Stray closing tag without matching opening tag
            self.unclosed_tags_count = self.unclosed_tags_count.saturating_add(1);
            if self.unclosed_tags_count > self.max_unclosed_tags {
                return Err(HtmlDefenseError::UnclosedTagQuotaExceeded {
                    count: self.unclosed_tags_count,
                    max_quota: self.max_unclosed_tags,
                });
            }
        }

        Ok(())
    }

    /// Validates unclosed tag count at end of stream.
    pub fn finalize(&mut self) -> Result<TagDepthReport, HtmlDefenseError> {
        let remaining_unclosed = self.stack.len();
        self.unclosed_tags_count = self
            .unclosed_tags_count
            .saturating_add(remaining_unclosed);

        if self.unclosed_tags_count > self.max_unclosed_tags {
            return Err(HtmlDefenseError::UnclosedTagQuotaExceeded {
                count: self.unclosed_tags_count,
                max_quota: self.max_unclosed_tags,
            });
        }

        Ok(TagDepthReport {
            max_depth_reached: self.max_depth_seen,
            total_start_tags: self.total_start_tags,
            total_end_tags: self.total_end_tags,
            unclosed_tags_count: self.unclosed_tags_count,
        })
    }

    /// Returns the current nesting depth on the stack.
    #[inline]
    #[must_use]
    pub fn current_depth(&self) -> usize {
        self.stack.len()
    }

    /// Returns the maximum nesting depth observed so far.
    #[inline]
    #[must_use]
    pub fn max_depth_reached(&self) -> usize {
        self.max_depth_seen
    }

    /// Resets internal stack and counters for a new document.
    pub fn reset(&mut self) {
        self.stack.clear();
        self.max_depth_seen = 0;
        self.unclosed_tags_count = 0;
        self.total_start_tags = 0;
        self.total_end_tags = 0;
    }
}
