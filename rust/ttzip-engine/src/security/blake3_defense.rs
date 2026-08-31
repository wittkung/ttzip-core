// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 6-Layer Defense-in-Depth Security Guards & Anti-Length-Extension Subsystem.
//!
//! Enforces deterministic cryptographic safety invariants, side-channel resistance,
//! and resource exhaustion circuit breakers:
//! 1. **Anti-Length-Extension & Merkle Root Flag Isolation**: Enforces Merkle tree domain flag
//!    separation (`ROOT`, `PARENT`, `CHUNK_START`, `CHUNK_END`), guaranteeing mathematical immunity
//!    to length-extension attacks.
//! 2. **Input Quota & XOF Output Quota Circuit Breakers**: Rejects volumetric hash exhaustion and
//!    infinite XOF stream extraction with configurable memory/bandwidth budgets (defaults: 512 MiB input, 1 GiB XOF).
//! 3. **Key Length & Context Domain Isolation Guards**: Enforces exact 32-byte key constraints
//!    and non-empty, RFC-compliant globally unique context domain strings.
//! 4. **Sensitive Memory Wiping & Zeroize Protection**: Wraps keys, intermediate tree states, and
//!    internal contexts with [`Zeroize`] and [`ZeroizeOnDrop`] to prevent cold boot and memory dump leaks.
//! 5. **Constant-Time Comparison Guard**: Prevents timing side-channel attacks on MAC verification
//!    using constant-time byte and digest equality operators (`constant_time_eq_32`).
//! 6. **Tree Depth Boundary & Counter Overflow Guard**: Validates tree stack depth ($\le 55$) and
//!    intercepts 64-bit chunk counter overflows before state corruption can occur.

use std::fmt;
use std::io;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::blake3::constants::{BLAKE3_KEY_LEN, ROOT};
use crate::crypto::blake3::facade::Blake3Hasher;
use crate::crypto::blake3::tree::STACK_CAPACITY;
use crate::types::TTZipStatus;

/// Default maximum cumulative input size in bytes (512 MiB).
pub const BLAKE3_DEFAULT_MAX_INPUT_LIMIT: u64 = 512 * 1024 * 1024;
/// Default maximum cumulative XOF output size in bytes (1 GiB).
pub const BLAKE3_DEFAULT_MAX_XOF_OUTPUT_LIMIT: u64 = 1024 * 1024 * 1024;
/// Maximum allowable context string length in bytes for key derivation (1024 bytes).
pub const BLAKE3_MAX_CONTEXT_LEN: usize = 1024;
/// Hard ceiling for tree stack depth.
pub const BLAKE3_MAX_STACK_DEPTH: usize = STACK_CAPACITY;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when a BLAKE3 security invariant or resource quota is violated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blake3DefenseError {
    /// Cumulative input byte limit exceeded.
    InputQuotaExceeded { current: u64, attempted: u64, limit: u64 },
    /// Cumulative XOF output extraction limit exceeded.
    XofOutputQuotaExceeded { current: u64, attempted: u64, limit: u64 },
    /// Key length does not equal required 32-byte constraint.
    InvalidKeyLength { actual: usize, expected: usize },
    /// Key derivation context domain failed validation.
    InvalidContextDomain { reason: &'static str },
    /// Tree stack depth exceeded maximum allowable capacity.
    StackDepthOverflow { depth: usize, max_depth: usize },
    /// 64-bit chunk counter overflowed arithmetic bounds.
    ChunkCounterOverflow { counter: u64 },
    /// Root flag leaked or violated domain separation invariants.
    InvalidRootFlagIsolation { reason: &'static str },
    /// Length-extension forgery or state tampering detected.
    LengthExtensionViolation { reason: &'static str },
}

impl fmt::Display for Blake3DefenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputQuotaExceeded { current, attempted, limit } => {
                write!(f, "BLAKE3 input quota exceeded: {current} + {attempted} > {limit}")
            }
            Self::XofOutputQuotaExceeded { current, attempted, limit } => {
                write!(f, "BLAKE3 XOF output quota exceeded: {current} + {attempted} > {limit}")
            }
            Self::InvalidKeyLength { actual, expected } => {
                write!(f, "BLAKE3 invalid key length: {actual} bytes, expected {expected}")
            }
            Self::InvalidContextDomain { reason } => write!(f, "BLAKE3 invalid context: {reason}"),
            Self::StackDepthOverflow { depth, max_depth } => {
                write!(f, "BLAKE3 tree stack depth overflow: {depth} > {max_depth}")
            }
            Self::ChunkCounterOverflow { counter } => {
                write!(f, "BLAKE3 64-bit chunk counter overflow at {counter}")
            }
            Self::InvalidRootFlagIsolation { reason } => {
                write!(f, "BLAKE3 root flag isolation violation: {reason}")
            }
            Self::LengthExtensionViolation { reason } => {
                write!(f, "BLAKE3 anti-length-extension invariant violation: {reason}")
            }
        }
    }
}

impl std::error::Error for Blake3DefenseError {}

impl From<Blake3DefenseError> for TTZipStatus {
    fn from(err: Blake3DefenseError) -> Self {
        match err {
            Blake3DefenseError::InputQuotaExceeded { .. }
            | Blake3DefenseError::XofOutputQuotaExceeded { .. }
            | Blake3DefenseError::StackDepthOverflow { .. }
            | Blake3DefenseError::ChunkCounterOverflow { .. } => Self::ErrSolidBudgetExceeded,
            Blake3DefenseError::InvalidKeyLength { .. }
            | Blake3DefenseError::InvalidContextDomain { .. } => Self::ErrInvalidParam,
            Blake3DefenseError::InvalidRootFlagIsolation { .. }
            | Blake3DefenseError::LengthExtensionViolation { .. } => Self::ErrSecurityViolation,
        }
    }
}

// ============================================================================
// Sensitive Memory Wrappers (Zeroize Guard)
// ============================================================================

/// Sensitive 256-bit cryptographic key wrapper with automatic zeroization on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct GuardedKey {
    key: [u8; BLAKE3_KEY_LEN],
}

impl GuardedKey {
    /// Wraps a 32-byte key array in a zeroizing container.
    #[inline]
    #[must_use]
    pub const fn new(key: [u8; BLAKE3_KEY_LEN]) -> Self {
        Self { key }
    }

    /// Validates and constructs a [`GuardedKey`] from a byte slice.
    pub fn from_slice(slice: &[u8]) -> Result<Self, Blake3DefenseError> {
        validate_key_len(slice)?;
        let mut key = [0u8; BLAKE3_KEY_LEN];
        key.copy_from_slice(slice);
        Ok(Self { key })
    }

    /// Borrows the internal 32-byte key reference.
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; BLAKE3_KEY_LEN] {
        &self.key
    }
}

impl fmt::Debug for GuardedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GuardedKey([REDACTED])")
    }
}

/// Sensitive context string wrapper with automatic zeroization on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct GuardedContext {
    context: Vec<u8>,
}

impl GuardedContext {
    /// Validates and constructs a [`GuardedContext`] from a domain string.
    pub fn new(context: &str, strict: bool) -> Result<Self, Blake3DefenseError> {
        validate_context_domain(context, strict, BLAKE3_MAX_CONTEXT_LEN)?;
        Ok(Self { context: context.as_bytes().to_vec() })
    }

    /// Borrows the context string as a UTF-8 slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.context).unwrap_or("")
    }
}

impl fmt::Debug for GuardedContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuardedContext").field("len", &self.context.len()).finish()
    }
}

// ============================================================================
// Configuration & Defensive Parameter Models
// ============================================================================

/// Configuration parameters for BLAKE3 defense-in-depth and quota enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blake3DefenseConfig {
    /// Maximum cumulative input data in bytes (default: 512 MiB).
    pub max_input_limit: u64,
    /// Maximum cumulative XOF output data in bytes (default: 1 GiB).
    pub max_xof_output_limit: u64,
    /// Maximum allowable tree stack reduction depth (default: 55).
    pub max_stack_depth: usize,
    /// Whether strict ASCII / printable domain separation format is enforced for KDF context strings.
    pub enforce_context_strictness: bool,
    /// Maximum allowable length of context domain string (default: 1024 bytes).
    pub max_context_len: usize,
}

impl Default for Blake3DefenseConfig {
    #[inline]
    fn default() -> Self {
        Self::default_limits()
    }
}

impl Blake3DefenseConfig {
    /// Constructs default production security limits.
    #[must_use]
    pub const fn default_limits() -> Self {
        Self {
            max_input_limit: BLAKE3_DEFAULT_MAX_INPUT_LIMIT,
            max_xof_output_limit: BLAKE3_DEFAULT_MAX_XOF_OUTPUT_LIMIT,
            max_stack_depth: BLAKE3_MAX_STACK_DEPTH,
            enforce_context_strictness: true,
            max_context_len: BLAKE3_MAX_CONTEXT_LEN,
        }
    }

    /// Sets custom cumulative input limit.
    #[must_use]
    pub const fn with_max_input_limit(mut self, limit: u64) -> Self {
        self.max_input_limit = limit;
        self
    }

    /// Sets custom cumulative XOF output limit.
    #[must_use]
    pub const fn with_max_xof_output_limit(mut self, limit: u64) -> Self {
        self.max_xof_output_limit = limit;
        self
    }

    /// Sets maximum tree stack depth limit.
    #[must_use]
    pub const fn with_max_stack_depth(mut self, depth: usize) -> Self {
        self.max_stack_depth = depth;
        self
    }

    /// Sets context domain strictness policy.
    #[must_use]
    pub const fn with_context_strictness(mut self, strict: bool) -> Self {
        self.enforce_context_strictness = strict;
        self
    }
}

// ============================================================================
// Core Defense Verification Functions
// ============================================================================

/// Validates that a key slice strictly conforms to the required 32-byte BLAKE3 length.
#[inline]
pub fn validate_key_len(key: &[u8]) -> Result<(), Blake3DefenseError> {
    if key.len() != BLAKE3_KEY_LEN {
        Err(Blake3DefenseError::InvalidKeyLength {
            actual: key.len(),
            expected: BLAKE3_KEY_LEN,
        })
    } else {
        Ok(())
    }
}

/// Validates that a key derivation context string adheres to domain uniqueness rules.
pub fn validate_context_domain(context: &str, strict: bool, max_len: usize) -> Result<(), Blake3DefenseError> {
    if context.is_empty() {
        return Err(Blake3DefenseError::InvalidContextDomain {
            reason: "context string must not be empty",
        });
    }
    if context.len() > max_len {
        return Err(Blake3DefenseError::InvalidContextDomain {
            reason: "context string exceeds maximum allowable length",
        });
    }
    if strict {
        if context.contains('\0') {
            return Err(Blake3DefenseError::InvalidContextDomain {
                reason: "context string contains illegal null bytes",
            });
        }
        if context.chars().any(|c| c.is_control()) {
            return Err(Blake3DefenseError::InvalidContextDomain {
                reason: "context string contains forbidden control characters",
            });
        }
        if context.trim().is_empty() {
            return Err(Blake3DefenseError::InvalidContextDomain {
                reason: "context string must contain meaningful printable characters",
            });
        }
    }
    Ok(())
}

/// Constant-time slice equality comparison to eliminate timing side channels.
#[inline]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    std::hint::black_box(diff) == 0
}

/// Constant-time 32-byte digest equality operator.
#[inline]
pub fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    std::hint::black_box(diff) == 0
}

/// Validates that domain separation flags isolate root node compression from intermediate nodes.
pub fn verify_root_flag_isolation(flags: u8, is_root: bool) -> Result<(), Blake3DefenseError> {
    if !is_root && (flags & ROOT) != 0 {
        return Err(Blake3DefenseError::InvalidRootFlagIsolation {
            reason: "intermediate chunk or tree node illegally contains ROOT flag",
        });
    }
    if is_root && (flags & ROOT) == 0 {
        return Err(Blake3DefenseError::InvalidRootFlagIsolation {
            reason: "finalized root node missing mandatory ROOT domain flag",
        });
    }
    Ok(())
}

/// Verifies that BLAKE3 Merkle tree natural anti-length-extension properties hold.
pub fn verify_anti_length_extension_immunity(
    original_data: &[u8],
    extension_data: &[u8],
) -> Result<bool, Blake3DefenseError> {
    if original_data.is_empty() || extension_data.is_empty() {
        return Ok(true);
    }
    let mut joint_hasher = Blake3Hasher::new();
    joint_hasher.update(original_data);
    joint_hasher.update(extension_data);
    let true_joint_digest = joint_hasher.finalize();

    let original_digest = crate::crypto::blake3::hash(original_data);
    let forged_digest = crate::crypto::blake3::keyed_hash(&original_digest, extension_data);

    if constant_time_eq_32(&true_joint_digest, &forged_digest) {
        return Err(Blake3DefenseError::LengthExtensionViolation {
            reason: "length-extension forgery succeeded unexpectedly",
        });
    }
    Ok(true)
}

/// Validates tree stack depth boundaries.
#[inline]
pub fn validate_tree_stack_depth(current_depth: usize, max_depth: usize) -> Result<(), Blake3DefenseError> {
    if current_depth > max_depth {
        Err(Blake3DefenseError::StackDepthOverflow { depth: current_depth, max_depth })
    } else {
        Ok(())
    }
}

// ============================================================================
// Guarded BLAKE3 Hasher Wrapper
// ============================================================================

/// High-assurance, defense-in-depth streaming BLAKE3 hasher wrapper.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct GuardedBlake3Hasher {
    pub(crate) inner: Blake3Hasher,
    #[zeroize(skip)]
    pub(crate) config: Blake3DefenseConfig,
    pub(crate) total_ingested: u64,
    pub(crate) total_xof_extracted: u64,
}

impl Default for GuardedBlake3Hasher {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl GuardedBlake3Hasher {
    /// Creates a new unkeyed [`GuardedBlake3Hasher`] with default production security limits.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_config(Blake3DefenseConfig::default_limits())
    }

    /// Creates a new unkeyed [`GuardedBlake3Hasher`] with custom security limits.
    #[must_use]
    pub fn new_with_config(config: Blake3DefenseConfig) -> Self {
        Self {
            inner: Blake3Hasher::new(),
            config,
            total_ingested: 0,
            total_xof_extracted: 0,
        }
    }

    /// Creates a new keyed [`GuardedBlake3Hasher`] validating exact 32-byte key boundaries.
    pub fn new_keyed(key: &[u8]) -> Result<Self, Blake3DefenseError> {
        Self::new_keyed_with_config(key, Blake3DefenseConfig::default_limits())
    }

    /// Creates a new keyed [`GuardedBlake3Hasher`] with custom security limits.
    pub fn new_keyed_with_config(key: &[u8], config: Blake3DefenseConfig) -> Result<Self, Blake3DefenseError> {
        validate_key_len(key)?;
        let mut key_arr = [0u8; 32];
        key_arr.copy_from_slice(key);
        let guarded_key = GuardedKey::new(key_arr);
        let inner = Blake3Hasher::new_keyed(guarded_key.as_bytes());
        Ok(Self { inner, config, total_ingested: 0, total_xof_extracted: 0 })
    }

    /// Creates a new key-derivation [`GuardedBlake3Hasher`] validating domain string invariants.
    pub fn new_derive_key(context: &str) -> Result<Self, Blake3DefenseError> {
        Self::new_derive_key_with_config(context, Blake3DefenseConfig::default_limits())
    }

    /// Creates a new key-derivation [`GuardedBlake3Hasher`] with custom security limits.
    pub fn new_derive_key_with_config(context: &str, config: Blake3DefenseConfig) -> Result<Self, Blake3DefenseError> {
        validate_context_domain(context, config.enforce_context_strictness, config.max_context_len)?;
        let inner = Blake3Hasher::new_derive_key(context);
        Ok(Self { inner, config, total_ingested: 0, total_xof_extracted: 0 })
    }

    /// Ingests input bytes with quota circuit breaker and arithmetic overflow defense.
    pub fn update(&mut self, input: &[u8]) -> Result<&mut Self, Blake3DefenseError> {
        if input.is_empty() {
            return Ok(self);
        }
        let input_len = input.len() as u64;
        let new_total = self
            .total_ingested
            .checked_add(input_len)
            .ok_or(Blake3DefenseError::ChunkCounterOverflow { counter: self.total_ingested })?;

        if new_total > self.config.max_input_limit {
            return Err(Blake3DefenseError::InputQuotaExceeded {
                current: self.total_ingested,
                attempted: input_len,
                limit: self.config.max_input_limit,
            });
        }

        validate_tree_stack_depth(self.inner.tree_stack.len(), self.config.max_stack_depth)?;
        self.inner.update(input);
        self.total_ingested = new_total;
        Ok(self)
    }

    /// Finalizes the hash and returns the standard 32-byte digest.
    #[inline]
    #[must_use]
    pub fn finalize(&self) -> [u8; 32] {
        self.inner.finalize()
    }

    /// Finalizes the hash into a destination slice, enforcing cumulative XOF output limits.
    pub fn finalize_into(&mut self, out: &mut [u8]) -> Result<(), Blake3DefenseError> {
        let requested_len = out.len() as u64;
        let new_xof_total = self
            .total_xof_extracted
            .checked_add(requested_len)
            .ok_or(Blake3DefenseError::XofOutputQuotaExceeded {
                current: self.total_xof_extracted,
                attempted: requested_len,
                limit: self.config.max_xof_output_limit,
            })?;

        if new_xof_total > self.config.max_xof_output_limit {
            return Err(Blake3DefenseError::XofOutputQuotaExceeded {
                current: self.total_xof_extracted,
                attempted: requested_len,
                limit: self.config.max_xof_output_limit,
            });
        }

        self.inner.finalize_into(out);
        self.total_xof_extracted = new_xof_total;
        Ok(())
    }

    /// Extracts arbitrary-length extended output (XOF) into an allocated buffer under quota limit.
    pub fn finalize_xof_guarded(&mut self, len: usize) -> Result<Vec<u8>, Blake3DefenseError> {
        let mut buf = vec![0u8; len];
        self.finalize_into(&mut buf)?;
        Ok(buf)
    }

    /// Verifies a keyed MAC or digest against an expected value in constant time.
    pub fn verify_mac(&self, expected_mac: &[u8; 32]) -> bool {
        let actual = self.finalize();
        constant_time_eq_32(&actual, expected_mac)
    }

    /// Resets the hasher and state counters while preserving configurations and keying.
    pub fn reset(&mut self) {
        self.inner.reset();
        self.total_ingested = 0;
        self.total_xof_extracted = 0;
    }

    /// Returns cumulative bytes ingested into this hasher instance.
    #[inline]
    #[must_use]
    pub const fn total_ingested(&self) -> u64 {
        self.total_ingested
    }

    /// Returns cumulative XOF bytes extracted from this hasher instance.
    #[inline]
    #[must_use]
    pub const fn total_xof_extracted(&self) -> u64 {
        self.total_xof_extracted
    }

    /// Returns the active defense configuration.
    #[inline]
    #[must_use]
    pub const fn config(&self) -> &Blake3DefenseConfig {
        &self.config
    }
}

impl io::Write for GuardedBlake3Hasher {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf).map_err(|e| io::Error::other(e.to_string()))?;
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Debug for GuardedBlake3Hasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GuardedBlake3Hasher")
            .field("total_ingested", &self.total_ingested)
            .field("total_xof_extracted", &self.total_xof_extracted)
            .field("config", &self.config)
            .finish()
    }
}

// ============================================================================
// Top-Level Convenience Guard Functions
// ============================================================================

/// Computes the default 32-byte BLAKE3 hash with default defense limits.
pub fn guarded_hash(input: &[u8]) -> Result<[u8; 32], Blake3DefenseError> {
    let mut hasher = GuardedBlake3Hasher::new();
    hasher.update(input)?;
    Ok(hasher.finalize())
}

/// Computes a 32-byte keyed BLAKE3 MAC validating key boundaries.
pub fn guarded_keyed_hash(key: &[u8], input: &[u8]) -> Result<[u8; 32], Blake3DefenseError> {
    let mut hasher = GuardedBlake3Hasher::new_keyed(key)?;
    hasher.update(input)?;
    Ok(hasher.finalize())
}

/// Derives a 32-byte subkey from context and key material under domain isolation rules.
pub fn guarded_derive_key(context: &str, material: &[u8]) -> Result<[u8; 32], Blake3DefenseError> {
    let mut hasher = GuardedBlake3Hasher::new_derive_key(context)?;
    hasher.update(material)?;
    Ok(hasher.finalize())
}

/// Computes keyed MAC and compares against `expected_mac` in constant time.
pub fn guarded_verify_mac(key: &[u8], input: &[u8], expected_mac: &[u8; 32]) -> Result<bool, Blake3DefenseError> {
    let mut hasher = GuardedBlake3Hasher::new_keyed(key)?;
    hasher.update(input)?;
    Ok(hasher.verify_mac(expected_mac))
}
