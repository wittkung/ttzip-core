// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-throughput Ed25519 batch signature verification using 128-bit pseudo-random scalar
//! folding and variable-time Multi-Scalar Multiplication (MSM), alongside divide-and-conquer
//! binary search for pinpointing faulty signatures.

use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::{Identity, VartimeMultiscalarMul};
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

use super::error::Ed25519Error;
use super::verifying::{Signature, VerifyingKey, SIGNATURE_LENGTH};
use crate::crypto::vault::get_random_bytes;

/// A single batch entry containing references to a verifying key, message, and signature.
#[derive(Clone, Copy, Debug)]
pub struct BatchItem<'a> {
    /// Verifying public key.
    pub verifying_key: &'a VerifyingKey,
    /// Message payload.
    pub message: &'a [u8],
    /// Ed25519 signature.
    pub signature: &'a Signature,
}

/// Accumulator and execution engine for high-performance batch signature verification.
#[derive(Default, Debug)]
pub struct BatchVerifier<'a> {
    items: Vec<BatchItem<'a>>,
}

impl<'a> BatchVerifier<'a> {
    /// Creates a new empty `BatchVerifier`.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Creates a `BatchVerifier` with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    /// Adds a signature verification request to the batch.
    pub fn add(&mut self, key: &'a VerifyingKey, message: &'a [u8], signature: &'a Signature) {
        self.items.push(BatchItem {
            verifying_key: key,
            message,
            signature,
        });
    }

    /// Adds a pre-formed `BatchItem` to the batch.
    pub fn add_item(&mut self, item: BatchItem<'a>) {
        self.items.push(item);
    }

    /// Returns the number of items in the batch.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the batch is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Verifies all signatures in the batch using OS entropy for 128-bit weights.
    pub fn verify(self) -> Result<(), Ed25519Error> {
        let mut seed = [0u8; 32];
        if get_random_bytes(&mut seed).is_err() {
            // Fallback deterministic seed derivation from batch elements
            let mut hasher = Sha512::new();
            for item in &self.items {
                hasher.update(item.verifying_key.as_bytes());
                hasher.update(item.signature.as_bytes());
            }
            let digest = hasher.finalize();
            seed.copy_from_slice(&digest[0..32]);
        }
        let result = self.verify_with_seed(&seed);
        seed.zeroize();
        result
    }

    /// Verifies all signatures in the batch deterministically using a supplied 32-byte seed.
    pub fn verify_with_seed(self, seed: &[u8; 32]) -> Result<(), Ed25519Error> {
        let n = self.items.len();
        if n == 0 {
            return Ok(());
        }

        // Decompress all points and canonicalize scalars
        let mut r_points = Vec::with_capacity(n);
        let mut a_points = Vec::with_capacity(n);
        let mut s_scalars = Vec::with_capacity(n);
        let mut k_scalars = Vec::with_capacity(n);
        let mut z_scalars = Vec::with_capacity(n);

        for (i, item) in self.items.iter().enumerate() {
            // 1. Verify scalar S is canonical
            let s_scalar_opt = Option::<Scalar>::from(Scalar::from_canonical_bytes(*item.signature.s_bytes()));
            let s = s_scalar_opt.ok_or(Ed25519Error::NonCanonicalScalar)?;
            s_scalars.push(s);

            // 2. Decompress point R
            let r_compressed = CompressedEdwardsY(*item.signature.r_bytes());
            let r_point = r_compressed.decompress().ok_or(Ed25519Error::InvalidSignatureFormat {
                actual_len: SIGNATURE_LENGTH,
            })?;
            r_points.push(r_point);

            // 3. Collect public key point A
            a_points.push(*item.verifying_key.point());

            // 4. Challenge scalar k_i = SHA-512(R_i || A_i || M_i) mod \ell
            let mut hasher = Sha512::new();
            hasher.update(item.signature.r_bytes());
            hasher.update(item.verifying_key.as_bytes());
            hasher.update(item.message);
            let mut challenge_digest = [0u8; 64];
            challenge_digest.copy_from_slice(&hasher.finalize());
            let k = Scalar::from_bytes_mod_order_wide(&challenge_digest);
            challenge_digest.zeroize();
            k_scalars.push(k);

            // 5. Derive 128-bit pseudo-random scalar weight z_i
            let mut z_hasher = Sha512::new();
            z_hasher.update(seed);
            z_hasher.update((i as u64).to_le_bytes());
            let z_digest = z_hasher.finalize();
            let mut z_bytes = [0u8; 32];
            z_bytes[0..16].copy_from_slice(&z_digest[0..16]);
            if z_bytes[0..16].iter().all(|&b| b == 0) {
                z_bytes[0] = 1;
            }
            let z = Scalar::from_bytes_mod_order(z_bytes);
            z_scalars.push(z);
        }

        // Compute basepoint scalar S_total = -\sum (z_i * S_i) mod \ell
        let mut s_total = Scalar::ZERO;
        for i in 0..n {
            s_total += z_scalars[i] * s_scalars[i];
        }
        let neg_s_total = -s_total;

        // Formulate scalars and points for Multi-Scalar Multiplication (MSM)
        // Equation: (-S_total) * B + \sum (z_i * R_i) + \sum ((z_i * k_i) * A_i) == Identity
        let mut scalars = Vec::with_capacity(1 + 2 * n);
        let mut points = Vec::with_capacity(1 + 2 * n);

        scalars.push(neg_s_total);
        points.push(ED25519_BASEPOINT_POINT);

        for i in 0..n {
            scalars.push(z_scalars[i]);
            points.push(r_points[i]);
        }

        for i in 0..n {
            let zk = z_scalars[i] * k_scalars[i];
            scalars.push(zk);
            points.push(a_points[i]);
        }

        let msm_result = EdwardsPoint::vartime_multiscalar_mul(scalars, points);

        if msm_result.ct_eq(&EdwardsPoint::identity()).into() {
            Ok(())
        } else {
            Err(Ed25519Error::BatchVerificationFailed)
        }
    }
}

/// Identifies indices of all invalid signatures in a batch using divide-and-conquer binary search.
///
/// If batch verification succeeds, returns an empty vector in O(1) batch time.
/// When invalid signatures exist, isolates the exact failure indices in O(k log N) time.
pub fn locate_faulty_signatures(
    verifying_keys: &[VerifyingKey],
    messages: &[&[u8]],
    signatures: &[Signature],
) -> Vec<usize> {
    let n = verifying_keys.len();
    if n != messages.len() || n != signatures.len() || n == 0 {
        return (0..n.max(messages.len()).max(signatures.len())).collect();
    }

    let mut faulty_indices = Vec::new();
    locate_faulty_recursive(
        verifying_keys,
        messages,
        signatures,
        0,
        n,
        &mut faulty_indices,
    );
    faulty_indices.sort_unstable();
    faulty_indices.dedup();
    faulty_indices
}

fn locate_faulty_recursive(
    keys: &[VerifyingKey],
    messages: &[&[u8]],
    signatures: &[Signature],
    offset: usize,
    len: usize,
    faulty: &mut Vec<usize>,
) {
    if len == 0 {
        return;
    }
    if len == 1 {
        let idx = offset;
        if keys[idx].verify(messages[idx], &signatures[idx]).is_err() {
            faulty.push(idx);
        }
        return;
    }

    // Attempt batch verification on the sub-slice
    let mut verifier = BatchVerifier::with_capacity(len);
    for i in 0..len {
        verifier.add(
            &keys[offset + i],
            messages[offset + i],
            &signatures[offset + i],
        );
    }

    if verifier.verify().is_ok() {
        // Entire partition is algebraically valid
        return;
    }

    // Partition contains at least one invalid signature; bisect
    let mid = len / 2;
    locate_faulty_recursive(keys, messages, signatures, offset, mid, faulty);
    locate_faulty_recursive(keys, messages, signatures, offset + mid, len - mid, faulty);
}
