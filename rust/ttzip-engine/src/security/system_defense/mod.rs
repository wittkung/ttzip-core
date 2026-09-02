// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip 6-Layer System Defense-in-Depth & Incremental Update Security Subsystem.
//!
//! Provides enterprise-grade defense against sandbox escapes, path traversal attacks,
//! memory exhaustion bombs, signature tampering, rollback downgrade attacks,
//! and sensitive credential leakage:
//!
//! 1. **Sandbox Escaping Defense Guard** ([`SandboxEscapingGuard`]):
//!    Lexical path normalization, jail root prefix containment, and step-by-step intermediate directory
//!    symlink (`!S_ISLNK`) verification.
//! 2. **Binary Delta Memory & Resource Guard** ([`BinaryDeltaMemoryBudgetGuard`]):
//!    Task resident memory ceiling (<= 64MB), maximum patch size (<= 512MB), expansion ratio
//!    watchdog (<= 1000x), and instruction quota circuit breaker (<= 100,000).
//! 3. **Appcast & Release Signature Guard** ([`AppcastSignatureGuard`]):
//!    Ed25519 small-subgroup denial, canonical scalar bounds ($S < \ell$), side-channel resistant
//!    constant-time equality (`constant_time_eq_64`), and version monotonicity assertion.
//! 4. **Temporary Directory Cleanup Guard** ([`TempDirectoryCleanupGuard`]):
//!    High-entropy UUID workspace isolation, deterministic RAII on-drop cleanup, and self-healing orphan purging.
//! 5. **Path Traversal & Zip-Slip Protection Guard** ([`PathTraversalProtectionGuard`]):
//!    Single-pass stack-based `..` neutralization, null-byte injection detection, and Windows/POSIX reserved device rejection.
//! 6. **Sensitive Credential Zeroize Container** ([`SensitiveCredentialBuffer`]):
//!    Volatile memory zeroization on drop and redacted debug/display logging.

pub mod appcast_sig;
pub mod delta_budget;
pub mod path_traversal;
pub mod pipeline;
pub mod sandbox_escape;
pub mod sensitive;
pub mod temp_cleanup;

#[cfg(test)]
mod tests;

pub use appcast_sig::AppcastSignatureGuard;
pub use delta_budget::{
    BinaryDeltaBudgetOptions, BinaryDeltaMemoryBudgetGuard, DeltaMemoryPermit,
    DEFAULT_MAX_DELTA_EXPANSION_RATIO, DEFAULT_MAX_DELTA_INSTRUCTIONS,
    DEFAULT_MAX_DELTA_MEMORY_BUDGET, DEFAULT_MAX_DELTA_PATCH_SIZE,
};
pub use path_traversal::{PathTraversalOptions, PathTraversalProtectionGuard};
pub use pipeline::{
    SystemDefenseOptions, SystemPreflightReport, SystemSecurityPipeline, SystemUpdateRequest,
};
pub use sandbox_escape::{SandboxEscapingGuard, SandboxEscapingOptions};
pub use sensitive::{SensitiveCredentialBuffer, SensitiveCredentialString};
pub use temp_cleanup::{TempDirectoryCleanupGuard, TempDirectoryGuard};

use crate::types::TTZipStatus;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when system defense invariants, sandbox boundaries, or quotas are breached.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SystemDefenseError {
    /// Sandbox jail escape attempt detected.
    #[error("Sandbox escape attempt detected: path '{path}', reason: {reason}")]
    SandboxEscapeAttempt { path: String, reason: String },

    /// Intermediate directory symlink escape detected.
    #[error("Symlink escaping detected on path '{path}': {reason}")]
    SymlinkEscapingDetected { path: String, reason: String },

    /// Single task resident memory budget exceeded.
    #[error("Delta memory budget exceeded: {allocated} bytes > maximum budget {max_budget} bytes")]
    DeltaMemoryBudgetExceeded { allocated: usize, max_budget: usize },

    /// Input patch file size exceeded configured maximum limit.
    #[error("Delta patch size exceeded: {size} bytes > maximum allowed {max_size} bytes")]
    DeltaPatchSizeExceeded { size: usize, max_size: usize },

    /// Patch expansion ratio exceeded decompression explosion limit.
    #[error("Delta expansion ratio exceeded: {ratio}x > maximum allowed {max_ratio}x")]
    DeltaExpansionRatioExceeded { ratio: usize, max_ratio: usize },

    /// Delta instruction count exceeded safety quota.
    #[error("Delta instruction count exceeded quota: {count} > maximum allowed {max_quota}")]
    DeltaInstructionQuotaExceeded { count: usize, max_quota: usize },

    /// Small subgroup public key detected in signature verification.
    #[error("Ed25519 small subgroup key rejected: prefix {key_prefix}")]
    SmallSubgroupKeyDetected { key_prefix: String },

    /// Non-canonical scalar S detected ($S \ge \ell$).
    #[error("Ed25519 non-canonical scalar S detected (S >= l)")]
    NonCanonicalScalarDetected,

    /// Malleable or tampered signature structure detected.
    #[error("Ed25519 signature malleability violation: {reason}")]
    MalleableSignatureDetected { reason: String },

    /// Cryptographic signature verification failed.
    #[error("Signature verification failed: {reason}")]
    SignatureVerificationFailed { reason: String },

    /// Version downgrade attempt detected.
    #[error("Version downgrade rejected: attempted '{attempted}' <= current '{current}'")]
    VersionDowngradeDetected { current: String, attempted: String },

    /// Path traversal or Zip-Slip attack detected.
    #[error("Path traversal attack detected: path '{path}', reason: {reason}")]
    PathTraversalAttackDetected { path: String, reason: String },

    /// Null-byte injection detected in path.
    #[error("Null-byte injection detected in path '{path}'")]
    NullByteInjectionDetected { path: String },

    /// Windows/POSIX reserved device name detected in path segment.
    #[error("Reserved system device name detected in path: '{segment}'")]
    ReservedDeviceNameDetected { segment: String },

    /// Path length exceeded maximum limit.
    #[error("Path length exceeded limit: {len} > {max_len} bytes")]
    PathTooLong { len: usize, max_len: usize },

    /// Temporary directory creation failed.
    #[error("Failed to create isolated temporary directory '{path}': {reason}")]
    TempDirectoryCreationFailed { path: String, reason: String },

    /// Temporary directory cleanup failed.
    #[error("Failed to clean temporary workspace: {reason}")]
    TempDirectoryCleanupFailed { reason: String },
}

impl From<SystemDefenseError> for TTZipStatus {
    fn from(err: SystemDefenseError) -> Self {
        match err {
            SystemDefenseError::DeltaMemoryBudgetExceeded { .. }
            | SystemDefenseError::DeltaPatchSizeExceeded { .. }
            | SystemDefenseError::DeltaExpansionRatioExceeded { .. }
            | SystemDefenseError::DeltaInstructionQuotaExceeded { .. } => {
                Self::ErrSolidBudgetExceeded
            }
            SystemDefenseError::PathTooLong { .. } => Self::ErrPathTooLong,
            SystemDefenseError::SandboxEscapeAttempt { .. }
            | SystemDefenseError::SymlinkEscapingDetected { .. }
            | SystemDefenseError::SmallSubgroupKeyDetected { .. }
            | SystemDefenseError::NonCanonicalScalarDetected
            | SystemDefenseError::MalleableSignatureDetected { .. }
            | SystemDefenseError::SignatureVerificationFailed { .. }
            | SystemDefenseError::VersionDowngradeDetected { .. }
            | SystemDefenseError::PathTraversalAttackDetected { .. }
            | SystemDefenseError::NullByteInjectionDetected { .. }
            | SystemDefenseError::ReservedDeviceNameDetected { .. }
            | SystemDefenseError::TempDirectoryCreationFailed { .. }
            | SystemDefenseError::TempDirectoryCleanupFailed { .. } => {
                Self::ErrSecurityViolation
            }
        }
    }
}
