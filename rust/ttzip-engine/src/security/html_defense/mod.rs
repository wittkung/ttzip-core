// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! HTML 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic parser insulation, active XSS stripping, network sandboxing,
//! nesting depth ceilings, attribute quotas, memory budget watchdogs, and volatile memory zeroization:
//!
//! 1. **Active Content Sanitizer** ([`HtmlSanitizerGuard`]):
//!    Purges `<script>`, `<iframe>`, `<object>`, `<embed>`, `<base>`, `<meta>`, `<foreignObject>`,
//!    strips all `on*` inline DOM event handlers, and neutralizes `javascript:` pseudo-protocols.
//! 2. **External Network Sandbox & CSP Injection** ([`ExternalNetworkSandboxGuard`]):
//!    Neutralizes external `http://`, `https://`, `//` remote URLs and injects strict air-gapped CSP headers.
//! 3. **Tag Nesting Depth Guard** ([`TagNestingDepthGuard`]):
//!    Enforces DOM element nesting recursion bounds (<= 64 levels) and unclosed tag quotas (<= 256).
//! 4. **Attribute Quota & Text Slice Memory Fuse** ([`AttributeQuotaGuard`]):
//!    Memory fuses for attributes (<= 128 attrs, <= 8KB per attr, <= 64KB total) and text chunks (<= 1MB).
//! 5. **HTML Resident Memory Budget Watchdog** ([`HtmlMemoryBudgetGuard`]):
//!    Task resident memory ceiling (<= 64MB) and 50MB safe preview truncation warning banner.
//! 6. **Sensitive HTML Memory Zeroize Guard** ([`SensitiveHtmlBuffer`]):
//!    Zeroize-on-drop volatile memory wiping and `madvise(MADV_DONTDUMP)` protection.

mod attribute_quota;
mod memory_budget;
mod network_sandbox;
mod pipeline;
mod sanitizer;
mod sensitive;
mod tag_depth;

#[cfg(test)]
mod tests;

pub use attribute_quota::{AttributeQuotaGuard, AttributeQuotaReport};
pub use memory_budget::{
    HtmlMemoryBudgetGuard, HtmlMemoryPermit, HTML_TRUNCATION_BANNER,
};
pub use network_sandbox::{
    ExternalNetworkSandboxGuard, NetworkSandboxOptions, NetworkSandboxReport,
    DEFAULT_STRICT_CSP_CONTENT, DEFAULT_VFS_URI_PREFIX,
};
pub use pipeline::{
    HtmlDefenseOptions, HtmlDefenseReport, HtmlSecurityPipeline, HtmlSecurityPipelineResult,
};
pub use sanitizer::{HtmlSanitizerGuard, SanitizerReport};
pub use sensitive::SensitiveHtmlBuffer;
pub use tag_depth::{TagDepthReport, TagNestingDepthGuard};

/// Default maximum allowable DOM element nesting depth (64 levels).
pub const DEFAULT_MAX_HTML_DEPTH: usize = 64;

/// Default maximum allowable unclosed tags before quota trip (256 tags).
pub const DEFAULT_MAX_UNCLOSED_TAGS: usize = 256;

/// Default maximum allowable attributes per single HTML element (128 attributes).
pub const DEFAULT_MAX_HTML_ATTRIBUTES_PER_ELEMENT: usize = 128;

/// Default maximum allowable single attribute key/value length (8 KiB).
pub const DEFAULT_MAX_HTML_ATTRIBUTE_LEN: usize = 8 * 1024;

/// Default maximum allowable cumulative attribute length per element (64 KiB).
pub const DEFAULT_MAX_HTML_TOTAL_ATTRIBUTE_LEN: usize = 64 * 1024;

/// Default maximum allowable plain text chunk slice length (1 MiB).
pub const DEFAULT_MAX_HTML_TEXT_CHUNK_LEN: usize = 1024 * 1024;

/// Default hard resident memory budget ceiling (64 MiB).
pub const DEFAULT_MAX_HTML_MEMORY_BUDGET: usize = 64 * 1024 * 1024;

/// Default threshold for safe preview truncation (50 MiB).
pub const DEFAULT_HTML_TRUNCATION_THRESHOLD: usize = 50 * 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when HTML security invariants, fuses, or parsing quotas are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HtmlDefenseError {
    /// Element hierarchy nesting depth exceeded recursion safeguard ceiling.
    #[error("HTML maximum element nesting depth exceeded ({depth} > {max_depth})")]
    TagDepthLimitExceeded { depth: usize, max_depth: usize },

    /// Cumulative unclosed tag count exceeded safety quota.
    #[error("HTML unclosed tag quota exceeded ({count} > {max_quota})")]
    UnclosedTagQuotaExceeded { count: usize, max_quota: usize },

    /// Attribute count on a single element exceeded structural complexity limit.
    #[error("HTML attribute count per element exceeded ({count} > {max})")]
    AttributeCountExceeded { count: usize, max: usize },

    /// Single attribute name or value length exceeded memory safety limit.
    #[error("HTML attribute length exceeded ({len} bytes > {max} bytes)")]
    AttributeLengthExceeded { len: usize, max: usize },

    /// Cumulative attribute length for an element exceeded safety budget.
    #[error("HTML cumulative element attribute length exceeded ({len} bytes > {max} bytes)")]
    TotalAttributeLengthExceeded { len: usize, max: usize },

    /// Plain text chunk length exceeded slice budget ceiling.
    #[error("HTML text chunk slice length exceeded ({len} bytes > {max} bytes)")]
    TextChunkLengthExceeded { len: usize, max: usize },

    /// Task resident memory budget exceeded configured quota.
    #[error("HTML memory budget exceeded: requested {requested} bytes, currently allocated {current_allocated} bytes, ceiling {limit} bytes")]
    MemoryBudgetExceeded {
        requested: usize,
        current_allocated: usize,
        limit: usize,
    },

    /// Streaming rewriter internal transformation error.
    #[error("HTML streaming rewriter error: {0}")]
    RewriterError(String),

    /// UTF-8 encoding or decoding error encountered.
    #[error("HTML UTF-8 encoding error: {0}")]
    Utf8Error(String),

    /// Underlying I/O error occurred during stream processing.
    #[error("HTML stream I/O error: {0}")]
    IoError(String),
}
