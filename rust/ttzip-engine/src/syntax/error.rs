// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Error types for TTZip AST parsing, query compilation, and outline extraction.

use thiserror::Error;

/// Result type alias for syntax operations.
pub type SyntaxResult<T> = Result<T, SyntaxError>;

/// Syntax engine errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SyntaxError {
    /// The requested language is not supported or not enabled in feature flags.
    #[error("Unsupported or unregistered language: {0}")]
    UnsupportedLanguage(String),

    /// Failed to initialize or configure the parser for the target language.
    #[error("Parser configuration failed: {0}")]
    ParserConfigurationFailed(String),

    /// Parsing timed out according to the configured timeout threshold.
    #[error("Parsing timed out")]
    ParseTimeout,

    /// Failed to compile Tree-sitter query S-expression.
    #[error("Invalid syntax query pattern: {0}")]
    QueryCompilationError(String),

    /// Failed to execute query or acquire cursor.
    #[error("Syntax query execution error: {0}")]
    QueryExecutionError(String),

    /// An invalid input edit or out-of-bounds byte range was supplied.
    #[error("Invalid edit range: {0}")]
    InvalidEditRange(String),

    /// General internal syntax error.
    #[error("Syntax engine internal error: {0}")]
    Internal(String),
}
