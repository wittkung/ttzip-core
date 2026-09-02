// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Appcast & Release Signature Defense Guard (`AppcastSignatureGuard`).
//!
//! Provides deterministic cryptographic verification, small-subgroup denial,
//! canonical scalar bounds checking ($S < \ell$), side-channel resistant
//! constant-time comparisons, and strict version monotonicity assertion
//! to prevent downgrade attacks and malicious appcast hijacking.

use ed25519_dalek::{Signature, VerifyingKey};

use super::SystemDefenseError;

/// Curve25519 group prime order $\ell$ in little-endian format.
/// $\ell = 2^{252} + 27742317777372353535851937790883648493$
pub const ED25519_ORDER_L: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

/// Edwards25519 small-subgroup points of order 1, 2, 4, 8 in compressed form.
pub const SMALL_SUBGROUP_POINTS: [[u8; 32]; 8] = [
    // Order 1: Identity (0, 1)
    [
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ],
    // Order 2: (0, -1)
    [
        0xec, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
    ],
    // Order 4: point 1
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ],
    // Order 4: point 2
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
    ],
    // Order 8: point 1
    [
        0x26, 0xe8, 0x95, 0x8f, 0xc2, 0xb2, 0x27, 0xb0,
        0x45, 0xc3, 0xf4, 0x89, 0xf2, 0xef, 0x98, 0xf0,
        0xd5, 0xdf, 0xac, 0x05, 0xd3, 0xc6, 0x33, 0x39,
        0xb1, 0x38, 0x02, 0x88, 0x6d, 0x53, 0xfc, 0x05,
    ],
    // Order 8: point 2
    [
        0xc7, 0x17, 0x6a, 0x70, 0x3d, 0x4d, 0xd8, 0x4f,
        0xba, 0x3c, 0x0b, 0x76, 0x0d, 0x10, 0x67, 0x0f,
        0x2a, 0x20, 0x53, 0xfa, 0x2c, 0x39, 0xcc, 0xc6,
        0x4e, 0xc7, 0xfd, 0x77, 0x92, 0xac, 0x03, 0x7a,
    ],
    // Order 8: point 3
    [
        0x26, 0xe8, 0x95, 0x8f, 0xc2, 0xb2, 0x27, 0xb0,
        0x45, 0xc3, 0xf4, 0x89, 0xf2, 0xef, 0x98, 0xf0,
        0xd5, 0xdf, 0xac, 0x05, 0xd3, 0xc6, 0x33, 0x39,
        0xb1, 0x38, 0x02, 0x88, 0x6d, 0x53, 0xfc, 0x85,
    ],
    // Order 8: point 4
    [
        0xc7, 0x17, 0x6a, 0x70, 0x3d, 0x4d, 0xd8, 0x4f,
        0xba, 0x3c, 0x0b, 0x76, 0x0d, 0x10, 0x67, 0x0f,
        0x2a, 0x20, 0x53, 0xfa, 0x2c, 0x39, 0xcc, 0xc6,
        0x4e, 0xc7, 0xfd, 0x77, 0x92, 0xac, 0x03, 0xfa,
    ],
];

/// Constant-time equality comparison between two 32-byte slices.
#[inline]
pub fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    std::hint::black_box(diff) == 0
}

/// Constant-time equality comparison between two 64-byte slices.
#[inline]
pub fn constant_time_eq_64(a: &[u8; 64], b: &[u8; 64]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..64 {
        diff |= a[i] ^ b[i];
    }
    std::hint::black_box(diff) == 0
}

/// Guard for validating cryptographic signatures on appcasts and software release manifests.
#[derive(Debug, Clone, Default)]
pub struct AppcastSignatureGuard;

impl AppcastSignatureGuard {
    /// Creates a new `AppcastSignatureGuard`.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Checks if a 32-byte compressed point belongs to any known small subgroup (orders 1, 2, 4, 8).
    #[inline]
    #[must_use]
    pub fn is_small_subgroup_point(point: &[u8; 32]) -> bool {
        for known in &SMALL_SUBGROUP_POINTS {
            if constant_time_eq_32(point, known) {
                return true;
            }
        }
        false
    }

    /// Verifies that scalar $S$ in an Ed25519 signature is strictly canonical ($S < \ell$).
    #[inline]
    #[must_use]
    pub fn is_canonical_scalar(s_bytes: &[u8; 32]) -> bool {
        for i in (0..32).rev() {
            if s_bytes[i] < ED25519_ORDER_L[i] {
                return true;
            }
            if s_bytes[i] > ED25519_ORDER_L[i] {
                return false;
            }
        }
        false
    }

    /// Verifies that public key and signature structure adhere to strict non-malleability rules.
    pub fn validate_signature_structure(
        public_key: &[u8; 32],
        signature: &[u8; 64],
    ) -> Result<(), SystemDefenseError> {
        // 1. Check small subgroup point on public key
        if Self::is_small_subgroup_point(public_key) {
            return Err(SystemDefenseError::SmallSubgroupKeyDetected {
                key_prefix: hex_prefix_8(public_key),
            });
        }

        // 2. Check small subgroup point on signature R component
        let mut r_bytes = [0u8; 32];
        r_bytes.copy_from_slice(&signature[0..32]);
        if Self::is_small_subgroup_point(&r_bytes) {
            return Err(SystemDefenseError::MalleableSignatureDetected {
                reason: "Signature commitment point R belongs to low-order torsion subgroup".to_string(),
            });
        }

        // 3. Check scalar S < \ell
        let mut s_bytes = [0u8; 32];
        s_bytes.copy_from_slice(&signature[32..64]);
        if !Self::is_canonical_scalar(&s_bytes) {
            return Err(SystemDefenseError::NonCanonicalScalarDetected);
        }

        Ok(())
    }

    /// Verifies an Ed25519 signature over `message` using `public_key`.
    pub fn verify_signature(
        &self,
        public_key: &[u8; 32],
        signature: &[u8; 64],
        message: &[u8],
    ) -> Result<(), SystemDefenseError> {
        Self::validate_signature_structure(public_key, signature)?;

        let vk = VerifyingKey::from_bytes(public_key).map_err(|e| {
            SystemDefenseError::SignatureVerificationFailed {
                reason: format!("Invalid public key encoding: {e}"),
            }
        })?;

        let sig = Signature::from_bytes(signature);

        vk.verify_strict(message, &sig).map_err(|e| {
            SystemDefenseError::SignatureVerificationFailed {
                reason: format!("Ed25519 signature mismatch: {e}"),
            }
        })
    }

    /// Validates version monotonicity to prevent downgrade / rollback attacks.
    /// Returns `Ok(())` if `new_version` is strictly greater than `current_version`.
    pub fn assert_version_monotonicity(
        &self,
        current_version: &str,
        new_version: &str,
    ) -> Result<(), SystemDefenseError> {
        let current_parts = parse_version_tuple(current_version);
        let new_parts = parse_version_tuple(new_version);

        if new_parts <= current_parts {
            return Err(SystemDefenseError::VersionDowngradeDetected {
                current: current_version.to_string(),
                attempted: new_version.to_string(),
            });
        }

        Ok(())
    }
}

/// Helper function to format first 8 bytes of a key as hex.
fn hex_prefix_8(bytes: &[u8; 32]) -> String {
    let mut s = String::with_capacity(16);
    for b in &bytes[0..8] {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Parses a version string into a sequence of numeric and lexical tuple components.
fn parse_version_tuple(ver: &str) -> Vec<u64> {
    let clean = ver.trim_start_matches(|c: char| c == 'v' || c == 'V');
    let mut parts = Vec::new();

    for segment in clean.split(|c: char| c == '.' || c == '-' || c == '+' || c == '_') {
        if let Ok(num) = segment.parse::<u64>() {
            parts.push(num);
        } else {
            // Extract leading digits if any
            let digits: String = segment.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(num) = digits.parse::<u64>() {
                parts.push(num);
            }
        }
    }

    if parts.is_empty() {
        parts.push(0);
    }
    parts
}
