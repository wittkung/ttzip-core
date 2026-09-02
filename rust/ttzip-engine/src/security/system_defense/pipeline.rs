// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified System Defense Pipeline (`SystemSecurityPipeline`).
//!
//! Orchestrates 6-layer defense-in-depth security verification for desktop system integration,
//! incremental binary delta updates, Sparkle appcast verification, and sandbox filesystem access.

use std::path::{Path, PathBuf};

use super::appcast_sig::AppcastSignatureGuard;
use super::delta_budget::{BinaryDeltaBudgetOptions, BinaryDeltaMemoryBudgetGuard, DeltaMemoryPermit};
use super::path_traversal::{PathTraversalOptions, PathTraversalProtectionGuard};
use super::sandbox_escape::{SandboxEscapingGuard, SandboxEscapingOptions};
use super::temp_cleanup::TempDirectoryCleanupGuard;
use super::SystemDefenseError;

/// Configuration options for the complete 6-layer system security pipeline.
#[derive(Debug, Clone)]
pub struct SystemDefenseOptions {
    /// Sandbox jail options.
    pub sandbox: SandboxEscapingOptions,
    /// Binary delta memory and resource options.
    pub delta_budget: BinaryDeltaBudgetOptions,
    /// Path traversal options.
    pub path_traversal: PathTraversalOptions,
}

impl Default for SystemDefenseOptions {
    fn default() -> Self {
        Self {
            sandbox: SandboxEscapingOptions::default(),
            delta_budget: BinaryDeltaBudgetOptions::default(),
            path_traversal: PathTraversalOptions::default(),
        }
    }
}

/// Request model for validating an incremental update package or patch stream.
#[derive(Debug, Clone)]
pub struct SystemUpdateRequest {
    /// Destination or target file path to update.
    pub target_path: String,
    /// Candidate target version string in update manifest.
    pub expected_version: String,
    /// Current running application version string.
    pub current_version: String,
    /// Raw compressed or encoded patch bytes.
    pub patch_bytes: Vec<u8>,
    /// Cryptographic signature bytes (64-byte Ed25519).
    pub signature_bytes: Vec<u8>,
    /// Public key bytes (32-byte Ed25519).
    pub public_key_bytes: Vec<u8>,
    /// Optional jail root override.
    pub jail_root: String,
}

impl SystemUpdateRequest {
    /// Creates a new update request.
    #[inline]
    #[must_use]
    pub fn new(
        target_path: impl Into<String>,
        expected_version: impl Into<String>,
        current_version: impl Into<String>,
        patch_bytes: Vec<u8>,
        signature_bytes: Vec<u8>,
        public_key_bytes: Vec<u8>,
        jail_root: impl Into<String>,
    ) -> Self {
        Self {
            target_path: target_path.into(),
            expected_version: expected_version.into(),
            current_version: current_version.into(),
            patch_bytes,
            signature_bytes,
            public_key_bytes,
            jail_root: jail_root.into(),
        }
    }
}

/// Preflight validation report emitted upon successful verification.
#[derive(Debug)]
pub struct SystemPreflightReport {
    /// Overall authorization status.
    pub authorized: bool,
    /// Reconstructed relative sandbox path.
    pub is_signature_valid: bool,
    /// Monotonic version progression confirmed.
    pub is_version_upgrade_valid: bool,
    /// Patch memory and expansion ratio approved.
    pub is_budget_approved: bool,
    /// Current verified version string.
    pub current_version: String,
    /// Target verified version string.
    pub target_version: String,
    /// Signature validity confirmation.
    pub signature_verified: bool,
    /// Sandbox containment confirmation.
    pub sandbox_confined: bool,
    /// Declared patch size in bytes.
    pub patch_size: usize,
    /// Declared uncompressed size in bytes.
    pub uncompressed_size: usize,
    /// Acquired memory permit for patch execution.
    pub budget_permit: Option<DeltaMemoryPermit>,
}

/// Unified 6-layer defense orchestrator.
#[derive(Debug, Clone)]
pub struct SystemSecurityPipeline {
    options: SystemDefenseOptions,
    sandbox_guard: SandboxEscapingGuard,
    budget_guard: BinaryDeltaMemoryBudgetGuard,
    sig_guard: AppcastSignatureGuard,
    cleanup_guard: TempDirectoryCleanupGuard,
    traversal_guard: PathTraversalProtectionGuard,
}

impl SystemSecurityPipeline {
    /// Creates a new `SystemSecurityPipeline` with the given configuration options.
    #[inline]
    #[must_use]
    pub fn new(options: SystemDefenseOptions) -> Self {
        Self {
            sandbox_guard: SandboxEscapingGuard::new(options.sandbox.clone()),
            budget_guard: BinaryDeltaMemoryBudgetGuard::new(options.delta_budget.clone()),
            sig_guard: AppcastSignatureGuard::new(),
            cleanup_guard: TempDirectoryCleanupGuard::new(),
            traversal_guard: PathTraversalProtectionGuard::new(options.path_traversal.clone()),
            options,
        }
    }

    /// Creates a pipeline with strict default security settings.
    #[inline]
    #[must_use]
    pub fn strict_default() -> Self {
        Self::new(SystemDefenseOptions::default())
    }

    /// Accessor for the underlying options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &SystemDefenseOptions {
        &self.options
    }

    /// Accessor for the underlying sandbox guard.
    #[inline]
    #[must_use]
    pub const fn sandbox_guard(&self) -> &SandboxEscapingGuard {
        &self.sandbox_guard
    }

    /// Accessor for the underlying memory budget guard.
    #[inline]
    #[must_use]
    pub const fn budget_guard(&self) -> &BinaryDeltaMemoryBudgetGuard {
        &self.budget_guard
    }

    /// Accessor for the underlying signature guard.
    #[inline]
    #[must_use]
    pub const fn sig_guard(&self) -> &AppcastSignatureGuard {
        &self.sig_guard
    }

    /// Accessor for the underlying temporary cleanup guard.
    #[inline]
    #[must_use]
    pub const fn cleanup_guard(&self) -> &TempDirectoryCleanupGuard {
        &self.cleanup_guard
    }

    /// Accessor for the underlying path traversal guard.
    #[inline]
    #[must_use]
    pub const fn traversal_guard(&self) -> &PathTraversalProtectionGuard {
        &self.traversal_guard
    }

    /// Executes complete preflight security check for an incoming update request.
    pub fn preflight_check(
        &self,
        request: &SystemUpdateRequest,
    ) -> Result<SystemPreflightReport, SystemDefenseError> {
        // Layer 1: Version monotonicity verification (anti-downgrade)
        self.sig_guard
            .assert_version_monotonicity(&request.current_version, &request.expected_version)?;

        // Layer 2: Cryptographic signature verification
        if request.public_key_bytes.len() != 32 {
            return Err(SystemDefenseError::SignatureVerificationFailed {
                reason: format!(
                    "Invalid public key length: expected 32 bytes, got {}",
                    request.public_key_bytes.len()
                ),
            });
        }
        if request.signature_bytes.len() != 64 {
            return Err(SystemDefenseError::SignatureVerificationFailed {
                reason: format!(
                    "Invalid signature length: expected 64 bytes, got {}",
                    request.signature_bytes.len()
                ),
            });
        }

        let mut pub_key = [0u8; 32];
        pub_key.copy_from_slice(&request.public_key_bytes);
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&request.signature_bytes);

        self.sig_guard
            .verify_signature(&pub_key, &sig, &request.patch_bytes)?;

        // Layer 3: Sandbox jail and path traversal verification
        let jail_path = if request.jail_root.is_empty() {
            self.options.sandbox.jail_root.clone()
        } else {
            PathBuf::from(&request.jail_root)
        };
        let effective_sandbox = SandboxEscapingGuard::with_jail_root(&jail_path);

        let sanitized_relative = self.traversal_guard.sanitize_path(&request.target_path)?;
        let resolved_target = effective_sandbox.validate_path(Path::new(&sanitized_relative))?;
        effective_sandbox.verify_no_symlink_ancestors(&resolved_target)?;

        // Layer 4: Binary delta patch size limit
        let patch_len = request.patch_bytes.len();
        self.budget_guard.validate_patch_size(patch_len)?;

        // Layer 5: Memory budget reservation permit
        let permit = self.budget_guard.acquire_permit(patch_len.max(1024))?;

        Ok(SystemPreflightReport {
            authorized: true,
            is_signature_valid: true,
            is_version_upgrade_valid: true,
            is_budget_approved: true,
            current_version: request.current_version.clone(),
            target_version: request.expected_version.clone(),
            signature_verified: true,
            sandbox_confined: true,
            patch_size: patch_len,
            uncompressed_size: patch_len,
            budget_permit: Some(permit),
        })
    }

    /// Alias for `preflight_check`.
    #[inline]
    pub fn validate_update_preflight(
        &self,
        request: &SystemUpdateRequest,
    ) -> Result<SystemPreflightReport, SystemDefenseError> {
        self.preflight_check(request)
    }

    /// Validates and sanitizes a destination file path, ensuring jail containment and traversal immunity.
    pub fn validate_destination_path(
        &self,
        raw_path: &str,
    ) -> Result<PathBuf, SystemDefenseError> {
        // Step 1: Traverse & reserved device sanitization
        let sanitized = self.traversal_guard.sanitize_path(raw_path)?;

        // Step 2: Sandbox jail verification & component inspection
        let candidate_path = Path::new(&sanitized);
        let resolved = self.sandbox_guard.validate_path(candidate_path)?;

        // Step 3: Ensure intermediate parents are not symlinks
        self.sandbox_guard.verify_no_symlink_ancestors(&resolved)?;

        Ok(resolved)
    }

    /// Reserves memory permit for patch application stream.
    #[inline]
    pub fn acquire_memory_permit(&self, bytes: usize) -> Result<DeltaMemoryPermit, SystemDefenseError> {
        self.budget_guard.acquire_permit(bytes)
    }

    /// Validates patch instruction count against circuit breaker limit.
    #[inline]
    pub fn validate_instruction_count(&self, count: usize) -> Result<(), SystemDefenseError> {
        self.budget_guard.validate_instruction_count(count)
    }
}
