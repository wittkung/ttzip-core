// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance Tree-sitter GLR syntax parser with microsecond incremental AST reuse.

use std::ops::Range;
use super::error::{SyntaxError, SyntaxResult};
use super::registry::SupportedLanguage;

/// High-throughput Tree-sitter AST parser with incremental editing and timeout protection.
pub struct TTZipSyntaxParser {
    #[cfg(feature = "syntax")]
    parser: tree_sitter::Parser,
    #[cfg(feature = "syntax")]
    current_tree: Option<tree_sitter::Tree>,
    current_language: Option<SupportedLanguage>,
    timeout_micros: Option<u64>,
}

impl Default for TTZipSyntaxParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TTZipSyntaxParser {
    /// Constructs a new unconfigured syntax parser instance.
    pub fn new() -> Self {
        #[cfg(feature = "syntax")]
        let parser = tree_sitter::Parser::new();

        Self {
            #[cfg(feature = "syntax")]
            parser,
            #[cfg(feature = "syntax")]
            current_tree: None,
            current_language: None,
            timeout_micros: None,
        }
    }

    /// Creates and configures a parser for the specified language.
    pub fn with_language(lang: SupportedLanguage) -> SyntaxResult<Self> {
        let mut parser = Self::new();
        parser.set_language(lang)?;
        Ok(parser)
    }

    /// Sets the active grammar language for subsequent parsing cycles.
    pub fn set_language(&mut self, lang: SupportedLanguage) -> SyntaxResult<()> {
        if self.current_language == Some(lang) {
            return Ok(());
        }

        #[cfg(feature = "syntax")]
        {
            let ts_lang = lang.tree_sitter_language().ok_or_else(|| {
                SyntaxError::UnsupportedLanguage(format!("{}: missing grammar", lang.id()))
            })?;

            self.parser
                .set_language(&ts_lang)
                .map_err(|e| SyntaxError::ParserConfigurationFailed(e.to_string()))?;
            self.current_tree = None;
        }

        self.current_language = Some(lang);
        Ok(())
    }

    /// Returns the currently active language, if configured.
    #[inline]
    pub fn current_language(&self) -> Option<SupportedLanguage> {
        self.current_language
    }

    /// Sets the timeout threshold in microseconds for parse execution.
    pub fn set_timeout_micros(&mut self, timeout_micros: Option<u64>) {
        self.timeout_micros = timeout_micros;
        #[cfg(feature = "syntax")]
        {
            self.parser
                .set_timeout_micros(timeout_micros.unwrap_or(0));
        }
    }

    /// Returns the active timeout threshold in microseconds.
    #[inline]
    pub fn timeout_micros(&self) -> Option<u64> {
        self.timeout_micros
    }

    /// Performs full AST parsing of the source text.
    #[cfg(feature = "syntax")]
    pub fn parse_full(&mut self, text: &str) -> SyntaxResult<&tree_sitter::Tree> {
        if self.current_language.is_none() {
            return Err(SyntaxError::ParserConfigurationFailed(
                "No language configured on parser".to_string(),
            ));
        }

        let tree = self
            .parser
            .parse(text, None)
            .ok_or(SyntaxError::ParseTimeout)?;

        self.current_tree = Some(tree);
        Ok(self.current_tree.as_ref().unwrap())
    }

    /// Performs microsecond-level incremental AST parsing by reusing unchanged subtrees.
    #[cfg(feature = "syntax")]
    pub fn parse_incremental(
        &mut self,
        new_text: &str,
        edit: &tree_sitter::InputEdit,
    ) -> SyntaxResult<&tree_sitter::Tree> {
        if self.current_language.is_none() {
            return Err(SyntaxError::ParserConfigurationFailed(
                "No language configured on parser".to_string(),
            ));
        }

        if let Some(mut old_tree) = self.current_tree.take() {
            old_tree.edit(edit);
            let tree = self
                .parser
                .parse(new_text, Some(&old_tree))
                .ok_or(SyntaxError::ParseTimeout)?;
            self.current_tree = Some(tree);
            Ok(self.current_tree.as_ref().unwrap())
        } else {
            self.parse_full(new_text)
        }
    }

    /// Returns a reference to the currently parsed syntax tree.
    #[cfg(feature = "syntax")]
    #[inline]
    pub fn tree(&self) -> Option<&tree_sitter::Tree> {
        self.current_tree.as_ref()
    }

    /// Takes ownership of the currently parsed tree, resetting the parser's internal tree cache.
    #[cfg(feature = "syntax")]
    #[inline]
    pub fn take_tree(&mut self) -> Option<tree_sitter::Tree> {
        self.current_tree.take()
    }

    /// Manually sets the cached syntax tree.
    #[cfg(feature = "syntax")]
    #[inline]
    pub fn set_tree(&mut self, tree: tree_sitter::Tree) {
        self.current_tree = Some(tree);
    }

    /// Resets the internal state and parsed AST tree.
    pub fn reset(&mut self) {
        #[cfg(feature = "syntax")]
        {
            self.parser.reset();
            self.current_tree = None;
        }
    }

    /// Checks if the current syntax tree contains any syntax errors or missing tokens.
    #[cfg(feature = "syntax")]
    pub fn has_error(&self) -> bool {
        self.current_tree
            .as_ref()
            .map(|t| t.root_node().has_error())
            .unwrap_or(false)
    }

    /// Compares two AST revisions and returns the slice of changed byte ranges.
    #[cfg(feature = "syntax")]
    pub fn compute_changed_ranges(
        old_tree: &tree_sitter::Tree,
        new_tree: &tree_sitter::Tree,
    ) -> Vec<Range<usize>> {
        old_tree
            .changed_ranges(new_tree)
            .map(|r| r.start_byte..r.end_byte)
            .collect()
    }
}
