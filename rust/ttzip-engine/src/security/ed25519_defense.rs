// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ed25519 6-Layer Defense-in-Depth Security Guards & Compliance Subsystem (RFC 8032, ZIP 215).
//!
//! Enforces deterministic cryptographic safety invariants, side-channel resistance,
//! anti-malleability, small-subgroup denial, and resource exhaustion circuit breakers:
//! 1. **Small-Subgroup & Cofactor Attack Denial Guard (`SubgroupAttackGuard`)**: Intercepts and rejects
//!    Curve25519 low-order torsion points (orders 1, 2, 4, 8) to immunize against small-subgroup key
//!    recovery and cofactor leakage attacks.
//! 2. **Anti-Malleability & Non-Canonical Scalar Guard (`MalleabilityGuard`)**: Rejects non-canonical
//!    scalars $S \ge \ell$ where $\ell = 2^{252} + 27742317777372353535851937790883648493$, preventing
//!    signature malleability and transaction malleability attacks.
//! 3. **Sensitive Private Key Zeroize & Stack Sanitization (`GuardedSigningKey`)**: Wraps signing keys
//!    in automatic zeroizing containers (`Zeroize`, `ZeroizeOnDrop`) and enforces redacted `Debug` formatting.
//! 4. **Constant-Time Comparison Guard (`constant_time_eq_64`)**: Eliminates timing side-channels in
//!    public key and signature comparisons using constant-time accumulators with black-box barriers.
//! 5. **Resource Quota & Certificate Chain Circuit Breakers (`Ed25519DefenseConfig`)**: Caps maximum
//!    message size (default: 1 GiB) and certificate chain recursion depth (default: 8) to prevent DoS/OOM.
//! 6. **Strongly-Typed Key & Signature Compile-Time Bounds**: Enforces exact 32-byte public keys and
//!    64-byte signatures at compile time via strong types [`Ed25519PublicKey`] and [`Ed25519Signature`].

use std::fmt;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::types::TTZipStatus;

/// Standard length of an Ed25519 public key in bytes.
pub const ED25519_PUBLIC_KEY_LEN: usize = 32;
/// Standard length of an Ed25519 secret seed/key in bytes.
pub const ED25519_SECRET_KEY_LEN: usize = 32;
/// Standard length of an Ed25519 signature in bytes.
pub const ED25519_SIGNATURE_LEN: usize = 64;

/// Default maximum allowable single message size in bytes for signature operations (1 GiB).
pub const ED25519_DEFAULT_MAX_MSG_SIZE: u64 = 1024 * 1024 * 1024;
/// Default maximum certificate / trust chain recursion depth (8 levels).
pub const ED25519_DEFAULT_MAX_CERT_CHAIN_DEPTH: usize = 8;

/// Curve25519 group prime order $\ell$ in little-endian byte format.
/// $\ell = 2^{252} + 27742317777372353535851937790883648493$
pub const ED25519_ORDER_L: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

/// Known Edwards25519 small-subgroup points of orders 1, 2, 4, 8 in standard compressed format.
pub const SMALL_SUBGROUP_POINTS: [[u8; 32]; 8] = [
    // Order 1: Identity point (0, 1)
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

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when an Ed25519 cryptographic invariant or quota limit is violated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ed25519DefenseError {
    /// Public key length does not equal required 32-byte constraint.
    InvalidPublicKeyLength { actual: usize, expected: usize },
    /// Secret key length does not equal required 32-byte constraint.
    InvalidSecretKeyLength { actual: usize, expected: usize },
    /// Signature length does not equal required 64-byte constraint.
    InvalidSignatureLength { actual: usize, expected: usize },
    /// Public key belongs to a small subgroup of order 1, 2, 4, or 8.
    SmallSubgroupKeyDetected { key: [u8; 32] },
    /// Signature scalar S is non-canonical ($S \ge \ell$).
    NonCanonicalScalarDetected,
    /// Curve point R or public key A failed canonical point decompression.
    NonCanonicalPointDetected,
    /// Signature malleability detected or structure tampered.
    MalleableSignatureDetected { reason: &'static str },
    /// Message size exceeds configured quota ceiling.
    MessageSizeLimitExceeded { size: u64, limit: u64 },
    /// Certificate chain depth exceeds maximum allowable recursion limit.
    CertChainDepthExceeded { depth: usize, limit: usize },
    /// Signature verification failed.
    VerificationFailed,
    /// Underlying cryptographic library error.
    CryptographicError { reason: String },
}

impl fmt::Display for Ed25519DefenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPublicKeyLength { actual, expected } => {
                write!(f, "Ed25519 invalid public key length: {actual} bytes, expected {expected}")
            }
            Self::InvalidSecretKeyLength { actual, expected } => {
                write!(f, "Ed25519 invalid secret key length: {actual} bytes, expected {expected}")
            }
            Self::InvalidSignatureLength { actual, expected } => {
                write!(f, "Ed25519 invalid signature length: {actual} bytes, expected {expected}")
            }
            Self::SmallSubgroupKeyDetected { key } => {
                write!(f, "Ed25519 small subgroup point rejected: {:02x?}", &key[..8])
            }
            Self::NonCanonicalScalarDetected => {
                write!(f, "Ed25519 signature rejected: non-canonical scalar S >= l")
            }
            Self::NonCanonicalPointDetected => {
                write!(f, "Ed25519 point decompression failed: non-canonical point encoding")
            }
            Self::MalleableSignatureDetected { reason } => {
                write!(f, "Ed25519 signature malleability guard violation: {reason}")
            }
            Self::MessageSizeLimitExceeded { size, limit } => {
                write!(f, "Ed25519 message size quota exceeded: {size} > {limit} bytes")
            }
            Self::CertChainDepthExceeded { depth, limit } => {
                write!(f, "Ed25519 certificate chain depth exceeded: {depth} > {limit}")
            }
            Self::VerificationFailed => write!(f, "Ed25519 cryptographic signature verification failed"),
            Self::CryptographicError { reason } => write!(f, "Ed25519 internal crypto error: {reason}"),
        }
    }
}

impl std::error::Error for Ed25519DefenseError {}

impl From<Ed25519DefenseError> for TTZipStatus {
    fn from(err: Ed25519DefenseError) -> Self {
        match err {
            Ed25519DefenseError::MessageSizeLimitExceeded { .. }
            | Ed25519DefenseError::CertChainDepthExceeded { .. } => Self::ErrSolidBudgetExceeded,
            Ed25519DefenseError::InvalidPublicKeyLength { .. }
            | Ed25519DefenseError::InvalidSecretKeyLength { .. }
            | Ed25519DefenseError::InvalidSignatureLength { .. } => Self::ErrInvalidParam,
            Ed25519DefenseError::SmallSubgroupKeyDetected { .. }
            | Ed25519DefenseError::NonCanonicalScalarDetected
            | Ed25519DefenseError::NonCanonicalPointDetected
            | Ed25519DefenseError::MalleableSignatureDetected { .. }
            | Ed25519DefenseError::VerificationFailed
            | Ed25519DefenseError::CryptographicError { .. } => Self::ErrSecurityViolation,
        }
    }
}

// ============================================================================
// Constant-Time Comparison Guards
// ============================================================================

/// Constant-time comparison between two 64-byte arrays (Ed25519 signatures).
#[inline]
pub fn constant_time_eq_64(a: &[u8; 64], b: &[u8; 64]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..64 {
        diff |= a[i] ^ b[i];
    }
    std::hint::black_box(diff) == 0
}

// ============================================================================
// Defense Guard 1: Small-Subgroup & Cofactor Attack Denial
// ============================================================================

/// Guard that intercepts and denies Curve25519 small-subgroup points of order 1, 2, 4, and 8.
pub struct SubgroupAttackGuard;

impl SubgroupAttackGuard {
    /// Determines whether a 32-byte compressed point matches any known small-subgroup element.
    #[must_use]
    pub fn is_small_subgroup_point(point: &[u8; 32]) -> bool {
        for known_small in &SMALL_SUBGROUP_POINTS {
            if crate::security::blake3_defense::constant_time_eq_32(point, known_small) {
                return true;
            }
        }
        false
    }

    /// Verifies that a point does not belong to any small subgroup and has valid point encoding.
    pub fn verify_canonical_point(point: &[u8; 32]) -> Result<(), Ed25519DefenseError> {
        if Self::is_small_subgroup_point(point) {
            return Err(Ed25519DefenseError::SmallSubgroupKeyDetected { key: *point });
        }
        // Verify that the point can be successfully decompressed into a valid curve point.
        VerifyingKey::from_bytes(point).map_err(|_| Ed25519DefenseError::NonCanonicalPointDetected)?;
        Ok(())
    }

    /// Verifies that a constructed [`VerifyingKey`] passes small-subgroup rejection.
    pub fn verify_verifying_key(key: &VerifyingKey) -> Result<(), Ed25519DefenseError> {
        let bytes = key.to_bytes();
        if Self::is_small_subgroup_point(&bytes) {
            Err(Ed25519DefenseError::SmallSubgroupKeyDetected { key: bytes })
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// Defense Guard 2: Signature Malleability & Non-Canonical Scalar Denial
// ============================================================================

/// Guard that rejects non-canonical signature scalar components ($S \ge \ell$) and malleable variants.
pub struct MalleabilityGuard;

impl MalleabilityGuard {
    /// Verifies that the 32-byte scalar $S$ is strictly smaller than the group order $\ell$ ($S < \ell$).
    #[must_use]
    pub fn is_canonical_scalar(s_bytes: &[u8; 32]) -> bool {
        // Compare S with L starting from most significant byte down to least significant byte.
        for i in (0..32).rev() {
            if s_bytes[i] < ED25519_ORDER_L[i] {
                return true;
            }
            if s_bytes[i] > ED25519_ORDER_L[i] {
                return false;
            }
        }
        // If S == L, it is non-canonical.
        false
    }

    /// Verifies that an Ed25519 64-byte signature structure conforms to strict canonical non-malleable rules.
    pub fn verify_signature_structure(sig_bytes: &[u8; 64]) -> Result<(), Ed25519DefenseError> {
        let mut r_bytes = [0u8; 32];
        let mut s_bytes = [0u8; 32];
        r_bytes.copy_from_slice(&sig_bytes[0..32]);
        s_bytes.copy_from_slice(&sig_bytes[32..64]);

        // Check 1: Scalar S must be strictly canonical (S < l).
        if !Self::is_canonical_scalar(&s_bytes) {
            return Err(Ed25519DefenseError::NonCanonicalScalarDetected);
        }

        // Check 2: Point R must not be a small-subgroup element.
        if SubgroupAttackGuard::is_small_subgroup_point(&r_bytes) {
            return Err(Ed25519DefenseError::MalleableSignatureDetected {
                reason: "signature commitment point R belongs to small subgroup",
            });
        }

        Ok(())
    }
}

// ============================================================================
// Defense Guard 3: Sensitive Private Key Zeroize & Stack Sanitization
// ============================================================================

/// Sensitive 256-bit Ed25519 private signing key container with automatic zeroization on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct GuardedSigningKey {
    secret_seed: [u8; ED25519_SECRET_KEY_LEN],
}

impl GuardedSigningKey {
    /// Constructs a zeroizing container from a 32-byte secret seed.
    #[inline]
    #[must_use]
    pub const fn new(secret_seed: [u8; ED25519_SECRET_KEY_LEN]) -> Self {
        Self { secret_seed }
    }

    /// Validates and constructs a [`GuardedSigningKey`] from a byte slice.
    pub fn from_slice(slice: &[u8]) -> Result<Self, Ed25519DefenseError> {
        if slice.len() != ED25519_SECRET_KEY_LEN {
            return Err(Ed25519DefenseError::InvalidSecretKeyLength {
                actual: slice.len(),
                expected: ED25519_SECRET_KEY_LEN,
            });
        }
        let mut seed = [0u8; ED25519_SECRET_KEY_LEN];
        seed.copy_from_slice(slice);
        Ok(Self { secret_seed: seed })
    }

    /// Borrows the internal 32-byte secret seed reference.
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ED25519_SECRET_KEY_LEN] {
        &self.secret_seed
    }

    /// Derives the underlying dalek [`SigningKey`].
    #[must_use]
    pub fn to_signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.secret_seed)
    }

    /// Computes the corresponding [`VerifyingKey`] and verifies it is not in a small subgroup.
    pub fn verifying_key(&self) -> Result<VerifyingKey, Ed25519DefenseError> {
        let signing = self.to_signing_key();
        let verifying = signing.verifying_key();
        SubgroupAttackGuard::verify_verifying_key(&verifying)?;
        Ok(verifying)
    }

    /// Returns the 32-byte compressed public key array.
    pub fn public_key_bytes(&self) -> Result<[u8; ED25519_PUBLIC_KEY_LEN], Ed25519DefenseError> {
        let verifying = self.verifying_key()?;
        Ok(verifying.to_bytes())
    }

    /// Signs a message after verifying input boundaries and ensuring non-malleability.
    pub fn sign(&self, message: &[u8]) -> Result<[u8; ED25519_SIGNATURE_LEN], Ed25519DefenseError> {
        let signing = self.to_signing_key();
        use ed25519_dalek::Signer;
        let signature: Signature = signing.sign(message);
        let sig_bytes = signature.to_bytes();
        MalleabilityGuard::verify_signature_structure(&sig_bytes)?;
        Ok(sig_bytes)
    }
}

impl fmt::Debug for GuardedSigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GuardedSigningKey([REDACTED])")
    }
}

// ============================================================================
// Defense Guard 5 & 6: Strong Types & Configuration Models
// ============================================================================

/// Strongly-typed 32-byte Ed25519 public key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ed25519PublicKey(pub [u8; ED25519_PUBLIC_KEY_LEN]);

impl Ed25519PublicKey {
    /// Validates and constructs an [`Ed25519PublicKey`] from a byte slice.
    pub fn from_slice(slice: &[u8]) -> Result<Self, Ed25519DefenseError> {
        if slice.len() != ED25519_PUBLIC_KEY_LEN {
            return Err(Ed25519DefenseError::InvalidPublicKeyLength {
                actual: slice.len(),
                expected: ED25519_PUBLIC_KEY_LEN,
            });
        }
        let mut arr = [0u8; ED25519_PUBLIC_KEY_LEN];
        arr.copy_from_slice(slice);
        SubgroupAttackGuard::verify_canonical_point(&arr)?;
        Ok(Self(arr))
    }

    /// Returns the raw 32-byte array.
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ED25519_PUBLIC_KEY_LEN] {
        &self.0
    }

    /// Parses the public key into a validated [`VerifyingKey`].
    pub fn to_verifying_key(&self) -> Result<VerifyingKey, Ed25519DefenseError> {
        SubgroupAttackGuard::verify_canonical_point(&self.0)?;
        VerifyingKey::from_bytes(&self.0).map_err(|e| Ed25519DefenseError::CryptographicError {
            reason: e.to_string(),
        })
    }
}

/// Strongly-typed 64-byte Ed25519 signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ed25519Signature(pub [u8; ED25519_SIGNATURE_LEN]);

impl Ed25519Signature {
    /// Validates and constructs an [`Ed25519Signature`] from a byte slice.
    pub fn from_slice(slice: &[u8]) -> Result<Self, Ed25519DefenseError> {
        if slice.len() != ED25519_SIGNATURE_LEN {
            return Err(Ed25519DefenseError::InvalidSignatureLength {
                actual: slice.len(),
                expected: ED25519_SIGNATURE_LEN,
            });
        }
        let mut arr = [0u8; ED25519_SIGNATURE_LEN];
        arr.copy_from_slice(slice);
        MalleabilityGuard::verify_signature_structure(&arr)?;
        Ok(Self(arr))
    }

    /// Returns the raw 64-byte signature array.
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ED25519_SIGNATURE_LEN] {
        &self.0
    }

    /// Borrows the 32-byte R commitment point component.
    #[inline]
    #[must_use]
    pub fn r_bytes(&self) -> &[u8] {
        &self.0[0..32]
    }

    /// Borrows the 32-byte S scalar component.
    #[inline]
    #[must_use]
    pub fn s_bytes(&self) -> &[u8] {
        &self.0[32..64]
    }

    /// Converts into a dalek [`Signature`].
    #[must_use]
    pub fn to_dalek_signature(&self) -> Signature {
        Signature::from_bytes(&self.0)
    }
}

/// Configuration parameters for Ed25519 defense-in-depth and quota enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519DefenseConfig {
    /// Maximum allowable message size in bytes (default: 1 GiB).
    pub max_message_size: u64,
    /// Maximum allowable certificate / plugin trust chain recursion depth (default: 8).
    pub max_cert_chain_depth: usize,
    /// Enforce strict RFC 8032 / ZIP 215 non-malleability and small-subgroup rejection.
    pub enforce_strict_mode: bool,
}

impl Default for Ed25519DefenseConfig {
    #[inline]
    fn default() -> Self {
        Self::default_limits()
    }
}

impl Ed25519DefenseConfig {
    /// Constructs default production security limits.
    #[must_use]
    pub const fn default_limits() -> Self {
        Self {
            max_message_size: ED25519_DEFAULT_MAX_MSG_SIZE,
            max_cert_chain_depth: ED25519_DEFAULT_MAX_CERT_CHAIN_DEPTH,
            enforce_strict_mode: true,
        }
    }

    /// Sets custom maximum message size limit.
    #[must_use]
    pub const fn with_max_message_size(mut self, limit: u64) -> Self {
        self.max_message_size = limit;
        self
    }

    /// Sets custom maximum certificate chain depth.
    #[must_use]
    pub const fn with_max_cert_chain_depth(mut self, depth: usize) -> Self {
        self.max_cert_chain_depth = depth;
        self
    }

    /// Sets strict mode enforcement.
    #[must_use]
    pub const fn with_strict_mode(mut self, strict: bool) -> Self {
        self.enforce_strict_mode = strict;
        self
    }
}

// ============================================================================
// High-Assurance Guarded Verifier
// ============================================================================

/// Node in an Ed25519 certificate or trust delegation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertNode {
    /// Subject public key in this certificate (32 bytes).
    pub subject_pubkey: [u8; ED25519_PUBLIC_KEY_LEN],
    /// Issuer signature over the subject data (64 bytes).
    pub issuer_signature: [u8; ED25519_SIGNATURE_LEN],
    /// Certificate payload (metadata, expiration, permissions).
    pub payload: Vec<u8>,
}

/// High-assurance, defense-in-depth Ed25519 verification engine.
#[derive(Debug, Clone)]
pub struct GuardedEd25519Verifier {
    config: Ed25519DefenseConfig,
}

impl Default for GuardedEd25519Verifier {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl GuardedEd25519Verifier {
    /// Creates a new verifier with default production limits.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self::new_with_config(Ed25519DefenseConfig::default_limits())
    }

    /// Creates a new verifier with custom configuration.
    #[inline]
    #[must_use]
    pub const fn new_with_config(config: Ed25519DefenseConfig) -> Self {
        Self { config }
    }

    /// Returns the active defense configuration.
    #[inline]
    #[must_use]
    pub const fn config(&self) -> &Ed25519DefenseConfig {
        &self.config
    }

    /// Verifies a signature over a message against a public key with 6-layer defense checks.
    pub fn verify(
        &self,
        public_key_bytes: &[u8],
        message: &[u8],
        signature_bytes: &[u8],
    ) -> Result<(), Ed25519DefenseError> {
        // Quota Check: Message size ceiling
        let msg_len = message.len() as u64;
        if msg_len > self.config.max_message_size {
            return Err(Ed25519DefenseError::MessageSizeLimitExceeded {
                size: msg_len,
                limit: self.config.max_message_size,
            });
        }

        // Strong Type & Invariant Check: Public Key
        let pub_key = Ed25519PublicKey::from_slice(public_key_bytes)?;
        let verifying_key = pub_key.to_verifying_key()?;

        // Strong Type & Invariant Check: Signature Structure (Scalar canonical check, R point check)
        let sig = Ed25519Signature::from_slice(signature_bytes)?;
        let dalek_sig = sig.to_dalek_signature();

        // Cryptographic Verification: RFC 8032 / ZIP 215 strict rules
        if self.config.enforce_strict_mode {
            verifying_key
                .verify_strict(message, &dalek_sig)
                .map_err(|_| Ed25519DefenseError::VerificationFailed)?;
        } else {
            use ed25519_dalek::Verifier;
            verifying_key
                .verify(message, &dalek_sig)
                .map_err(|_| Ed25519DefenseError::VerificationFailed)?;
        }

        Ok(())
    }

    /// Verifies a recursive certificate / plugin delegation chain up to a trusted root public key.
    pub fn verify_cert_chain(
        &self,
        chain: &[CertNode],
        target_data: &[u8],
        target_sig: &[u8; ED25519_SIGNATURE_LEN],
        root_pubkey: &[u8; ED25519_PUBLIC_KEY_LEN],
    ) -> Result<(), Ed25519DefenseError> {
        if chain.len() > self.config.max_cert_chain_depth {
            return Err(Ed25519DefenseError::CertChainDepthExceeded {
                depth: chain.len(),
                limit: self.config.max_cert_chain_depth,
            });
        }

        // Verify root to intermediate certificates
        let mut current_issuer_pubkey = *root_pubkey;
        for node in chain {
            self.verify(&current_issuer_pubkey, &node.payload, &node.issuer_signature)?;
            current_issuer_pubkey = node.subject_pubkey;
        }

        // Verify leaf target signature using terminal subject public key
        self.verify(&current_issuer_pubkey, target_data, target_sig)?;

        Ok(())
    }
}

// ============================================================================
// Top-Level Convenience Guard Functions
// ============================================================================

/// Verifies an Ed25519 signature against a public key with default defense limits.
pub fn guarded_verify_ed25519(
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), Ed25519DefenseError> {
    let verifier = GuardedEd25519Verifier::new();
    verifier.verify(public_key, message, signature)
}

/// Signs a message with a private secret seed under 6-layer defense checks.
pub fn guarded_sign_ed25519(
    secret_key: &[u8],
    message: &[u8],
) -> Result<[u8; ED25519_SIGNATURE_LEN], Ed25519DefenseError> {
    let guarded_key = GuardedSigningKey::from_slice(secret_key)?;
    guarded_key.sign(message)
}
