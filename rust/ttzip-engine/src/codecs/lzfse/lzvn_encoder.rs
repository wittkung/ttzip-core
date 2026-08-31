// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Rust Apple LZVN ultra-high-speed 14-bit 4-Way associative hash matching encoder.
//!
//! Provides zero-allocation stream encoding, Knuth 3-byte multiplicative hashing,
//! 8-category variable-length opcode generation, thread-local encoder pooling,
//! and 100% Apple LZVN bit-exact roundtrip compatibility.

use crate::types::TTZipStatus;
use std::cell::RefCell;

// MARK: - Constants

/// Number of bits returned by Knuth 3-byte multiplicative hash function (16,384 table entries).
pub const LZVN_ENCODE_HASH_BITS: usize = 14;

/// Number of stored candidate offsets per hash bucket in 4-Way associative cache.
pub const LZVN_ENCODE_OFFSETS_PER_HASH: usize = 4;

/// Total number of entries in the LZVN hash table (1 << 14 = 16,384).
pub const LZVN_ENCODE_HASH_VALUES: usize = 1 << LZVN_ENCODE_HASH_BITS;

/// Maximum match distance representable in LZVN encoding (65,535 bytes).
pub const LZVN_ENCODE_MAX_DISTANCE: usize = 0xFFFF;

/// Minimum safety margin from buffer end during match search (8 bytes).
pub const LZVN_ENCODE_MIN_MARGIN: usize = 8;

/// Maximum un-emitted literal backlog before forcing literal dispatch (400 bytes).
pub const LZVN_ENCODE_MAX_LITERAL_BACKLOG: usize = 400;

/// Minimum source buffer length required for LZVN dictionary compression (8 bytes).
pub const LZVN_ENCODE_MIN_SRC_SIZE: usize = 8;

/// Minimum destination buffer length for LZVN encoding (8 bytes for EOS).
pub const LZVN_ENCODE_MIN_DST_SIZE: usize = 8;

/// Maximum source buffer length for single-shot compression (4GB).
pub const LZVN_ENCODE_MAX_SRC_SIZE: usize = 0xFFFF_FFFF;

// MARK: - Table Entry & Match Info Types

/// 32-byte 4-Way associative hash table entry containing 4 signed source indices and 4 cached 32-bit values.
#[repr(C, align(32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LzvnEncodeEntry {
    pub indices: [i32; LZVN_ENCODE_OFFSETS_PER_HASH],
    pub values: [u32; LZVN_ENCODE_OFFSETS_PER_HASH],
}

impl Default for LzvnEncodeEntry {
    #[inline]
    fn default() -> Self {
        Self {
            indices: [-(LZVN_ENCODE_MAX_DISTANCE as i32); LZVN_ENCODE_OFFSETS_PER_HASH],
            values: [0; LZVN_ENCODE_OFFSETS_PER_HASH],
        }
    }
}

/// Match candidate metadata used during LZVN lazy evaluation and greedy parsing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LzvnMatchInfo {
    pub m_begin: usize,
    pub m_end: usize,
    pub m: usize,
    pub d: usize,
    pub k: isize,
}

// MARK: - Low-Level Fast I/O & Bit Operations

#[inline(always)]
fn load4(src: &[u8], offset: usize) -> u32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&src[offset..offset + 4]);
    u32::from_le_bytes(buf)
}

#[inline(always)]
fn store2(dst: &mut [u8], offset: usize, val: u16) {
    dst[offset..offset + 2].copy_from_slice(&val.to_le_bytes());
}

#[inline(always)]
fn store4(dst: &mut [u8], offset: usize, val: u32) {
    dst[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
}

#[inline(always)]
fn store8(dst: &mut [u8], offset: usize, val: u64) {
    dst[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
}

/// Knuth 3-byte multiplicative hash function mapping 24-bit prefix to 14-bit index.
#[inline(always)]
pub fn hash3i(i: u32) -> usize {
    let truncated = i & 0x00FF_FFFF;
    let h = (truncated.wrapping_mul(1 + (1 << 6) + (1 << 12))) >> 12;
    (h & ((LZVN_ENCODE_HASH_VALUES - 1) as u32)) as usize
}

/// Returns number of trailing matching bytes (0..4) between two 32-bit words via XOR and CTZ.
#[inline(always)]
pub fn trailing_zero_bytes(x: u32) -> usize {
    if x == 0 {
        4
    } else {
        (x.trailing_zeros() >> 3) as usize
    }
}

// MARK: - Match Finding & Forward/Backward Expansion

#[inline]
fn find_match(
    src: &[u8],
    l_begin: usize,
    m0_pos: isize,
    m_begin: usize,
    match_info: &mut LzvnMatchInfo,
) -> bool {
    if m0_pos < 0 {
        return false;
    }
    let m0_u = m0_pos as usize;
    if m_begin + 4 > src.len() || m0_u + 4 > src.len() {
        return false;
    }
    let vi = load4(src, m_begin);
    let vj = load4(src, m0_u);
    let n = trailing_zero_bytes(vi ^ vj);
    find_match_n(src, l_begin, m0_u, m_begin, n, match_info)
}

#[inline]
fn find_match_n(
    src: &[u8],
    l_begin: usize,
    m0_pos: usize,
    mut m_begin: usize,
    mut n: usize,
    match_info: &mut LzvnMatchInfo,
) -> bool {
    if n < 3 || m_begin <= m0_pos {
        return false;
    }
    let d = m_begin - m0_pos;
    if d == 0 || d > LZVN_ENCODE_MAX_DISTANCE {
        return false;
    }

    // Forward expansion
    let mut m_end = m_begin + n;
    while n == 4 && m_end + 4 <= src.len() {
        let vi = load4(src, m_end);
        let vj = load4(src, m_end - d);
        n = trailing_zero_bytes(vi ^ vj);
        m_end += n;
    }
    if n == 4 {
        while m_end < src.len() && src[m_end] == src[m_end - d] {
            m_end += 1;
        }
    }

    // Backward expansion over un-emitted literal window
    let mut m0_back = m0_pos;
    while m0_back > 0 && m_begin > l_begin && src[m_begin - 1] == src[m0_back - 1] {
        m0_back -= 1;
        m_begin -= 1;
    }

    let m_len = m_end - m_begin;
    let k = (m_len as isize) - if d < 0x600 { 2 } else { 3 };
    match_info.m_begin = m_begin;
    match_info.m_end = m_end;
    match_info.m = m_len;
    match_info.d = d;
    match_info.k = k;
    true
}

#[inline(always)]
fn update_best(best: &mut LzvnMatchInfo, candidate: &LzvnMatchInfo) {
    if candidate.k > best.k || (candidate.k == best.k && candidate.m_end > best.m_end + 1) {
        *best = *candidate;
    }
}

// MARK: - Opcode Emitters

#[inline]
fn emit_literal(
    src: &[u8],
    src_pos: usize,
    dst: &mut [u8],
    dst_pos: &mut usize,
    dst_limit: usize,
    mut l: usize,
) -> bool {
    let mut p = src_pos;
    while l > 15 {
        let x = l.min(271);
        if *dst_pos + x + 2 > dst_limit {
            return false;
        }
        store2(dst, *dst_pos, 0xE0 + (((x - 16) as u16) << 8));
        *dst_pos += 2;
        l -= x;
        dst[*dst_pos..*dst_pos + x].copy_from_slice(&src[p..p + x]);
        *dst_pos += x;
        p += x;
    }
    if l > 0 {
        if *dst_pos + l + 1 > dst_limit {
            return false;
        }
        dst[*dst_pos] = 0xE0 + (l as u8);
        *dst_pos += 1;
        dst[*dst_pos..*dst_pos + l].copy_from_slice(&src[p..p + l]);
        *dst_pos += l;
    }
    true
}

#[derive(Debug, Clone, Copy)]
struct LzvnMatchEmitArgs {
    src_pos: usize,
    dst_limit: usize,
    l: usize,
    m: usize,
    d: usize,
    d_prev: usize,
}

#[inline]
fn emit_match_instruction(
    src: &[u8],
    dst: &mut [u8],
    dst_pos: &mut usize,
    args: LzvnMatchEmitArgs,
) -> bool {
    let mut p = args.src_pos;
    let mut l = args.l;
    let mut m = args.m;
    let dst_limit = args.dst_limit;
    let d = args.d;
    let d_prev = args.d_prev;
    while l > 15 {
        let x = l.min(271);
        if *dst_pos + x + 2 > dst_limit {
            return false;
        }
        store2(dst, *dst_pos, 0xE0 + (((x - 16) as u16) << 8));
        *dst_pos += 2;
        l -= x;
        dst[*dst_pos..*dst_pos + x].copy_from_slice(&src[p..p + x]);
        *dst_pos += x;
        p += x;
    }
    if l > 3 {
        if *dst_pos + l + 1 > dst_limit {
            return false;
        }
        dst[*dst_pos] = 0xE0 + (l as u8);
        *dst_pos += 1;
        dst[*dst_pos..*dst_pos + l].copy_from_slice(&src[p..p + l]);
        *dst_pos += l;
        p += l;
        l = 0;
    }


    let mut x = m.min(10 - 2 * l);
    m -= x;
    x -= 3;

    let literal_data = if p + 4 <= src.len() {
        load4(src, p)
    } else {
        let mut b = [0u8; 4];
        let rem = src.len().saturating_sub(p);
        b[..rem].copy_from_slice(&src[p..p + rem]);
        u32::from_le_bytes(b)
    };

    if *dst_pos + 8 >= dst_limit {
        return false;
    }

    if d == d_prev {
        if l == 0 {
            dst[*dst_pos] = 0xF0 + ((x + 3) as u8);
            *dst_pos += 1;
        } else {
            dst[*dst_pos] = ((l << 6) + (x << 3) + 6) as u8;
            *dst_pos += 1;
        }
        if *dst_pos + 4 <= dst.len() {
            store4(dst, *dst_pos, literal_data);
        } else {
            let bytes = literal_data.to_le_bytes();
            dst[*dst_pos..*dst_pos + l].copy_from_slice(&bytes[..l]);
        }
        *dst_pos += l;
    } else if d < 1536 {
        // Small distance (d < 2048 - 512 = 1536)
        dst[*dst_pos] = ((d >> 8) + (l << 6) + (x << 3)) as u8;
        dst[*dst_pos + 1] = (d & 0xFF) as u8;
        *dst_pos += 2;
        if *dst_pos + 4 <= dst.len() {
            store4(dst, *dst_pos, literal_data);
        } else {
            let bytes = literal_data.to_le_bytes();
            dst[*dst_pos..*dst_pos + l].copy_from_slice(&bytes[..l]);
        }
        *dst_pos += l;
    } else if d >= (1 << 14) || m == 0 || (x + 3) + m > 34 {
        // Large distance
        dst[*dst_pos] = ((l << 6) + (x << 3) + 7) as u8;
        store2(dst, *dst_pos + 1, d as u16);
        *dst_pos += 3;
        if *dst_pos + 4 <= dst.len() {
            store4(dst, *dst_pos, literal_data);
        } else {
            let bytes = literal_data.to_le_bytes();
            dst[*dst_pos..*dst_pos + l].copy_from_slice(&bytes[..l]);
        }
        *dst_pos += l;
    } else {
        // Medium distance
        x += m;
        m = 0;
        dst[*dst_pos] = (0xA0 + (x >> 2) + (l << 3)) as u8;
        store2(dst, *dst_pos + 1, ((d << 2) | (x & 3)) as u16);
        *dst_pos += 3;
        if *dst_pos + 4 <= dst.len() {
            store4(dst, *dst_pos, literal_data);
        } else {
            let bytes = literal_data.to_le_bytes();
            dst[*dst_pos..*dst_pos + l].copy_from_slice(&bytes[..l]);
        }
        *dst_pos += l;
    }

    // Issue remaining match
    while m > 15 {
        if *dst_pos + 2 >= dst_limit {
            return false;
        }
        let chunk = m.min(271);
        store2(dst, *dst_pos, 0xF0 + (((chunk - 16) as u16) << 8));
        *dst_pos += 2;
        m -= chunk;
    }
    if m > 0 {
        if *dst_pos + 1 >= dst_limit {
            return false;
        }
        dst[*dst_pos] = 0xF0 + (m as u8);
        *dst_pos += 1;
    }

    true
}

// MARK: - LZVN Encoder Engine

/// Pure Rust LZVN fast encoder holding 14-bit associative match table and lazy state.
pub struct LzvnEncoder {
    pub table: Box<[LzvnEncodeEntry; LZVN_ENCODE_HASH_VALUES]>,
    pub pending: LzvnMatchInfo,
    pub d_prev: usize,
    pub src_literal: usize,
    pub dst_pos: usize,
}

impl Default for LzvnEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl LzvnEncoder {
    /// Allocates a new zero-initialized LZVN encoder with 512KB hash table.
    pub fn new() -> Self {
        let entry = LzvnEncodeEntry::default();
        let vec = vec![entry; LZVN_ENCODE_HASH_VALUES];
        let boxed_slice = vec.into_boxed_slice();
        let raw = Box::into_raw(boxed_slice) as *mut [LzvnEncodeEntry; LZVN_ENCODE_HASH_VALUES];
        Self {
            table: unsafe { Box::from_raw(raw) },
            pending: LzvnMatchInfo::default(),
            d_prev: 0,
            src_literal: 0,
            dst_pos: 0,
        }
    }

    /// Resets all internal encoder states and hash table entries for subsequent buffer re-use.
    pub fn reset(&mut self) {
        self.table.fill(LzvnEncodeEntry::default());
        self.pending = LzvnMatchInfo::default();
        self.d_prev = 0;
        self.src_literal = 0;
        self.dst_pos = 0;
    }

    #[inline]
    fn emit_match_helper(&mut self, src: &[u8], dst: &mut [u8], match_info: LzvnMatchInfo) -> bool {
        let l = match_info.m_begin - self.src_literal;
        let m = match_info.m;
        let d = match_info.d;
        let d_prev = self.d_prev;
        let dst_limit = dst.len().saturating_sub(8);

        if !emit_match_instruction(
            src,
            dst,
            &mut self.dst_pos,
            LzvnMatchEmitArgs {
                src_pos: self.src_literal,
                dst_limit,
                l,
                m,
                d,
                d_prev,
            },
        ) {
            return false;
        }

        self.d_prev = match_info.d;
        self.src_literal = match_info.m_end;
        true
    }

    #[inline]
    fn emit_literal_helper(&mut self, src: &[u8], dst: &mut [u8], n: usize) -> bool {
        let dst_limit = dst.len().saturating_sub(8);
        if !emit_literal(
            src,
            self.src_literal,
            dst,
            &mut self.dst_pos,
            dst_limit,
            n,
        ) {
            return false;
        }
        self.src_literal += n;
        true
    }

    /// Compresses `src` buffer into `dst` slice using LZVN dictionary and literal encoding.
    pub fn encode(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        if dst.len() < LZVN_ENCODE_MIN_DST_SIZE {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        self.reset();

        let src_len = src.len().min(LZVN_ENCODE_MAX_SRC_SIZE);
        if src_len >= LZVN_ENCODE_MIN_SRC_SIZE {
            let src_current_end = src_len - LZVN_ENCODE_MIN_MARGIN;
            let mut src_current = 0;

            while src_current < src_current_end {
                let vi = load4(src, src_current);
                let h = hash3i(vi);
                let e = self.table[h];

                let updated_e = LzvnEncodeEntry {
                    indices: [
                        src_current as i32,
                        e.indices[0],
                        e.indices[1],
                        e.indices[2],
                    ],
                    values: [
                        vi,
                        e.values[0],
                        e.values[1],
                        e.values[2],
                    ],
                };

                if src_current < self.src_literal {
                    self.table[h] = updated_e;
                    src_current += 1;
                    continue;
                }

                let mut incoming = LzvnMatchInfo::default();

                for k in 0..LZVN_ENCODE_OFFSETS_PER_HASH {
                    let ik = e.indices[k] as isize;
                    let diff = e.values[k] ^ vi;
                    let nk = trailing_zero_bytes(diff);
                    let mut m1 = LzvnMatchInfo::default();
                    if ik >= 0 && find_match_n(src, self.src_literal, ik as usize, src_current, nk, &mut m1) {
                        update_best(&mut incoming, &m1);
                    }
                }

                if self.d_prev != 0 && src_current >= self.d_prev {
                    let m0_prev = (src_current - self.d_prev) as isize;
                    let mut m1 = LzvnMatchInfo::default();
                    if find_match(src, self.src_literal, m0_prev, src_current, &mut m1) {
                        m1.k = (m1.m as isize) - 1;
                        update_best(&mut incoming, &m1);
                    }
                }

                if incoming.m == 0 {
                    if src_current - self.src_literal >= LZVN_ENCODE_MAX_LITERAL_BACKLOG {
                        if self.pending.m != 0 {
                            if !self.emit_match_helper(src, dst, self.pending) {
                                return Err(TTZipStatus::ErrCompressionFailed);
                            }
                            self.pending = LzvnMatchInfo::default();
                        } else if !self.emit_literal_helper(src, dst, 271) {
                            return Err(TTZipStatus::ErrCompressionFailed);
                        }
                    }
                    self.table[h] = updated_e;
                    src_current += 1;
                    continue;
                }

                if self.pending.m == 0 {
                    self.pending = incoming;
                } else if self.pending.m_end <= incoming.m_begin {
                    if !self.emit_match_helper(src, dst, self.pending) {
                        return Err(TTZipStatus::ErrCompressionFailed);
                    }
                    self.pending = incoming;
                } else {
                    if incoming.k > self.pending.k {
                        self.pending = incoming;
                    }
                    if !self.emit_match_helper(src, dst, self.pending) {
                        return Err(TTZipStatus::ErrCompressionFailed);
                    }
                    self.pending = LzvnMatchInfo::default();
                }

                self.table[h] = updated_e;
                src_current += 1;
            }
        }

        // Emit final trailing literals
        let remaining_literals = src_len.saturating_sub(self.src_literal);
        if remaining_literals > 0 {
            let dst_limit = dst.len().saturating_sub(8);
            if !emit_literal(
                src,
                self.src_literal,
                dst,
                &mut self.dst_pos,
                dst_limit,
                remaining_literals,
            ) {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
            self.src_literal += remaining_literals;
        }

        // Emit 8-byte End-Of-Stream (EOS) command marker
        if dst.len() < self.dst_pos + 8 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        store8(dst, self.dst_pos, 0x06);
        self.dst_pos += 8;

        if self.src_literal != src_len {
            Err(TTZipStatus::ErrCompressionFailed)
        } else {
            Ok(self.dst_pos)
        }
    }
}

// MARK: - Thread-Local Scratch Pool & Public Safe Facades

thread_local! {
    static LZVN_ENCODER_TLS: RefCell<LzvnEncoder> = RefCell::new(LzvnEncoder::new());
}

/// Computes worst-case output buffer size for LZVN block compression.
#[inline]
pub fn lzvn_compress_bound(src_size: usize) -> usize {
    src_size.saturating_add(src_size / 8 + 1024)
}

/// Compresses a buffer with Apple LZVN using thread-local pooled 512KB match table.
pub fn lzvn_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }
    if dst.len() < LZVN_ENCODE_MIN_DST_SIZE {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    LZVN_ENCODER_TLS.with(|cell| {
        let mut encoder = cell.borrow_mut();
        encoder.encode(src, dst)
    })
}

/// Compresses a buffer with pure Rust LZVN encoder using freshly allocated match state.
pub fn lzvn_compress_pure_rust(src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
    let mut encoder = LzvnEncoder::new();
    encoder.encode(src, dst)
}

/// Compresses a raw memory buffer into a newly allocated `Vec<u8>` with LZVN compression.
pub fn lzvn_compress_raw(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }
    let bound = lzvn_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lzvn_compress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

/// Compatibility alias for `lzvn_compress_raw`.
#[inline]
pub fn lzvn_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    lzvn_compress_raw(src)
}

/// Compresses a buffer to newly allocated `Vec<u8>` with pure Rust LZVN encoder.
pub fn lzvn_compress_pure_rust_to_vec(src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
    let bound = lzvn_compress_bound(src.len());
    let mut out = vec![0u8; bound];
    let written = lzvn_compress_pure_rust(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}
