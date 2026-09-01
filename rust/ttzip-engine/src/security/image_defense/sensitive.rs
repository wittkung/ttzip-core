// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-leakage sensitive image buffer with automatic zeroize-on-drop memory erasure.

use std::ops::{Deref, DerefMut};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secure pixel and metadata buffer guaranteed to be wiped upon deallocation.
#[derive(Debug, Default, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SensitiveImageBuffer {
    data: Vec<u8>,
}

impl SensitiveImageBuffer {
    /// Creates a new empty sensitive buffer.
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Creates a sensitive buffer pre-allocated with a specified byte capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Creates a sensitive buffer of specified length initialized with zeroed bytes.
    pub fn zeroed(len: usize) -> Self {
        Self {
            data: vec![0u8; len],
        }
    }

    /// Wraps an existing vector into a sensitive buffer.
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Clones a byte slice into a newly allocated sensitive buffer.
    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
        }
    }

    /// Appends elements from a byte slice.
    pub fn extend_from_slice(&mut self, slice: &[u8]) {
        self.data.extend_from_slice(slice);
    }

    /// Returns a read-only slice of the buffer.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable slice of the buffer.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns the length of the buffer in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the buffer contains 0 bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Immediately wipes all bytes in the buffer with zeroization barriers and clears length.
    pub fn wipe(&mut self) {
        self.data.zeroize();
        self.data.clear();
    }

    /// Constant-time comparison between two sensitive image buffers.
    pub fn ct_eq(&self, other: &Self) -> bool {
        if self.data.len() != other.data.len() {
            return false;
        }
        self.data.ct_eq(&other.data).into()
    }
}

impl Deref for SensitiveImageBuffer {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SensitiveImageBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl From<Vec<u8>> for SensitiveImageBuffer {
    fn from(data: Vec<u8>) -> Self {
        Self::from_vec(data)
    }
}

impl From<&[u8]> for SensitiveImageBuffer {
    fn from(slice: &[u8]) -> Self {
        Self::from_slice(slice)
    }
}
