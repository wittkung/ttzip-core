// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Inverse Burrows-Wheeler Transform (Inverse BWT) and matrix reconstruction engine.
//!
//! Features:
//! - FAST mode: 32-bit in-place fused `tt[p] = (i << 8) | L[i]` single-instruction pointer chasing.
//! - SMALL mode: 2.5 bytes/symbol compact memory representation with in-place cycle pointer reversal.

use crate::types::TTZipStatus;

/// Performs fast Inverse BWT in $O(N)$ time using 4 bytes/symbol packed `tt` vector.
pub fn inverse_bwt_fast(
    transformed_l: &[u8],
    orig_ptr: usize,
    dst: &mut [u8],
) -> Result<(), TTZipStatus> {
    let nblock = transformed_l.len();
    if nblock == 0 {
        return Ok(());
    }
    if orig_ptr >= nblock {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if dst.len() < nblock {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    // 1. Calculate character frequency histogram unzftab[256]
    let mut unzftab = [0u32; 256];
    for &ch in transformed_l {
        unzftab[ch as usize] += 1;
    }

    // 2. Compute cumulative base frequency table cftab[257]
    let mut cftab = [0u32; 257];
    cftab[1..=256].copy_from_slice(&unzftab[..256]);
    for i in 1..=256 {
        cftab[i] += cftab[i - 1];
    }

    // 3. Construct 32-bit packed fused vector tt[p] = (i << 8) | L[i]
    let mut tt = vec![0u32; nblock];
    for i in 0..nblock {
        let uc = transformed_l[i];
        let p = cftab[uc as usize] as usize;
        if p >= nblock {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        tt[p] = ((i as u32) << 8) | (uc as u32);
        cftab[uc as usize] += 1;
    }

    // 4. Single-step pointer chasing from orig_ptr
    let mut t_pos = orig_ptr;
    for out_idx in 0..nblock {
        let val = tt[t_pos];
        dst[out_idx] = (val & 0xFF) as u8;
        t_pos = (val >> 8) as usize;
        if t_pos >= nblock && out_idx + 1 < nblock {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
    }

    Ok(())
}

/// Performs memory-compact Inverse BWT using 2.5 bytes/symbol `ll16` + `ll4` vectors.
pub fn inverse_bwt_small(
    transformed_l: &[u8],
    orig_ptr: usize,
    dst: &mut [u8],
) -> Result<(), TTZipStatus> {
    let nblock = transformed_l.len();
    if nblock == 0 {
        return Ok(());
    }
    if orig_ptr >= nblock {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    if dst.len() < nblock {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut unzftab = [0u32; 256];
    for &ch in transformed_l {
        unzftab[ch as usize] += 1;
    }

    let mut cftab = [0u32; 257];
    cftab[1..=256].copy_from_slice(&unzftab[..256]);
    for i in 1..=256 {
        cftab[i] += cftab[i - 1];
    }

    let mut cftab_copy = cftab;

    // ll16 (lower 16 bits) and ll4 (upper 4 bits)
    let mut ll16 = vec![0u16; nblock];
    let mut ll4 = vec![0u8; nblock.div_ceil(2)];

    for i in 0..nblock {
        let uc = transformed_l[i];
        let p = cftab_copy[uc as usize] as usize;
        if p >= nblock {
            return Err(TTZipStatus::ErrExtractionFailed);
        }
        set_ll(&mut ll16, &mut ll4, i, p as u32);
        cftab_copy[uc as usize] += 1;
    }

    // In-place cycle pointer reversal
    let mut i = orig_ptr;
    let mut j = get_ll(&ll16, &ll4, i) as usize;
    loop {
        let tmp = get_ll(&ll16, &ll4, j) as usize;
        set_ll(&mut ll16, &mut ll4, j, i as u32);
        i = j;
        j = tmp;
        if i == orig_ptr {
            break;
        }
    }

    // Recover characters using binary search in cftab
    let mut t_pos = orig_ptr;
    for out_idx in 0..nblock {
        let ch = index_into_f(t_pos as u32, &cftab);
        dst[out_idx] = ch as u8;
        t_pos = get_ll(&ll16, &ll4, t_pos) as usize;
    }

    Ok(())
}

#[inline(always)]
fn get_ll(ll16: &[u16], ll4: &[u8], i: usize) -> u32 {
    let lo = ll16[i] as u32;
    let hi_byte = ll4[i >> 1] as u32;
    let hi = if (i & 1) == 0 {
        hi_byte & 0x0F
    } else {
        (hi_byte >> 4) & 0x0F
    };
    (hi << 16) | lo
}

#[inline(always)]
fn set_ll(ll16: &mut [u16], ll4: &mut [u8], i: usize, val: u32) {
    ll16[i] = (val & 0xFFFF) as u16;
    let hi = ((val >> 16) & 0x0F) as u8;
    let byte_idx = i >> 1;
    if (i & 1) == 0 {
        ll4[byte_idx] = (ll4[byte_idx] & 0xF0) | hi;
    } else {
        ll4[byte_idx] = (ll4[byte_idx] & 0x0F) | (hi << 4);
    }
}

/// Binary search to find character in sorted F column using cumulative table.
#[inline(always)]
fn index_into_f(indx: u32, cftab: &[u32; 257]) -> usize {
    let mut nb = 0;
    let mut na = 256;
    while na - nb > 1 {
        let mid = (nb + na) >> 1;
        if indx >= cftab[mid] {
            nb = mid;
        } else {
            na = mid;
        }
    }
    nb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inverse_bwt_roundtrip() {
        // "banana" BWT output L: "annbba", orig_ptr
        let raw = b"banana";
        // BWT of banana:
        // sorted cyclic shifts:
        // 0: abanab -> b
        // 1: anaban -> a
        // 2: ananab -> b
        // 3: banana -> a
        // 4: nabana -> a
        // 5: nanaba -> a
        // Let's test with real BWT output
        let (orig_ptr, l) = crate::codecs::bzip2::blocksort::bwt_block_sort(raw, 30).unwrap();
        let mut restored = vec![0u8; raw.len()];
        inverse_bwt_fast(&l, orig_ptr, &mut restored).unwrap();
        assert_eq!(&restored, raw);

        let mut restored_small = vec![0u8; raw.len()];
        inverse_bwt_small(&l, orig_ptr, &mut restored_small).unwrap();
        assert_eq!(&restored_small, raw);
    }
}
