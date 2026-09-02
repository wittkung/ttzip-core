// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Sensitive Credential Zeroization & Memory Guard (`SensitiveCredentialZeroize`).
//!
//! Provides automatic stack/heap memory scrubbing upon drop (`Zeroize`, `ZeroizeOnDrop`),
//! redacted formatting (`Debug`, `Display`), and secure access wrappers for private keys,
//! tokens, and cryptographic secrets.

use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secure container for sensitive byte arrays with automatic zeroization on drop.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveCredentialBuffer {
    data: Vec<u8>,
}

impl SensitiveCredentialBuffer {
    /// Creates a new sensitive buffer by taking ownership of a byte vector.
    #[inline]
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Creates a new sensitive buffer from a borrowed byte slice.
    #[inline]
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
        }
    }

    /// Exposes a read-only slice of the secret payload.
    #[inline]
    #[must_use]
    pub fn expose_secret(&self) -> &[u8] {
        &self.data
    }

    /// Borrows the secret buffer as a byte slice.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Borrows the secret buffer as a byte slice.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Zeroizes and empties the secret buffer immediately.
    #[inline]
    pub fn clear(&mut self) {
        self.data.zeroize();
        self.data.clear();
    }

    /// Returns the length of the secret buffer in bytes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the secret buffer is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl fmt::Debug for SensitiveCredentialBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SensitiveCredentialBuffer([REDACTED {} bytes])", self.data.len())
    }
}

impl fmt::Display for SensitiveCredentialBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED {} bytes]", self.data.len())
    }
}

/// Secure container for sensitive strings (e.g. passwords, authentication tokens, API keys).
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveCredentialString {
    inner: String,
}

impl SensitiveCredentialString {
    /// Creates a new sensitive string from an owned String.
    #[inline]
    #[must_use]
    pub fn new(inner: String) -> Self {
        Self { inner }
    }

    /// Creates a new sensitive string from a string slice.
    #[inline]
    #[must_use]
    pub fn from_str_slice(s: &str) -> Self {
        Self {
            inner: s.to_string(),
        }
    }

    /// Exposes the underlying secret string slice.
    #[inline]
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }

    /// Returns the byte length of the secret string.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the secret string is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl fmt::Debug for SensitiveCredentialString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SensitiveCredentialString([REDACTED])")
    }
}

impl fmt::Display for SensitiveCredentialString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}
