// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Rust Apple LZFSE 4-Way associative hash matcher and backward FSE encoder.

use super::fse::{
    fse_init_encoder_table, fse_normalize_freq, lzfse_encode_v1_freq_table, FseEncoderEntry,
};
use super::tables::*;
use crate::types::TTZipStatus;

// MARK: - History Table and Match Types

/// 32-byte aligned 4-Way history line containing 4 candidate match positions and their 4-byte values.
#[repr(C, align(32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LzfseHistorySet {
    pub pos: [i32; LZFSE_ENCODE_HASH_WIDTH],
    pub value: [u32; LZFSE_ENCODE_HASH_WIDTH],
}

impl Default for LzfseHistorySet {
    #[inline]
    fn default() -> Self {
        const INVALID_POS: i32 = -4 * (LZFSE_ENCODE_MAX_D_VALUE as i32);
        Self {
            pos: [INVALID_POS; LZFSE_ENCODE_HASH_WIDTH],
            value: [0; LZFSE_ENCODE_HASH_WIDTH],
        }
    }
}

/// 16,384-bucket 4-Way hash table for LZFSE match finding.
pub struct LzfseMatchTable {
    pub entries: Box<[LzfseHistorySet; LZFSE_ENCODE_HASH_VALUES]>,
}

impl Default for LzfseMatchTable {
    fn default() -> Self {
        Self::new()
    }
}

impl LzfseMatchTable {
    /// Creates a newly zero-initialized match table with all invalid positions.
    pub fn new() -> Self {
        let default_entry = LzfseHistorySet::default();
        let vec = vec![default_entry; LZFSE_ENCODE_HASH_VALUES];
        let boxed_slice = vec.into_boxed_slice();
        let raw = Box::into_raw(boxed_slice) as *mut [LzfseHistorySet; LZFSE_ENCODE_HASH_VALUES];
        Self {
            entries: unsafe { Box::from_raw(raw) },
        }
    }

    /// Resets all buckets to the default invalid state without reallocating.
    #[inline]
    pub fn reset(&mut self) {
        self.entries.fill(LzfseHistorySet::default());
    }
}

/// Raw match found by 4-Way hash matcher.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct LzfseRawMatch {
    pub pos: usize,
    pub ref_pos: usize,
    pub length: usize,
}

/// Triplet of (Literal Length, Match Length, Match Distance) for LMD encoding.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct LmdTriplet {
    pub l: u32,
    pub m: u32,
    pub d: u32,
}

// MARK: - Knuth Multiplicative Hash

/// Knuth 32-bit golden ratio hash function producing 14-bit table index.
#[inline(always)]
pub fn hash_4bytes(x: u32) -> usize {
    ((x.wrapping_mul(0x9e37_79b1)) >> (32 - LZFSE_ENCODE_HASH_BITS)) as usize
}

// MARK: - Bitstream Writer

/// Output bitstream accumulator for backward FSE entropy coding.
#[derive(Debug, Default, Clone)]
pub struct FseOutStream {
    pub accum: u64,
    pub accum_nbits: i32,
}

impl FseOutStream {
    #[inline]
    pub fn new() -> Self {
        Self {
            accum: 0,
            accum_nbits: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, n: i32, b: u64) {
        if n > 0 {
            debug_assert!(self.accum_nbits + n <= 64);
            let mask = if n == 64 {
                !0u64
            } else {
                (1u64 << n) - 1
            };
            self.accum |= (b & mask) << self.accum_nbits;
            self.accum_nbits += n;
        }
    }

    #[inline]
    pub fn flush(&mut self, buf: &mut Vec<u8>) {
        let nbits = self.accum_nbits & !7;
        let bytes_to_write = (nbits >> 3) as usize;
        for i in 0..bytes_to_write {
            buf.push(((self.accum >> (i * 8)) & 0xFF) as u8);
        }
        self.accum >>= nbits;
        self.accum_nbits -= nbits;
    }

    #[inline]
    pub fn finish(&mut self, buf: &mut Vec<u8>) -> i32 {
        let nbits = (self.accum_nbits + 7) & !7;
        let bytes_to_write = (nbits >> 3) as usize;
        for i in 0..bytes_to_write {
            buf.push(((self.accum >> (i * 8)) & 0xFF) as u8);
        }
        self.accum = 0;
        self.accum_nbits -= nbits;
        self.accum_nbits
    }
}

// MARK: - Backward FSE State Step

#[inline]
fn fse_encode_step(
    state: &mut u16,
    encoder_table: &[FseEncoderEntry],
    out: &mut FseOutStream,
    symbol: u8,
) {
    let s = *state as i32;
    let e = &encoder_table[symbol as usize];
    let s0 = e.s0 as i32;
    let k = e.k as i32;
    let delta0 = e.delta0 as i32;
    let delta1 = e.delta1 as i32;

    let hi = s >= s0;
    let nbits = if hi { k } else { k - 1 };
    let delta = if hi { delta0 } else { delta1 };

    let mask = if nbits <= 0 {
        0
    } else if nbits >= 64 {
        !0u64
    } else {
        (1u64 << nbits) - 1
    };
    let b = (s as u64) & mask;
    out.push(nbits, b);
    *state = (delta + (s >> nbits)) as u16;
}

// MARK: - 4-Way Associative Match Finder

/// Scans input slice using 4-Way associative hash table, SWAR 64-bit comparison, and lazy evaluation.
pub fn find_matches_4way(
    src: &[u8],
    table: &mut LzfseMatchTable,
    literals: &mut Vec<u8>,
    matches: &mut Vec<LzfseRawMatch>,
) {
    if src.len() < 8 {
        literals.extend_from_slice(src);
        return;
    }

    let src_len = src.len();
    let src_encode_end = src_len.saturating_sub(8);
    let mut src_literal: usize = 0;
    let mut pending = LzfseRawMatch::default();

    let mut pos: usize = 0;
    while pos < src_encode_end {
        let x = u32::from_le_bytes(src[pos..pos + 4].try_into().unwrap());
        let h_idx = hash_4bytes(x);
        let h = table.entries[h_idx];

        let new_h = LzfseHistorySet {
            pos: [pos as i32, h.pos[0], h.pos[1], h.pos[2]],
            value: [x, h.value[0], h.value[1], h.value[2]],
        };

        if pos < src_literal {
            table.entries[h_idx] = new_h;
            pos += 1;
            continue;
        }

        let mut incoming = LzfseRawMatch {
            pos,
            ref_pos: 0,
            length: 0,
        };

        for k in 0..LZFSE_ENCODE_HASH_WIDTH {
            if (h.value[k] ^ x) != 0 {
                continue;
            }

            let ref_i = h.pos[k];
            if ref_i < 0 {
                continue;
            }
            let ref_u = ref_i as usize;
            if ref_u >= pos || (pos - ref_u) > LZFSE_ENCODE_MAX_D_VALUE {
                continue;
            }

            let max_match = src_len - pos - 8;
            let mut match_len = 4;

            while match_len + 8 <= max_match {
                let s_ref = u64::from_le_bytes(
                    src[ref_u + match_len..ref_u + match_len + 8]
                        .try_into()
                        .unwrap(),
                );
                let s_pos = u64::from_le_bytes(
                    src[pos + match_len..pos + match_len + 8]
                        .try_into()
                        .unwrap(),
                );
                let diff = s_ref ^ s_pos;
                if diff == 0 {
                    match_len += 8;
                } else {
                    match_len += (diff.trailing_zeros() >> 3) as usize;
                    break;
                }
            }

            if match_len > incoming.length {
                incoming.length = match_len;
                incoming.ref_pos = ref_u;
            }
        }

        if incoming.length == 0 {
            let n_lit = pos.saturating_sub(src_literal);
            if n_lit > 8 * LZFSE_ENCODE_MAX_L_VALUE {
                if pending.length > 0 {
                    emit_match(src, &mut src_literal, &pending, literals, matches);
                    pending = LzfseRawMatch::default();
                } else {
                    let chunk_len = LZFSE_ENCODE_MAX_L_VALUE.min(src_len - src_literal);
                    literals.extend_from_slice(&src[src_literal..src_literal + chunk_len]);
                    src_literal += chunk_len;
                }
            }
            table.entries[h_idx] = new_h;
            pos += 1;
            continue;
        }

        if incoming.length > LZFSE_ENCODE_MAX_MATCH_LENGTH {
            incoming.length = LZFSE_ENCODE_MAX_MATCH_LENGTH;
        }

        // Reverse Match Extension
        while incoming.pos > src_literal
            && incoming.ref_pos > 0
            && src[incoming.ref_pos - 1] == src[incoming.pos - 1]
        {
            incoming.pos -= 1;
            incoming.ref_pos -= 1;
            incoming.length += 1;
        }

        // Lazy Evaluation Heuristic
        if incoming.length >= LZFSE_ENCODE_GOOD_MATCH {
            if pending.length > 0 {
                emit_match(src, &mut src_literal, &pending, literals, matches);
                pending = LzfseRawMatch::default();
            }
            emit_match(src, &mut src_literal, &incoming, literals, matches);
        } else if pending.length == 0 {
            pending = incoming;
        } else if pending.pos + pending.length <= incoming.pos {
            emit_match(src, &mut src_literal, &pending, literals, matches);
            pending = incoming;
        } else {
            if incoming.length > pending.length {
                emit_match(src, &mut src_literal, &incoming, literals, matches);
            } else {
                emit_match(src, &mut src_literal, &pending, literals, matches);
            }
            pending = LzfseRawMatch::default();
        }

        table.entries[h_idx] = new_h;
        pos += 1;
    }

    if pending.length > 0 {
        emit_match(src, &mut src_literal, &pending, literals, matches);
    }

    if src_literal < src_len {
        literals.extend_from_slice(&src[src_literal..src_len]);
    }
}

#[inline]
fn emit_match(
    src: &[u8],
    src_literal: &mut usize,
    m: &LzfseRawMatch,
    literals: &mut Vec<u8>,
    matches: &mut Vec<LzfseRawMatch>,
) {
    if m.pos >= *src_literal {
        let lit_count = m.pos - *src_literal;
        if lit_count > 0 {
            literals.extend_from_slice(&src[*src_literal..m.pos]);
        }
        matches.push(*m);
        *src_literal = m.pos + m.length;
    }
}

// MARK: - LMD Splitting and Prev-Distance Delta Filter

/// Splits raw matches into standard (L, M, D) triplets conforming to LZFSE max value bounds.
pub fn split_lmd_matches(
    src_len: usize,
    raw_matches: &[LzfseRawMatch],
    out_triplets: &mut Vec<LmdTriplet>,
) {
    let mut src_cursor: usize = 0;

    for m in raw_matches {
        let mut l = (m.pos - src_cursor) as u32;
        let mut m_len = m.length as u32;
        let d = (m.pos - m.ref_pos) as u32;

        while l > LZFSE_ENCODE_MAX_L_VALUE as u32 {
            out_triplets.push(LmdTriplet {
                l: LZFSE_ENCODE_MAX_L_VALUE as u32,
                m: 0,
                d: 1,
            });
            l -= LZFSE_ENCODE_MAX_L_VALUE as u32;
        }

        while m_len > LZFSE_ENCODE_MAX_M_VALUE as u32 {
            out_triplets.push(LmdTriplet {
                l,
                m: LZFSE_ENCODE_MAX_M_VALUE as u32,
                d,
            });
            l = 0;
            m_len -= LZFSE_ENCODE_MAX_M_VALUE as u32;
        }

        if l > 0 || m_len > 0 {
            out_triplets.push(LmdTriplet { l, m: m_len, d });
        }

        src_cursor = m.pos + m.length;
    }

    if src_cursor < src_len {
        let mut l = (src_len - src_cursor) as u32;
        while l > LZFSE_ENCODE_MAX_L_VALUE as u32 {
            out_triplets.push(LmdTriplet {
                l: LZFSE_ENCODE_MAX_L_VALUE as u32,
                m: 0,
                d: 1,
            });
            l -= LZFSE_ENCODE_MAX_L_VALUE as u32;
        }
        if l > 0 {
            out_triplets.push(LmdTriplet { l, m: 0, d: 1 });
        }
    }
}

/// Applies 1-Deep distance caching to eliminate repeat match offsets (`d_prev` elimination).
#[inline]
pub fn apply_d_prev_filter(triplets: &mut [LmdTriplet]) {
    let mut d_prev: u32 = 0;
    for t in triplets.iter_mut() {
        if t.m > 0 {
            if t.d == d_prev {
                t.d = 0;
            } else {
                d_prev = t.d;
            }
        }
    }
}

// MARK: - Block & Stream Encoder

/// Encodes a single uncompressed slice into an LZFSE V2 compressed block or fallback uncompressed block.
pub fn lzfse_encode_block(
    src: &[u8],
    table: &mut LzfseMatchTable,
    dst: &mut Vec<u8>,
) -> Result<(), TTZipStatus> {
    if src.is_empty() {
        return Ok(());
    }

    let mut literals = Vec::with_capacity(src.len());
    let mut raw_matches = Vec::with_capacity(src.len() / 4);

    find_matches_4way(src, table, &mut literals, &mut raw_matches);

    let mut triplets = Vec::with_capacity(raw_matches.len() + 4);
    split_lmd_matches(src.len(), &raw_matches, &mut triplets);
    apply_d_prev_filter(&mut triplets);

    // Ensure literals count is a multiple of 4 for interleaved 4-stream FSE
    while literals.len() % 4 != 0 {
        literals.push(0);
    }

    let n_matches = triplets.len();
    let n_literals = literals.len();

    let mut l_occ = [0u32; LZFSE_ENCODE_L_SYMBOLS];
    let mut m_occ = [0u32; LZFSE_ENCODE_M_SYMBOLS];
    let mut d_occ = [0u32; LZFSE_ENCODE_D_SYMBOLS];
    let mut literal_occ = [0u32; LZFSE_ENCODE_LITERAL_SYMBOLS];

    for t in &triplets {
        l_occ[l_base_from_value(t.l as i32) as usize] += 1;
        m_occ[m_base_from_value(t.m as i32) as usize] += 1;
        d_occ[d_base_from_value(t.d as i32) as usize] += 1;
    }
    for &b in &literals {
        literal_occ[b as usize] += 1;
    }

    let mut l_freq = [0u16; LZFSE_ENCODE_L_SYMBOLS];
    let mut m_freq = [0u16; LZFSE_ENCODE_M_SYMBOLS];
    let mut d_freq = [0u16; LZFSE_ENCODE_D_SYMBOLS];
    let mut literal_freq = [0u16; LZFSE_ENCODE_LITERAL_SYMBOLS];

    fse_normalize_freq(
        LZFSE_ENCODE_L_STATES,
        LZFSE_ENCODE_L_SYMBOLS,
        &l_occ,
        &mut l_freq,
    );
    fse_normalize_freq(
        LZFSE_ENCODE_M_STATES,
        LZFSE_ENCODE_M_SYMBOLS,
        &m_occ,
        &mut m_freq,
    );
    fse_normalize_freq(
        LZFSE_ENCODE_D_STATES,
        LZFSE_ENCODE_D_SYMBOLS,
        &d_occ,
        &mut d_freq,
    );
    fse_normalize_freq(
        LZFSE_ENCODE_LITERAL_STATES,
        LZFSE_ENCODE_LITERAL_SYMBOLS,
        &literal_occ,
        &mut literal_freq,
    );

    let mut freq_bytes = Vec::new();
    lzfse_encode_v1_freq_table(
        &l_freq,
        &m_freq,
        &d_freq,
        &literal_freq,
        &mut freq_bytes,
    );

    let mut l_encoder = [FseEncoderEntry::default(); LZFSE_ENCODE_L_SYMBOLS];
    let mut m_encoder = [FseEncoderEntry::default(); LZFSE_ENCODE_M_SYMBOLS];
    let mut d_encoder = [FseEncoderEntry::default(); LZFSE_ENCODE_D_SYMBOLS];
    let mut lit_encoder = [FseEncoderEntry::default(); LZFSE_ENCODE_LITERAL_SYMBOLS];

    fse_init_encoder_table(
        LZFSE_ENCODE_L_STATES,
        LZFSE_ENCODE_L_SYMBOLS,
        &l_freq,
        &mut l_encoder,
    )?;
    fse_init_encoder_table(
        LZFSE_ENCODE_M_STATES,
        LZFSE_ENCODE_M_SYMBOLS,
        &m_freq,
        &mut m_encoder,
    )?;
    fse_init_encoder_table(
        LZFSE_ENCODE_D_STATES,
        LZFSE_ENCODE_D_SYMBOLS,
        &d_freq,
        &mut d_encoder,
    )?;
    fse_init_encoder_table(
        LZFSE_ENCODE_LITERAL_STATES,
        LZFSE_ENCODE_LITERAL_SYMBOLS,
        &literal_freq,
        &mut lit_encoder,
    )?;

    // 1. Encode 4 interleaved literal streams backwards
    let mut lit_out = FseOutStream::new();
    let mut lit_payload = Vec::new();
    let mut s0: u16 = 0;
    let mut s1: u16 = 0;
    let mut s2: u16 = 0;
    let mut s3: u16 = 0;

    let mut i = n_literals;
    while i > 0 {
        i -= 4;
        fse_encode_step(&mut s3, &lit_encoder, &mut lit_out, literals[i + 3]);
        fse_encode_step(&mut s2, &lit_encoder, &mut lit_out, literals[i + 2]);
        fse_encode_step(&mut s1, &lit_encoder, &mut lit_out, literals[i + 1]);
        fse_encode_step(&mut s0, &lit_encoder, &mut lit_out, literals[i]);
        lit_out.flush(&mut lit_payload);
    }
    let literal_bits = lit_out.finish(&mut lit_payload);

    // 2. Encode LMD stream backwards
    let mut lmd_out = FseOutStream::new();
    let mut lmd_payload = vec![0u8; 8]; // 8-byte prefix padding
    let mut l_state: u16 = 0;
    let mut m_state: u16 = 0;
    let mut d_state: u16 = 0;

    let mut j = n_matches;
    while j > 0 {
        j -= 1;
        let t = &triplets[j];

        // Distance D
        let d_val = t.d as i32;
        let d_sym = d_base_from_value(d_val);
        let d_nbits = D_EXTRA_BITS[d_sym as usize] as i32;
        let d_bits = (d_val - D_BASE_VALUE[d_sym as usize]) as u64;
        lmd_out.push(d_nbits, d_bits);
        fse_encode_step(&mut d_state, &d_encoder, &mut lmd_out, d_sym);
        lmd_out.flush(&mut lmd_payload);

        // Match length M
        let m_val = t.m as i32;
        let m_sym = m_base_from_value(m_val);
        let m_nbits = M_EXTRA_BITS[m_sym as usize] as i32;
        let m_bits = (m_val - M_BASE_VALUE[m_sym as usize]) as u64;
        lmd_out.push(m_nbits, m_bits);
        fse_encode_step(&mut m_state, &m_encoder, &mut lmd_out, m_sym);
        lmd_out.flush(&mut lmd_payload);

        // Literal length L
        let l_val = t.l as i32;
        let l_sym = l_base_from_value(l_val);
        let l_nbits = L_EXTRA_BITS[l_sym as usize] as i32;
        let l_bits = (l_val - L_BASE_VALUE[l_sym as usize]) as u64;
        lmd_out.push(l_nbits, l_bits);
        fse_encode_step(&mut l_state, &l_encoder, &mut lmd_out, l_sym);
        lmd_out.flush(&mut lmd_payload);
    }
    let lmd_bits = lmd_out.finish(&mut lmd_payload);

    let header_size = (32 + freq_bytes.len()) as u32;
    let total_block_len = (header_size as usize) + lit_payload.len() + lmd_payload.len();

    // If compressed block is larger than uncompressed source, emit uncompressed block
    if total_block_len >= src.len() + 12 {
        dst.extend_from_slice(&LZFSE_UNCOMPRESSED_BLOCK_MAGIC.to_le_bytes());
        dst.extend_from_slice(&(src.len() as u32).to_le_bytes());
        dst.extend_from_slice(src);
        return Ok(());
    }

    // Pack LZFSE V2 Header
    let packed0: u64 = ((n_literals as u64) & 0xF_FFFF)
        | (((lit_payload.len() as u64) & 0xF_FFFF) << 20)
        | (((n_matches as u64) & 0xF_FFFF) << 40)
        | ((((literal_bits + 7) as u64) & 0x7) << 60);

    let packed1: u64 = ((s0 as u64) & 0x3FF)
        | (((s1 as u64) & 0x3FF) << 10)
        | (((s2 as u64) & 0x3FF) << 20)
        | (((s3 as u64) & 0x3FF) << 30)
        | (((lmd_payload.len() as u64) & 0xF_FFFF) << 40)
        | ((((lmd_bits + 7) as u64) & 0x7) << 60);

    let packed2: u64 = (header_size as u64)
        | (((l_state as u64) & 0x3FF) << 32)
        | (((m_state as u64) & 0x3FF) << 42)
        | (((d_state as u64) & 0x3FF) << 52);

    dst.extend_from_slice(&LZFSE_COMPRESSEDV2_BLOCK_MAGIC.to_le_bytes());
    dst.extend_from_slice(&(src.len() as u32).to_le_bytes());
    dst.extend_from_slice(&packed0.to_le_bytes());
    dst.extend_from_slice(&packed1.to_le_bytes());
    dst.extend_from_slice(&packed2.to_le_bytes());
    dst.extend_from_slice(&freq_bytes);
    dst.extend_from_slice(&lit_payload);
    dst.extend_from_slice(&lmd_payload);

    Ok(())
}

/// Compresses a full buffer with Apple LZFSE format, chunking large buffers in 256KB blocks and emitting EOS.
pub fn lzfse_compress_pure_rust(src: &[u8], dst: &mut Vec<u8>) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }

    let initial_len = dst.len();
    let mut table = LzfseMatchTable::new();

    const CHUNK_SIZE: usize = 256 * 1024; // 256KB blocks
    let mut offset = 0;

    while offset < src.len() {
        let chunk_end = (offset + CHUNK_SIZE).min(src.len());
        table.reset();
        lzfse_encode_block(&src[offset..chunk_end], &mut table, dst)?;
        offset = chunk_end;
    }

    // Append end-of-stream block
    dst.extend_from_slice(&LZFSE_ENDOFSTREAM_BLOCK_MAGIC.to_le_bytes());

    Ok(dst.len() - initial_len)
}
