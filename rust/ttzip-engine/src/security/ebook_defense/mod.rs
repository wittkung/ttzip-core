// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! E-book 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic container insulation, manifest quota fuses, TOC recursion limits,
//! PalmDOC LZ77 bounds checking, active content sandboxing, memory budgets, and sensitive zeroization:
//! 1. **Manifest Item Count Guard** ([`ManifestItemCountGuard`]):
//!    Interception of manifest flooding bombs (items <= 10,000, OPF <= 10MB, DTD entity blocking).
//! 2. **TOC Recursion Depth Guard** ([`TocRecursionDepthGuard`]):
//!    Explicit non-recursive iterative traversal protecting against deep nesting (depth <= 16) and cycles.
//! 3. **PalmDOC Decompress Guard** ([`PalmDocDecompressGuard`]):
//!    Strict sliding window backreference verification (record <= 4,096 bytes) and checked EXTH arithmetic.
//! 4. **E-book Presentation Sandbox Guard** ([`EbookSandboxGuard`]):
//!    Zero-copy tag sanitizer purging `<script>`, `<iframe>`, `on*` events, and dangerous protocols.
//! 5. **E-book Memory Budget Guard** ([`EbookMemoryBudgetGuard`]):
//!    Systemic memory watchdog enforcing <= 64 MB resident task memory and <= 16 MB chapter limits.
//! 6. **Sensitive E-book Buffer** ([`SensitiveEbookBuffer`]):
//!    Zero-allocation / zeroize-on-drop volatile memory wiping for decrypted manuscripts.

mod manifest_count;
mod memory_budget;
mod palmdoc;
mod pipeline;
mod sandbox;
mod sensitive;
mod toc_depth;

#[cfg(test)]
mod tests;

pub use manifest_count::{ManifestItem, ManifestItemCountGuard};
pub use memory_budget::{EbookMemoryBudgetGuard, MemoryPermit};
pub use palmdoc::PalmDocDecompressGuard;
pub use pipeline::{
    EbookSecurityConfig, EbookSecurityPipeline, ManifestInspectionReport, SanitizedContentReport,
    TocInspectionReport,
};
pub use sandbox::{ContentSanitizationReport, EbookSandboxGuard};
pub use sensitive::SensitiveEbookBuffer;
pub use toc_depth::{TocEntry, TocRecursionDepthGuard};

// ============================================================================
// Defense Constants & Limits
// ============================================================================

/// Default maximum allowable manifest items in an OPF file (10,000 items).
pub const MAX_MANIFEST_ITEMS: usize = 10_000;

/// Default maximum allowable uncompressed OPF file size (10 MiB).
pub const MAX_OPF_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Default maximum allowable href attribute string length (1,024 bytes).
pub const MAX_HREF_LENGTH: usize = 1024;

/// Default maximum allowable item ID attribute string length (256 bytes).
pub const MAX_ITEM_ID_LENGTH: usize = 256;

/// Default maximum allowable TOC nesting hierarchy depth (16 levels).
pub const MAX_TOC_DEPTH: usize = 16;

/// Default maximum allowable total navigation nodes in TOC tree (5,000 nodes).
pub const MAX_TOC_NODES: usize = 5_000;

/// Maximum allowable uncompressed size of a single PalmDOC record (4,096 bytes).
pub const PALMDOC_MAX_RECORD_SIZE: usize = 4096;

/// Maximum allowable EXTH records in a MOBI header (2,048 records).
pub const MOBI_EXTH_MAX_RECORDS: usize = 2048;

/// Default maximum allowable global resident memory for an e-book task (64 MiB).
pub const DEFAULT_MAX_GLOBAL_EBOOK_BUDGET: usize = 64 * 1024 * 1024;

/// Default maximum allowable uncompressed size for a single chapter viewport (16 MiB).
pub const DEFAULT_MAX_CHAPTER_VIEWPORT_BUDGET: usize = 16 * 1024 * 1024;

/// Default maximum allowable memory budget for raster image decoding (24 MiB).
pub const DEFAULT_MAX_IMAGE_BUDGET: usize = 24 * 1024 * 1024;

/// Default maximum allowable memory budget for font subsets and CSS cache (8 MiB).
pub const DEFAULT_MAX_FONT_BUDGET: usize = 8 * 1024 * 1024;

/// Default maximum allowable memory budget for parser scratch arena (12 MiB).
pub const DEFAULT_MAX_SCRATCH_BUDGET: usize = 12 * 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when e-book security invariants, memory fuses, or format limits are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EbookDefenseError {
    /// OPF manifest document file size exceeded the configured security limit.
    #[error("OPF manifest file size {size} bytes exceeds security limit {limit} bytes")]
    OpfFileTooLarge { size: u64, limit: u64 },

    /// Total count of manifest items exceeded the safety ceiling.
    #[error("Manifest item count {count} exceeds security ceiling {limit}")]
    ManifestItemCountExceeded { count: usize, limit: usize },

    /// Manifest attribute length exceeded the maximum allowable threshold.
    #[error("Manifest attribute '{attr}' length {len} exceeds security limit {limit}")]
    AttributeLengthExceeded {
        attr: &'static str,
        len: usize,
        limit: usize,
    },

    /// DTD external entity declarations detected (Billion Laughs / XXE prevention).
    #[error("DTD entity declarations and external entities are strictly forbidden")]
    DtdEntitiesForbidden,

    /// Duplicate manifest item identifier detected.
    #[error("Duplicate manifest item ID detected: '{0}'")]
    DuplicateItemId(String),

    /// TOC navigation tree nesting depth exceeded the safety ceiling.
    #[error("TOC hierarchy nesting depth {depth} exceeds security limit {limit}")]
    TocNestingDepthExceeded { depth: usize, limit: usize },

    /// Total navigation nodes in TOC tree exceeded configured maximum.
    #[error("TOC total navigation nodes count {count} exceeds ceiling {limit}")]
    TocTotalNodesExceeded { count: usize, limit: usize },

    /// Circular reference loop detected during TOC graph traversal.
    #[error("TOC cyclic reference loop detected at node: '{node_id}'")]
    TocCyclicReferenceDetected { node_id: String },

    /// Empty navigation label encountered in TOC entry.
    #[error("TOC navigation label cannot be empty")]
    EmptyNavLabel,

    /// PalmDOC LZ77 backreference distance exceeded the currently decoded buffer length.
    #[error("PalmDOC backreference distance {distance} exceeds current decoded buffer length {current_len}")]
    IllegalBackreferenceDistance { distance: usize, current_len: usize },

    /// PalmDOC record uncompressed size exceeded the 4,096-byte hardware record limit.
    #[error("PalmDOC record buffer overflow: attempted {attempted_len} bytes exceeds limit {limit} bytes")]
    RecordBufferOverflow {
        attempted_len: usize,
        limit: usize,
    },

    /// Unexpected premature end of compressed PalmDOC bitstream.
    #[error("Unexpected end of file while parsing compressed PalmDOC stream")]
    UnexpectedEof,

    /// Corrupted PalmDOC bitstream or invalid header structure encountered.
    #[error("Corrupted PalmDOC bitstream: {0}")]
    CorruptedBitstream(String),

    /// Arithmetic integer overflow encountered during MOBI EXTH header calculation.
    #[error("Arithmetic integer overflow encountered while parsing MOBI EXTH records")]
    ExthIntegerOverflow,

    /// EXTH record length exceeded remaining container boundary bytes.
    #[error("EXTH record length {record_len} bytes exceeds remaining buffer {remaining_bytes} bytes")]
    ExthRecordOutOfBounds {
        record_len: usize,
        remaining_bytes: usize,
    },

    /// Global e-book memory budget ceiling exceeded.
    #[error("E-book memory budget exceeded: requested {requested} bytes + allocated {current_allocated} bytes exceeds ceiling {limit} bytes")]
    MemoryBudgetExceeded {
        requested: usize,
        current_allocated: usize,
        limit: usize,
    },

    /// Single chapter uncompressed size exceeded viewport budget limit.
    #[error("Single chapter content size {size} bytes exceeds viewport budget limit {limit} bytes")]
    ChapterExceedsViewportLimit { size: usize, limit: usize },

    /// Malformed XML or broken OPF stream encountered.
    #[error("Malformed e-book XML / OPF syntax: {0}")]
    MalformedXml(String),

    /// Underlying parser or I/O error occurred.
    #[error("Underlying I/O or parser error: {0}")]
    ParserError(String),
}
