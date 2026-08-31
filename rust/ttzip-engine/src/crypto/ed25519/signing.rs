// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ed25519 signature generation, scalar clamping, deterministic nonce derivation,
//! and zeroize-protected signing keys (RFC 8032).

use curve25519_dalek::constants::ED25519_BASEPOINT_TABLE;
use curve25519_dalek::scalar::Scalar;
use sha2::{Digest, Sha512};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::error::Ed25519Error;
use super::verifying::{Signature, VerifyingKey};

/// Length of an Ed25519 secret seed in bytes.
pub const SECRET_KEY_LENGTH: usize = 32;

/// Clamps a 32-byte scalar according to RFC 8032 specification.
///
/// Clears the lowest 3 bits of the first byte (making it a multiple of 8 to clear cofactor),
/// clears the highest bit of the last byte, and sets the second highest bit of the last byte.
#[inline]
pub fn scalar_clamp(scalar_bytes: &mut [u8; 32]) {
    scalar_bytes[0] &= 248;
    scalar_bytes[31] &= 127;
    scalar_bytes[31] |= 64;
}

/// Expands a 32-byte private seed via SHA-512 into a clamped secret scalar
/// and a 32-byte prefix used for deterministic nonce generation.
#[inline]
pub fn expand_secret_key(seed: &[u8; 32]) -> (Scalar, [u8; 32]) {
    let mut hasher = Sha512::new();
    hasher.update(seed);
    let mut hash_output = [0u8; 64];
    hash_output.copy_from_slice(&hasher.finalize());

    let mut scalar_bytes = [0u8; 32];
    scalar_bytes.copy_from_slice(&hash_output[0..32]);
    scalar_clamp(&mut scalar_bytes);

    let scalar = Scalar::from_bytes_mod_order(scalar_bytes);
    let mut prefix = [0u8; 32];
    prefix.copy_from_slice(&hash_output[32..64]);

    // Securely erase intermediate expanded scalar bytes
    scalar_bytes.zeroize();
    hash_output.zeroize();

    (scalar, prefix)
}

/// Derives the RFC 8032 deterministic nonce scalar `r = SHA-512(k || M) mod \ell`.
#[inline]
pub fn derive_deterministic_nonce(prefix: &[u8; 32], message: &[u8]) -> Scalar {
    let mut hasher = Sha512::new();
    hasher.update(prefix);
    hasher.update(message);
    let mut digest = [0u8; 64];
    digest.copy_from_slice(&hasher.finalize());
    let r = Scalar::from_bytes_mod_order_wide(&digest);
    digest.zeroize();
    r
}

/// Signs a message using a raw secret seed and returns a 64-byte Ed25519 signature.
pub fn sign(secret_seed: &[u8; 32], message: &[u8]) -> Signature {
    let key = SigningKey::from_bytes(secret_seed);
    key.sign(message)
}

/// 256-bit Ed25519 Signing Key with zeroization on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SigningKey {
    seed: [u8; SECRET_KEY_LENGTH],
}

impl SigningKey {
    /// Constructs a `SigningKey` from a 32-byte secret seed.
    pub fn from_bytes(seed: &[u8; SECRET_KEY_LENGTH]) -> Self {
        Self { seed: *seed }
    }

    /// Constructs a `SigningKey` from a slice of length 32.
    pub fn from_slice(slice: &[u8]) -> Result<Self, Ed25519Error> {
        if slice.len() != SECRET_KEY_LENGTH {
            return Err(Ed25519Error::InvalidSecretKeyFormat {
                actual_len: slice.len(),
            });
        }
        let mut seed = [0u8; SECRET_KEY_LENGTH];
        seed.copy_from_slice(slice);
        Ok(Self { seed })
    }

    /// Returns the 32-byte secret seed.
    pub fn to_bytes(&self) -> [u8; SECRET_KEY_LENGTH] {
        self.seed
    }

    /// Borrows the internal seed reference.
    pub fn as_bytes(&self) -> &[u8; SECRET_KEY_LENGTH] {
        &self.seed
    }

    /// Derives the corresponding `VerifyingKey` (public key).
    pub fn verifying_key(&self) -> VerifyingKey {
        let (scalar, mut prefix) = expand_secret_key(&self.seed);
        prefix.zeroize();
        let point = ED25519_BASEPOINT_TABLE * &scalar;
        VerifyingKey::from_point(point)
    }

    /// Generates an Ed25519 signature for the given message (RFC 8032 deterministic).
    pub fn sign(&self, message: &[u8]) -> Signature {
        let (scalar, mut prefix) = expand_secret_key(&self.seed);
        let public_point = ED25519_BASEPOINT_TABLE * &scalar;
        let public_bytes = public_point.compress().to_bytes();

        // r = SHA-512(prefix || message) mod \ell
        let r = derive_deterministic_nonce(&prefix, message);
        prefix.zeroize();

        // R = r * B
        let r_point = ED25519_BASEPOINT_TABLE * &r;
        let r_bytes = r_point.compress().to_bytes();

        // k = SHA-512(R || A || message) mod \ell
        let mut hasher = Sha512::new();
        hasher.update(r_bytes);
        hasher.update(public_bytes);
        hasher.update(message);
        let mut challenge_digest = [0u8; 64];
        challenge_digest.copy_from_slice(&hasher.finalize());
        let k = Scalar::from_bytes_mod_order_wide(&challenge_digest);
        challenge_digest.zeroize();

        // S = (r + k * s) mod \ell
        let s = r + (k * scalar);
        let s_bytes = s.to_bytes();

        let mut signature_bytes = [0u8; 64];
        signature_bytes[0..32].copy_from_slice(&r_bytes);
        signature_bytes[32..64].copy_from_slice(&s_bytes);

        Signature::from_bytes(&signature_bytes)
    }
}
