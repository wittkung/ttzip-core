// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 6-Layer Defense-in-Depth HTML Security Pipeline Orchestrator.
//!
//! Coordinates all six defense layers into a unified, high-performance streaming pipeline:
//! 1. [`super::HtmlSanitizerGuard`]: Strips `<script>`, `<iframe>`, `on*` events, and dangerous protocols.
//! 2. [`super::ExternalNetworkSandboxGuard`]: Neutralizes external URLs, rewrites VFS paths, and injects CSP.
//! 3. [`super::TagNestingDepthGuard`]: Enforces depth <= 64 levels and unclosed quota <= 256.
//! 4. [`super::AttributeQuotaGuard`]: Enforces attrs <= 128, single attr <= 8KB, total <= 64KB, text <= 1MB.
//! 5. [`super::HtmlMemoryBudgetGuard`]: Enforces memory <= 64MB watchdog and 50MB truncation banner.
//! 6. [`super::SensitiveHtmlBuffer`]: Memory zeroization on drop and `MADV_DONTDUMP` protection.

use std::cell::RefCell;
use std::rc::Rc;

use lol_html::html_content::ContentType;
use lol_html::{element, text, HtmlRewriter, OutputSink, Settings};

use super::{
    AttributeQuotaGuard, AttributeQuotaReport, ExternalNetworkSandboxGuard, HtmlDefenseError,
    HtmlMemoryBudgetGuard, HtmlSanitizerGuard, NetworkSandboxOptions, NetworkSandboxReport,
    SanitizerReport, SensitiveHtmlBuffer, TagDepthReport, TagNestingDepthGuard,
    DEFAULT_HTML_TRUNCATION_THRESHOLD, DEFAULT_MAX_HTML_ATTRIBUTES_PER_ELEMENT,
    DEFAULT_MAX_HTML_ATTRIBUTE_LEN, DEFAULT_MAX_HTML_DEPTH, DEFAULT_MAX_HTML_MEMORY_BUDGET,
    DEFAULT_MAX_HTML_TEXT_CHUNK_LEN, DEFAULT_MAX_HTML_TOTAL_ATTRIBUTE_LEN,
    DEFAULT_MAX_UNCLOSED_TAGS, DEFAULT_VFS_URI_PREFIX,
};

/// Configuration options for the HTML 6-layer defense pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlDefenseOptions {
    /// Maximum allowable DOM element nesting depth (default: 64).
    pub max_depth: usize,
    /// Maximum allowable unclosed tag count before trip (default: 256).
    pub max_unclosed_tags: usize,
    /// Maximum allowable attributes per single element (default: 128).
    pub max_attributes_per_element: usize,
    /// Maximum allowable single attribute key/value length (default: 8 KiB).
    pub max_single_attribute_len: usize,
    /// Maximum allowable cumulative attribute length per element (default: 64 KiB).
    pub max_total_attribute_len: usize,
    /// Maximum allowable plain text chunk slice length (default: 1 MiB).
    pub max_text_chunk_len: usize,
    /// Hard resident memory budget ceiling (default: 64 MiB).
    pub memory_budget_limit: usize,
    /// Memory threshold for safe preview truncation (default: 50 MiB).
    pub memory_truncation_threshold: usize,
    /// VFS prefix to prepend to internal archive paths (default: `ttzip-vfs://`).
    pub vfs_prefix: String,
    /// Whether to inject the strict CSP `<meta>` tag (default: true).
    pub inject_csp: bool,
    /// Optional custom CSP policy directive string.
    pub custom_csp: Option<String>,
    /// Whether to block and neutralize external remote network URLs (default: true).
    pub block_external_network: bool,
}

impl Default for HtmlDefenseOptions {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_HTML_DEPTH,
            max_unclosed_tags: DEFAULT_MAX_UNCLOSED_TAGS,
            max_attributes_per_element: DEFAULT_MAX_HTML_ATTRIBUTES_PER_ELEMENT,
            max_single_attribute_len: DEFAULT_MAX_HTML_ATTRIBUTE_LEN,
            max_total_attribute_len: DEFAULT_MAX_HTML_TOTAL_ATTRIBUTE_LEN,
            max_text_chunk_len: DEFAULT_MAX_HTML_TEXT_CHUNK_LEN,
            memory_budget_limit: DEFAULT_MAX_HTML_MEMORY_BUDGET,
            memory_truncation_threshold: DEFAULT_HTML_TRUNCATION_THRESHOLD,
            vfs_prefix: DEFAULT_VFS_URI_PREFIX.to_string(),
            inject_csp: true,
            custom_csp: None,
            block_external_network: true,
        }
    }
}

/// Comprehensive audit metrics and report across all 6 defense layers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HtmlDefenseReport {
    /// Active content sanitizer report.
    pub sanitizer: SanitizerReport,
    /// External network sandbox and CSP injection report.
    pub network_sandbox: NetworkSandboxReport,
    /// DOM hierarchy recursion and tag depth report.
    pub tag_depth: TagDepthReport,
    /// Attribute quotas and text chunk report.
    pub attribute_quota: AttributeQuotaReport,
    /// Whether the document was truncated due to the 50 MiB threshold.
    pub was_truncated: bool,
    /// Input document byte length.
    pub input_bytes: usize,
    /// Sanitized output byte length.
    pub output_bytes: usize,
}

/// Result produced by the HTML security pipeline execution.
#[derive(Debug)]
pub struct HtmlSecurityPipelineResult {
    /// Zeroized-on-drop sensitive memory buffer containing sanitized HTML.
    pub sanitized_html: SensitiveHtmlBuffer,
    /// Full audit report detailing all sanitizations, quotas, and depth metrics.
    pub report: HtmlDefenseReport,
}

/// Output sink for streaming `lol_html` rewriter chunks.
struct StreamingBufferSink(Rc<RefCell<Vec<u8>>>);

impl OutputSink for StreamingBufferSink {
    fn handle_chunk(&mut self, chunk: &[u8]) {
        self.0.borrow_mut().extend_from_slice(chunk);
    }
}

/// Unified 6-layer HTML security defense pipeline orchestrator.
#[derive(Debug, Clone)]
pub struct HtmlSecurityPipeline {
    options: HtmlDefenseOptions,
    sanitizer_guard: HtmlSanitizerGuard,
    network_guard: ExternalNetworkSandboxGuard,
    attribute_guard: AttributeQuotaGuard,
    memory_guard: HtmlMemoryBudgetGuard,
}

impl Default for HtmlSecurityPipeline {
    fn default() -> Self {
        Self::new(HtmlDefenseOptions::default())
    }
}

impl HtmlSecurityPipeline {
    /// Creates a new security pipeline with custom options.
    #[must_use]
    pub fn new(options: HtmlDefenseOptions) -> Self {
        let sanitizer_guard = HtmlSanitizerGuard::new();
        let network_guard = ExternalNetworkSandboxGuard::new(NetworkSandboxOptions {
            vfs_prefix: options.vfs_prefix.clone(),
            inject_csp: options.inject_csp,
            custom_csp: options.custom_csp.clone(),
            block_external_network: options.block_external_network,
        });
        let attribute_guard = AttributeQuotaGuard::new(
            options.max_attributes_per_element,
            options.max_single_attribute_len,
            options.max_total_attribute_len,
            options.max_text_chunk_len,
        );
        let memory_guard = HtmlMemoryBudgetGuard::new(
            options.memory_budget_limit,
            options.memory_truncation_threshold,
        );

        Self {
            options,
            sanitizer_guard,
            network_guard,
            attribute_guard,
            memory_guard,
        }
    }

    /// Sanitizes an HTML UTF-8 string through all 6 defense layers.
    pub fn sanitize_html(&self, html_input: &str) -> Result<HtmlSecurityPipelineResult, HtmlDefenseError> {
        let input_bytes = html_input.len();

        // 1. Memory Budget Pre-Flight & Safe Truncation Guard
        let (processed_html, was_truncated) = self.memory_guard.truncate_with_banner(html_input);

        // Reserve memory budget
        let _permit = self.memory_guard.allocate(processed_html.len().saturating_mul(2))?;

        // 2. Setup Audit Reports and Guards
        let sanitizer_report = SanitizerReport::default();
        let network_report = NetworkSandboxReport::default();
        let attribute_report = AttributeQuotaReport::default();
        let mut depth_guard = TagNestingDepthGuard::new(self.options.max_depth, self.options.max_unclosed_tags);

        // 3. Fast pre-scan validation for tag depth & attribute quotas to guarantee hard limits
        let mut pre_scan_attribute_report = AttributeQuotaReport::default();
        self.pre_scan_validate(&processed_html, &mut depth_guard, &mut pre_scan_attribute_report)?;

        // 4. Streaming LOL-HTML Rewriting Pipeline
        let output_vec = Rc::new(RefCell::new(Vec::with_capacity(processed_html.len() + 512)));
        let error_cell: Rc<RefCell<Option<HtmlDefenseError>>> = Rc::new(RefCell::new(None));
        let csp_injected_cell = Rc::new(RefCell::new(false));

        let sanitizer = self.sanitizer_guard.clone();
        let network = self.network_guard.clone();
        let attribute_guard = self.attribute_guard;
        let inject_csp = self.options.inject_csp;
        let csp_tag = self.network_guard.generate_csp_meta_tag();

        let sanitizer_cell = Rc::new(RefCell::new(sanitizer_report));
        let network_cell = Rc::new(RefCell::new(network_report));
        let attribute_cell = Rc::new(RefCell::new(attribute_report));

        let element_handlers = vec![
            // Inject CSP meta tag inside <head> element if present
            element!("head", {
                let csp_injected = Rc::clone(&csp_injected_cell);
                let network_cell = Rc::clone(&network_cell);
                let csp_tag = csp_tag.clone();
                move |el| {
                    if inject_csp && !*csp_injected.borrow() {
                        el.prepend(&csp_tag, ContentType::Html);
                        *csp_injected.borrow_mut() = true;
                        network_cell.borrow_mut().csp_injected = true;
                    }
                    Ok(())
                }
            }),
            // Universal element handler: sanitizes tags, attributes, and rewrites URLs
            element!("*", {
                let sanitizer = sanitizer.clone();
                let network = network.clone();
                let sanitizer_cell = Rc::clone(&sanitizer_cell);
                let network_cell = Rc::clone(&network_cell);
                let attribute_cell = Rc::clone(&attribute_cell);
                let error_cell = Rc::clone(&error_cell);

                move |el| {
                    let tag_name = el.tag_name().to_ascii_lowercase();

                    // Guard 1: Forbidden executable and context hijacking tags
                    if sanitizer.is_forbidden_tag(&tag_name) {
                        el.remove();
                        sanitizer_cell.borrow_mut().stripped_tags_count += 1;
                        return Ok(());
                    }

                    // Guard 1: SVG dangerous sub-elements
                    if sanitizer.is_dangerous_svg_tag(&tag_name) {
                        el.remove();
                        sanitizer_cell.borrow_mut().sanitized_svg_elements += 1;
                        return Ok(());
                    }

                    // Guard 4: Attribute Quotas & Sanitization
                    let mut attrs_to_remove = Vec::new();
                    let mut attrs_to_set = Vec::new();
                    let raw_attrs: Vec<(String, String)> = el
                        .attributes()
                        .iter()
                        .map(|a| (a.name(), a.value()))
                        .collect();

                    if let Err(e) = attribute_guard.validate_element_attributes(
                        &raw_attrs,
                        &mut attribute_cell.borrow_mut(),
                    ) {
                        *error_cell.borrow_mut() = Some(e);
                        return Ok(());
                    }

                    for (name, val) in raw_attrs {
                        let lower_name = name.to_ascii_lowercase();

                        // Guard 1: Strip on* event handlers
                        if HtmlSanitizerGuard::is_event_attribute(&lower_name) {
                            attrs_to_remove.push(name);
                            sanitizer_cell.borrow_mut().stripped_events_count += 1;
                            continue;
                        }

                        // Guard 1: Dangerous protocols in URI attributes
                        if sanitizer.is_uri_attribute(&lower_name) {
                            if sanitizer.is_dangerous_uri(&val) {
                                attrs_to_remove.push(name);
                                sanitizer_cell.borrow_mut().neutralized_protocols_count += 1;
                                continue;
                            }

                            // Guard 2: Network sandbox & VFS rewrite
                            let rewritten = network.sanitize_and_rewrite_uri(&val, &mut network_cell.borrow_mut());
                            if rewritten != val {
                                attrs_to_set.push((name, rewritten));
                            }
                        }
                    }

                    for name in attrs_to_remove {
                        el.remove_attribute(&name);
                    }
                    for (name, val) in attrs_to_set {
                        if let Err(e) = el.set_attribute(&name, &val) {
                            *error_cell.borrow_mut() = Some(HtmlDefenseError::RewriterError(e.to_string()));
                        }
                    }

                    Ok(())
                }
            }),
            // Plain text chunk slice guard
            text!("*", {
                let attribute_guard = attribute_guard;
                let attribute_cell = Rc::clone(&attribute_cell);
                let error_cell = Rc::clone(&error_cell);

                move |t| {
                    if let Err(e) = attribute_guard.validate_text_chunk(t.as_str(), &mut attribute_cell.borrow_mut()) {
                        *error_cell.borrow_mut() = Some(e);
                    }
                    Ok(())
                }
            }),
        ];

        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: element_handlers,
                ..Settings::default()
            },
            StreamingBufferSink(Rc::clone(&output_vec)),
        );

        // Execute streaming rewrite
        rewriter
            .write(processed_html.as_bytes())
            .map_err(|e| HtmlDefenseError::RewriterError(e.to_string()))?;
        rewriter
            .end()
            .map_err(|e| HtmlDefenseError::RewriterError(e.to_string()))?;

        // Check if any error was encountered during rewriter pass
        if let Some(err) = error_cell.borrow_mut().take() {
            return Err(err);
        }

        let output_bytes_vec = Rc::try_unwrap(output_vec)
            .map_err(|_| HtmlDefenseError::RewriterError("Buffer lock contention".to_string()))?
            .into_inner();

        let output_str = String::from_utf8(output_bytes_vec)
            .map_err(|e| HtmlDefenseError::Utf8Error(e.to_string()))?;

        // Finalize network guard CSP injection if document lacked <head>
        let mut final_network_report = Rc::try_unwrap(network_cell)
            .unwrap_or_default()
            .into_inner();
        let final_html = if self.options.inject_csp && !*csp_injected_cell.borrow() {
            self.network_guard
                .inject_csp_header(&output_str, &mut final_network_report)
        } else {
            output_str
        };

        let tag_depth_report = depth_guard.finalize()?;
        let output_len = final_html.len();

        // 6. Wrap in Sensitive Volatile Buffer
        let sensitive_buffer = SensitiveHtmlBuffer::from_string(final_html);

        let report = HtmlDefenseReport {
            sanitizer: Rc::try_unwrap(sanitizer_cell).unwrap_or_default().into_inner(),
            network_sandbox: final_network_report,
            tag_depth: tag_depth_report,
            attribute_quota: Rc::try_unwrap(attribute_cell).unwrap_or_default().into_inner(),
            was_truncated,
            input_bytes,
            output_bytes: output_len,
        };

        Ok(HtmlSecurityPipelineResult {
            sanitized_html: sensitive_buffer,
            report,
        })
    }

    /// Pre-scan validation to enforce tag depth and attribute quotas before streaming rewrite.
    fn pre_scan_validate(
        &self,
        html: &str,
        depth_guard: &mut TagNestingDepthGuard,
        attribute_report: &mut AttributeQuotaReport,
    ) -> Result<(), HtmlDefenseError> {
        let mut i = 0;
        let bytes = html.as_bytes();
        let len = bytes.len();

        while i < len {
            if bytes[i] == b'<' {
                if i + 1 < len && (bytes[i + 1] == b'!' || bytes[i + 1] == b'?') {
                    // Skip comments / doctype / processing instructions
                    while i < len && bytes[i] != b'>' {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    continue;
                }

                let is_closing = i + 1 < len && bytes[i + 1] == b'/';
                let start_tag = if is_closing { i + 2 } else { i + 1 };
                let mut end_tag = start_tag;

                while end_tag < len && bytes[end_tag] != b'>' && !bytes[end_tag].is_ascii_whitespace() && bytes[end_tag] != b'/' {
                    end_tag += 1;
                }

                if let Ok(tag_name) = std::str::from_utf8(&bytes[start_tag..end_tag]) {
                    let trimmed_tag = tag_name.trim();
                    if !trimmed_tag.is_empty() {
                        if is_closing {
                            depth_guard.on_element_end(trimmed_tag)?;
                        } else {
                            let mut is_self_closing = false;
                            // Check until closing '>'
                            let mut tag_scan = end_tag;
                            let mut attr_count = 0usize;
                            let mut total_attr_len = 0usize;

                            while tag_scan < len && bytes[tag_scan] != b'>' {
                                if bytes[tag_scan] == b'/' && tag_scan + 1 < len && bytes[tag_scan + 1] == b'>' {
                                    is_self_closing = true;
                                }
                                // Count attributes roughly in pre-scan
                                if bytes[tag_scan].is_ascii_whitespace() {
                                    while tag_scan < len && bytes[tag_scan].is_ascii_whitespace() {
                                        tag_scan += 1;
                                    }
                                    if tag_scan < len && bytes[tag_scan] != b'>' && bytes[tag_scan] != b'/' {
                                        attr_count = attr_count.saturating_add(1);
                                        if attr_count > self.options.max_attributes_per_element {
                                            return Err(HtmlDefenseError::AttributeCountExceeded {
                                                count: attr_count,
                                                max: self.options.max_attributes_per_element,
                                            });
                                        }

                                        let mut attr_start = tag_scan;
                                        while attr_start < len && bytes[attr_start] != b'=' && bytes[attr_start] != b'>' && !bytes[attr_start].is_ascii_whitespace() {
                                            attr_start += 1;
                                        }
                                        let attr_name_len = attr_start.saturating_sub(tag_scan);
                                        let mut attr_val_len = 0usize;

                                        if attr_start < len && bytes[attr_start] == b'=' {
                                            let mut val_scan = attr_start + 1;
                                            if val_scan < len && (bytes[val_scan] == b'"' || bytes[val_scan] == b'\'') {
                                                let quote = bytes[val_scan];
                                                val_scan += 1;
                                                let val_start = val_scan;
                                                while val_scan < len && bytes[val_scan] != quote {
                                                    val_scan += 1;
                                                }
                                                attr_val_len = val_scan.saturating_sub(val_start);
                                                if val_scan < len {
                                                    val_scan += 1;
                                                }
                                            } else {
                                                let val_start = val_scan;
                                                while val_scan < len && !bytes[val_scan].is_ascii_whitespace() && bytes[val_scan] != b'>' {
                                                    val_scan += 1;
                                                }
                                                attr_val_len = val_scan.saturating_sub(val_start);
                                            }
                                            tag_scan = val_scan;
                                        } else {
                                            tag_scan = attr_start;
                                        }

                                        let attr_len = attr_name_len.saturating_add(attr_val_len);
                                        if attr_len > self.options.max_single_attribute_len {
                                            return Err(HtmlDefenseError::AttributeLengthExceeded {
                                                len: attr_len,
                                                max: self.options.max_single_attribute_len,
                                            });
                                        }
                                        total_attr_len = total_attr_len.saturating_add(attr_len);
                                        if total_attr_len > self.options.max_total_attribute_len {
                                            return Err(HtmlDefenseError::TotalAttributeLengthExceeded {
                                                len: total_attr_len,
                                                max: self.options.max_total_attribute_len,
                                            });
                                        }

                                        attribute_report.total_attributes_checked = attribute_report.total_attributes_checked.saturating_add(1);
                                        if attr_len > attribute_report.max_single_attribute_len {
                                            attribute_report.max_single_attribute_len = attr_len;
                                        }
                                        continue;
                                    }
                                }
                                tag_scan += 1;
                            }

                            if attr_count > attribute_report.max_attributes_in_single_element {
                                attribute_report.max_attributes_in_single_element = attr_count;
                            }
                            if total_attr_len > attribute_report.max_total_attribute_len_in_element {
                                attribute_report.max_total_attribute_len_in_element = total_attr_len;
                            }

                            depth_guard.on_element_start(trimmed_tag, is_self_closing)?;
                        }
                    }
                }
            }
            i += 1;
        }

        Ok(())
    }

    /// Sanitizes raw bytes by parsing as UTF-8.
    pub fn sanitize_bytes(&self, bytes: &[u8]) -> Result<HtmlSecurityPipelineResult, HtmlDefenseError> {
        let s = std::str::from_utf8(bytes).map_err(|e| HtmlDefenseError::Utf8Error(e.to_string()))?;
        self.sanitize_html(s)
    }

    /// Returns the pipeline options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &HtmlDefenseOptions {
        &self.options
    }
}
