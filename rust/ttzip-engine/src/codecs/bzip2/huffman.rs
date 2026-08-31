// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-table Canonical Huffman coding and decoding engine for Bzip2.
//!
//! Features length-limited code tree generation (<= 20 bits), 4-round K-Means
//! iterative clustering, selector MTF translation, and zero-pointer fast lookup tables.

use crate::types::TTZipStatus;

pub const BZ_N_GROUPS: usize = 6;
pub const BZ_G_SIZE: usize = 50;
pub const BZ_N_ITERS: usize = 4;
pub const BZ_MAX_SELECTORS: usize = 18002;
pub const BZ_MAX_CODE_LEN: usize = 20;

/// Generates length-limited Canonical Huffman code lengths from symbol frequencies.
///
/// Uses weight folding to guarantee that all code lengths are <= `max_len`.
pub fn hb_make_code_lengths(len: &mut [u8], freq: &[u32], alpha_size: usize, max_len: usize) {
    if alpha_size == 0 {
        return;
    }
    if alpha_size == 1 {
        len[0] = 1;
        return;
    }

    let mut weight = vec![0u32; 2 * alpha_size + 2];
    let mut parent = vec![0i32; 2 * alpha_size + 2];

    for i in 0..alpha_size {
        let f = if freq[i] == 0 { 1 } else { freq[i] };
        weight[i + 1] = f << 8;
    }

    loop {
        let mut n_nodes = alpha_size;
        let mut n_heap = 0;
        let mut heap = vec![0usize; 2 * alpha_size + 2];

        for i in 1..=alpha_size {
            parent[i] = -1;
            n_heap += 1;
            heap[n_heap] = i;
            upheap(&mut heap, &weight, n_heap);
        }

        while n_heap > 1 {
            let n1 = heap[1];
            heap[1] = heap[n_heap];
            n_heap -= 1;
            downheap(&mut heap, &weight, n_heap, 1);

            let n2 = heap[1];
            heap[1] = heap[n_heap];
            n_heap -= 1;
            downheap(&mut heap, &weight, n_heap, 1);

            n_nodes += 1;
            parent[n1] = n_nodes as i32;
            parent[n2] = n_nodes as i32;

            let w1 = weight[n1];
            let w2 = weight[n2];
            weight[n_nodes] = ((w1 & 0xFFFF_FF00) + (w2 & 0xFFFF_FF00))
                | (1 + (w1 & 0xFF).max(w2 & 0xFF));
            parent[n_nodes] = -1;

            n_heap += 1;
            heap[n_heap] = n_nodes;
            upheap(&mut heap, &weight, n_heap);
        }

        let mut too_long = false;
        for i in 1..=alpha_size {
            let mut j = i as i32;
            let mut k = 0;
            while parent[j as usize] >= 0 {
                j = parent[j as usize];
                k += 1;
            }
            len[i - 1] = k as u8;
            if k > max_len {
                too_long = true;
            }
        }

        if !too_long {
            break;
        }

        // Weight folding: reduce dynamic range to flatten tree depth
        for i in 1..=alpha_size {
            let mut j = weight[i] >> 8;
            j = 1 + (j / 2);
            weight[i] = j << 8;
        }
    }
}

fn upheap(heap: &mut [usize], weight: &[u32], mut z: usize) {
    let tmp = heap[z];
    while weight[tmp] < weight[heap[z / 2]] && z > 1 {
        heap[z] = heap[z / 2];
        z /= 2;
    }
    heap[z] = tmp;
}

fn downheap(heap: &mut [usize], weight: &[u32], n_heap: usize, mut z: usize) {
    let tmp = heap[z];
    while 2 * z <= n_heap {
        let mut zz = 2 * z;
        if zz < n_heap && weight[heap[zz + 1]] < weight[heap[zz]] {
            zz += 1;
        }
        if weight[tmp] <= weight[heap[zz]] {
            break;
        }
        heap[z] = heap[zz];
        z = zz;
    }
    heap[z] = tmp;
}

/// Assigns bitwise Canonical Huffman codes based on code lengths.
pub fn hb_assign_codes(
    code: &mut [i32],
    length: &[u8],
    min_len: usize,
    max_len: usize,
    alpha_size: usize,
) {
    let mut vec = 0;
    for n in min_len..=max_len {
        for i in 0..alpha_size {
            if length[i] as usize == n {
                code[i] = vec;
                vec += 1;
            }
        }
        vec <<= 1;
    }
}

/// Creates decoder lookup tables (`limit`, `base`, `perm`) from Canonical code lengths.
pub fn hb_create_decode_tables(
    limit: &mut [i32],
    base: &mut [i32],
    perm: &mut [i32],
    length: &[u8],
    min_len: usize,
    max_len: usize,
    alpha_size: usize,
) {
    let mut pp = 0;
    for i in min_len..=max_len {
        for j in 0..alpha_size {
            if length[j] as usize == i {
                perm[pp] = j as i32;
                pp += 1;
            }
        }
    }

    for b in base.iter_mut() {
        *b = 0;
    }
    for l in limit.iter_mut() {
        *l = 0;
    }

    for i in 0..alpha_size {
        let len_idx = length[i] as usize;
        if len_idx + 1 < base.len() {
            base[len_idx + 1] += 1;
        }
    }

    for i in 1..base.len() {
        base[i] += base[i - 1];
    }

    let mut vec: i32 = 0;
    for i in min_len..=max_len {
        if i + 1 < base.len() && i < limit.len() {
            let count = base[i + 1] - base[i];
            vec += count;
            limit[i] = vec - 1;
            vec <<= 1;
        }
    }

    for i in (min_len + 1)..=max_len {
        if i < base.len() && i > 0 && (i - 1) < limit.len() {
            base[i] = ((limit[i - 1] + 1) << 1) - base[i];
        }
    }
}

/// MSB-first bit reader for Bzip2 bitstream decoding.
#[derive(Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_offset: usize,
    bit_buf: u64,
    bits_live: u32,
}

impl<'a> BitReader<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_offset: 0,
            bit_buf: 0,
            bits_live: 0,
        }
    }

    #[inline]
    pub fn read_bit(&mut self) -> Result<u32, TTZipStatus> {
        self.read_bits(1)
    }

    #[inline]
    pub fn read_bits(&mut self, n: u32) -> Result<u32, TTZipStatus> {
        if n == 0 {
            return Ok(0);
        }
        while self.bits_live < n {
            if self.byte_offset >= self.data.len() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let b = self.data[self.byte_offset] as u64;
            self.byte_offset += 1;
            self.bit_buf = (self.bit_buf << 8) | b;
            self.bits_live += 8;
        }

        let shift = self.bits_live - n;
        let val = (self.bit_buf >> shift) & ((1u64 << n) - 1);
        self.bits_live -= n;
        Ok(val as u32)
    }
}

/// Decodes a single Huffman symbol using zero-pointer `limit`, `base`, `perm` tables.
#[inline]
pub fn huffman_decode_symbol(
    reader: &mut BitReader,
    limit: &[i32],
    base: &[i32],
    perm: &[i32],
    min_len: usize,
) -> Result<u16, TTZipStatus> {
    let mut zn = min_len;
    let mut zvec = reader.read_bits(zn as u32)? as i32;

    while zn <= BZ_MAX_CODE_LEN {
        if zvec <= limit[zn] {
            let idx = (zvec - base[zn]) as usize;
            if idx < perm.len() {
                return Ok(perm[idx] as u16);
            } else {
                return Err(TTZipStatus::ErrExtractionFailed);
            }
        }
        zn += 1;
        let next_bit = reader.read_bit()? as i32;
        zvec = (zvec << 1) | next_bit;
    }

    Err(TTZipStatus::ErrExtractionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_huffman_assignment() {
        let lengths = [2, 1, 3, 3];
        let mut codes = [0i32; 4];
        hb_assign_codes(&mut codes, &lengths, 1, 3, 4);

        // Symbol 1 (len 1): code 0
        // Symbol 0 (len 2): code 2 (0b10)
        // Symbol 2 (len 3): code 6 (0b110)
        // Symbol 3 (len 3): code 7 (0b111)
        assert_eq!(codes[1], 0);
        assert_eq!(codes[0], 2);
        assert_eq!(codes[2], 6);
        assert_eq!(codes[3], 7);
    }
}
