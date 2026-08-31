// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bzip2 Move-to-Front (MTF) transform, RLE1 input preprocessor, and RLE2 zero-run coder.

use crate::types::TTZipStatus;

pub const BZ_RUNA: u16 = 0;
pub const BZ_RUNB: u16 = 1;
pub const MAX_ALPHA_SIZE: usize = 258;

/// Compresses a raw input byte sequence using Bzip2 RLE1.
///
/// Runs of 4..=255 identical bytes are encoded as: 4 identical bytes + (run_length - 4).
pub fn rle1_compress(src: &[u8], dst: &mut Vec<u8>) {
    if src.is_empty() {
        return;
    }

    let mut i = 0;
    while i < src.len() {
        let ch = src[i];
        let mut run_len = 1;
        while i + run_len < src.len() && src[i + run_len] == ch && run_len < 255 {
            run_len += 1;
        }

        if run_len < 4 {
            for _ in 0..run_len {
                dst.push(ch);
            }
        } else {
            for _ in 0..4 {
                dst.push(ch);
            }
            dst.push((run_len - 4) as u8);
        }

        i += run_len;
    }
}

/// Decompresses an RLE1-encoded stream back to original bytes.
pub fn rle1_decompress(src: &[u8], dst: &mut Vec<u8>) -> Result<(), TTZipStatus> {
    if src.is_empty() {
        return Ok(());
    }

    let mut i = 0;
    let mut run_len = 0;
    let mut prev_ch = 0u8;

    while i < src.len() {
        let ch = src[i];
        dst.push(ch);
        i += 1;

        if run_len == 0 || ch == prev_ch {
            run_len += 1;
        } else {
            run_len = 1;
        }
        prev_ch = ch;

        if run_len == 4 {
            if i >= src.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let extra_count = src[i] as usize;
            i += 1;
            for _ in 0..extra_count {
                dst.push(ch);
            }
            run_len = 0;
        }
    }

    Ok(())
}

/// Computes symbol presence map and compact translation tables.
pub fn make_symbol_maps(in_use: &[bool; 256]) -> ([u8; 256], [u8; 256], usize) {
    let mut seq_to_unseq = [0u8; 256];
    let mut unseq_to_seq = [0u8; 256];
    let mut n_in_use = 0;

    for i in 0..256 {
        if in_use[i] {
            seq_to_unseq[n_in_use] = i as u8;
            unseq_to_seq[i] = n_in_use as u8;
            n_in_use += 1;
        }
    }

    (seq_to_unseq, unseq_to_seq, n_in_use)
}

/// Generates MTF values and RLE2 zero-run codes from BWT output.
pub fn generate_mtf_values(
    bwt_output: &[u8],
    in_use: &[bool; 256],
    mtf_symbols: &mut Vec<u16>,
    mtf_freq: &mut [u32; MAX_ALPHA_SIZE],
) {
    mtf_symbols.clear();
    for f in mtf_freq.iter_mut() {
        *f = 0;
    }

    let (_seq_to_unseq, unseq_to_seq, n_in_use) = make_symbol_maps(in_use);
    if n_in_use == 0 {
        return;
    }

    // Initialize MTF table with dense 0..n_in_use symbols
    let mut yy = [0u8; 256];
    for i in 0..n_in_use {
        yy[i] = i as u8;
    }

    let mut z_pend: u32 = 0;

    for &raw_ch in bwt_output {
        let dense_ch = unseq_to_seq[raw_ch as usize];

        // Find dense_ch in yy
        let mut idx = 0;
        while idx < n_in_use && yy[idx] != dense_ch {
            idx += 1;
        }

        if idx == 0 {
            z_pend += 1;
        } else {
            // Emit accumulated zero run
            if z_pend > 0 {
                emit_rle2_zero_run(z_pend, mtf_symbols, mtf_freq);
                z_pend = 0;
            }

            // Move to front
            let tmp = yy[idx];
            for k in (1..=idx).rev() {
                yy[k] = yy[k - 1];
            }
            yy[0] = tmp;

            // Emit symbol (idx + 1)
            let sym = (idx + 1) as u16;
            mtf_symbols.push(sym);
            if (sym as usize) < MAX_ALPHA_SIZE {
                mtf_freq[sym as usize] += 1;
            }
        }
    }

    // Emit trailing zero run if any
    if z_pend > 0 {
        emit_rle2_zero_run(z_pend, mtf_symbols, mtf_freq);
    }

    // Append EOB symbol (n_in_use + 1)
    let eob = (n_in_use + 1) as u16;
    mtf_symbols.push(eob);
    if (eob as usize) < MAX_ALPHA_SIZE {
        mtf_freq[eob as usize] += 1;
    }
}

/// Emits RLE2 bijective base-2 zero run symbols (RUNA / RUNB).
#[inline]
fn emit_rle2_zero_run(
    mut run_len: u32,
    mtf_symbols: &mut Vec<u16>,
    mtf_freq: &mut [u32; MAX_ALPHA_SIZE],
) {
    run_len -= 1;
    loop {
        if (run_len & 1) != 0 {
            mtf_symbols.push(BZ_RUNB);
            mtf_freq[BZ_RUNB as usize] += 1;
        } else {
            mtf_symbols.push(BZ_RUNA);
            mtf_freq[BZ_RUNA as usize] += 1;
        }

        if run_len < 2 {
            break;
        }
        run_len = (run_len - 2) / 2;
    }
}

/// Decodes RLE2 symbols and performs inverse MTF to reconstruct BWT output byte sequence.
pub fn rle2_decode_and_inverse_mtf(
    mtf_symbols: &[u16],
    in_use: &[bool; 256],
    dst: &mut Vec<u8>,
) -> Result<(), TTZipStatus> {
    let (seq_to_unseq, _unseq_to_seq, n_in_use) = make_symbol_maps(in_use);
    if n_in_use == 0 {
        return Ok(());
    }

    let mut yy = [0u8; 256];
    for i in 0..n_in_use {
        yy[i] = i as u8;
    }

    let eob = (n_in_use + 1) as u16;
    let mut i = 0;

    while i < mtf_symbols.len() {
        let sym = mtf_symbols[i];
        if sym == eob {
            break;
        }

        if sym == BZ_RUNA || sym == BZ_RUNB {
            // Bijective base-2 accumulator
            let mut es: i64 = -1;
            let mut factor: i64 = 1;

            while i < mtf_symbols.len() && (mtf_symbols[i] == BZ_RUNA || mtf_symbols[i] == BZ_RUNB) {
                let bit_val = if mtf_symbols[i] == BZ_RUNA { 1 } else { 2 };
                es += bit_val * factor;
                factor *= 2;
                i += 1;
            }

            let zero_ch = seq_to_unseq[yy[0] as usize];
            let count = (es + 1) as usize;
            for _ in 0..count {
                dst.push(zero_ch);
            }
        } else {
            // Non-zero symbol: idx = sym - 1
            let idx = (sym - 1) as usize;
            if idx >= n_in_use {
                return Err(TTZipStatus::ErrExtractionFailed);
            }

            let dense_ch = yy[idx];
            let raw_ch = seq_to_unseq[dense_ch as usize];
            dst.push(raw_ch);

            // Move to front
            for k in (1..=idx).rev() {
                yy[k] = yy[k - 1];
            }
            yy[0] = dense_ch;

            i += 1;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rle1_roundtrip() {
        let input = b"AAAAABBBBCDDDDDDDDDEEE";
        let mut compressed = Vec::new();
        rle1_compress(input, &mut compressed);

        let mut decompressed = Vec::new();
        rle1_decompress(&compressed, &mut decompressed).unwrap();
        assert_eq!(decompressed, input);
    }

    #[test]
    fn test_mtf_rle2_roundtrip() {
        let bwt_l = b"annbbaaa";
        let mut in_use = [false; 256];
        for &b in bwt_l {
            in_use[b as usize] = true;
        }

        let mut mtf_symbols = Vec::new();
        let mut mtf_freq = [0u32; MAX_ALPHA_SIZE];
        generate_mtf_values(bwt_l, &in_use, &mut mtf_symbols, &mut mtf_freq);

        let mut restored = Vec::new();
        rle2_decode_and_inverse_mtf(&mtf_symbols, &in_use, &mut restored).unwrap();
        assert_eq!(restored, bwt_l);
    }
}
