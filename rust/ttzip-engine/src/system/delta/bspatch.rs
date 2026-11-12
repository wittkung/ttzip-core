// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
            if ctrl.diff_len > 0 {
                apply_diff_segment(old_data, dest, diff_data, old_pos, new_pos, diff_pos, ctrl.diff_len)?;
                new_pos += ctrl.diff_len;
                old_pos += ctrl.diff_len;
                diff_pos += ctrl.diff_len;
            }

            if ctrl.extra_len > 0 {
                apply_extra_segment(dest, extra_data, new_pos, extra_pos, ctrl.extra_len)?;
                new_pos += ctrl.extra_len;
                extra_pos += ctrl.extra_len;
            }

            old_pos = compute_next_old_pos(old_pos, ctrl.seek_offset, old_data.len(), idx, controls)?;
        }

        if new_pos != new_len {
            return Err(DeltaError::CorruptedPatch(format!(
                "Reconstructed length mismatch: generated {} bytes, expected {} bytes",
                new_pos, new_len
            )));
        }

        Ok(())
    }
}

fn apply_diff_segment(
    old_data: &[u8],
    dest: &mut [u8],
    diff_data: &[u8],
    old_pos: usize,
    new_pos: usize,
    diff_pos: usize,
    diff_len: usize,
) -> DeltaResult<()> {
    let new_len = dest.len();
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
        dest[new_pos + i] = old_data[old_pos + i].wrapping_add(diff_data[diff_pos + i]);
    }
    Ok(())
}

fn apply_extra_segment(
    dest: &mut [u8],
    extra_data: &[u8],
    new_pos: usize,
    extra_pos: usize,
    extra_len: usize,
) -> DeltaResult<()> {
    let new_len = dest.len();
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
    Ok(())
}

fn compute_next_old_pos(
    old_pos: usize,
    seek_offset: i64,
    old_len: usize,
    idx: usize,
    controls: &[BsDiffControl],
) -> DeltaResult<usize> {
    let next_old_pos = (old_pos as i64).checked_add(seek_offset).ok_or_else(|| {
        DeltaError::CorruptedPatch(format!(
            "Control #{} seek offset arithmetic overflow: pos={}, offset={}",
            idx, old_pos, seek_offset
        ))
    })?;

    if next_old_pos < 0 {
        return Err(DeltaError::OutOfBoundsSeek {
            offset: next_old_pos,
            boundary: old_len,
        });
    }

    let next_old_usize = next_old_pos as usize;
    if next_old_usize > old_len && idx + 1 < controls.len() && controls[idx + 1].diff_len > 0 {
        return Err(DeltaError::OutOfBoundsSeek {
            offset: next_old_pos,
            boundary: old_len,
        });
    }

    Ok(next_old_usize)
}
