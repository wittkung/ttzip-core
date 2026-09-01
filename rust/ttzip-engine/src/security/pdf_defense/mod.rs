// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic parser insulation, memory quota fuses, recursion limits,
//! active content sandboxing, and sensitive memory zeroization:
//! 1. **Indirect Reference Cycle Guard** ([`IndirectReferenceCycleGuard`]):
//!    Interception of cyclic indirect object reference bombs (active ancestor tracking, recursion depth <= 64, object visits <= 100,000).
//! 2. **Stream Expansion Quota Guard** ([`StreamExpansionQuotaGuard`]):
//!    Circuit breaker against PDF Flate/LZW decompression bombs (single stream <= 32MB, expansion ratio <= 200x).
//! 3. **Page Tree Depth Guard** ([`PageTreeDepthGuard`]):
//!    Explicit non-recursive iterative stack traversal protecting against degenerate page tree nesting (depth <= 32).
//! 4. **Malicious Action Sandbox Guard** ([`MaliciousActionSandboxGuard`]):
//!    Complete insulation against malicious JavaScript, OS command launch, dangerous URI schemes, and form exfiltration.
//! 5. **PDF Encryption Guard** ([`PdfEncryptionGuard`]):
//!    Cipher suite classification (Standard RC4, AES-128, AES-256), cryptographic downgrade attack prevention, and constant-time password probing.
//! 6. **Sensitive PDF Buffer** ([`SensitivePdfBuffer`]):
//!    Zero-allocation / zeroize-on-drop volatile memory protection for passwords, decrypted streams, and confidential text.

mod action_sandbox;
mod cycle_guard;
mod encryption;
mod page_tree;
mod pipeline;
mod sensitive;
mod stream_quota;

#[cfg(test)]
mod tests;

pub use action_sandbox::{
    ActionPolicy, ActionThreat, ActionThreatLevel, MaliciousActionSandboxGuard, SandboxReport,
};
pub use cycle_guard::{ActiveReferenceScope, IndirectReferenceCycleGuard};
pub use encryption::{
    CipherSuite, EncryptionInspectionReport, EncryptionSecurityPolicy, PdfEncryptionGuard,
    PDF_STANDARD_PASSWORD_PADDING,
};
pub use page_tree::{PageTreeDepthGuard, PageTreeInspectionResult, PageTreeNode};
pub use pipeline::{PdfSecurityConfig, PdfSecurityInspectionReport, PdfSecurityPipeline, SanitizationReport};
pub use sensitive::SensitivePdfBuffer;
pub use stream_quota::{StreamExpansionQuotaGuard, StreamInspectionResult};

// ============================================================================
// Defense Constants & Limits
// ============================================================================

/// Default maximum allowable indirect reference recursion depth (64 levels).
pub const DEFAULT_MAX_INDIRECT_DEPTH: usize = 64;

/// Default maximum allowable cumulative indirect object visits per document (100,000 objects).
pub const DEFAULT_MAX_OBJECT_VISITS: usize = 100_000;

/// Default maximum allowable uncompressed single stream payload size (32 MiB).
pub const DEFAULT_MAX_SINGLE_STREAM_BYTES: usize = 32 * 1024 * 1024;

/// Default maximum allowable ratio of uncompressed bytes to compressed stream size (200.0x).
pub const DEFAULT_MAX_STREAM_EXPANSION_RATIO: f64 = 200.0;

/// Default maximum allowable cumulative uncompressed stream payload size (128 MiB).
pub const DEFAULT_MAX_TOTAL_STREAM_BYTES: usize = 128 * 1024 * 1024;

/// Default maximum allowable page tree nesting depth (32 levels).
pub const DEFAULT_MAX_PAGE_TREE_DEPTH: usize = 32;

/// Default maximum allowable total page count per document (100,000 pages).
pub const DEFAULT_MAX_PAGE_COUNT: usize = 100_000;

/// Default maximum allowable active action definitions per document (1024 actions).
pub const DEFAULT_MAX_MALICIOUS_ACTIONS: usize = 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when PDF security invariants, memory fuses, or format guards are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PdfDefenseError {
    /// Cyclic indirect object reference detected during graph traversal.
    #[error("Cyclic indirect object reference detected at object ({obj_id:?}): path chain: {path}")]
    CycleDetected {
        obj_id: (u32, u16),
        path: String,
    },

    /// Indirect object reference recursion depth exceeded configured safety limit.
    #[error("Indirect reference recursion depth limit exceeded ({depth} > {max_depth})")]
    MaxRecursionDepthExceeded {
        depth: usize,
        max_depth: usize,
    },

    /// Cumulative object resolution count exceeded maximum budget.
    #[error("Indirect object visit count quota exceeded ({count} > {max_count})")]
    ObjectCountExceeded {
        count: usize,
        max_count: usize,
    },

    /// Uncompressed stream payload exceeded maximum allowable size limit.
    #[error("PDF stream size quota exceeded ({size} bytes > {max_size} bytes)")]
    StreamSizeExceeded {
        size: usize,
        max_size: usize,
    },

    /// Stream decompression expansion ratio exceeded safety threshold.
    #[error("PDF stream expansion ratio exceeded ({ratio:.2}x > {max_ratio:.2}x, {compressed} compressed -> {uncompressed} uncompressed bytes)")]
    StreamExpansionRatioExceeded {
        ratio: f64,
        max_ratio: f64,
        compressed: usize,
        uncompressed: usize,
    },

    /// Cumulative uncompressed streams volume exceeded overall document memory budget.
    #[error("Cumulative PDF stream bytes quota exceeded ({total_bytes} bytes > {max_bytes} bytes)")]
    TotalStreamBytesExceeded {
        total_bytes: usize,
        max_bytes: usize,
    },

    /// Page tree nesting depth exceeded safety ceiling.
    #[error("Page tree hierarchy depth exceeded limit ({depth} > {max_depth})")]
    PageTreeDepthExceeded {
        depth: usize,
        max_depth: usize,
    },

    /// Total page count in page tree exceeded configured safety limit.
    #[error("Document page count exceeded limit ({count} > {max_count})")]
    PageCountExceeded {
        count: usize,
        max_count: usize,
    },

    /// Active malicious action or forbidden executable content detected.
    #[error("Malicious action detected: {action_type} - {details}")]
    MaliciousActionDetected {
        action_type: String,
        details: String,
    },

    /// Insecure or downgraded encryption configuration detected.
    #[error("Insecure PDF encryption detected (Filter: {filter}, Algorithm: {algorithm}): {reason}")]
    InsecureEncryptionDetected {
        filter: String,
        algorithm: String,
        reason: String,
    },

    /// Cryptographic decryption or key validation failed.
    #[error("PDF decryption failed: {reason}")]
    DecryptionFailed {
        reason: String,
    },

    /// Document is encrypted and requires a valid password.
    #[error("PDF document is encrypted and requires password: {reason}")]
    PasswordRequired {
        reason: String,
    },

    /// Sensitive memory buffer zeroize operation failed.
    #[error("Sensitive PDF buffer zeroize failure: {reason}")]
    ZeroizeFailed {
        reason: String,
    },

    /// Malformed PDF syntax or corrupted object structure encountered.
    #[error("Malformed PDF structure: {reason}")]
    MalformedPdf {
        reason: String,
        offset: Option<usize>,
    },

    /// Underlying PDF parser error occurred.
    #[error("PDF parser error: {0}")]
    ParserError(String),
}
