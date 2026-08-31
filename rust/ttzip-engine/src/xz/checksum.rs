// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance composite streaming checksum engine for XZ containers.
//!
//! Provides hardware/table-accelerated CRC32, CRC64 (ECMA-182 LSb-first),
//! and SHA-256 verification compliant with the XZ File Format Specification.

use core::hash::Hasher;
use sha2::Digest;

/// ECMA-182 CRC-64 reversed polynomial (LSb-first) used in XZ format.
const CRC64_ECMA_REVERSED_POLY: u64 = 0xC96C_5795_D787_0F42;

/// Precomputed Slicing-by-8 lookup table for high-throughput CRC64 computation.
static CRC64_TABLE: [[u64; 256]; 8] = {
    let mut table = [[0u64; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u64;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ CRC64_ECMA_REVERSED_POLY;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[0][i] = crc;
        i += 1;
    }

    let mut j = 0;
    while j < 256 {
        let mut i = 1;
        while i < 8 {
            let prev = table[i - 1][j];
            table[i][j] = (prev >> 8) ^ table[0][(prev & 0xFF) as usize];
            i += 1;
        }
        j += 1;
    }
    table
};

/// High-performance CRC-64 ECMA-182 streaming hasher for XZ containers.
///
/// Uses LSb-first polynomial `0xC96C_5795_D787_0F42` with 8-way table slicing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzCrc64 {
    state: u64,
}

impl Default for XzCrc64 {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl XzCrc64 {
    /// Creates a new CRC-64 hasher initialized with the standard seed (`!0`).
    #[inline]
    pub const fn new() -> Self {
        Self { state: !0 }
    }

    /// Creates a new CRC-64 hasher with a custom starting state.
    #[inline]
    pub const fn with_state(state: u64) -> Self {
        Self { state }
    }

    /// Resets internal state to the standard seed (`!0`).
    #[inline]
    pub fn reset(&mut self) {
        self.state = !0;
    }

    /// Incrementally updates the checksum with incoming byte slice.
    #[inline]
    pub fn update(&mut self, data: &[u8]) {
        let mut crc = self.state;
        let mut chunks = data.chunks_exact(8);

        for chunk in &mut chunks {
            let val = u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3],
                chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
            let term = crc ^ val;
            crc = CRC64_TABLE[7][(term & 0xFF) as usize]
                ^ CRC64_TABLE[6][((term >> 8) & 0xFF) as usize]
                ^ CRC64_TABLE[5][((term >> 16) & 0xFF) as usize]
                ^ CRC64_TABLE[4][((term >> 24) & 0xFF) as usize]
                ^ CRC64_TABLE[3][((term >> 32) & 0xFF) as usize]
                ^ CRC64_TABLE[2][((term >> 40) & 0xFF) as usize]
                ^ CRC64_TABLE[1][((term >> 48) & 0xFF) as usize]
                ^ CRC64_TABLE[0][((term >> 56) & 0xFF) as usize];
        }

        for &b in chunks.remainder() {
            let idx = ((crc as u8) ^ b) as usize;
            crc = CRC64_TABLE[0][idx] ^ (crc >> 8);
        }

        self.state = crc;
    }

    /// Finalizes the checksum and returns the computed 64-bit integer.
    #[inline]
    pub fn finish(&self) -> u64 {
        !self.state
    }

    /// Alias for `finish()`.
    #[inline]
    pub fn digest(&self) -> u64 {
        self.finish()
    }

    /// Returns the 8-byte checksum in Little-Endian byte order as defined in XZ spec.
    #[inline]
    pub fn digest_bytes(&self) -> [u8; 8] {
        self.finish().to_le_bytes()
    }
}

impl Hasher for XzCrc64 {
    #[inline]
    fn finish(&self) -> u64 {
        self.finish()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }
}

/// Computes XZ-compliant CRC-64 checksum for a complete slice.
#[inline]
pub fn crc64_xz(data: &[u8]) -> u64 {
    let mut hasher = XzCrc64::new();
    hasher.update(data);
    hasher.finish()
}

/// Updates an existing CRC-64 checksum with additional data.
#[inline]
pub fn crc64_xz_update(seed: u64, data: &[u8]) -> u64 {
    let mut hasher = XzCrc64::with_state(!seed);
    hasher.update(data);
    hasher.finish()
}

/// Supported XZ Stream / Block Check Types as defined in XZ specification §5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum XzChecksumType {
    /// No integrity check (0 bytes).
    None = 0x00,
    /// IEEE 802.3 CRC-32 (4 bytes, Little-Endian).
    Crc32 = 0x01,
    /// ECMA-182 LSb-first CRC-64 (8 bytes, Little-Endian).
    Crc64 = 0x04,
    /// SHA-256 (32 bytes).
    Sha256 = 0x0A,
}

impl XzChecksumType {
    /// Attempts to parse an XZ Check ID into a supported `XzChecksumType`.
    pub fn from_id(id: u8) -> Result<Self, XzChecksumError> {
        match id {
            0x00 => Ok(Self::None),
            0x01 => Ok(Self::Crc32),
            0x04 => Ok(Self::Crc64),
            0x0A => Ok(Self::Sha256),
            other => Err(XzChecksumError::UnsupportedCheckType(other)),
        }
    }

    /// Returns the raw 4-bit Check ID.
    #[inline]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// Returns the digest size in bytes for this check type.
    #[inline]
    pub const fn check_size(self) -> usize {
        match self {
            Self::None => 0,
            Self::Crc32 => 4,
            Self::Crc64 => 8,
            Self::Sha256 => 32,
        }
    }
}

/// Errors originating from XZ checksum computation or verification.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XzChecksumError {
    /// Encountered an unsupported or reserved Check Type ID.
    #[error("unsupported XZ check type ID: {0:#04x}")]
    UnsupportedCheckType(u8),

    /// Digest buffer length mismatch.
    #[error("invalid digest length for {check_type:?}: expected {expected} bytes, got {actual} bytes")]
    InvalidDigestLength {
        check_type: XzChecksumType,
        expected: usize,
        actual: usize,
    },

    /// Checksum verification mismatch.
    #[error("XZ checksum mismatch for {check_type:?}: expected {expected}, actual {actual}")]
    ChecksumMismatch {
        check_type: XzChecksumType,
        expected: String,
        actual: String,
    },
}

/// Polymorphic streaming checksum engine for XZ containers.
///
/// Supports zero-allocation incremental updates and constant-time verification.
#[derive(Clone)]
pub enum XzChecksumEngine {
    /// No check.
    None,
    /// IEEE 802.3 CRC-32.
    Crc32(crc32fast::Hasher),
    /// ECMA-182 CRC-64.
    Crc64(XzCrc64),
    /// SHA-256.
    Sha256(sha2::Sha256),
}

impl core::fmt::Debug for XzChecksumEngine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "XzChecksumEngine::None"),
            Self::Crc32(_) => write!(f, "XzChecksumEngine::Crc32"),
            Self::Crc64(crc) => write!(f, "XzChecksumEngine::Crc64({:#018x})", crc.finish()),
            Self::Sha256(_) => write!(f, "XzChecksumEngine::Sha256"),
        }
    }
}

impl Default for XzChecksumEngine {
    #[inline]
    fn default() -> Self {
        Self::none()
    }
}

impl XzChecksumEngine {
    /// Creates a new checksum engine initialized for the specified `XzChecksumType`.
    pub fn new(check_type: XzChecksumType) -> Self {
        match check_type {
            XzChecksumType::None => Self::None,
            XzChecksumType::Crc32 => Self::Crc32(crc32fast::Hasher::new()),
            XzChecksumType::Crc64 => Self::Crc64(XzCrc64::new()),
            XzChecksumType::Sha256 => Self::Sha256(sha2::Sha256::new()),
        }
    }

    /// Creates a None (no check) engine.
    #[inline]
    pub const fn none() -> Self {
        Self::None
    }

    /// Creates a CRC-32 engine.
    #[inline]
    pub fn crc32() -> Self {
        Self::Crc32(crc32fast::Hasher::new())
    }

    /// Creates a CRC-64 engine.
    #[inline]
    pub fn crc64() -> Self {
        Self::Crc64(XzCrc64::new())
    }

    /// Creates a SHA-256 engine.
    #[inline]
    pub fn sha256() -> Self {
        Self::Sha256(sha2::Sha256::new())
    }

    /// Returns the associated `XzChecksumType`.
    #[inline]
    pub fn check_type(&self) -> XzChecksumType {
        match self {
            Self::None => XzChecksumType::None,
            Self::Crc32(_) => XzChecksumType::Crc32,
            Self::Crc64(_) => XzChecksumType::Crc64,
            Self::Sha256(_) => XzChecksumType::Sha256,
        }
    }

    /// Returns the expected digest size in bytes.
    #[inline]
    pub fn check_size(&self) -> usize {
        self.check_type().check_size()
    }

    /// Incrementally updates the internal checksum state with incoming data.
    #[inline]
    pub fn update(&mut self, data: &[u8]) {
        match self {
            Self::None => {}
            Self::Crc32(hasher) => hasher.update(data),
            Self::Crc64(hasher) => hasher.update(data),
            Self::Sha256(hasher) => hasher.update(data),
        }
    }

    /// Computes and returns the finalized digest bytes formatted according to XZ spec.
    pub fn digest(&self) -> Vec<u8> {
        match self {
            Self::None => Vec::new(),
            Self::Crc32(hasher) => hasher.clone().finalize().to_le_bytes().to_vec(),
            Self::Crc64(hasher) => hasher.digest_bytes().to_vec(),
            Self::Sha256(hasher) => hasher.clone().finalize().to_vec(),
        }
    }

    /// Writes the finalized digest directly into the destination slice without allocation.
    pub fn digest_into(&self, out: &mut [u8]) -> Result<usize, XzChecksumError> {
        let size = self.check_size();
        if out.len() < size {
            return Err(XzChecksumError::InvalidDigestLength {
                check_type: self.check_type(),
                expected: size,
                actual: out.len(),
            });
        }

        match self {
            Self::None => Ok(0),
            Self::Crc32(hasher) => {
                let bytes = hasher.clone().finalize().to_le_bytes();
                out[..4].copy_from_slice(&bytes);
                Ok(4)
            }
            Self::Crc64(hasher) => {
                let bytes = hasher.digest_bytes();
                out[..8].copy_from_slice(&bytes);
                Ok(8)
            }
            Self::Sha256(hasher) => {
                let bytes = hasher.clone().finalize();
                out[..32].copy_from_slice(&bytes);
                Ok(32)
            }
        }
    }

    /// Verifies that the internal computed digest matches the expected raw byte slice.
    pub fn verify(&self, expected: &[u8]) -> Result<(), XzChecksumError> {
        let expected_size = self.check_size();
        if expected.len() != expected_size {
            return Err(XzChecksumError::InvalidDigestLength {
                check_type: self.check_type(),
                expected: expected_size,
                actual: expected.len(),
            });
        }

        let mut calculated_buf = [0u8; 32];
        let calc_len = self.digest_into(&mut calculated_buf[..expected_size])?;
        let calculated = &calculated_buf[..calc_len];

        // Constant-time comparison to prevent timing side channels.
        let mut diff = 0u8;
        for (a, b) in calculated.iter().zip(expected.iter()) {
            diff |= a ^ b;
        }

        if diff != 0 {
            return Err(XzChecksumError::ChecksumMismatch {
                check_type: self.check_type(),
                expected: hex_encode(expected),
                actual: hex_encode(calculated),
            });
        }

        Ok(())
    }

    /// Resets the engine state for reuse.
    pub fn reset(&mut self) {
        match self {
            Self::None => {}
            Self::Crc32(hasher) => *hasher = crc32fast::Hasher::new(),
            Self::Crc64(hasher) => hasher.reset(),
            Self::Sha256(hasher) => *hasher = sha2::Sha256::new(),
        }
    }
}

/// Helper function to format byte slices into hex strings.
fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use core::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xz_crc64_calibration_vector() {
        let data = b"123456789";
        let crc = crc64_xz(data);
        assert_eq!(crc, 0x995DC9BBDF1939FA);
        assert_eq!(XzCrc64::default().finish(), 0);
    }

    #[test]
    fn test_xz_checksum_engine_roundtrip() {
        let payload = b"TTZip High-Performance Archiving and Compression Engine";

        let mut engine = XzChecksumEngine::crc64();
        engine.update(payload);
        let digest = engine.digest();
        assert_eq!(digest.len(), 8);
        assert!(engine.verify(&digest).is_ok());

        let mut corrupted = digest.clone();
        corrupted[0] ^= 0xFF;
        assert!(engine.verify(&corrupted).is_err());
    }
}
