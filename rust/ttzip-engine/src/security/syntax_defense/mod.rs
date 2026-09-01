// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Syntax 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic parsing insulation, memory fuses, and recursion limits:
//! 1. **AST Depth Guard** ([`AstDepthGuard`]):
//!    Extreme AST recursion & stack overflow circuit breaker (depth <= 256 levels).
//! 2. **Error Recovery Timeout Guard** ([`ErrorRecoveryTimeoutGuard`]):
//!    GLR error recovery loop and branch explosion circuit breaker (steps <= 10000 steps).
//! 3. **Query Pattern Safety Guard** ([`QueryPatternSafetyGuard`]):
//!    Malformed and explosive S-expression query pattern injection defense (depth <= 32 levels).
//! 4. **Line Length Fuse Guard** ([`LineLengthFuseGuard`]):
//!    Giant single-line source code lexical memory fuse (single line <= 128KB).
//! 5. **Parsing Timeout Guard** ([`ParsingTimeoutGuard`]):
//!    Grammar parsing microsecond hard ceiling circuit breaker (<= 20ms / 20000μs).
//! 6. **Sensitive Token Buffer** ([`SensitiveTokenBuffer`]):
//!    Zero memory leak & sensitive source code token zeroize-on-drop erasure.

mod buffer;
mod guards;
mod pipeline;

#[cfg(test)]
mod tests;

pub use buffer::{SensitiveToken, SensitiveTokenBuffer};
pub use guards::{
    AstDepthGuard, ErrorRecoveryTimeoutGuard, LineLengthFuseGuard, ParsingTimeoutGuard,
    QueryPatternSafetyGuard,
};
pub use pipeline::SyntaxSecurityPipeline;

/// Default maximum allowable AST nesting depth (256 levels).
pub const DEFAULT_MAX_AST_DEPTH: usize = 256;

/// Default maximum allowable GLR error recovery search steps (10000 steps).
pub const DEFAULT_MAX_RECOVERY_STEPS: usize = 10000;

/// Default maximum allowable S-expression query pattern nesting depth (32 levels).
pub const DEFAULT_MAX_QUERY_DEPTH: usize = 32;

/// Default maximum allowable S-expression query pattern length (4096 bytes).
pub const DEFAULT_MAX_QUERY_PATTERN_LEN: usize = 4096;

/// Default maximum allowable query predicate count (64 predicates).
pub const DEFAULT_MAX_QUERY_PREDICATES: usize = 64;

/// Default maximum allowable single-line length in source code (128 KiB).
pub const DEFAULT_MAX_LINE_LENGTH: usize = 128 * 1024;

/// Default parsing timeout in microseconds (20 ms = 20000 μs).
pub const DEFAULT_PARSING_TIMEOUT_MICROS: u64 = 20_000;

/// Default maximum allowable token buffer capacity before spill defense (1 MiB).
pub const DEFAULT_MAX_TOKEN_BUFFER_BYTES: usize = 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when syntax security invariants, fuses, or parsing limits are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SyntaxDefenseError {
    /// AST nesting recursion depth exceeded the safety threshold.
    #[error("AST recursion depth exceeded ({depth} > {max_depth})")]
    AstDepthExceeded { depth: usize, max_depth: usize },

    /// GLR error recovery steps exceeded computational budget limit.
    #[error("GLR error recovery step quota exceeded ({steps} > {max_steps})")]
    ErrorRecoveryLimitExceeded { steps: usize, max_steps: usize },

    /// S-expression query pattern violates safety constraints.
    #[error("Query pattern safety violation: {reason}")]
    QueryPatternViolation { reason: String },

    /// Single line byte length exceeded the lexical memory fuse ceiling.
    #[error("Line length exceeded on line {line_number} ({len} bytes > {max_len} bytes)")]
    LineLengthExceeded {
        line_number: usize,
        len: usize,
        max_len: usize,
    },

    /// Parsing time exceeded the hard microsecond limit.
    #[error("Parsing timeout exceeded ({elapsed_micros} μs > {max_micros} μs)")]
    ParsingTimeout {
        elapsed_micros: u64,
        max_micros: u64,
    },

    /// Malformed syntax or broken input encountered during defense validation.
    #[error("Malformed syntax: {reason} at byte offset {byte_offset}")]
    MalformedSyntax {
        reason: String,
        byte_offset: usize,
    },

    /// Sensitive token buffer capacity exceeded allowable limits.
    #[error("Token buffer overflow: {size} bytes > {max_size} bytes")]
    TokenBufferOverflow { size: usize, max_size: usize },

    /// Language unsupported by the syntax defense subsystem.
    #[error("Unsupported language for syntax defense: {0}")]
    UnsupportedLanguage(String),

    /// Underlying parser error encountered.
    #[error("Syntax parser error: {0}")]
    ParserError(String),
}
