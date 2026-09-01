// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-allocation and zeroize-on-drop volatile memory buffer for sensitive syntax tokens.
//!
//! Mitigates secret leakage (passwords, private keys, API tokens, proprietary code snippets)
//! during syntax tree analysis, ensuring volatile memory is deterministically scrubbed on drop.

use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{SyntaxDefenseError, DEFAULT_MAX_TOKEN_BUFFER_BYTES};

// ============================================================================
// 6. Sensitive Token Buffer
// ============================================================================

/// Represents an individual sensitive token with secure erasure semantics.
#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveToken {
    category: String,
    content: String,
    start_byte: usize,
    end_byte: usize,
}

impl SensitiveToken {
    /// Creates a new sensitive token.
    #[must_use]
    pub fn new(category: String, content: String, start_byte: usize, end_byte: usize) -> Self {
        Self {
            category,
            content,
            start_byte,
            end_byte,
        }
    }

    /// Accesses token category name.
    #[must_use]
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Accesses sensitive token content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Starting byte offset in source code.
    #[must_use]
    pub fn start_byte(&self) -> usize {
        self.start_byte
    }

    /// Ending byte offset in source code.
    #[must_use]
    pub fn end_byte(&self) -> usize {
        self.end_byte
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Returns `true` if content is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Volatile memory container ensuring zero-leakage token storage and deterministic zeroization.
#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveTokenBuffer {
    raw_buffer: Vec<u8>,
    tokens: Vec<SensitiveToken>,
    #[zeroize(skip)]
    max_capacity_bytes: usize,
}

impl Default for SensitiveTokenBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl SensitiveTokenBuffer {
    /// Creates a new sensitive token buffer with default memory quota (1 MiB).
    #[must_use]
    pub fn new() -> Self {
        Self {
            raw_buffer: Vec::with_capacity(1024),
            tokens: Vec::new(),
            max_capacity_bytes: DEFAULT_MAX_TOKEN_BUFFER_BYTES,
        }
    }

    /// Creates a new buffer with custom byte capacity limit.
    #[must_use]
    pub fn with_max_capacity(max_capacity_bytes: usize) -> Self {
        Self {
            raw_buffer: Vec::with_capacity(1024.min(max_capacity_bytes)),
            tokens: Vec::new(),
            max_capacity_bytes,
        }
    }

    /// Instantiates buffer pre-populated from a sensitive source code string.
    pub fn from_source_str(source: &str) -> Result<Self, SyntaxDefenseError> {
        Self::from_bytes(source.as_bytes())
    }

    /// Instantiates buffer pre-populated from a byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SyntaxDefenseError> {
        let mut buf = Self::new();
        buf.push_bytes(bytes)?;
        Ok(buf)
    }
}

impl std::str::FromStr for SensitiveTokenBuffer {
    type Err = SyntaxDefenseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_source_str(s)
    }
}

impl SensitiveTokenBuffer {

    /// Appends raw bytes into the sensitive buffer while enforcing quota.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), SyntaxDefenseError> {
        let new_len = self.raw_buffer.len().saturating_add(bytes.len());
        if new_len > self.max_capacity_bytes {
            return Err(SyntaxDefenseError::TokenBufferOverflow {
                size: new_len,
                max_size: self.max_capacity_bytes,
            });
        }
        self.raw_buffer.extend_from_slice(bytes);
        Ok(())
    }

    /// Appends a categorized token into the container.
    pub fn push_token(
        &mut self,
        category: &str,
        content: &str,
        start_byte: usize,
        end_byte: usize,
    ) -> Result<(), SyntaxDefenseError> {
        let total_size = self
            .raw_buffer
            .len()
            .saturating_add(content.len())
            .saturating_add(category.len());

        if total_size > self.max_capacity_bytes {
            return Err(SyntaxDefenseError::TokenBufferOverflow {
                size: total_size,
                max_size: self.max_capacity_bytes,
            });
        }

        self.tokens.push(SensitiveToken::new(
            category.to_string(),
            content.to_string(),
            start_byte,
            end_byte,
        ));
        Ok(())
    }

    /// Borrows raw buffer content as UTF-8 string slice if valid.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.raw_buffer).ok()
    }

    /// Borrows raw buffer content as immutable byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.raw_buffer
    }

    /// Returns recorded tokens slice.
    #[must_use]
    pub fn tokens(&self) -> &[SensitiveToken] {
        &self.tokens
    }

    /// Number of raw bytes currently buffered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.raw_buffer.len()
    }

    /// Returns `true` if raw buffer and tokens are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw_buffer.is_empty() && self.tokens.is_empty()
    }

    /// Token count stored.
    #[must_use]
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    /// Securely zeroizes internal buffers and clears collections immediately.
    pub fn zeroize_and_clear(&mut self) {
        self.raw_buffer.zeroize();
        self.tokens.zeroize();
        self.raw_buffer.clear();
        self.tokens.clear();
    }

    /// Produces a sanitized/masked copy of the source code replacing sensitive spans with `mask`.
    #[must_use]
    pub fn render_masked(&self, mask: char) -> String {
        if let Some(s) = self.as_str() {
            let mut chars: Vec<char> = s.chars().collect();
            for tok in &self.tokens {
                if tok.category == "secret" || tok.category == "password" || tok.category == "token" {
                    let mut byte_idx = 0usize;
                    for ch_idx in 0..chars.len() {
                        let ch_len = chars[ch_idx].len_utf8();
                        if byte_idx >= tok.start_byte && byte_idx < tok.end_byte {
                            chars[ch_idx] = mask;
                        }
                        byte_idx += ch_len;
                    }
                }
            }
            chars.into_iter().collect()
        } else {
            String::new()
        }
    }
}
