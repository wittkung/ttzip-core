// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified 6-Layer E-book Security Pipeline.
//!
//! Orchestrates the full lifecycle defense guards across container ingress,
//! OPF manifest quotas, TOC hierarchy validation, PalmDOC LZ77 decompression,
//! HTML/SVG sandboxing, memory budgets, and sensitive memory zeroization.

use std::io::BufRead;

use super::{
    ContentSanitizationReport, EbookDefenseError, EbookMemoryBudgetGuard, EbookSandboxGuard,
    ManifestItemCountGuard, MemoryPermit, PalmDocDecompressGuard, SensitiveEbookBuffer,
    TocRecursionDepthGuard, DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET, DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
    MAX_HREF_LENGTH, MAX_ITEM_ID_LENGTH, MAX_MANIFEST_ITEMS, MAX_OPF_FILE_SIZE, MAX_TOC_DEPTH,
    MAX_TOC_NODES,
};

/// Configuration parameters for the 6-layer e-book security pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EbookSecurityConfig {
    /// Maximum number of items permitted in the OPF `<manifest>`.
    pub max_manifest_items: usize,
    /// Maximum allowed file size for the OPF document in bytes.
    pub max_opf_file_size: u64,
    /// Maximum allowed length for an item `href` attribute.
    pub max_href_length: usize,
    /// Maximum allowed length for an item `id` attribute.
    pub max_item_id_length: usize,
    /// Maximum allowed hierarchy depth for TOC navigation trees.
    pub max_toc_depth: usize,
    /// Maximum allowed total navigation nodes in the TOC tree.
    pub max_toc_nodes: usize,
    /// Global resident memory budget ceiling in bytes (default: 64 MB).
    pub max_global_memory_budget: usize,
    /// Viewport single-chapter uncompressed size ceiling in bytes (default: 16 MB).
    pub max_chapter_viewport_budget: usize,
    /// Whether to enforce strict active script / tag sanitization.
    pub enforce_strict_sandbox: bool,
}

impl Default for EbookSecurityConfig {
    fn default() -> Self {
        Self {
            max_manifest_items: MAX_MANIFEST_ITEMS,
            max_opf_file_size: MAX_OPF_FILE_SIZE,
            max_href_length: MAX_HREF_LENGTH,
            max_item_id_length: MAX_ITEM_ID_LENGTH,
            max_toc_depth: MAX_TOC_DEPTH,
            max_toc_nodes: MAX_TOC_NODES,
            max_global_memory_budget: DEFAULT_MAX_GLOBAL_EBOOK_BUDGET,
            max_chapter_viewport_budget: DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET,
            enforce_strict_sandbox: true,
        }
    }
}

/// Inspection report emitted after evaluating an OPF manifest document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestInspectionReport {
    /// Number of verified manifest items.
    pub items_count: usize,
    /// Verified OPF stream length in bytes.
    pub opf_bytes: u64,
}

/// Inspection report emitted after evaluating a TOC navigation tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocInspectionReport {
    /// Total number of navigation nodes.
    pub nodes_count: usize,
    /// Maximum observed hierarchy nesting depth.
    pub max_depth: usize,
}

/// Inspection report emitted after evaluating and sanitizing HTML/SVG chapter content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedContentReport {
    /// Original payload length in bytes.
    pub original_bytes: usize,
    /// Sanitized clean payload length in bytes.
    pub sanitized_bytes: usize,
    /// Detailed breakdown of stripped tags and neutralized handlers.
    pub sanitization_report: ContentSanitizationReport,
}

/// Comprehensive 6-layer defense pipeline orchestrator.
#[derive(Debug)]
pub struct EbookSecurityPipeline {
    config: EbookSecurityConfig,
    manifest_guard: ManifestItemCountGuard,
    toc_guard: TocRecursionDepthGuard,
    memory_guard: EbookMemoryBudgetGuard,
}

impl Default for EbookSecurityPipeline {
    fn default() -> Self {
        Self::new(EbookSecurityConfig::default())
    }
}

impl EbookSecurityPipeline {
    /// Creates a new e-book security pipeline with the specified configuration.
    pub fn new(config: EbookSecurityConfig) -> Self {
        let memory_guard = EbookMemoryBudgetGuard::new(
            config.max_global_memory_budget,
            config.max_chapter_viewport_budget,
        );
        Self {
            config,
            manifest_guard: ManifestItemCountGuard::new(),
            toc_guard: TocRecursionDepthGuard::new(),
            memory_guard,
        }
    }

    /// Evaluates and parses an OPF manifest stream through Guard 1.
    pub fn inspect_opf_manifest<R: BufRead>(
        &mut self,
        reader: R,
        stream_len: u64,
    ) -> Result<ManifestInspectionReport, EbookDefenseError> {
        self.manifest_guard.parse_opf_stream(reader, stream_len)?;
        Ok(ManifestInspectionReport {
            items_count: self.manifest_guard.len(),
            opf_bytes: stream_len,
        })
    }

    /// Validates a list of TOC hierarchy entries through Guard 2.
    pub fn validate_toc_hierarchy(
        &mut self,
        entries: &[(String, String, String, usize, Option<usize>)],
    ) -> Result<TocInspectionReport, EbookDefenseError> {
        self.toc_guard.clear();
        let mut max_depth_observed = 0;

        for (id, title, href, depth, parent_idx) in entries {
            self.toc_guard.push_node(
                id.clone(),
                title.clone(),
                href.clone(),
                *depth,
                *parent_idx,
            )?;
            max_depth_observed = max_depth_observed.max(*depth);
        }

        Ok(TocInspectionReport {
            nodes_count: self.toc_guard.len(),
            max_depth: max_depth_observed,
        })
    }

    /// Decompresses a PalmDOC LZ77 record into a zeroized sensitive buffer through Guard 3 and Guard 6.
    pub fn decompress_palmdoc_record(
        &self,
        compressed: &[u8],
    ) -> Result<SensitiveEbookBuffer, EbookDefenseError> {
        let raw_decompressed = PalmDocDecompressGuard::decompress_record(compressed)?;
        Ok(SensitiveEbookBuffer::from_vec(raw_decompressed))
    }

    /// Inspects and sanitizes chapter content through Guard 4.
    pub fn sanitize_chapter_content(
        &self,
        raw_html: &str,
    ) -> Result<(String, SanitizedContentReport), EbookDefenseError> {
        let original_bytes = raw_html.len();
        self.memory_guard.validate_chapter_size(original_bytes)?;

        if self.config.enforce_strict_sandbox {
            let (sanitized, report) = EbookSandboxGuard::sanitize_xhtml_content(raw_html);
            let sanitized_bytes = sanitized.len();
            Ok((
                sanitized,
                SanitizedContentReport {
                    original_bytes,
                    sanitized_bytes,
                    sanitization_report: report,
                },
            ))
        } else {
            Ok((
                raw_html.to_string(),
                SanitizedContentReport {
                    original_bytes,
                    sanitized_bytes: original_bytes,
                    sanitization_report: ContentSanitizationReport::default(),
                },
            ))
        }
    }

    /// Acquires a memory allocation permit through Guard 5.
    pub fn acquire_memory_permit(
        &self,
        size: usize,
    ) -> Result<MemoryPermit<'_>, EbookDefenseError> {
        self.memory_guard.allocate(size)
    }

    /// Returns a reference to the pipeline's active configuration.
    #[inline]
    pub fn config(&self) -> &EbookSecurityConfig {
        &self.config
    }

    /// Returns a reference to the internal manifest guard.
    #[inline]
    pub fn manifest_guard(&self) -> &ManifestItemCountGuard {
        &self.manifest_guard
    }

    /// Returns a reference to the internal TOC guard.
    #[inline]
    pub fn toc_guard(&self) -> &TocRecursionDepthGuard {
        &self.toc_guard
    }

    /// Returns a reference to the internal memory budget guard.
    #[inline]
    pub fn memory_guard(&self) -> &EbookMemoryBudgetGuard {
        &self.memory_guard
    }
}
