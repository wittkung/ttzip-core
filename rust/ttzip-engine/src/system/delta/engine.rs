// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified high-level binary delta engine facade.
//!
//! Provides one-shot patch generation (`create_patch`), verification and application
//! (`apply_patch`), execution metrics collection, and topological tree hashing.

use crate::checksum::xxh32;
use crate::system::delta::archive::TTZipDeltaArchive;
use crate::system::delta::bsdiff::TTZipBsDiff;
use crate::system::delta::bspatch::TTZipBsPatch;
use crate::system::delta::types::{
    DeltaCommand, DeltaError, DeltaPatchHeader, DeltaPatchResult, DeltaResult,
};
use sha2::{Digest, Sha256};

/// Domain separation seed for TTZip topological delta tree hashing ("TTZP").
pub const DELTA_TREE_HASH_SEED: u32 = 0x5454_5A50;

/// Unified binary delta patching engine.
pub struct TTZipDeltaEngine;

impl TTZipDeltaEngine {
    /// Computes the 32-bit topological tree hash for a data buffer.
    #[inline]
    pub fn calculate_tree_hash(data: &[u8]) -> u32 {
        xxh32(data, DELTA_TREE_HASH_SEED)
    }

    /// Creates a compressed `spk!` binary delta patch from `old_data` to `new_data`.
    pub fn create_patch(old_data: &[u8], new_data: &[u8]) -> DeltaResult<Vec<u8>> {
        Self::create_patch_with_commands(old_data, new_data, &[])
    }

    /// Creates a compressed `spk!` binary delta patch along with custom structural metadata commands.
    pub fn create_patch_with_commands(
        old_data: &[u8],
        new_data: &[u8],
        commands: &[DeltaCommand],
    ) -> DeltaResult<Vec<u8>> {
        let before_tree_hash = Self::calculate_tree_hash(old_data);
        let after_tree_hash = Self::calculate_tree_hash(new_data);
        let uncompressed_size = new_data.len() as u64;

        let patch = TTZipBsDiff::diff(old_data, new_data)?;
        let archive = TTZipDeltaArchive::create(
            before_tree_hash,
            after_tree_hash,
            uncompressed_size,
            &patch,
            commands,
        )?;

        archive.serialize()
    }

    /// Applies a compressed `spk!` delta patch to `old_data`, returning the reconstructed byte vector.
    pub fn apply_patch(old_data: &[u8], patch_data: &[u8]) -> DeltaResult<Vec<u8>> {
        let (output, _) = Self::apply_patch_with_result(old_data, patch_data)?;
        Ok(output)
    }

    /// Applies a compressed `spk!` delta patch and returns both reconstructed bytes and execution telemetry.
    pub fn apply_patch_with_result(
        old_data: &[u8],
        patch_data: &[u8],
    ) -> DeltaResult<(Vec<u8>, DeltaPatchResult)> {
        let archive = TTZipDeltaArchive::deserialize(patch_data)?;

        // 1. Verify before_tree_hash of source data
        let actual_before_hash = Self::calculate_tree_hash(old_data);
        if archive.header.before_tree_hash != actual_before_hash {
            return Err(DeltaError::SourceHashMismatch {
                expected: archive.header.before_tree_hash,
                actual: actual_before_hash,
            });
        }

        // 2. Decompress diff streams
        let patch = archive.decompress_patch()?;
        let target_len = archive.header.uncompressed_size as usize;
        let instructions_applied = patch.controls.len();

        // 3. Apply bspatch state machine
        let reconstructed = TTZipBsPatch::apply(old_data, target_len, &patch)?;

        // 4. Verify after_tree_hash of reconstructed target
        let actual_after_hash = Self::calculate_tree_hash(&reconstructed);
        if archive.header.after_tree_hash != actual_after_hash {
            return Err(DeltaError::TargetHashMismatch {
                expected: archive.header.after_tree_hash,
                actual: actual_after_hash,
            });
        }

        // 5. Compute telemetry SHA-256
        let mut hasher = Sha256::new();
        hasher.update(&reconstructed);
        let sha256_bytes = hasher.finalize();
        let mut sha256_hex = String::with_capacity(64);
        for byte in sha256_bytes {
            use std::fmt::Write;
            let _ = write!(&mut sha256_hex, "{:02x}", byte);
        }

        let telemetry = DeltaPatchResult {
            bytes_in: patch_data.len(),
            bytes_out: reconstructed.len(),
            instructions_applied,
            sha256_hex,
        };

        Ok((reconstructed, telemetry))
    }

    /// Inspects the header of a serialized delta patch without decompressing payload streams.
    #[inline]
    pub fn inspect_header(patch_data: &[u8]) -> DeltaResult<DeltaPatchHeader> {
        DeltaPatchHeader::from_bytes(patch_data)
    }
}
