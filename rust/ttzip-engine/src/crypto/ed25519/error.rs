// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed error definitions for Ed25519 operations, batch verification,
//! and PKI certificate chain validations.

use thiserror::Error;

/// Error variants encountered during Ed25519 signing, verification, and PKI operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Ed25519Error {
    /// The signature format or length is invalid.
    #[error("Invalid signature format: expected 64 bytes, got {actual_len} bytes")]
    InvalidSignatureFormat {
        /// Actual length provided in bytes.
        actual_len: usize,
    },

    /// The public key format is invalid or point decompression failed.
    #[error("Invalid public key format: {reason}")]
    InvalidPublicKeyFormat {
        /// Detailed reason for public key failure.
        reason: String,
    },

    /// The secret key format or length is invalid.
    #[error("Invalid secret key format: expected 32 bytes, got {actual_len} bytes")]
    InvalidSecretKeyFormat {
        /// Actual length provided in bytes.
        actual_len: usize,
    },

    /// Mathematical verification of the signature failed.
    #[error("Ed25519 signature verification equation failed")]
    SignatureVerificationFailed,

    /// Scalar S is non-canonical (S >= \ell), violating RFC 8032 / Zip215 strict rules.
    #[error("Non-canonical scalar S: value is greater than or equal to curve order \\ell")]
    NonCanonicalScalar,

    /// High-throughput batch verification failed.
    #[error("Batch verification failed: folded multi-scalar multiplication did not equate to identity point")]
    BatchVerificationFailed,

    /// Certificate is expired based on the verification timestamp.
    #[error("Certificate expired: valid from {valid_from} to {valid_until}, current time {current_time}")]
    CertificateExpired {
        /// Certificate start timestamp in seconds since Unix epoch.
        valid_from: u64,
        /// Certificate expiration timestamp in seconds since Unix epoch.
        valid_until: u64,
        /// Reference verification timestamp in seconds since Unix epoch.
        current_time: u64,
    },

    /// Certificate is not yet valid based on the verification timestamp.
    #[error("Certificate not yet valid: valid from {valid_from} to {valid_until}, current time {current_time}")]
    CertificateNotYetValid {
        /// Certificate start timestamp in seconds since Unix epoch.
        valid_from: u64,
        /// Certificate expiration timestamp in seconds since Unix epoch.
        valid_until: u64,
        /// Reference verification timestamp in seconds since Unix epoch.
        current_time: u64,
    },

    /// Certificate hierarchy chain is broken (issuer mismatch).
    #[error("Certificate chain broken: expected issuer '{expected_issuer}', got '{actual_issuer}'")]
    CertificateChainBroken {
        /// Expected parent subject ID.
        expected_issuer: String,
        /// Actual issuer recorded on child certificate.
        actual_issuer: String,
    },

    /// Certificate hierarchy level does not match expected tier.
    #[error("Invalid certificate tier: expected '{expected}', found '{actual}'")]
    InvalidCertificateLevel {
        /// Expected certificate level name.
        expected: String,
        /// Actual certificate level found.
        actual: String,
    },

    /// Manifest authentication failed.
    #[error("Plugin manifest authentication failed: {0}")]
    ManifestVerificationFailed(String),

    /// Serialization or deserialization failure during canonical encoding.
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
