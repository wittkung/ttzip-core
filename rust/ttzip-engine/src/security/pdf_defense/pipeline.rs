// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF Unified 6-Layer Security Defense Pipeline.
//!
//! Orchestrates indirect reference cycle protection, stream expansion quota熔断,
//! iterative page tree depth verification, malicious action sandboxing,
//! cryptographic downgrade defense, and sensitive memory zeroization.

use super::{
    ActionPolicy, ActionThreat, EncryptionInspectionReport, EncryptionSecurityPolicy,
    IndirectReferenceCycleGuard, MaliciousActionSandboxGuard, PageTreeDepthGuard,
    PageTreeInspectionResult, PdfDefenseError, PdfEncryptionGuard, SensitivePdfBuffer,
    StreamExpansionQuotaGuard, StreamInspectionResult, DEFAULT_MAX_INDIRECT_DEPTH,
    DEFAULT_MAX_OBJECT_VISITS, DEFAULT_MAX_PAGE_COUNT, DEFAULT_MAX_PAGE_TREE_DEPTH,
    DEFAULT_MAX_SINGLE_STREAM_BYTES, DEFAULT_MAX_STREAM_EXPANSION_RATIO,
    DEFAULT_MAX_TOTAL_STREAM_BYTES,
};

/// Configuration parameters for the PDF security defense pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfSecurityConfig {
    /// Maximum allowable indirect reference recursion depth.
    pub max_indirect_depth: usize,
    /// Maximum cumulative indirect object resolution visits.
    pub max_object_visits: usize,
    /// Maximum allowable uncompressed size for a single stream.
    pub max_single_stream_bytes: usize,
    /// Maximum allowable stream expansion ratio.
    pub max_stream_expansion_ratio: f64,
    /// Maximum allowable cumulative uncompressed stream payload size.
    pub max_total_stream_bytes: usize,
    /// Maximum allowable page tree nesting depth.
    pub max_page_tree_depth: usize,
    /// Maximum allowable total page count per document.
    pub max_page_count: usize,
    /// Active content sandboxing policy.
    pub action_policy: ActionPolicy,
    /// Cryptographic cipher suite and downgrade policy.
    pub encryption_policy: EncryptionSecurityPolicy,
}

impl Default for PdfSecurityConfig {
    fn default() -> Self {
        Self {
            max_indirect_depth: DEFAULT_MAX_INDIRECT_DEPTH,
            max_object_visits: DEFAULT_MAX_OBJECT_VISITS,
            max_single_stream_bytes: DEFAULT_MAX_SINGLE_STREAM_BYTES,
            max_stream_expansion_ratio: DEFAULT_MAX_STREAM_EXPANSION_RATIO,
            max_total_stream_bytes: DEFAULT_MAX_TOTAL_STREAM_BYTES,
            max_page_tree_depth: DEFAULT_MAX_PAGE_TREE_DEPTH,
            max_page_count: DEFAULT_MAX_PAGE_COUNT,
            action_policy: ActionPolicy::RejectAllActiveContent,
            encryption_policy: EncryptionSecurityPolicy::EnforceModernAesOnly,
        }
    }
}

/// Comprehensive report aggregating findings across all 6 defense layers.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfSecurityInspectionReport {
    /// Object reference graph validation statistics.
    pub total_objects_visited: usize,
    /// Stream inspection and decompression statistics.
    pub stream_stats: StreamInspectionResult,
    /// Page tree structure analysis.
    pub page_tree_stats: PageTreeInspectionResult,
    /// Action sandbox threats and security report.
    pub threats_detected: Vec<ActionThreat>,
    /// Document encryption parameters and cipher suite.
    pub encryption_report: EncryptionInspectionReport,
    /// Whether the document passed all 6 defense layers safely.
    pub is_safe: bool,
}

/// Report produced during document sanitization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SanitizationReport {
    /// List of stripped or neutralized action threats.
    pub threats_neutralized: Vec<ActionThreat>,
    /// Whether modifications were applied to the document.
    pub is_modified: bool,
}

/// Composite pipeline executing the 6-layer defense inspection and sanitization.
#[derive(Debug, Clone)]
pub struct PdfSecurityPipeline {
    config: PdfSecurityConfig,
    cycle_guard: IndirectReferenceCycleGuard,
    stream_guard: StreamExpansionQuotaGuard,
    page_tree_guard: PageTreeDepthGuard,
    action_guard: MaliciousActionSandboxGuard,
    encryption_guard: PdfEncryptionGuard,
}

impl Default for PdfSecurityPipeline {
    fn default() -> Self {
        Self::new(PdfSecurityConfig::default())
    }
}

impl PdfSecurityPipeline {
    /// Creates a new pipeline with the provided configuration.
    pub fn new(config: PdfSecurityConfig) -> Self {
        let cycle_guard = IndirectReferenceCycleGuard::with_limits(
            config.max_indirect_depth,
            config.max_object_visits,
        );
        let stream_guard = StreamExpansionQuotaGuard::with_limits(
            config.max_single_stream_bytes,
            config.max_stream_expansion_ratio,
            config.max_total_stream_bytes,
        );
        let page_tree_guard = PageTreeDepthGuard::with_limits(
            config.max_page_tree_depth,
            config.max_page_count,
        );
        let action_guard = MaliciousActionSandboxGuard::new(config.action_policy);
        let encryption_guard = PdfEncryptionGuard::new(config.encryption_policy);

        Self {
            config,
            cycle_guard,
            stream_guard,
            page_tree_guard,
            action_guard,
            encryption_guard,
        }
    }

    /// Returns a reference to the active pipeline configuration.
    pub fn config(&self) -> &PdfSecurityConfig {
        &self.config
    }

    /// Inspects raw PDF bytes by loading into a memory document and passing through all 6 guards.
    pub fn inspect_bytes(&mut self, pdf_bytes: &[u8]) -> Result<PdfSecurityInspectionReport, PdfDefenseError> {
        let doc = lopdf::Document::load_mem(pdf_bytes).map_err(|e| {
            PdfDefenseError::MalformedPdf {
                reason: format!("PDF document parser failed: {e}"),
                offset: None,
            }
        })?;

        self.inspect_document(&doc)
    }

    /// Evaluates an existing `lopdf::Document` through all 6 defense layers.
    pub fn inspect_document(
        &mut self,
        doc: &lopdf::Document,
    ) -> Result<PdfSecurityInspectionReport, PdfDefenseError> {
        // Layer 5: Encryption & Downgrade Defense Check
        let encryption_report = self.encryption_guard.inspect_document(doc)?;

        // Layer 1: Indirect Reference Cycle & Depth Verification
        let total_objects_visited = self.cycle_guard.validate_lopdf_doc(doc)?;

        // Layer 2: Stream Expansion Quota & Decompression Bomb Verification
        let stream_stats = self.stream_guard.inspect_all_streams(doc)?;

        // Layer 3: Page Tree Depth & Iterative Stack Traversal
        let page_tree_stats = self.page_tree_guard.collect_pages_iterative(doc)?;

        // Layer 4: Malicious Action Sandbox Insulation
        let sandbox_report = self.action_guard.inspect_document(doc)?;

        Ok(PdfSecurityInspectionReport {
            total_objects_visited,
            stream_stats,
            page_tree_stats,
            threats_detected: sandbox_report.threats,
            encryption_report,
            is_safe: true,
        })
    }

    /// Sanitizes an existing `lopdf::Document` by removing all hazardous active elements.
    pub fn sanitize_document(
        &self,
        doc: &mut lopdf::Document,
    ) -> Result<SanitizationReport, PdfDefenseError> {
        let sandbox_rep = self.action_guard.sanitize_document(doc)?;
        Ok(SanitizationReport {
            threats_neutralized: sandbox_rep.threats,
            is_modified: sandbox_rep.is_sanitized,
        })
    }

    /// Safely extracts text content into a `SensitivePdfBuffer` with zero-on-drop protection.
    pub fn extract_safe_text(
        &mut self,
        pdf_bytes: &[u8],
        max_pages: Option<u32>,
    ) -> Result<SensitivePdfBuffer, PdfDefenseError> {
        let doc = lopdf::Document::load_mem(pdf_bytes).map_err(|e| {
            PdfDefenseError::MalformedPdf {
                reason: format!("PDF text extraction load error: {e}"),
                offset: None,
            }
        })?;

        // Inspect document security first
        let report = self.inspect_document(&doc)?;
        if report.encryption_report.is_encrypted && !report.encryption_report.is_open_with_empty_password {
            return Err(PdfDefenseError::PasswordRequired {
                reason: "Document is password protected and cannot be extracted without credentials".to_string(),
            });
        }

        let total_pages = report.page_tree_stats.page_count as u32;
        let pages_to_extract = match max_pages {
            Some(n) => n.min(total_pages),
            None => total_pages,
        };

        if pages_to_extract == 0 {
            return Ok(SensitivePdfBuffer::new());
        }

        let page_nums: Vec<u32> = (1..=pages_to_extract).collect();
        let extracted = doc.extract_text(&page_nums).unwrap_or_default();

        Ok(SensitivePdfBuffer::from_str_slice(extracted.trim()))
    }
}
