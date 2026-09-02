// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust `bspatch` state machine and additive patch applicator.
//!
//! Reconstructs target binaries with wrapped byte-addition arithmetic `(old + diff) mod 256`,
//! strict boundary checking, and out-of-bounds overflow tripwires.

use crate::system::delta::bsdiff::{BsDiffControl, BsDiffPatch};
use crate::system::delta::types::{DeltaError, DeltaResult};

/// Pure Safe Rust high-throughput bspatch state machine.
pub struct TTZipBsPatch;

impl TTZipBsPatch {
    /// Applies a parsed `BsDiffPatch` to `old_data` to reconstruct target payload of `new_len` bytes.
    #[inline]
    pub fn apply(
        old_data: &[u8],
        new_len: usize,
        patch: &BsDiffPatch,
    ) -> DeltaResult<Vec<u8>> {
        Self::apply_streams(
            old_data,
            new_len,
            &patch.controls,
            &patch.diff_data,
            &patch.extra_data,
        )
    }

    /// Applies raw control triplets, additive diff bytes, and literal extra bytes to `old_data`.
    pub fn apply_streams(
        old_data: &[u8],
        new_len: usize,
        controls: &[BsDiffControl],
        diff_data: &[u8],
        extra_data: &[u8],
    ) -> DeltaResult<Vec<u8>> {
        if new_len == 0 {
            return Ok(Vec::new());
        }

        let mut output = vec![0u8; new_len];
        Self::apply_to_slice(old_data, &mut output, controls, diff_data, extra_data)?;
        Ok(output)
    }

    /// Applies patch streams directly into a pre-allocated destination buffer.
    pub fn apply_to_slice(
        old_data: &[u8],
        dest: &mut [u8],
        controls: &[BsDiffControl],
        diff_data: &[u8],
        extra_data: &[u8],
    ) -> DeltaResult<()> {
        let new_len = dest.len();
        let mut old_pos: usize = 0;
        let mut new_pos: usize = 0;
        let mut diff_pos: usize = 0;
        let mut extra_pos: usize = 0;

        for (idx, ctrl) in controls.iter().enumerate() {
            let diff_len = ctrl.diff_len;
            let extra_len = ctrl.extra_len;
            let seek_offset = ctrl.seek_offset;

            // 1. Additive Diff Block: (old + diff) mod 256
            if diff_len > 0 {
                if new_pos + diff_len > new_len {
                    return Err(DeltaError::TargetBufferOverflow {
                        requested: new_pos + diff_len,
                        capacity: new_len,
                    });
                }
                if diff_pos + diff_len > diff_data.len() {
                    return Err(DeltaError::TruncatedData {
                        needed: diff_pos + diff_len,
                        available: diff_data.len(),
                    });
                }
                if old_pos + diff_len > old_data.len() {
                    return Err(DeltaError::OutOfBoundsSeek {
                        offset: (old_pos + diff_len) as i64,
                        boundary: old_data.len(),
                    });
                }

                for i in 0..diff_len {
                    let old_byte = old_data[old_pos + i];
                    let diff_byte = diff_data[diff_pos + i];
                    dest[new_pos + i] = old_byte.wrapping_add(diff_byte);
                }

                new_pos += diff_len;
                old_pos += diff_len;
                diff_pos += diff_len;
            }

            // 2. Literal Extra Block: direct insertion
            if extra_len > 0 {
                if new_pos + extra_len > new_len {
                    return Err(DeltaError::TargetBufferOverflow {
                        requested: new_pos + extra_len,
                        capacity: new_len,
                    });
                }
                if extra_pos + extra_len > extra_data.len() {
                    return Err(DeltaError::TruncatedData {
                        needed: extra_pos + extra_len,
                        available: extra_data.len(),
                    });
                }

                dest[new_pos..new_pos + extra_len]
                    .copy_from_slice(&extra_data[extra_pos..extra_pos + extra_len]);

                new_pos += extra_len;
                extra_pos += extra_len;
            }

            // 3. Relative Seek Displacement in old data
            let next_old_pos = (old_pos as i64).checked_add(seek_offset).ok_or_else(|| {
                DeltaError::CorruptedPatch(format!(
                    "Control #{} seek offset arithmetic overflow: pos={}, offset={}",
                    idx, old_pos, seek_offset
                ))
            })?;

            if next_old_pos < 0 {
                return Err(DeltaError::OutOfBoundsSeek {
                    offset: next_old_pos,
                    boundary: old_data.len(),
                });
            }

            let next_old_usize = next_old_pos as usize;
            if next_old_usize > old_data.len() {
                // If there are subsequent controls with diff_len > 0, bounds check will trip on diff_len
                // But allow pointing to EOF if this was the last instruction or followed by extra-only
                if idx + 1 < controls.len() && controls[idx + 1].diff_len > 0 {
                    return Err(DeltaError::OutOfBoundsSeek {
                        offset: next_old_pos,
                        boundary: old_data.len(),
                    });
                }
            }

            old_pos = next_old_usize;
        }

        // Integrity verification: target buffer must be completely filled
        if new_pos != new_len {
            return Err(DeltaError::CorruptedPatch(format!(
                "Reconstructed length mismatch: generated {} bytes, expected {} bytes",
                new_pos, new_len
            )));
        }

        Ok(())
    }
}
