// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified syntax security pipeline orchestrating all 6 defense layers.

use super::buffer::SensitiveTokenBuffer;
use super::guards::{
    AstDepthGuard, ErrorRecoveryTimeoutGuard, LineLengthFuseGuard, ParsingTimeoutGuard,
    QueryPatternSafetyGuard,
};
use super::SyntaxDefenseError;
use crate::standards::syntax_highlight::{SupportedLanguage, TokenSpan};

/// Unified 6-layer defense orchestrator for deterministic and safe syntax parsing.
#[derive(Debug, Clone)]
pub struct SyntaxSecurityPipeline {
    ast_depth_guard: AstDepthGuard,
    recovery_guard: ErrorRecoveryTimeoutGuard,
    query_guard: QueryPatternSafetyGuard,
    line_fuse_guard: LineLengthFuseGuard,
    timeout_guard: ParsingTimeoutGuard,
    token_buffer: SensitiveTokenBuffer,
}

impl Default for SyntaxSecurityPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxSecurityPipeline {
    /// Creates a new syntax security pipeline with default production parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ast_depth_guard: AstDepthGuard::new(),
            recovery_guard: ErrorRecoveryTimeoutGuard::new(),
            query_guard: QueryPatternSafetyGuard::new(),
            line_fuse_guard: LineLengthFuseGuard::new(),
            timeout_guard: ParsingTimeoutGuard::new(),
            token_buffer: SensitiveTokenBuffer::new(),
        }
    }

    /// Validates raw source code text against pre-parsing static defense layers.
    pub fn validate_source_text(&self, text: &str) -> Result<(), SyntaxDefenseError> {
        self.timeout_guard.check_timeout()?;
        self.line_fuse_guard.scan_source(text)?;
        Ok(())
    }

    /// Validates raw bytes against pre-parsing defense layers.
    pub fn validate_bytes(&self, bytes: &[u8]) -> Result<(), SyntaxDefenseError> {
        self.timeout_guard.check_timeout()?;
        self.line_fuse_guard.scan_bytes(bytes)?;
        Ok(())
    }

    /// Validates an S-expression Tree-sitter query pattern.
    pub fn validate_query_pattern(&self, pattern: &str) -> Result<(), SyntaxDefenseError> {
        self.query_guard.validate_pattern(pattern)
    }

    /// Securely parses source code into an AST and token stream under 6-layer defense envelope.
    #[cfg(feature = "syntax")]
    pub fn parse_securely(
        &mut self,
        source: &str,
        lang: SupportedLanguage,
    ) -> Result<(tree_sitter::Tree, Vec<TokenSpan>), SyntaxDefenseError> {
        self.validate_source_text(source)?;

        let ts_lang = lang.get_tree_sitter_language().ok_or_else(|| {
            SyntaxDefenseError::UnsupportedLanguage(format!("{lang:?}"))
        })?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&ts_lang)
            .map_err(|e| SyntaxDefenseError::ParserError(format!("{e:?}")))?;

        self.timeout_guard.apply_to_parser(&mut parser);

        let tree = parser.parse(source, None).ok_or_else(|| {
            if self.timeout_guard.is_expired() {
                SyntaxDefenseError::ParsingTimeout {
                    elapsed_micros: self.timeout_guard.elapsed_micros(),
                    max_micros: 20_000,
                }
            } else {
                SyntaxDefenseError::ParserError("Tree-sitter parser returned None".to_string())
            }
        })?;

        self.ast_depth_guard.inspect_tree(&tree)?;
        self.recovery_guard.scan_for_error_nodes(&tree)?;

        let spans = crate::standards::syntax_highlight::highlight_spans(
            source,
            match lang {
                SupportedLanguage::Rust => "rs",
                SupportedLanguage::Swift => "swift",
                SupportedLanguage::C => "c",
                SupportedLanguage::Cpp => "cpp",
                SupportedLanguage::Python => "py",
                SupportedLanguage::JavaScript => "js",
                SupportedLanguage::TypeScript => "ts",
                SupportedLanguage::Json => "json",
                SupportedLanguage::Markdown => "md",
                SupportedLanguage::Html => "html",
                SupportedLanguage::Css => "css",
            },
        );

        Ok((tree, spans))
    }

    /// Borrows the AST depth guard.
    #[must_use]
    pub fn ast_depth_guard(&self) -> &AstDepthGuard {
        &self.ast_depth_guard
    }

    /// Mutably borrows the AST depth guard.
    pub fn ast_depth_guard_mut(&mut self) -> &mut AstDepthGuard {
        &mut self.ast_depth_guard
    }

    /// Borrows the error recovery guard.
    #[must_use]
    pub fn recovery_guard(&self) -> &ErrorRecoveryTimeoutGuard {
        &self.recovery_guard
    }

    /// Mutably borrows the error recovery guard.
    pub fn recovery_guard_mut(&mut self) -> &mut ErrorRecoveryTimeoutGuard {
        &mut self.recovery_guard
    }

    /// Borrows the query pattern safety guard.
    #[must_use]
    pub fn query_guard(&self) -> &QueryPatternSafetyGuard {
        &self.query_guard
    }

    /// Borrows the line length fuse guard.
    #[must_use]
    pub fn line_fuse_guard(&self) -> &LineLengthFuseGuard {
        &self.line_fuse_guard
    }

    /// Borrows the parsing timeout guard.
    #[must_use]
    pub fn timeout_guard(&self) -> &ParsingTimeoutGuard {
        &self.timeout_guard
    }

    /// Borrows the sensitive token buffer.
    #[must_use]
    pub fn token_buffer(&self) -> &SensitiveTokenBuffer {
        &self.token_buffer
    }

    /// Mutably borrows the sensitive token buffer.
    pub fn token_buffer_mut(&mut self) -> &mut SensitiveTokenBuffer {
        &mut self.token_buffer
    }
}
