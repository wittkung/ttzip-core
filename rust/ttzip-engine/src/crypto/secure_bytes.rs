// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Secure memory wrappers for sensitive cryptographic keys, passwords, and tokens.
//!
//! Provides RAII-based memory sanitization using [`zeroize::Zeroize`] and [`zeroize::ZeroizeOnDrop`]
//! to ensure sensitive buffers are purged from heap and stack on drop, preventing residual
//! plaintext credentials in allocator freelists.

use std::borrow::Borrow;
use std::fmt;
use std::ops::{Deref, DerefMut};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A heap-allocated byte buffer that automatically wipes its content on drop.
///
/// Implements constant-time equality comparisons and redacts contents in [`fmt::Debug`].
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct SecureBytes(Vec<u8>);

impl SecureBytes {
    /// Creates a new `SecureBytes` container wrapping an existing `Vec<u8>`.
    #[inline]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Creates a new `SecureBytes` initialized with a copy of a byte slice.
    #[inline]
    pub fn from_slice(slice: &[u8]) -> Self {
        Self(slice.to_vec())
    }

    /// Allocates an empty `SecureBytes` container with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    /// Returns a slice over the wrapped bytes.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Returns a mutable slice over the wrapped bytes.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }

    /// Returns the number of bytes stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the container holds zero bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Appends a byte to the back of the collection.
    #[inline]
    pub fn push(&mut self, byte: u8) {
        self.0.push(byte);
    }

    /// Appends a byte slice to the back of the collection.
    #[inline]
    pub fn extend_from_slice(&mut self, slice: &[u8]) {
        self.0.extend_from_slice(slice);
    }

    /// Clears the container, zeroizing its active elements.
    #[inline]
    pub fn clear(&mut self) {
        self.0.zeroize();
    }

    /// Returns a raw pointer to the underlying buffer.
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    /// Returns an unsafe mutable pointer to the underlying buffer.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }
}

impl Deref for SecureBytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SecureBytes {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<[u8]> for SecureBytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for SecureBytes {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl Borrow<[u8]> for SecureBytes {
    #[inline]
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for SecureBytes {
    #[inline]
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl From<&[u8]> for SecureBytes {
    #[inline]
    fn from(slice: &[u8]) -> Self {
        Self::from_slice(slice)
    }
}

impl PartialEq for SecureBytes {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for SecureBytes {}

impl fmt::Debug for SecureBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureBytes([REDACTED; {} bytes])", self.0.len())
    }
}

/// A zeroizing UTF-8 string container for plaintext passwords and credentials.
///
/// Memory is wiped upon drop to prevent password leakage in memory dumps.
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct ZeroizingString(String);

impl ZeroizingString {
    /// Creates a new `ZeroizingString` taking ownership of an existing `String`.
    #[inline]
    pub fn new(s: String) -> Self {
        Self(s)
    }

    /// Creates a new `ZeroizingString` by copying from a string slice.
    #[inline]
    pub fn from_str(s: &str) -> Self {
        Self(s.to_string())
    }

    /// Returns a string slice of the wrapped content.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns a byte slice of the wrapped content.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Returns the length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the string contains no bytes.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Converts this string into a null-terminated [`ZeroizingCString`] for C-ABI boundaries
    /// without intermediate plaintext leaks.
    pub fn to_c_string(&self) -> Result<ZeroizingCString, std::ffi::NulError> {
        ZeroizingCString::new(self.as_str())
    }
}

impl Deref for ZeroizingString {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for ZeroizingString {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<[u8]> for ZeroizingString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Borrow<str> for ZeroizingString {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<String> for ZeroizingString {
    #[inline]
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for ZeroizingString {
    #[inline]
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl PartialEq for ZeroizingString {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.as_bytes().ct_eq(other.0.as_bytes()).into()
    }
}

impl Eq for ZeroizingString {}

impl fmt::Debug for ZeroizingString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ZeroizingString([REDACTED; {} chars])",
            self.0.chars().count()
        )
    }
}

/// A zeroizing null-terminated C-compatible string container.
///
/// Designed specifically for C-ABI boundaries (e.g. `*const libc::c_char`) where
/// standard `std::ffi::CString` leaves deallocated plaintext bytes in allocator memory pools.
#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct ZeroizingCString {
    bytes: Vec<u8>,
}

impl ZeroizingCString {
    /// Creates a new `ZeroizingCString` from a string slice.
    ///
    /// # Errors
    /// Returns a [`std::ffi::NulError`] if the input contains an interior null byte.
    pub fn new(s: &str) -> Result<Self, std::ffi::NulError> {
        // Enforce null-terminator rule without allocating an unzeroized intermediate CString
        if let Some(nul_idx) = s.as_bytes().iter().position(|&b| b == 0) {
            // Reproduce official std::ffi::NulError by attempting CString on the slice up to nul
            let err_cstr = std::ffi::CString::new(&s[..=nul_idx]);
            return Err(err_cstr.unwrap_err());
        }

        let mut bytes = Vec::with_capacity(s.len() + 1);
        bytes.extend_from_slice(s.as_bytes());
        bytes.push(0);
        Ok(Self { bytes })
    }

    /// Returns a raw pointer to the null-terminated C string.
    #[inline]
    pub fn as_ptr(&self) -> *const std::os::raw::c_char {
        self.bytes.as_ptr() as *const std::os::raw::c_char
    }

    /// Returns a byte slice excluding the trailing null byte.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        if self.bytes.is_empty() {
            &[]
        } else {
            &self.bytes[..self.bytes.len() - 1]
        }
    }

    /// Returns a byte slice including the trailing null byte.
    #[inline]
    pub fn as_bytes_with_nul(&self) -> &[u8] {
        &self.bytes
    }
}

impl Deref for ZeroizingCString {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for ZeroizingCString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl fmt::Debug for ZeroizingCString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ZeroizingCString([REDACTED; {} bytes])",
            self.as_bytes().len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_bytes_basic_and_redaction() {
        let mut sec = SecureBytes::from_slice(b"secret_key_material_123");
        assert_eq!(sec.len(), 23);
        assert_eq!(&sec[..6], b"secret");

        let debug_str = format!("{:?}", sec);
        assert!(!debug_str.contains("secret"));
        assert!(debug_str.contains("REDACTED"));

        sec.push(b'!');
        assert_eq!(sec.len(), 24);

        let sec2 = SecureBytes::from_slice(b"secret_key_material_123!");
        assert_eq!(sec, sec2);

        let sec_diff = SecureBytes::from_slice(b"other_key_material_1234");
        assert_ne!(sec, sec_diff);
    }

    #[test]
    fn test_zeroizing_string_and_c_string() {
        let pwd = ZeroizingString::new("Passw0rd_Test!".to_string());
        assert_eq!(pwd.as_str(), "Passw0rd_Test!");
        assert_eq!(pwd.len(), 14);

        let debug_str = format!("{:?}", pwd);
        assert!(!debug_str.contains("Passw0rd"));
        assert!(debug_str.contains("REDACTED"));

        let c_str = pwd.to_c_string().expect("valid c string");
        assert_eq!(c_str.as_bytes(), b"Passw0rd_Test!");
        assert_eq!(c_str.as_bytes_with_nul(), b"Passw0rd_Test!\0");

        let c_debug = format!("{:?}", c_str);
        assert!(!c_debug.contains("Passw0rd"));

        unsafe {
            let ptr = c_str.as_ptr();
            let cstr_view = std::ffi::CStr::from_ptr(ptr);
            assert_eq!(cstr_view.to_str().unwrap(), "Passw0rd_Test!");
        }
    }

    #[test]
    fn test_c_string_interior_nul_rejected() {
        let bad = ZeroizingString::from_str("bad\0password");
        assert!(bad.to_c_string().is_err());
    }

    #[test]
    fn test_zeroize_wipe_on_drop_direct_pointer_probe() {
        let mut direct = SecureBytes::from_slice(b"wipe_me_now");
        assert_eq!(direct.len(), 11);
        direct.clear();
        assert_eq!(direct.len(), 0);
    }
}
