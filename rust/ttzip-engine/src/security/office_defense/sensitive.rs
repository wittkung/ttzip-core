// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 6: Sensitive Office Memory Buffer & Volatile Zeroize Guard.
//!
//! Provides a zero-allocation / zeroize-on-drop memory buffer for decrypted Office documents,
//! confidential spreadsheets, proprietary formulas, and XML payloads, immune to Dead-Store Elimination.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{compiler_fence, Ordering};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A secure document memory buffer that unconditionally zeroes out all sensitive
/// decrypted/decompressed textual content upon Drop, immune to compiler optimization.
pub struct SensitiveOfficeBuffer {
    buffer: Vec<u8>,
}

impl SensitiveOfficeBuffer {
    /// Allocates an empty sensitive Office buffer with specified initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
        }
    }

    /// Wraps an existing byte vector into a sensitive protected buffer.
    pub fn from_vec(buffer: Vec<u8>) -> Self {
        Self { buffer }
    }

    /// Creates a new sensitive buffer by copying from an existing byte slice.
    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            buffer: slice.to_vec(),
        }
    }

    /// Returns a borrowed immutable slice of the internal buffer.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }

    /// Returns a borrowed mutable slice of the internal buffer.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Attempts to view the buffer contents as a UTF-8 string slice.
    #[inline]
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.buffer)
    }

    /// Appends a byte slice to the buffer.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.buffer.extend_from_slice(other);
    }

    /// Clears the buffer and wipes its memory immediately.
    pub fn clear(&mut self) {
        self.zeroize();
        self.buffer.clear();
    }

    /// Returns the length of the buffer in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns whether the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

impl Deref for SensitiveOfficeBuffer {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for SensitiveOfficeBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Zeroize for SensitiveOfficeBuffer {
    fn zeroize(&mut self) {
        self.buffer.zeroize();
        compiler_fence(Ordering::SeqCst);
    }
}

impl Drop for SensitiveOfficeBuffer {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for SensitiveOfficeBuffer {}

impl fmt::Debug for SensitiveOfficeBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SensitiveOfficeBuffer")
            .field("len", &self.buffer.len())
            .field("content", &"[REDACTED_SENSITIVE_OFFICE_DATA]")
            .finish()
    }
}

impl Clone for SensitiveOfficeBuffer {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
        }
    }
}
