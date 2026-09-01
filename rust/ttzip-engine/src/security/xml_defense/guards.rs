// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XML security guards: XXE isolation, entity expansion quotas, recursion depth, and memory fuses.

use quick_xml::events::Event;

use super::{
    XmlDefenseError, DEFAULT_MAX_ATTRIBUTE_LEN, DEFAULT_MAX_ATTRIBUTES_PER_ELEMENT,
    DEFAULT_MAX_CDATA_LEN, DEFAULT_MAX_ENTITY_EXPANSIONS, DEFAULT_MAX_EXPANDED_BYTES,
    DEFAULT_MAX_XML_DEPTH, DEFAULT_XML_MAX_EXPANSION_RATIO,
};

// ============================================================================
// 1. XXE External Entity Guard
// ============================================================================

/// Physical insulation guard against XML External Entity (XXE) vulnerabilities and DTD exploits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XxeExternalEntityGuard;

impl XxeExternalEntityGuard {
    /// Scans raw XML bytes for prohibited DTD, external entity, or external URI patterns.
    pub fn scan_for_xxe(xml_bytes: &[u8]) -> Result<(), XmlDefenseError> {
        let xml_str = match std::str::from_utf8(xml_bytes) {
            Ok(s) => s,
            Err(_) => {
                return Self::scan_raw_bytes_for_xxe(xml_bytes);
            }
        };
        Self::scan_str_for_xxe(xml_str)
    }

    /// Instance method forwarding to `scan_for_xxe`.
    pub fn scan(&self, xml_bytes: &[u8]) -> Result<(), XmlDefenseError> {
        Self::scan_for_xxe(xml_bytes)
    }

    /// Scans a UTF-8 XML string slice for dangerous entity declarations and external schemas.
    pub fn scan_str_for_xxe(xml: &str) -> Result<(), XmlDefenseError> {
        let lower = xml.to_ascii_lowercase();

        // 1. Check for external DTD declarations
        if let Some(doctype_pos) = lower.find("<!doctype") {
            let rest = &lower[doctype_pos..];
            if let Some(end_bracket) = rest.find('>') {
                let doctype_content = &rest[..end_bracket];
                if doctype_content.contains("system") || doctype_content.contains("public") {
                    return Err(XmlDefenseError::XxeViolation {
                        reason: "External DOCTYPE with SYSTEM or PUBLIC identifier rejected".to_string(),
                    });
                }
            }
        }

        // 2. Check for inline entity declarations
        if lower.contains("<!entity") {
            if lower.contains("system") {
                return Err(XmlDefenseError::XxeViolation {
                    reason: "External SYSTEM entity declaration detected".to_string(),
                });
            }
            if lower.contains("public") {
                return Err(XmlDefenseError::XxeViolation {
                    reason: "External PUBLIC entity declaration detected".to_string(),
                });
            }
            if lower.contains("%") {
                return Err(XmlDefenseError::XxeViolation {
                    reason: "Parameter entity declaration detected in DTD subset".to_string(),
                });
            }
        }

        // 3. Check for dangerous URI schemes in suspicious contexts
        for scheme in &["file://", "expect://", "gopher://", "php://", "dict://", "ftp://"] {
            if lower.contains(scheme) {
                return Err(XmlDefenseError::XxeViolation {
                    reason: format!("Dangerous external URI scheme '{scheme}' detected in XML source"),
                });
            }
        }

        Ok(())
    }

    /// Direct byte scanner for non-UTF8 or raw streams.
    fn scan_raw_bytes_for_xxe(bytes: &[u8]) -> Result<(), XmlDefenseError> {
        let dangerous_keywords = [
            b"<!doctype".as_slice(),
            b"<!entity".as_slice(),
            b"system".as_slice(),
            b"public".as_slice(),
            b"file://".as_slice(),
            b"expect://".as_slice(),
            b"gopher://".as_slice(),
            b"php://".as_slice(),
        ];

        let lower: Vec<u8> = bytes.iter().map(|b| b.to_ascii_lowercase()).collect();
        for &kw in &dangerous_keywords {
            if lower.windows(kw.len()).any(|w| w == kw) {
                return Err(XmlDefenseError::XxeViolation {
                    reason: format!(
                        "Potentially hazardous XXE token '{}' detected in raw stream",
                        String::from_utf8_lossy(kw)
                    ),
                });
            }
        }
        Ok(())
    }

    /// Validates a parsed DOCTYPE event string against external references.
    pub fn sanitize_doctype(doctype_text: &str) -> Result<(), XmlDefenseError> {
        let lower = doctype_text.to_ascii_lowercase();
        if lower.contains("system") || lower.contains("public") || lower.contains("<!entity") {
            return Err(XmlDefenseError::XxeViolation {
                reason: format!("Prohibited DTD construct in DOCTYPE: {doctype_text}"),
            });
        }
        Ok(())
    }

    /// Inspects a `quick_xml` event for XXE-related constructs.
    pub fn inspect_event(event: &Event<'_>) -> Result<(), XmlDefenseError> {
        match event {
            Event::DocType(e) => {
                let text = String::from_utf8_lossy(e.as_ref());
                Self::sanitize_doctype(&text)?;
            }
            Event::PI(e) => {
                let pi_text = String::from_utf8_lossy(e.as_ref()).to_ascii_lowercase();
                if pi_text.contains("xml-stylesheet") && (pi_text.contains("http://") || pi_text.contains("https://") || pi_text.contains("file://")) {
                    return Err(XmlDefenseError::XxeViolation {
                        reason: "External stylesheet reference in processing instruction rejected".to_string(),
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Returns `true` if a given URI string targets hazardous external schemes.
    #[must_use]
    pub fn is_dangerous_system_id(system_id: &str) -> bool {
        let lower = system_id.to_ascii_lowercase();
        lower.starts_with("file:")
            || lower.starts_with("http:")
            || lower.starts_with("https:")
            || lower.starts_with("ftp:")
            || lower.starts_with("gopher:")
            || lower.starts_with("expect:")
            || lower.starts_with("php:")
            || lower.starts_with("dict:")
    }
}

// ============================================================================
// 2. Entity Expansion Quota Guard
// ============================================================================

/// Quota circuit breaker tracking entity expansion count, expanded byte size, and expansion ratio.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityExpansionQuotaGuard {
    expansion_count: usize,
    total_expanded_bytes: usize,
    input_bytes: usize,
    max_expansions: usize,
    max_expanded_bytes: usize,
    max_expansion_ratio: f64,
}

impl Default for EntityExpansionQuotaGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityExpansionQuotaGuard {
    /// Creates a new quota guard with default safety limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            expansion_count: 0,
            total_expanded_bytes: 0,
            input_bytes: 0,
            max_expansions: DEFAULT_MAX_ENTITY_EXPANSIONS,
            max_expanded_bytes: DEFAULT_MAX_EXPANDED_BYTES,
            max_expansion_ratio: DEFAULT_XML_MAX_EXPANSION_RATIO,
        }
    }

    /// Creates a new quota guard with explicit custom limits.
    #[must_use]
    pub fn with_limits(max_expansions: usize, max_expanded_bytes: usize, max_expansion_ratio: f64) -> Self {
        Self {
            expansion_count: 0,
            total_expanded_bytes: 0,
            input_bytes: 0,
            max_expansions,
            max_expanded_bytes,
            max_expansion_ratio,
        }
    }

    /// Records initial input byte count for ratio calculation.
    pub fn record_input_bytes(&mut self, bytes: usize) {
        self.input_bytes = self.input_bytes.saturating_add(bytes);
    }

    /// Records an entity expansion and verifies quotas.
    pub fn record_expansion(&mut self, _entity_name: &str, expanded_bytes: usize) -> Result<(), XmlDefenseError> {
        self.expansion_count = self.expansion_count.saturating_add(1);
        if self.expansion_count > self.max_expansions {
            return Err(XmlDefenseError::EntityExpansionLimitExceeded {
                count: self.expansion_count,
                max: self.max_expansions,
            });
        }

        self.total_expanded_bytes = self.total_expanded_bytes.saturating_add(expanded_bytes);
        if self.total_expanded_bytes > self.max_expanded_bytes {
            return Err(XmlDefenseError::ExpansionBytesExceeded {
                bytes: self.total_expanded_bytes,
                max: self.max_expanded_bytes,
            });
        }

        self.check_expansion_ratio()?;
        Ok(())
    }

    /// Evaluates current expansion ratio against safety threshold.
    pub fn check_expansion_ratio(&self) -> Result<(), XmlDefenseError> {
        if self.input_bytes > 0 && self.total_expanded_bytes > 1024 {
            let ratio = (self.total_expanded_bytes as f64) / (self.input_bytes as f64);
            if ratio > self.max_expansion_ratio {
                return Err(XmlDefenseError::ExpansionRatioExceeded {
                    ratio,
                    max: self.max_expansion_ratio,
                });
            }
        }
        Ok(())
    }

    /// Returns cumulative number of entity expansions processed.
    #[must_use]
    pub fn expansion_count(&self) -> usize {
        self.expansion_count
    }

    /// Returns cumulative total expanded bytes.
    #[must_use]
    pub fn total_expanded_bytes(&self) -> usize {
        self.total_expanded_bytes
    }

    /// Resets all internal counters.
    pub fn reset(&mut self) {
        self.expansion_count = 0;
        self.total_expanded_bytes = 0;
        self.input_bytes = 0;
    }
}

// ============================================================================
// 3. Maximum Depth Guard
// ============================================================================

/// Tracks and restricts XML element nesting depth to prevent stack overflow attacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaxDepthGuard {
    current_depth: usize,
    max_depth: usize,
    tag_stack: Vec<String>,
}

impl Default for MaxDepthGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl MaxDepthGuard {
    /// Creates a new depth guard with default maximum depth of 64 levels.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_depth: 0,
            max_depth: DEFAULT_MAX_XML_DEPTH,
            tag_stack: Vec::with_capacity(32),
        }
    }

    /// Creates a new depth guard with a custom maximum depth.
    #[must_use]
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            current_depth: 0,
            max_depth,
            tag_stack: Vec::with_capacity(max_depth.min(128)),
        }
    }

    /// Pushes an element tag name onto the stack and verifies nesting limits.
    pub fn push_element(&mut self, tag: &str) -> Result<usize, XmlDefenseError> {
        let new_depth = self.current_depth.saturating_add(1);
        if new_depth > self.max_depth {
            return Err(XmlDefenseError::MaxDepthExceeded {
                depth: new_depth,
                max_depth: self.max_depth,
            });
        }
        self.current_depth = new_depth;
        self.tag_stack.push(tag.to_string());
        Ok(self.current_depth)
    }

    /// Pops an element from the stack, matching tag name if provided.
    pub fn pop_element(&mut self, _tag: &str) -> Result<usize, XmlDefenseError> {
        if self.current_depth == 0 {
            return Ok(0);
        }
        self.tag_stack.pop();
        self.current_depth = self.current_depth.saturating_sub(1);
        Ok(self.current_depth)
    }

    /// Returns current nesting depth.
    #[must_use]
    pub fn current_depth(&self) -> usize {
        self.current_depth
    }

    /// Returns an immutable reference to the tag hierarchy stack.
    #[must_use]
    pub fn tag_stack(&self) -> &[String] {
        &self.tag_stack
    }

    /// Resets depth and clears tag stack.
    pub fn reset(&mut self) {
        self.current_depth = 0;
        self.tag_stack.clear();
    }
}

// ============================================================================
// 4. Attribute & CDATA Memory Fuse Guard
// ============================================================================

/// Memory circuit breaker restricting attribute dimensions, counts, and CDATA payload sizes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeAndCDataFuseGuard {
    max_attribute_len: usize,
    max_attributes_per_element: usize,
    max_cdata_len: usize,
}

impl Default for AttributeAndCDataFuseGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl AttributeAndCDataFuseGuard {
    /// Creates a new fuse guard with standard security limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_attribute_len: DEFAULT_MAX_ATTRIBUTE_LEN,
            max_attributes_per_element: DEFAULT_MAX_ATTRIBUTES_PER_ELEMENT,
            max_cdata_len: DEFAULT_MAX_CDATA_LEN,
        }
    }

    /// Creates a new fuse guard with custom parameters.
    #[must_use]
    pub fn with_limits(
        max_attribute_len: usize,
        max_attributes_per_element: usize,
        max_cdata_len: usize,
    ) -> Self {
        Self {
            max_attribute_len,
            max_attributes_per_element,
            max_cdata_len,
        }
    }

    /// Inspects an individual attribute key and value slice length.
    pub fn inspect_attribute(&self, key: &[u8], value: &[u8]) -> Result<(), XmlDefenseError> {
        let total_len = key.len().saturating_add(value.len());
        if total_len > self.max_attribute_len {
            return Err(XmlDefenseError::AttributeLengthExceeded {
                len: total_len,
                max: self.max_attribute_len,
            });
        }
        Ok(())
    }

    /// Inspects total attribute count for a single XML element.
    pub fn inspect_attribute_count(&self, count: usize) -> Result<(), XmlDefenseError> {
        if count > self.max_attributes_per_element {
            return Err(XmlDefenseError::AttributeCountExceeded {
                count,
                max: self.max_attributes_per_element,
            });
        }
        Ok(())
    }

    /// Inspects CDATA payload byte length against configured ceiling.
    pub fn inspect_cdata(&self, cdata_bytes: &[u8]) -> Result<(), XmlDefenseError> {
        self.inspect_cdata_len(cdata_bytes.len())
    }

    /// Direct byte length check for CDATA blocks.
    pub fn inspect_cdata_len(&self, len: usize) -> Result<(), XmlDefenseError> {
        if len > self.max_cdata_len {
            return Err(XmlDefenseError::CDataLengthExceeded {
                len,
                max: self.max_cdata_len,
            });
        }
        Ok(())
    }
}
