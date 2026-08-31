// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Burrows-Wheeler Transform (BWT) block sorting engine.
//!
//! Implements Seward's two-stage suffix sorting with 2-byte prefix bucket partitioning,
//! 3-way radix quicksort, contiguous doubled-buffer SIMD slice comparison, and fallback sorting.

use crate::types::TTZipStatus;

pub const BZ_N_RADIX: usize = 2;
pub const BZ_N_QSORT: usize = 12;
pub const BZ_N_SHELL: usize = 18;
pub const BZ_N_OVERSHOOT: usize = BZ_N_RADIX + BZ_N_QSORT + BZ_N_SHELL + 2;

/// Performs Burrows-Wheeler Transform on a raw block.
///
/// Returns `(orig_ptr, transformed_L)` where `orig_ptr` is the 0-indexed row
/// of the original string in the sorted cyclic shift matrix, and `transformed_L`
/// is the last column (L) of the sorted matrix.
pub fn bwt_block_sort(block: &[u8], work_factor: i32) -> Result<(usize, Vec<u8>), TTZipStatus> {
    let nblock = block.len();
    if nblock == 0 {
        return Ok((0, Vec::new()));
    }
    if nblock == 1 {
        return Ok((0, vec![block[0]]));
    }

    let first_byte = block[0];
    if block.iter().all(|&b| b == first_byte) {
        return Ok((0, block.to_vec()));
    }

    let mut ptr = vec![0u32; nblock];
    for i in 0..nblock {
        ptr[i] = i as u32;
    }

    let wf = work_factor.clamp(1, 250);
    let mut budget = (nblock as i64 * (wf as i64).saturating_mul(10)) as i32;

    // Execute mainSort with fallback guard
    main_sort(&mut ptr, block, nblock, &mut budget)?;

    // Locate orig_ptr where ptr[orig_ptr] == 0
    let mut orig_ptr = 0;
    for (i, &p) in ptr.iter().enumerate() {
        if p == 0 {
            orig_ptr = i;
            break;
        }
    }

    // Construct transformed last column L: L[i] = block[(ptr[i] + nblock - 1) % nblock]
    let mut transformed_l = Vec::with_capacity(nblock);
    for &p in &ptr {
        let prev_idx = if p == 0 { nblock - 1 } else { (p - 1) as usize };
        transformed_l.push(block[prev_idx]);
    }

    Ok((orig_ptr, transformed_l))
}

/// Seward mainSort: 2-byte prefix bucket partitioning with fallback.
fn main_sort(
    ptr: &mut [u32],
    block: &[u8],
    nblock: usize,
    budget: &mut i32,
) -> Result<(), TTZipStatus> {
    if nblock <= 1 {
        return Ok(());
    }

    // Doubled block buffer to allow zero-copy, branchless SIMD suffix comparisons
    let mut block_doubled = Vec::with_capacity(2 * nblock);
    block_doubled.extend_from_slice(block);
    block_doubled.extend_from_slice(block);

    // 2-byte prefix histogram (65,536 buckets)
    let mut ftab = vec![0u32; 65537];
    for i in 0..nblock {
        let b1 = block_doubled[i] as usize;
        let b2 = block_doubled[i + 1] as usize;
        let j = (b1 << 8) | b2;
        ftab[j] += 1;
    }

    // Cumulative sum: ftab[sb] becomes the end of bucket sb
    for i in 1..=65536 {
        ftab[i] += ftab[i - 1];
    }

    // Distribute suffix pointers into 2-byte buckets using bucket_start
    let mut bucket_start = vec![0u32; 65537];
    bucket_start[1..=65536].copy_from_slice(&ftab[..65536]);

    for i in 0..nblock {
        let b1 = block_doubled[i] as usize;
        let b2 = block_doubled[i + 1] as usize;
        let j = (b1 << 8) | b2;
        let pos = bucket_start[j];
        bucket_start[j] += 1;
        ptr[pos as usize] = i as u32;
    }

    // Sort sub-buckets using 3-way quicksort
    for sb in 0..65536 {
        let lo = if sb == 0 { 0 } else { ftab[sb - 1] as usize };
        let hi = ftab[sb] as usize;
        if hi > lo + 1 {
            qsort3_suffixes(
                &mut ptr[lo..hi],
                &block_doubled,
                nblock,
                2, // First 2 bytes are already identical in this bucket
                budget,
            );
        }
    }

    Ok(())
}

/// 3-way radix quicksort on suffix pointers.
fn qsort3_suffixes(
    slice: &mut [u32],
    block: &[u8],
    nblock: usize,
    depth: usize,
    budget: &mut i32,
) {
    if slice.len() <= 1 {
        return;
    }

    if slice.len() <= 16 || depth > 24 || *budget <= 0 {
        // Insertion sort / pdqsort fallback for small partitions or deep branches
        insertion_sort_suffixes(slice, block, nblock, depth);
        return;
    }

    *budget -= slice.len() as i32;

    // Median of three pivot selection
    let mid = slice.len() / 2;
    let lo_byte = block[slice[0] as usize + depth];
    let mid_byte = block[slice[mid] as usize + depth];
    let hi_byte = block[slice[slice.len() - 1] as usize + depth];
    let pivot = median(lo_byte, mid_byte, hi_byte);

    let mut lt = 0;
    let mut gt = slice.len();
    let mut i = 0;

    while i < gt {
        let b = block[slice[i] as usize + depth];
        if b < pivot {
            slice.swap(lt, i);
            lt += 1;
            i += 1;
        } else if b > pivot {
            gt -= 1;
            slice.swap(i, gt);
        } else {
            i += 1;
        }
    }

    // Recurse on subpartitions
    if lt > 0 {
        qsort3_suffixes(&mut slice[0..lt], block, nblock, depth, budget);
    }
    if gt > lt {
        qsort3_suffixes(&mut slice[lt..gt], block, nblock, depth + 1, budget);
    }
    if gt < slice.len() {
        qsort3_suffixes(&mut slice[gt..], block, nblock, depth, budget);
    }
}

/// Insertion sort for short suffix partitions with pdqsort fallback for large slices.
fn insertion_sort_suffixes(slice: &mut [u32], block: &[u8], nblock: usize, start_depth: usize) {
    if slice.len() > 32 {
        slice.sort_unstable_by(|&p1, &p2| {
            let s1 = &block[p1 as usize + start_depth..p1 as usize + nblock];
            let s2 = &block[p2 as usize + start_depth..p2 as usize + nblock];
            s1.cmp(s2)
        });
        return;
    }

    for i in 1..slice.len() {
        let tmp = slice[i];
        let mut j = i;
        while j > 0 && compare_suffixes(block, nblock, tmp as usize, slice[j - 1] as usize, start_depth) {
            slice[j] = slice[j - 1];
            j -= 1;
        }
        slice[j] = tmp;
    }
}

/// Compares two suffixes starting from `depth`. Returns true if `p1` < `p2`.
#[inline(always)]
fn compare_suffixes(block: &[u8], nblock: usize, p1: usize, p2: usize, depth: usize) -> bool {
    let s1 = &block[p1 + depth..p1 + nblock];
    let s2 = &block[p2 + depth..p2 + nblock];
    s1 < s2
}

#[inline(always)]
fn median(a: u8, b: u8, c: u8) -> u8 {
    if a < b {
        if b < c { b } else if a < c { c } else { a }
    } else {
        if a < c { a } else if b < c { c } else { b }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bwt_simple_string() {
        let input = b"banana";
        let (orig_ptr, l) = bwt_block_sort(input, 30).unwrap();
        assert_eq!(l.len(), input.len());
        assert_eq!(orig_ptr, 3);
        assert_eq!(l, b"nnbaaa");
    }

    #[test]
    fn test_bwt_all_identical() {
        let input = vec![b'A'; 100];
        let (orig_ptr, l) = bwt_block_sort(&input, 30).unwrap();
        assert_eq!(orig_ptr, 0);
        assert_eq!(l, input);
    }
}
