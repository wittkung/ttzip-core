// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 4: Attribute Quota & Text Slice Memory Fuse Guard.
//!
//! Enforces deterministic structural complexity and memory ceilings on HTML elements:
//! - Maximum attributes per element <= 128
//! - Maximum single attribute length <= 8 KiB (8,192 bytes)
//! - Maximum cumulative attribute length per element <= 64 KiB (65,536 bytes)
//! - Maximum plain text chunk slice <= 1 MiB (1,048,576 bytes)
//!
//! Defends against quadratic attribute expansion bombs, huge attribute buffer stuffing,
//! and extreme plain-text memory exhaustion vectors.

use super::{
    HtmlDefenseError, DEFAULT_MAX_HTML_ATTRIBUTES_PER_ELEMENT, DEFAULT_MAX_HTML_ATTRIBUTE_LEN,
    DEFAULT_MAX_HTML_TEXT_CHUNK_LEN, DEFAULT_MAX_HTML_TOTAL_ATTRIBUTE_LEN,
};

/// Inspection metrics and report from attribute and text chunk validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttributeQuotaReport {
    /// Total number of attributes validated across the document.
    pub total_attributes_checked: usize,
    /// Maximum attribute count observed on a single element.
    pub max_attributes_in_single_element: usize,
    /// Maximum byte length observed for a single attribute.
    pub max_single_attribute_len: usize,
    /// Maximum cumulative attribute byte length observed on a single element.
    pub max_total_attribute_len_in_element: usize,
    /// Maximum plain text chunk byte length observed.
    pub max_text_chunk_len: usize,
}

/// Guard enforcing structural and size quotas on attributes and plain text chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttributeQuotaGuard {
    max_attributes_per_element: usize,
    max_single_attribute_len: usize,
    max_total_attribute_len: usize,
    max_text_chunk_len: usize,
}

impl Default for AttributeQuotaGuard {
    fn default() -> Self {
        Self {
            max_attributes_per_element: DEFAULT_MAX_HTML_ATTRIBUTES_PER_ELEMENT,
            max_single_attribute_len: DEFAULT_MAX_HTML_ATTRIBUTE_LEN,
            max_total_attribute_len: DEFAULT_MAX_HTML_TOTAL_ATTRIBUTE_LEN,
            max_text_chunk_len: DEFAULT_MAX_HTML_TEXT_CHUNK_LEN,
        }
    }
}

impl AttributeQuotaGuard {
    /// Creates a new attribute quota guard with custom configured limits.
    #[must_use]
    pub const fn new(
        max_attributes_per_element: usize,
        max_single_attribute_len: usize,
        max_total_attribute_len: usize,
        max_text_chunk_len: usize,
    ) -> Self {
        Self {
            max_attributes_per_element,
            max_single_attribute_len,
            max_total_attribute_len,
            max_text_chunk_len,
        }
    }

    /// Validates an entire slice/collection of (name, value) attributes for a single element.
    pub fn validate_element_attributes<K, V>(
        &self,
        attributes: &[(K, V)],
        report: &mut AttributeQuotaReport,
    ) -> Result<(), HtmlDefenseError>
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let count = attributes.len();
        if count > self.max_attributes_per_element {
            return Err(HtmlDefenseError::AttributeCountExceeded {
                count,
                max: self.max_attributes_per_element,
            });
        }

        let mut total_len = 0usize;

        for (name, val) in attributes {
            let name_str = name.as_ref();
            let val_str = val.as_ref();
            let attr_len = name_str.len().saturating_add(val_str.len());

            if attr_len > self.max_single_attribute_len {
                return Err(HtmlDefenseError::AttributeLengthExceeded {
                    len: attr_len,
                    max: self.max_single_attribute_len,
                });
            }

            total_len = total_len.saturating_add(attr_len);
            if total_len > self.max_total_attribute_len {
                return Err(HtmlDefenseError::TotalAttributeLengthExceeded {
                    len: total_len,
                    max: self.max_total_attribute_len,
                });
            }

            report.total_attributes_checked = report.total_attributes_checked.saturating_add(1);
            if attr_len > report.max_single_attribute_len {
                report.max_single_attribute_len = attr_len;
            }
        }

        if count > report.max_attributes_in_single_element {
            report.max_attributes_in_single_element = count;
        }
        if total_len > report.max_total_attribute_len_in_element {
            report.max_total_attribute_len_in_element = total_len;
        }

        Ok(())
    }

    /// Validates a single attribute pair without full element context.
    pub fn validate_single_attribute(
        &self,
        name: &str,
        value: &str,
        report: &mut AttributeQuotaReport,
    ) -> Result<(), HtmlDefenseError> {
        let attr_len = name.len().saturating_add(value.len());
        if attr_len > self.max_single_attribute_len {
            return Err(HtmlDefenseError::AttributeLengthExceeded {
                len: attr_len,
                max: self.max_single_attribute_len,
            });
        }
        report.total_attributes_checked = report.total_attributes_checked.saturating_add(1);
        if attr_len > report.max_single_attribute_len {
            report.max_single_attribute_len = attr_len;
        }
        Ok(())
    }

    /// Validates a plain text chunk slice against the text chunk length limit.
    pub fn validate_text_chunk(
        &self,
        text: &str,
        report: &mut AttributeQuotaReport,
    ) -> Result<(), HtmlDefenseError> {
        self.validate_text_bytes(text.as_bytes(), report)
    }

    /// Validates raw byte slice length of a text chunk.
    pub fn validate_text_bytes(
        &self,
        bytes: &[u8],
        report: &mut AttributeQuotaReport,
    ) -> Result<(), HtmlDefenseError> {
        let len = bytes.len();
        if len > self.max_text_chunk_len {
            return Err(HtmlDefenseError::TextChunkLengthExceeded {
                len,
                max: self.max_text_chunk_len,
            });
        }
        if len > report.max_text_chunk_len {
            report.max_text_chunk_len = len;
        }
        Ok(())
    }

    /// Returns the maximum allowed attributes per single element.
    #[inline]
    #[must_use]
    pub const fn max_attributes_per_element(&self) -> usize {
        self.max_attributes_per_element
    }

    /// Returns the maximum allowed single attribute length in bytes.
    #[inline]
    #[must_use]
    pub const fn max_single_attribute_len(&self) -> usize {
        self.max_single_attribute_len
    }

    /// Returns the maximum allowed cumulative attribute length per element.
    #[inline]
    #[must_use]
    pub const fn max_total_attribute_len(&self) -> usize {
        self.max_total_attribute_len
    }

    /// Returns the maximum allowed text chunk length in bytes.
    #[inline]
    #[must_use]
    pub const fn max_text_chunk_len(&self) -> usize {
        self.max_text_chunk_len
    }
}
