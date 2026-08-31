// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Brotli RFC 7932 command and distance code lookup tables and algorithms.
//!
//! Provides compile-time generated 704-element command lookup tables (`CMD_LUT`)
//! and dynamic distance code postfix/direct prefix resolution structures.

/// Command lookup entry representing literal insert length, match copy length, and distance attributes.
#[derive(Clone, Copy, Debug)]
pub struct CmdLutElement {
    pub insert_len_offset: u16,
    pub insert_len_extra_bits: u8,
    pub copy_len_offset: u16,
    pub copy_len_extra_bits: u8,
    pub distance_code: i8,
    pub distance_context: u8,
}

/// Computes the compile-time 704-entry command lookup table per RFC 7932 Section 5.
const fn build_cmd_lut() -> [CmdLutElement; 704] {
    let k_insert_extra: [u8; 24] = [
        0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 12, 14, 24,
    ];
    let k_copy_extra: [u8; 24] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 24,
    ];
    let k_cell_pos: [usize; 11] = [0, 1, 0, 1, 8, 9, 2, 16, 10, 17, 18];

    let mut insert_offsets = [0u16; 24];
    let mut copy_offsets = [0u16; 24];
    insert_offsets[0] = 0;
    copy_offsets[0] = 2;

    let mut i = 0;
    while i < 23 {
        insert_offsets[i + 1] = insert_offsets[i] + (1u16 << k_insert_extra[i]);
        copy_offsets[i + 1] = copy_offsets[i] + (1u16 << k_copy_extra[i]);
        i += 1;
    }

    let mut lut = [CmdLutElement {
        insert_len_offset: 0,
        insert_len_extra_bits: 0,
        copy_len_offset: 0,
        copy_len_extra_bits: 0,
        distance_code: 0,
        distance_context: 0,
    }; 704];

    let mut symbol = 0;
    while symbol < 704 {
        let cell_idx = symbol >> 6;
        let cell_pos = k_cell_pos[cell_idx];
        let copy_code = ((cell_pos << 3) & 0x18) + (symbol & 0x7);
        let copy_len_offset = copy_offsets[copy_code];
        let insert_code = (cell_pos & 0x18) + ((symbol >> 3) & 0x7);

        let copy_len_extra_bits = k_copy_extra[copy_code];
        let context = if copy_len_offset > 4 {
            3
        } else {
            (copy_len_offset - 2) as u8
        };
        let distance_code = if cell_idx >= 2 { -1 } else { 0 };
        let insert_len_extra_bits = k_insert_extra[insert_code];
        let insert_len_offset = insert_offsets[insert_code];

        lut[symbol] = CmdLutElement {
            insert_len_offset,
            insert_len_extra_bits,
            copy_len_offset,
            copy_len_extra_bits,
            distance_code,
            distance_context: context,
        };
        symbol += 1;
    }
    lut
}

/// Static precomputed 704-element command lookup table.
pub static CMD_LUT: [CmdLutElement; 704] = build_cmd_lut();

/// Precalculates distance prefix code lookup table for regular distance codes per RFC 7932 Section 4.
pub fn calculate_distance_lut(
    npostfix: u32,
    ndirect: usize,
    alphabet_size_limit: usize,
) -> (Vec<u8>, Vec<usize>) {
    let mut dist_extra_bits = vec![0u8; alphabet_size_limit];
    let mut dist_offsets = vec![0usize; alphabet_size_limit];

    let postfix = 1usize << npostfix;
    let mut bits = 1u8;
    let mut half = 0usize;
    let mut i = 16usize;

    for j in 0..ndirect {
        if i < alphabet_size_limit {
            dist_extra_bits[i] = 0;
            dist_offsets[i] = j + 1;
            i += 1;
        }
    }

    while i < alphabet_size_limit {
        let base = ndirect + ((((2 + half) << bits) - 4) << npostfix) + 1;
        for j in 0..postfix {
            if i < alphabet_size_limit {
                dist_extra_bits[i] = bits;
                dist_offsets[i] = base + j;
                i += 1;
            }
        }
        bits += half as u8;
        half ^= 1;
    }

    (dist_extra_bits, dist_offsets)
}
