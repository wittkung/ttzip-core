// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! XML 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic parser insulation, memory quota fuses, and recursion limits:
//! 1. **XXE External Entity Guard** ([`XxeExternalEntityGuard`]):
//!    Physical insulation against XML External Entity attacks, DTD injection, and external URIs.
//! 2. **Entity Expansion Quota Guard** ([`EntityExpansionQuotaGuard`]):
//!    Billion Laughs / 1032x quadratic expansion bomb interception and expansion quota circuit breaker.
//! 3. **Maximum Depth Guard** ([`MaxDepthGuard`]):
//!    Extreme element nesting recursion interception (depth <= 64 levels).
//! 4. **Attribute and CDATA Fuse Guard** ([`AttributeAndCDataFuseGuard`]):
//!    Memory fuse for oversized attributes (<= 64KB, <= 1024 attrs) and CDATA sections (<= 16MB).
//! 5. **Malformed Stream Recovery Guard** ([`MalformedStreamRecoveryGuard`]):
//!    Self-healing, safe escaping, and sanitization for truncated or malformed XML streams.
//! 6. **Sensitive XML Buffer** ([`SensitiveXmlBuffer`]):
//!    Zero-allocation / zeroize-on-drop memory erasure for credentials and secret XML metadata.

pub mod guards;
pub mod pipeline;
pub mod recovery;

#[cfg(test)]
mod tests;

pub use guards::{
    AttributeAndCDataFuseGuard, EntityExpansionQuotaGuard, MaxDepthGuard, XxeExternalEntityGuard,
};
pub use pipeline::XmlSecurityPipeline;
pub use recovery::{MalformedStreamRecoveryGuard, SensitiveXmlBuffer};

/// Default maximum allowable element nesting depth (64 levels).
pub const DEFAULT_MAX_XML_DEPTH: usize = 64;
/// Default maximum allowable entity expansions before trip (1000 times).
pub const DEFAULT_MAX_ENTITY_EXPANSIONS: usize = 1000;
/// Default maximum allowable cumulative expanded bytes (16 MiB).
pub const DEFAULT_MAX_EXPANDED_BYTES: usize = 16 * 1024 * 1024;
/// Default maximum allowable expansion ratio (10.0x input size).
pub const DEFAULT_XML_MAX_EXPANSION_RATIO: f64 = 10.0;
/// Default maximum allowable single attribute key/value length (64 KiB).
pub const DEFAULT_MAX_ATTRIBUTE_LEN: usize = 64 * 1024;
/// Default maximum allowable attribute count per single XML element (1024 attributes).
pub const DEFAULT_MAX_ATTRIBUTES_PER_ELEMENT: usize = 1024;
/// Default maximum allowable CDATA section payload length (16 MiB).
pub const DEFAULT_MAX_CDATA_LEN: usize = 16 * 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when XML security invariants, fuses, or parsing quotas are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum XmlDefenseError {
    /// XXE injection attempt or forbidden DTD external entity declaration detected.
    #[error("XXE injection attempt detected: {reason}")]
    XxeViolation { reason: String },

    /// Entity expansion count exceeded configured safety quota.
    #[error("XML entity expansion count quota exceeded ({count} > {max})")]
    EntityExpansionLimitExceeded { count: usize, max: usize },

    /// Total expanded byte volume exceeded memory circuit breaker limit.
    #[error("XML expanded bytes quota exceeded ({bytes} bytes > {max} bytes)")]
    ExpansionBytesExceeded { bytes: usize, max: usize },

    /// Overall entity expansion ratio exceeded safety factor threshold.
    #[error("XML expansion ratio exceeded ({ratio:.2}x > {max:.2}x)")]
    ExpansionRatioExceeded { ratio: f64, max: f64 },

    /// Element hierarchy nesting depth exceeded recursion safeguard ceiling.
    #[error("XML maximum element nesting depth exceeded ({depth} > {max_depth})")]
    MaxDepthExceeded { depth: usize, max_depth: usize },

    /// Single attribute name or value length exceeded memory safety budget.
    #[error("XML attribute length exceeded ({len} bytes > {max} bytes)")]
    AttributeLengthExceeded { len: usize, max: usize },

    /// Total attributes per element exceeded structural complexity limit.
    #[error("XML attribute count per element exceeded ({count} > {max})")]
    AttributeCountExceeded { count: usize, max: usize },

    /// CDATA payload length exceeded allocated buffer ceiling.
    #[error("XML CDATA section length exceeded ({len} bytes > {max} bytes)")]
    CDataLengthExceeded { len: usize, max: usize },

    /// Malformed XML syntax or broken stream encountered.
    #[error("Malformed XML stream: {reason} (offset: {offset})")]
    MalformedXml { reason: String, offset: usize },

    /// Premature end-of-stream with unclosed tag stack.
    #[error("Unexpected XML EOF: unclosed tags: {unclosed_tags:?}")]
    UnexpectedEof { unclosed_tags: Vec<String> },

    /// Underlying parser or I/O error occurred.
    #[error("XML parser error: {0}")]
    ParserError(String),
}
