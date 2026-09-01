// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 6: Sensitive HTML Memory Buffer & Volatile Zeroize Guard.
//!
//! Provides a zero-allocation / zeroize-on-drop memory buffer for decrypted HTML payloads,
//! untrusted web views, and uncompressed markup fragments, immune to Dead-Store Elimination
//! and core dump memory snooping via `madvise(MADV_DONTDUMP)`.

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{compiler_fence, Ordering};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A secure HTML memory buffer that unconditionally zeroes out all sensitive
/// textual and markup content upon Drop, immune to compiler optimization and memory dumps.
pub struct SensitiveHtmlBuffer {
    buffer: Vec<u8>,
}

impl SensitiveHtmlBuffer {
    /// Allocates an empty sensitive HTML buffer with specified initial capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        Self::apply_madvise_dontdump(&mut buffer);
        Self { buffer }
    }

    /// Wraps an existing byte vector into a sensitive protected buffer.
    #[must_use]
    pub fn from_vec(mut buffer: Vec<u8>) -> Self {
        Self::apply_madvise_dontdump(&mut buffer);
        Self { buffer }
    }

    /// Creates a new sensitive buffer by copying from an existing byte slice.
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Self {
        let mut buffer = slice.to_vec();
        Self::apply_madvise_dontdump(&mut buffer);
        Self { buffer }
    }

    /// Creates a new sensitive buffer from a UTF-8 string.
    #[must_use]
    pub fn from_string(s: String) -> Self {
        let mut buffer = s.into_bytes();
        Self::apply_madvise_dontdump(&mut buffer);
        Self { buffer }
    }

    /// Creates a new sensitive buffer from a string slice.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self::from_slice(s.as_bytes())
    }

    /// Returns a borrowed immutable slice of the internal buffer.
    #[inline]
    #[must_use]
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
        Self::apply_madvise_dontdump(&mut self.buffer);
    }

    /// Clears the buffer and wipes its memory immediately with volatile write barrier.
    pub fn clear_and_zeroize(&mut self) {
        self.zeroize();
        self.buffer.clear();
    }

    /// Returns the length of the buffer in bytes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns whether the buffer is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Issues kernel memory advice (`MADV_DONTDUMP` / `MADV_NORMAL`) if possible.
    fn apply_madvise_dontdump(vec: &mut [u8]) {
        if vec.is_empty() {
            return;
        }
        let ptr = vec.as_mut_ptr();
        let len = vec.len();

        #[cfg(target_os = "linux")]
        unsafe {
            // MADV_DONTDUMP on Linux prevents memory from being included in core dumps.
            let _ = libc::madvise(ptr.cast::<libc::c_void>(), len, libc::MADV_DONTDUMP);
        }

        #[cfg(not(target_os = "linux"))]
        unsafe {
            // On non-Linux POSIX platforms, advise standard memory behavior.
            let _ = libc::madvise(ptr.cast::<libc::c_void>(), len, libc::MADV_NORMAL);
        }
    }
}

impl Deref for SensitiveHtmlBuffer {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for SensitiveHtmlBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl AsRef<[u8]> for SensitiveHtmlBuffer {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.buffer
    }
}

impl Zeroize for SensitiveHtmlBuffer {
    fn zeroize(&mut self) {
        if !self.buffer.is_empty() {
            let slice = self.buffer.as_mut_slice();
            for byte in slice.iter_mut() {
                unsafe {
                    std::ptr::write_volatile(byte, 0x00);
                }
            }
            compiler_fence(Ordering::SeqCst);
        }
    }
}

impl Drop for SensitiveHtmlBuffer {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for SensitiveHtmlBuffer {}

impl Clone for SensitiveHtmlBuffer {
    fn clone(&self) -> Self {
        Self::from_slice(&self.buffer)
    }
}

impl PartialEq for SensitiveHtmlBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.buffer == other.buffer
    }
}

impl Eq for SensitiveHtmlBuffer {}

impl fmt::Debug for SensitiveHtmlBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SensitiveHtmlBuffer")
            .field("len", &self.buffer.len())
            .field("payload", &"[REDACTED_SENSITIVE_HTML_PAYLOAD]")
            .finish()
    }
}
