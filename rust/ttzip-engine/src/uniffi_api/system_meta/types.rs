// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records, Enums, and Errors for System Updates, Delta Patching, and Appcast Metadata.

use serde::{Deserialize, Serialize};

/// Supported binary serialization and compression formats for delta patches.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize, uniffi::Enum)]
pub enum UniFFIDeltaFormat {
    /// Raw uncompressed byte-level delta instructions (fastest creation/application).
    #[default]
    RawByteBlock,
    /// Zstandard compressed delta payload for optimal bandwidth minimization.
    ZstdCompressed,
    /// Standard Flate/Deflate compressed delta payload.
    FlateCompressed,
}

/// Result of an in-memory or stream delta patch application operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct UniFFIDeltaPatchResult {
    /// Whether patch reconstruction and hash verification succeeded.
    pub success: bool,
    /// Size in bytes of the applied delta patch package.
    pub patch_size: u64,
    /// Size in bytes of the reconstructed target payload.
    pub target_size: u64,
    /// Hex-encoded SHA-256 digest of the reconstructed target data.
    pub target_hash: String,
    /// Whether the patch was executed directly in memory without disk staging.
    pub applied_in_memory: bool,
    /// Execution duration in milliseconds.
    pub duration_ms: f64,
    /// Reconstructed target binary bytes.
    pub patched_bytes: Vec<u8>,
}

/// Single release entry in an Appcast update feed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct UniFFIAppcastItem {
    /// Semantic version string (e.g. "1.2.0").
    pub version: String,
    /// Monotonically increasing build integer (e.g. 10200).
    pub build_number: u64,
    /// Minimum compatible macOS version requirement (e.g. "14.0").
    pub min_os_version: String,
    /// Optional URL pointing to release notes or changelog markdown/html.
    pub release_notes_url: Option<String>,
    /// Full package download URL (.zip, .dmg, or .pkg).
    pub download_url: String,
    /// Full package payload size in bytes.
    pub download_size: u64,
    /// Detached Ed25519 digital signature of the full package in Base64 representation.
    pub signature_ed25519: String,
    /// Hex-encoded NIST SHA-256 digest of the full target package.
    pub sha256: String,
    /// Optional URL for delta patch package from a specific previous base version.
    pub delta_patch_url: Option<String>,
    /// Previous base version string required by the delta patch (e.g. "1.1.9").
    pub delta_base_version: Option<String>,
    /// Detached Ed25519 digital signature of the delta patch payload in Base64 representation.
    pub delta_signature_ed25519: Option<String>,
    /// Delta patch package payload size in bytes.
    pub delta_size: Option<u64>,
    /// Whether this update is marked as a critical security patch.
    pub is_critical: bool,
    /// Publication timestamp in seconds since Unix epoch.
    pub published_at_epoch_secs: i64,
}

/// Comprehensive Appcast feed metadata and parsed items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct UniFFIAppcastMetadata {
    /// Distribution channel (e.g. "stable", "beta", "nightly").
    pub channel: String,
    /// Application feed title or product display name.
    pub title: String,
    /// Source feed URL.
    pub feed_url: String,
    /// Latest available semantic version string in the feed.
    pub latest_version: String,
    /// Latest available build integer in the feed.
    pub latest_build: u64,
    /// List of all parsed update candidate items.
    pub items: Vec<UniFFIAppcastItem>,
    /// Whether feed digital signature passed cryptographic verification.
    pub signature_valid: bool,
    /// Timestamp when this feed was checked/retrieved.
    pub checked_at_epoch_secs: i64,
}

/// Strongly-typed error enum mapped directly to Swift `throws UniFFISystemError`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum UniFFISystemError {
    /// Digital signature or public key validation failure.
    #[error("Invalid Ed25519 signature: {reason}")]
    InvalidSignature { reason: String },

    /// Binary delta patch creation or application failure.
    #[error("Delta patch operation failed: {reason}")]
    PatchFailed { reason: String },

    /// Attempted update violates version monotonicity (downgrade prevention).
    #[error("Version downgrade is strictly forbidden: current {current_version} vs incoming {incoming_version}")]
    VersionDowngradeForbidden {
        current_version: String,
        incoming_version: String,
    },

    /// File system or stream I/O failure.
    #[error("I/O error during system operation: {message}")]
    IoError { message: String },

    /// Corrupt data, magic mismatch, or integrity checksum failure.
    #[error("Corrupt data or magic mismatch: {details}")]
    CorruptData { details: String },

    /// Appcast feed parsing error.
    #[error("Appcast feed parse failure: {details}")]
    AppcastParseError { details: String },

    /// System update or patch operation was explicitly cancelled.
    #[error("System operation was cancelled")]
    Cancelled,

    /// General security policy or integrity verification violation.
    #[error("Security or integrity verification failed: {reason}")]
    VerificationFailed { reason: String },
}

impl UniFFISystemError {
    /// Constructs a patch failure error variant.
    pub fn patch_err(msg: impl std::fmt::Display) -> Self {
        Self::PatchFailed {
            reason: msg.to_string(),
        }
    }

    /// Constructs an I/O error variant with descriptive context.
    pub fn io_err(msg: impl std::fmt::Display) -> Self {
        Self::IoError {
            message: msg.to_string(),
        }
    }

    /// Constructs a verification failure error.
    pub fn verify_err(msg: impl std::fmt::Display) -> Self {
        Self::VerificationFailed {
            reason: msg.to_string(),
        }
    }
}
