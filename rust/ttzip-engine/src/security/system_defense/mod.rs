// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemDefenseError {
    /// Sandbox jail escape attempt detected.
    SandboxEscapeAttempt { path: String, reason: String },

    /// Intermediate directory symlink escape detected.
    SymlinkEscapingDetected { path: String, reason: String },

    /// Single task resident memory budget exceeded.
    DeltaMemoryBudgetExceeded { allocated: usize, max_budget: usize },

    /// Input patch file size exceeded configured maximum limit.
    DeltaPatchSizeExceeded { size: usize, max_size: usize },

    /// Patch expansion ratio exceeded decompression explosion limit.
    DeltaExpansionRatioExceeded { ratio: usize, max_ratio: usize },

    /// Delta instruction count exceeded safety quota.
    DeltaInstructionQuotaExceeded { count: usize, max_quota: usize },

    /// Small subgroup public key detected in signature verification.
    SmallSubgroupKeyDetected { key_prefix: String },

    /// Non-canonical scalar S detected ($S \ge \ell$).
    NonCanonicalScalarDetected,

    /// Malleable or tampered signature structure detected.
    MalleableSignatureDetected { reason: String },

    /// Cryptographic signature verification failed.
    SignatureVerificationFailed { reason: String },

    /// Version downgrade attempt detected.
    VersionDowngradeDetected { current: String, attempted: String },

    /// Path traversal or Zip-Slip attack detected.
    PathTraversalAttackDetected { path: String, reason: String },

    /// Null-byte injection detected in path.
    NullByteInjectionDetected { path: String },

    /// Windows/POSIX reserved device name detected in path segment.
    ReservedDeviceNameDetected { segment: String },

    /// Path length exceeded maximum limit.
    PathTooLong { len: usize, max_len: usize },

    /// Temporary directory creation failed.
    TempDirectoryCreationFailed { path: String, reason: String },

    /// Temporary directory cleanup failed.
    TempDirectoryCleanupFailed { reason: String },
}

impl std::fmt::Display for SystemDefenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SandboxEscapeAttempt { path, reason } => {
                write!(f, "Sandbox escape attempt detected: path '{}', reason: {reason}", path.escape_debug())
            }
            Self::SymlinkEscapingDetected { path, reason } => {
                write!(f, "Symlink escaping detected on path '{}': {reason}", path.escape_debug())
            }
            Self::DeltaMemoryBudgetExceeded { allocated, max_budget } => {
                write!(f, "Delta memory budget exceeded: {allocated} bytes > maximum budget {max_budget} bytes")
            }
            Self::DeltaPatchSizeExceeded { size, max_size } => {
                write!(f, "Delta patch size exceeded: {size} bytes > maximum allowed {max_size} bytes")
            }
            Self::DeltaExpansionRatioExceeded { ratio, max_ratio } => {
                write!(f, "Delta expansion ratio exceeded: {ratio}x > maximum allowed {max_ratio}x")
            }
            Self::DeltaInstructionQuotaExceeded { count, max_quota } => {
                write!(f, "Delta instruction count exceeded quota: {count} > maximum allowed {max_quota}")
            }
            Self::SmallSubgroupKeyDetected { key_prefix } => {
                write!(f, "Ed25519 small subgroup key rejected: prefix {}", key_prefix.escape_debug())
            }
            Self::NonCanonicalScalarDetected => {
                write!(f, "Ed25519 non-canonical scalar S detected (S >= l)")
            }
            Self::MalleableSignatureDetected { reason } => {
                write!(f, "Ed25519 signature malleability violation: {reason}")
            }
            Self::SignatureVerificationFailed { reason } => {
                write!(f, "Signature verification failed: {reason}")
            }
            Self::VersionDowngradeDetected { current, attempted } => {
                write!(f, "Version downgrade rejected: attempted '{}' <= current '{}'", attempted.escape_debug(), current.escape_debug())
            }
            Self::PathTraversalAttackDetected { path, reason } => {
                write!(f, "Path traversal attack detected: path '{}', reason: {reason}", path.escape_debug())
            }
            Self::NullByteInjectionDetected { path } => {
                write!(f, "Null-byte injection detected in path '{}'", path.escape_debug())
            }
            Self::ReservedDeviceNameDetected { segment } => {
                write!(f, "Reserved system device name detected in path: '{}'", segment.escape_debug())
            }
            Self::PathTooLong { len, max_len } => {
                write!(f, "Path length exceeded limit: {len} > {max_len} bytes")
            }
            Self::TempDirectoryCreationFailed { path, reason } => {
                write!(f, "Failed to create isolated temporary directory '{}': {reason}", path.escape_debug())
            }
            Self::TempDirectoryCleanupFailed { reason } => {
                write!(f, "Failed to clean temporary workspace: {reason}")
            }
        }
    }
}

impl std::error::Error for SystemDefenseError {}

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
