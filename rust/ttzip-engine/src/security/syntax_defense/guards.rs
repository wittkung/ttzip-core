// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Core syntax security guards: AST depth fuses, GLR error recovery circuit breakers,
//! S-expression query sanitizers, line-length memory fuses, and microsecond timeout monitors.

use std::time::Instant;

use super::{
    SyntaxDefenseError, DEFAULT_MAX_AST_DEPTH, DEFAULT_MAX_LINE_LENGTH, DEFAULT_MAX_QUERY_DEPTH,
    DEFAULT_MAX_QUERY_PATTERN_LEN, DEFAULT_MAX_QUERY_PREDICATES, DEFAULT_MAX_RECOVERY_STEPS,
    DEFAULT_PARSING_TIMEOUT_MICROS,
};

// ============================================================================
// 1. AST Depth Guard
// ============================================================================

/// Protects against excessively nested AST trees and call-stack overflow attacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstDepthGuard {
    current_depth: usize,
    max_depth: usize,
    peak_depth: usize,
}

impl Default for AstDepthGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl AstDepthGuard {
    /// Creates a new AST depth guard with the default limit (256 levels).
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_depth: 0,
            max_depth: DEFAULT_MAX_AST_DEPTH,
            peak_depth: 0,
        }
    }

    /// Creates a new AST depth guard with a custom maximum depth ceiling.
    #[must_use]
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            current_depth: 0,
            max_depth,
            peak_depth: 0,
        }
    }

    /// Increments the current nesting depth and validates against the ceiling.
    pub fn push_depth(&mut self) -> Result<usize, SyntaxDefenseError> {
        let new_depth = self.current_depth.saturating_add(1);
        if new_depth > self.max_depth {
            return Err(SyntaxDefenseError::AstDepthExceeded {
                depth: new_depth,
                max_depth: self.max_depth,
            });
        }
        self.current_depth = new_depth;
        self.peak_depth = self.peak_depth.max(new_depth);
        Ok(self.current_depth)
    }

    /// Decrements the current nesting depth.
    pub fn pop_depth(&mut self) -> usize {
        self.current_depth = self.current_depth.saturating_sub(1);
        self.current_depth
    }

    /// Validates an arbitrary depth value against configured maximum.
    pub fn validate_depth(&self, depth: usize) -> Result<(), SyntaxDefenseError> {
        if depth > self.max_depth {
            return Err(SyntaxDefenseError::AstDepthExceeded {
                depth,
                max_depth: self.max_depth,
            });
        }
        Ok(())
    }

    /// Returns current active depth.
    #[must_use]
    pub fn current_depth(&self) -> usize {
        self.current_depth
    }

    /// Returns configured maximum allowable depth.
    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Returns peak depth recorded during session.
    #[must_use]
    pub fn peak_depth(&self) -> usize {
        self.peak_depth
    }

    /// Resets active depth counters.
    pub fn reset(&mut self) {
        self.current_depth = 0;
        self.peak_depth = 0;
    }

    /// Iteratively inspects a `tree_sitter::Tree` without recursion, verifying depth limits.
    #[cfg(feature = "syntax")]
    pub fn inspect_tree(&mut self, tree: &tree_sitter::Tree) -> Result<usize, SyntaxDefenseError> {
        let mut cursor = tree.walk();
        let mut depth = 0usize;
        let mut max_observed = 0usize;

        loop {
            max_observed = max_observed.max(depth);
            if depth > self.max_depth {
                return Err(SyntaxDefenseError::AstDepthExceeded {
                    depth,
                    max_depth: self.max_depth,
                });
            }

            if cursor.goto_first_child() {
                depth = depth.saturating_add(1);
                continue;
            }

            if cursor.goto_next_sibling() {
                continue;
            }

            loop {
                if !cursor.goto_parent() {
                    self.peak_depth = self.peak_depth.max(max_observed);
                    return Ok(max_observed);
                }
                depth = depth.saturating_sub(1);
                if cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

// ============================================================================
// 2. Error Recovery Timeout Guard
// ============================================================================

/// Circuit breaker mitigating GLR parsing state loops, branch explosions, and excessive error recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorRecoveryTimeoutGuard {
    step_count: usize,
    max_steps: usize,
    error_node_count: usize,
    branch_count: usize,
}

impl Default for ErrorRecoveryTimeoutGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorRecoveryTimeoutGuard {
    /// Creates a new error recovery guard with default step budget (10000 steps).
    #[must_use]
    pub fn new() -> Self {
        Self {
            step_count: 0,
            max_steps: DEFAULT_MAX_RECOVERY_STEPS,
            error_node_count: 0,
            branch_count: 0,
        }
    }

    /// Creates a new error recovery guard with a custom step limit.
    #[must_use]
    pub fn with_max_steps(max_steps: usize) -> Self {
        Self {
            step_count: 0,
            max_steps,
            error_node_count: 0,
            branch_count: 0,
        }
    }

    /// Records a single parsing or traversal step and verifies the budget.
    pub fn record_step(&mut self) -> Result<usize, SyntaxDefenseError> {
        self.record_steps(1)
    }

    /// Records multiple parsing or traversal steps.
    pub fn record_steps(&mut self, count: usize) -> Result<usize, SyntaxDefenseError> {
        self.step_count = self.step_count.saturating_add(count);
        if self.step_count > self.max_steps {
            return Err(SyntaxDefenseError::ErrorRecoveryLimitExceeded {
                steps: self.step_count,
                max_steps: self.max_steps,
            });
        }
        Ok(self.step_count)
    }

    /// Records an error node encountered in the AST.
    pub fn record_error_node(&mut self) {
        self.error_node_count = self.error_node_count.saturating_add(1);
    }

    /// Records a GLR alternative branch explored.
    pub fn record_branch(&mut self) {
        self.branch_count = self.branch_count.saturating_add(1);
    }

    /// Checks whether the step budget is still healthy.
    pub fn check_budget(&self) -> Result<(), SyntaxDefenseError> {
        if self.step_count > self.max_steps {
            return Err(SyntaxDefenseError::ErrorRecoveryLimitExceeded {
                steps: self.step_count,
                max_steps: self.max_steps,
            });
        }
        Ok(())
    }

    /// Returns cumulative step count.
    #[must_use]
    pub fn step_count(&self) -> usize {
        self.step_count
    }

    /// Returns cumulative error node count.
    #[must_use]
    pub fn error_node_count(&self) -> usize {
        self.error_node_count
    }

    /// Returns cumulative branch count.
    #[must_use]
    pub fn branch_count(&self) -> usize {
        self.branch_count
    }

    /// Resets all counters.
    pub fn reset(&mut self) {
        self.step_count = 0;
        self.error_node_count = 0;
        self.branch_count = 0;
    }

    /// Scans a `tree_sitter::Tree` for error nodes and verifies budget.
    #[cfg(feature = "syntax")]
    pub fn scan_for_error_nodes(&mut self, tree: &tree_sitter::Tree) -> Result<usize, SyntaxDefenseError> {
        let mut cursor = tree.walk();
        let mut steps = 0usize;
        let mut errors = 0usize;

        loop {
            steps = steps.saturating_add(1);
            if steps > self.max_steps {
                self.step_count = self.step_count.saturating_add(steps);
                return Err(SyntaxDefenseError::ErrorRecoveryLimitExceeded {
                    steps: self.step_count,
                    max_steps: self.max_steps,
                });
            }

            let node = cursor.node();
            if node.is_error() || node.is_missing() {
                errors = errors.saturating_add(1);
            }

            if cursor.goto_first_child() {
                continue;
            }

            if cursor.goto_next_sibling() {
                continue;
            }

            loop {
                if !cursor.goto_parent() {
                    self.step_count = self.step_count.saturating_add(steps);
                    self.error_node_count = self.error_node_count.saturating_add(errors);
                    return Ok(errors);
                }
                if cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

// ============================================================================
// 3. Query Pattern Safety Guard
// ============================================================================

/// Defends against malformed, overly nested, or explosive Tree-sitter S-expression query patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPatternSafetyGuard {
    max_depth: usize,
    max_len: usize,
    max_predicates: usize,
}

impl Default for QueryPatternSafetyGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryPatternSafetyGuard {
    /// Creates a new query pattern safety guard with default limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_depth: DEFAULT_MAX_QUERY_DEPTH,
            max_len: DEFAULT_MAX_QUERY_PATTERN_LEN,
            max_predicates: DEFAULT_MAX_QUERY_PREDICATES,
        }
    }

    /// Creates a new query pattern guard with custom parameters.
    #[must_use]
    pub fn with_limits(max_depth: usize, max_len: usize, max_predicates: usize) -> Self {
        Self {
            max_depth,
            max_len,
            max_predicates,
        }
    }

    /// Validates an S-expression query string against nesting depth, size, and injection patterns.
    pub fn validate_pattern(&self, pattern: &str) -> Result<(), SyntaxDefenseError> {
        let trimmed = pattern.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        if pattern.len() > self.max_len {
            return Err(SyntaxDefenseError::QueryPatternViolation {
                reason: format!(
                    "Query pattern length exceeded ({} bytes > {} bytes)",
                    pattern.len(),
                    self.max_len
                ),
            });
        }

        let mut depth = 0usize;
        let mut max_observed_depth = 0usize;
        let mut predicate_count = 0usize;
        let mut in_string = false;
        let mut in_escape = false;
        let mut in_comment = false;

        let bytes = pattern.as_bytes();
        let len = bytes.len();
        let mut i = 0usize;

        while i < len {
            let b = bytes[i];

            if in_comment {
                if b == b'\n' {
                    in_comment = false;
                }
                i += 1;
                continue;
            }

            if in_string {
                if in_escape {
                    in_escape = false;
                } else if b == b'\\' {
                    in_escape = true;
                } else if b == b'"' {
                    in_string = false;
                }
                i += 1;
                continue;
            }

            match b {
                b';' => {
                    in_comment = true;
                }
                b'"' => {
                    in_string = true;
                }
                b'(' => {
                    depth = depth.saturating_add(1);
                    max_observed_depth = max_observed_depth.max(depth);
                    if depth > self.max_depth {
                        return Err(SyntaxDefenseError::QueryPatternViolation {
                            reason: format!(
                                "S-expression query nesting depth exceeded ({} > {})",
                                depth, self.max_depth
                            ),
                        });
                    }
                }
                b')' => {
                    if depth == 0 {
                        return Err(SyntaxDefenseError::QueryPatternViolation {
                            reason: "Unbalanced closing parenthesis ')' in query pattern".to_string(),
                        });
                    }
                    depth = depth.saturating_sub(1);
                }
                b'#' => {
                    predicate_count = predicate_count.saturating_add(1);
                    if predicate_count > self.max_predicates {
                        return Err(SyntaxDefenseError::QueryPatternViolation {
                            reason: format!(
                                "Query predicate count exceeded ({} > {})",
                                predicate_count, self.max_predicates
                            ),
                        });
                    }
                }
                _ => {}
            }

            i += 1;
        }

        if in_string {
            return Err(SyntaxDefenseError::QueryPatternViolation {
                reason: "Unterminated string literal in query pattern".to_string(),
            });
        }

        if depth != 0 {
            return Err(SyntaxDefenseError::QueryPatternViolation {
                reason: format!("Unclosed parenthesis nesting (remaining depth: {depth})"),
            });
        }

        Ok(())
    }

    /// Compiles a query safely after verifying S-expression invariants.
    #[cfg(feature = "syntax")]
    pub fn compile_safe_query(
        &self,
        language: &tree_sitter::Language,
        pattern: &str,
    ) -> Result<tree_sitter::Query, SyntaxDefenseError> {
        self.validate_pattern(pattern)?;
        tree_sitter::Query::new(language, pattern)
            .map_err(|e| SyntaxDefenseError::ParserError(format!("Query compilation failed: {e:?}")))
    }
}

// ============================================================================
// 4. Line Length Fuse Guard
// ============================================================================

/// Protects against giant single-line source code bombs that induce unbounded lexical memory allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineLengthFuseGuard {
    max_line_length: usize,
}

impl Default for LineLengthFuseGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl LineLengthFuseGuard {
    /// Creates a new line length fuse guard with default limit (128 KB).
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_line_length: DEFAULT_MAX_LINE_LENGTH,
        }
    }

    /// Creates a new line length fuse guard with custom byte ceiling.
    #[must_use]
    pub fn with_max_line_length(max_line_length: usize) -> Self {
        Self { max_line_length }
    }

    /// Scans a UTF-8 source string and validates every line length against ceiling.
    pub fn scan_source(&self, source: &str) -> Result<(), SyntaxDefenseError> {
        self.scan_bytes(source.as_bytes())
    }

    /// Scans raw bytes for newline delimiters, verifying line sizes.
    pub fn scan_bytes(&self, bytes: &[u8]) -> Result<(), SyntaxDefenseError> {
        let mut line_number = 1usize;
        let mut line_len = 0usize;

        for &b in bytes {
            if b == b'\n' {
                if line_len > self.max_line_length {
                    return Err(SyntaxDefenseError::LineLengthExceeded {
                        line_number,
                        len: line_len,
                        max_len: self.max_line_length,
                    });
                }
                line_number = line_number.saturating_add(1);
                line_len = 0;
            } else {
                line_len = line_len.saturating_add(1);
                if line_len > self.max_line_length {
                    return Err(SyntaxDefenseError::LineLengthExceeded {
                        line_number,
                        len: line_len,
                        max_len: self.max_line_length,
                    });
                }
            }
        }

        if line_len > self.max_line_length {
            return Err(SyntaxDefenseError::LineLengthExceeded {
                line_number,
                len: line_len,
                max_len: self.max_line_length,
            });
        }

        Ok(())
    }

    /// Sanitizes long lines by truncating and inserting a marker.
    #[must_use]
    pub fn sanitize_long_lines(&self, source: &str, replacement: &str) -> String {
        let mut output = String::with_capacity(source.len());
        for (i, line) in source.lines().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            if line.len() > self.max_line_length {
                let safe_slice = &line[..self.max_line_length.min(line.len())];
                output.push_str(safe_slice);
                output.push_str(replacement);
            } else {
                output.push_str(line);
            }
        }
        output
    }

    /// Configured maximum allowable single line length.
    #[must_use]
    pub fn max_line_length(&self) -> usize {
        self.max_line_length
    }
}

// ============================================================================
// 5. Parsing Timeout Guard
// ============================================================================

/// High-precision microsecond circuit breaker guarding against unbounded grammar parsing execution.
#[derive(Debug, Clone)]
pub struct ParsingTimeoutGuard {
    start_time: Instant,
    max_micros: u64,
}

impl Default for ParsingTimeoutGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ParsingTimeoutGuard {
    /// Creates and immediately arms a new timeout guard with default limit (20 ms / 20000 μs).
    #[must_use]
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            max_micros: DEFAULT_PARSING_TIMEOUT_MICROS,
        }
    }

    /// Creates and arms a timeout guard with explicit microsecond budget.
    #[must_use]
    pub fn with_timeout_micros(max_micros: u64) -> Self {
        Self {
            start_time: Instant::now(),
            max_micros,
        }
    }

    /// Checks if the allocated budget has elapsed; trips with error if expired.
    pub fn check_timeout(&self) -> Result<(), SyntaxDefenseError> {
        let elapsed = self.elapsed_micros();
        if elapsed > self.max_micros {
            return Err(SyntaxDefenseError::ParsingTimeout {
                elapsed_micros: elapsed,
                max_micros: self.max_micros,
            });
        }
        Ok(())
    }

    /// Returns elapsed microseconds since armed.
    #[must_use]
    pub fn elapsed_micros(&self) -> u64 {
        self.start_time.elapsed().as_micros() as u64
    }

    /// Returns remaining microseconds before timeout.
    #[must_use]
    pub fn remaining_micros(&self) -> u64 {
        let elapsed = self.elapsed_micros();
        self.max_micros.saturating_sub(elapsed)
    }

    /// Returns `true` if the timeout budget is exhausted.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.elapsed_micros() > self.max_micros
    }

    /// Resets the clock to now.
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
    }

    /// Configures underlying Tree-sitter parser with hardware timeout.
    #[cfg(feature = "syntax")]
    pub fn apply_to_parser(&self, parser: &mut tree_sitter::Parser) {
        parser.set_timeout_micros(self.max_micros);
    }
}
