// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF Sensitive Memory Buffer with Zeroize Protection.
//!
//! Provides zero-allocation / zeroize-on-drop volatile memory containment for
//! sensitive document artifacts including passwords, derived cryptographic keys,
//! intermediate decrypted stream buffers, and confidential extracted text.

use std::ops::{Deref, DerefMut};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A memory-safe buffer container that guarantees zeroization upon deallocation.
#[derive(Debug, Default, Clone, Zeroize, ZeroizeOnDrop)]
pub struct SensitivePdfBuffer {
    data: Vec<u8>,
}

impl SensitivePdfBuffer {
    /// Creates a new empty sensitive buffer.
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Creates a new sensitive buffer pre-allocated with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Constructs a sensitive buffer by copying a byte slice.
    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
        }
    }

    /// Constructs a sensitive buffer taking ownership of an existing byte vector.
    pub fn from_vec(vec: Vec<u8>) -> Self {
        Self { data: vec }
    }

    /// Constructs a sensitive buffer from a string slice.
    pub fn from_str_slice(s: &str) -> Self {
        Self {
            data: s.as_bytes().to_vec(),
        }
    }

    /// Returns a slice view of the protected data.
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable slice view of the protected data.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns the length of the protected buffer in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the buffer contains zero bytes.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Clears and zeroizes the contents immediately.
    pub fn clear_and_zeroize(&mut self) {
        self.data.zeroize();
        self.data.clear();
    }

    /// Decodes the protected payload to a lossy UTF-8 string.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.data).to_string()
    }

    /// Performs a constant-time equality check against another byte slice to prevent timing attacks.
    pub fn constant_time_eq(&self, other: &[u8]) -> bool {
        if self.data.len() != other.len() {
            return false;
        }
        self.data.ct_eq(other).into()
    }
}

impl Deref for SensitivePdfBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SensitivePdfBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl AsRef<[u8]> for SensitivePdfBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl AsMut<[u8]> for SensitivePdfBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl From<Vec<u8>> for SensitivePdfBuffer {
    fn from(vec: Vec<u8>) -> Self {
        Self::from_vec(vec)
    }
}

impl From<&[u8]> for SensitivePdfBuffer {
    fn from(slice: &[u8]) -> Self {
        Self::from_slice(slice)
    }
}

impl From<&str> for SensitivePdfBuffer {
    fn from(s: &str) -> Self {
        Self::from_str_slice(s)
    }
}

impl std::str::FromStr for SensitivePdfBuffer {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_str_slice(s))
    }
}

impl PartialEq for SensitivePdfBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.constant_time_eq(&other.data)
    }
}

impl Eq for SensitivePdfBuffer {}
