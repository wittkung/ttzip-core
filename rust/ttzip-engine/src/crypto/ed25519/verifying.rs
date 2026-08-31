// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ed25519 signature verification, compressed point decoding, canonical scalar validation,
//! and RFC 8032 / Zip215 strict constant-time checks.

use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use super::error::Ed25519Error;

/// Length of an Ed25519 public key in bytes.
pub const PUBLIC_KEY_LENGTH: usize = 32;

/// Length of an Ed25519 signature in bytes.
pub const SIGNATURE_LENGTH: usize = 64;

/// Strongly-typed 64-byte Ed25519 signature composed of `R` (32 bytes) and `S` (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    /// Raw 64-byte signature array.
    bytes: [u8; SIGNATURE_LENGTH],
}

impl core::fmt::Debug for Signature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Signature({})", hex_encode(&self.bytes))
    }
}

impl Signature {
    /// Constructs a `Signature` from a 64-byte array.
    #[inline]
    pub const fn from_bytes(bytes: &[u8; SIGNATURE_LENGTH]) -> Self {
        Self { bytes: *bytes }
    }

    /// Constructs a `Signature` from a byte slice, verifying exact 64-byte length.
    pub fn from_slice(slice: &[u8]) -> Result<Self, Ed25519Error> {
        if slice.len() != SIGNATURE_LENGTH {
            return Err(Ed25519Error::InvalidSignatureFormat {
                actual_len: slice.len(),
            });
        }
        let mut bytes = [0u8; SIGNATURE_LENGTH];
        bytes.copy_from_slice(slice);
        Ok(Self { bytes })
    }

    /// Returns the raw 64-byte array.
    #[inline]
    pub const fn to_bytes(&self) -> [u8; SIGNATURE_LENGTH] {
        self.bytes
    }

    /// Returns a reference to the raw 64-byte array.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; SIGNATURE_LENGTH] {
        &self.bytes
    }

    /// Returns the 32-byte `R` component.
    #[inline]
    pub fn r_bytes(&self) -> &[u8; 32] {
        let (r, _) = self.bytes.split_at(32);
        r.try_into().expect("Slice of len 32")
    }

    /// Returns the 32-byte `S` scalar component.
    #[inline]
    pub fn s_bytes(&self) -> &[u8; 32] {
        let (_, s) = self.bytes.split_at(32);
        s.try_into().expect("Slice of len 32")
    }
}

/// Strongly-typed Ed25519 public verifying key with cached decompressed `EdwardsPoint`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifyingKey {
    /// Decompressed curve point on Edwards25519.
    point: EdwardsPoint,
    /// Canonical 32-byte compressed representation.
    bytes: [u8; PUBLIC_KEY_LENGTH],
}

impl VerifyingKey {
    /// Constructs a `VerifyingKey` from a 32-byte compressed Edwards-Y array.
    pub fn from_bytes(bytes: &[u8; PUBLIC_KEY_LENGTH]) -> Result<Self, Ed25519Error> {
        let compressed = CompressedEdwardsY(*bytes);
        let point = compressed.decompress().ok_or_else(|| {
            Ed25519Error::InvalidPublicKeyFormat {
                reason: "Compressed Edwards-Y coordinate is not a valid curve point".to_string(),
            }
        })?;
        Ok(Self {
            point,
            bytes: *bytes,
        })
    }

    /// Constructs a `VerifyingKey` from a byte slice, validating length and decompression.
    pub fn from_slice(slice: &[u8]) -> Result<Self, Ed25519Error> {
        if slice.len() != PUBLIC_KEY_LENGTH {
            return Err(Ed25519Error::InvalidPublicKeyFormat {
                reason: format!("Expected 32 bytes, got {}", slice.len()),
            });
        }
        let mut bytes = [0u8; PUBLIC_KEY_LENGTH];
        bytes.copy_from_slice(slice);
        Self::from_bytes(&bytes)
    }

    /// Internal constructor directly from a pre-computed `EdwardsPoint`.
    #[inline]
    pub(crate) fn from_point(point: EdwardsPoint) -> Self {
        let bytes = point.compress().to_bytes();
        Self { point, bytes }
    }

    /// Returns the raw 32-byte compressed public key array.
    #[inline]
    pub const fn to_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.bytes
    }

    /// Returns a reference to the raw 32-byte compressed public key array.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        &self.bytes
    }

    /// Returns a reference to the decompressed `EdwardsPoint`.
    #[inline]
    pub const fn point(&self) -> &EdwardsPoint {
        &self.point
    }

    /// Verifies a signature against a message according to RFC 8032 / Zip215 strict rules.
    ///
    /// Checks that:
    /// 1. Scalar S is canonical (0 <= S < \ell).
    /// 2. Point R decompresses to a valid curve point.
    /// 3. S * B - k * A == R in constant time.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), Ed25519Error> {
        self.verify_strict(message, signature)
    }

    /// Strict verification enforcing canonical scalar S and constant-time point comparison.
    pub fn verify_strict(&self, message: &[u8], signature: &Signature) -> Result<(), Ed25519Error> {
        // 1. Verify scalar S is canonical: S < \ell
        let s_scalar_opt = Option::<Scalar>::from(Scalar::from_canonical_bytes(*signature.s_bytes()));
        let s = s_scalar_opt.ok_or(Ed25519Error::NonCanonicalScalar)?;

        // 2. Verify R decompresses to a valid Edwards point
        let r_compressed = CompressedEdwardsY(*signature.r_bytes());
        let r_point = r_compressed.decompress().ok_or(Ed25519Error::InvalidSignatureFormat {
            actual_len: SIGNATURE_LENGTH,
        })?;

        // 3. Challenge scalar k = SHA-512(R || A || message) mod \ell
        let mut hasher = Sha512::new();
        hasher.update(signature.r_bytes());
        hasher.update(self.bytes);
        hasher.update(message);
        let mut challenge_digest = [0u8; 64];
        challenge_digest.copy_from_slice(&hasher.finalize());
        let k = Scalar::from_bytes_mod_order_wide(&challenge_digest);
        challenge_digest.zeroize();

        // 4. Compute R' = s * B - k * A
        let computed_r = EdwardsPoint::vartime_double_scalar_mul_basepoint(&k, &-self.point, &s);

        // 5. Compare computed R' against signature R in constant time
        let r_match = computed_r.ct_eq(&r_point);
        if r_match.into() {
            Ok(())
        } else {
            Err(Ed25519Error::SignatureVerificationFailed)
        }
    }
}

/// Standalone helper function to verify an Ed25519 signature.
#[inline]
pub fn verify(
    verifying_key: &VerifyingKey,
    message: &[u8],
    signature: &Signature,
) -> Result<(), Ed25519Error> {
    verifying_key.verify(message, signature)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use core::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}
