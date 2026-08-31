// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! BLAKE3 Keyed Hashing (MAC) and Key Derivation Function (KDF) implementations.
//!
//! Provides domain-separated pseudorandom functions (PRF), message authentication codes (MAC),
//! and context-isolated key derivation matching the official BLAKE3 standard specification.

use super::{Blake3, OutputReader};

/// Constructs an incremental [`Blake3`] hasher initialized for keyed hashing mode.
///
/// In keyed hash mode, the standard IV is replaced with the 32-byte user key,
/// and the `KEYED_HASH` domain separation flag is applied.
#[inline]
pub fn new_keyed(key: &[u8; 32]) -> Blake3 {
    Blake3::new_keyed(key)
}

/// Computes a one-shot BLAKE3 keyed hash (MAC) over `input` with `key`.
///
/// Returns a 32-byte message authentication code.
#[inline]
pub fn keyed_hash(key: &[u8; 32], input: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3::new_keyed(key);
    hasher.update(input);
    hasher.finalize()
}

/// Constructs an incremental [`Blake3`] hasher initialized for key derivation mode with `context`.
///
/// Key derivation employs a two-stage domain separation:
/// 1. The globally unique `context` string is hashed with standard IV and `DERIVE_KEY_CONTEXT` flag
///    to derive a 32-byte context key.
/// 2. The context key initializes the key derivation hasher with `DERIVE_KEY_MATERIAL` flag.
#[inline]
pub fn new_derive_key(context: &str) -> Blake3 {
    Blake3::new_derive_key(context)
}

/// Derives a standard 32-byte subkey from the provided `context` string and `material`.
///
/// The `context` string must be hardcoded, globally unique, and application-specific
/// (e.g., `"ttzip 2026-08-31 archive payload key"`).
#[inline]
pub fn derive_key(context: &str, material: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3::new_derive_key(context);
    hasher.update(material);
    hasher.finalize()
}

/// Derives an extensible output stream ([`OutputReader`]) from `context` and `material`.
///
/// Enables arbitrary-length subkey material generation and random seeking.
#[inline]
pub fn derive_key_xof(context: &str, material: &[u8]) -> OutputReader {
    let mut hasher = Blake3::new_derive_key(context);
    hasher.update(material);
    hasher.finalize_xof()
}

/// Derives subkey material of arbitrary length directly into `out`.
#[inline]
pub fn derive_key_into(context: &str, material: &[u8], out: &mut [u8]) {
    let mut reader = derive_key_xof(context, material);
    reader.fill(out);
}
