// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Traditional PKZIP 3-Key Stream Cipher implementation with hardware acceleration and Zeroize.

pub mod scalar;
pub mod simd;
#[cfg(test)]
pub mod tests;

use zeroize::{Zeroize, ZeroizeOnDrop};

pub use scalar::{
    crc32_byte, decrypt_byte_key, update_keys, ZIPCRYPTO_KEY0_INIT, ZIPCRYPTO_KEY1_INIT,
    ZIPCRYPTO_KEY2_INIT, ZIPCRYPTO_MULT,
};
pub use simd::{decrypt_stream_fast, encrypt_stream_fast, update_keys_fast, ZipCryptoBatch4};

/// 3-Key state for the traditional PKZIP stream cipher (`key0`, `key1`, `key2`).
///
/// Automatically zeroes secret key state out of memory when dropped.
#[repr(C)]
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct ZipCryptoKeys {
    pub key0: u32,
    pub key1: u32,
    pub key2: u32,
}

impl core::fmt::Debug for ZipCryptoKeys {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ZipCryptoKeys")
            .field("key0", &"[REDACTED]")
            .field("key1", &"[REDACTED]")
            .field("key2", &"[REDACTED]")
            .finish()
    }
}

impl Default for ZipCryptoKeys {
    #[inline]
    fn default() -> Self {
        Self {
            key0: ZIPCRYPTO_KEY0_INIT,
            key1: ZIPCRYPTO_KEY1_INIT,
            key2: ZIPCRYPTO_KEY2_INIT,
        }
    }
}

impl ZipCryptoKeys {
    /// Creates a new uninitialized `ZipCryptoKeys` structure with standard initial constants.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Initializes cipher keys from a password byte slice.
    pub fn from_password(password: &[u8]) -> Self {
        let mut keys = Self::default();
        keys.init_with_password(password);
        keys
    }

    /// Updates current key state with the password bytes.
    #[inline]
    pub fn init_with_password(&mut self, password: &[u8]) {
        for &b in password {
            self.update(b);
        }
    }

    /// Updates internal key states using the provided plaintext byte.
    #[inline(always)]
    pub fn update(&mut self, plain_byte: u8) {
        update_keys_fast(&mut self.key0, &mut self.key1, &mut self.key2, plain_byte);
    }

    /// Returns the current keystream byte.
    #[inline(always)]
    pub fn keystream_byte(&self) -> u8 {
        decrypt_byte_key(self.key2)
    }

    /// Decrypts a single byte and advances the internal key state.
    #[inline(always)]
    pub fn decrypt_byte(&mut self, cipher_byte: u8) -> u8 {
        let k = self.keystream_byte();
        let plain = cipher_byte ^ k;
        self.update(plain);
        plain
    }

    /// Encrypts a single byte and advances the internal key state.
    #[inline(always)]
    pub fn encrypt_byte(&mut self, plain_byte: u8) -> u8 {
        let k = self.keystream_byte();
        let cipher = plain_byte ^ k;
        self.update(plain_byte);
        cipher
    }

    /// Decrypts a slice of bytes in place.
    #[inline]
    pub fn decrypt_slice(&mut self, data: &mut [u8]) {
        decrypt_stream_fast(&mut self.key0, &mut self.key1, &mut self.key2, data);
    }

    /// Encrypts a slice of bytes in place.
    #[inline]
    pub fn encrypt_slice(&mut self, data: &mut [u8]) {
        encrypt_stream_fast(&mut self.key0, &mut self.key1, &mut self.key2, data);
    }

    /// Decrypts source buffer into destination buffer.
    #[inline]
    pub fn decrypt_copy(&mut self, src: &[u8], dst: &mut [u8]) {
        let len = src.len().min(dst.len());
        dst[..len].copy_from_slice(&src[..len]);
        self.decrypt_slice(&mut dst[..len]);
    }

    /// Encrypts source buffer into destination buffer.
    #[inline]
    pub fn encrypt_copy(&mut self, src: &[u8], dst: &mut [u8]) {
        let len = src.len().min(dst.len());
        dst[..len].copy_from_slice(&src[..len]);
        self.encrypt_slice(&mut dst[..len]);
    }
}

/// Convenience helper to decrypt a slice given a password.
pub fn zipcrypto_decrypt_slice(password: &[u8], data: &mut [u8]) {
    let mut keys = ZipCryptoKeys::from_password(password);
    keys.decrypt_slice(data);
}

/// Convenience helper to encrypt a slice given a password.
pub fn zipcrypto_encrypt_slice(password: &[u8], data: &mut [u8]) {
    let mut keys = ZipCryptoKeys::from_password(password);
    keys.encrypt_slice(data);
}

use crate::types::TTZipStatus;

/// PKZIP Traditional ZipCrypto stream cipher engine.
///
/// Encapsulates the 3-Key state machine with 12-byte encryption header verification,
/// streaming payload encryption/decryption, and `Zeroize` memory wiping.
#[repr(C)]
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct ZipCryptoEngine {
    pub keys: ZipCryptoKeys,
}

impl core::fmt::Debug for ZipCryptoEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ZipCryptoEngine")
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

impl ZipCryptoEngine {
    /// Initializes a new `ZipCryptoEngine` from a plaintext password slice.
    #[inline]
    pub fn new(password: &[u8]) -> Self {
        Self {
            keys: ZipCryptoKeys::from_password(password),
        }
    }

    /// Creates an engine from pre-initialized keys.
    #[inline]
    pub fn from_keys(keys: ZipCryptoKeys) -> Self {
        Self { keys }
    }

    /// Verifies the 12-byte encryption header against the expected check byte.
    ///
    /// The 12-byte header is decrypted in place. The 12th byte (index 11) is compared
    /// against `expected_check_byte`.
    /// - If matching, the key state is successfully primed and ready to decrypt payload data.
    /// - If mismatching, returns `Err(TTZipStatus::ErrInvalidPassword)`.
    pub fn decrypt_header(&mut self, header: &[u8; 12], expected_check_byte: u8) -> Result<(), TTZipStatus> {
        let mut dec_hdr = *header;
        self.keys.decrypt_slice(&mut dec_hdr);
        if dec_hdr[11] == expected_check_byte {
            Ok(())
        } else {
            Err(TTZipStatus::ErrInvalidPassword)
        }
    }

    /// Verifies a 12-byte encryption header and returns an initialized `ZipCryptoEngine`.
    ///
    /// # Check Byte Derivation (PKZIP APPNOTE)
    /// - When `bit3_data_descriptor` is `true`:
    ///   Uses high byte of MS-DOS modification time: `((dos_time >> 8) & 0xFF) as u8`.
    /// - When `bit3_data_descriptor` is `false`:
    ///   Uses high byte of CRC-32: `((crc32 >> 24) & 0xFF) as u8`.
    pub fn verify_and_init(
        password: &[u8],
        header: &[u8; 12],
        crc32: u32,
        dos_time: u16,
        bit3_data_descriptor: bool,
    ) -> Result<Self, TTZipStatus> {
        let expected_check_byte = if bit3_data_descriptor {
            ((dos_time >> 8) & 0xFF) as u8
        } else {
            ((crc32 >> 24) & 0xFF) as u8
        };

        let mut engine = Self::new(password);
        engine.decrypt_header(header, expected_check_byte)?;
        Ok(engine)
    }

    /// Generates a 12-byte encrypted header given an expected check byte and 11 random bytes.
    ///
    /// Encrypts the 12-byte header with the internal keys, updating the key state so that
    /// following `encrypt_slice` calls seamlessly continue encrypting payload data.
    pub fn generate_header(&mut self, expected_check_byte: u8, random_11_bytes: &[u8; 11]) -> [u8; 12] {
        let mut header = [0u8; 12];
        header[..11].copy_from_slice(random_11_bytes);
        header[11] = expected_check_byte;
        self.keys.encrypt_slice(&mut header);
        header
    }

    /// Decrypts a slice of bytes in place, advancing the internal key state.
    #[inline]
    pub fn decrypt_slice(&mut self, data: &mut [u8]) {
        self.keys.decrypt_slice(data);
    }

    /// Encrypts a slice of bytes in place, advancing the internal key state.
    #[inline]
    pub fn encrypt_slice(&mut self, data: &mut [u8]) {
        self.keys.encrypt_slice(data);
    }

    /// Decrypts a single byte and advances internal key state.
    #[inline]
    pub fn decrypt_byte(&mut self, cipher_byte: u8) -> u8 {
        self.keys.decrypt_byte(cipher_byte)
    }

    /// Encrypts a single byte and advances internal key state.
    #[inline]
    pub fn encrypt_byte(&mut self, plain_byte: u8) -> u8 {
        self.keys.encrypt_byte(plain_byte)
    }

    /// Decrypts source buffer into destination buffer.
    #[inline]
    pub fn decrypt_copy(&mut self, src: &[u8], dst: &mut [u8]) {
        self.keys.decrypt_copy(src, dst);
    }

    /// Encrypts source buffer into destination buffer.
    #[inline]
    pub fn encrypt_copy(&mut self, src: &[u8], dst: &mut [u8]) {
        self.keys.encrypt_copy(src, dst);
    }
}

