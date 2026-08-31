// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Ultra-fast 1-pass length-limited canonical Huffman tree builder, Precode RLE encoder,
//! and 64-bit branchless bitstream emitter optimized for high-throughput DEFLATE compression.

use std::cmp::min;

/// Number of symbols in the precode alphabet (0..18).
pub const DEFLATE_NUM_PRECODE_SYMS: usize = 19;

/// Maximum number of literal/length symbols in DEFLATE.
pub const DEFLATE_NUM_LITLEN_SYMS: usize = 288;

/// Maximum number of offset symbols in DEFLATE.
pub const DEFLATE_NUM_OFFSET_SYMS: usize = 32;

/// Maximum number of symbols across all DEFLATE Huffman alphabets.
pub const DEFLATE_MAX_NUM_SYMS: usize = 288;

/// Number of literal symbols (0..255).
pub const DEFLATE_NUM_LITERALS: usize = 256;

/// Symbol index representing end-of-block (256).
pub const DEFLATE_END_OF_BLOCK: usize = 256;

/// First length symbol index (257).
pub const DEFLATE_FIRST_LEN_SYM: usize = 257;

/// Maximum codeword length for literal/length alphabet when 14-bit optimization is enabled.
pub const MAX_LITLEN_CODEWORD_LEN: usize = 14;

/// RFC 1951 maximum codeword length for literal/length alphabet.
pub const DEFLATE_MAX_LITLEN_CODEWORD_LEN: usize = 15;

/// Maximum codeword length for offset alphabet.
pub const MAX_OFFSET_CODEWORD_LEN: usize = 15;

/// RFC 1951 alias for maximum offset codeword length.
pub const DEFLATE_MAX_OFFSET_CODEWORD_LEN: usize = MAX_OFFSET_CODEWORD_LEN;

/// Maximum codeword length for precode alphabet.
pub const MAX_PRE_CODEWORD_LEN: usize = 7;

/// RFC 1951 alias for maximum precode codeword length.
pub const DEFLATE_MAX_PRE_CODEWORD_LEN: usize = MAX_PRE_CODEWORD_LEN;

/// Maximum codeword length across all DEFLATE codes.
pub const DEFLATE_MAX_CODEWORD_LEN: usize = 15;

/// Number of bits used to pack symbol index in the combined sort slot.
pub const NUM_SYMBOL_BITS: usize = 10;

/// Mask for extracting symbol index from packed slot.
pub const SYMBOL_MASK: u32 = (1 << NUM_SYMBOL_BITS) - 1;

/// Mask for extracting frequency from packed slot.
pub const FREQ_MASK: u32 = !SYMBOL_MASK;

/// Canonical permutation order for writing precode codeword lengths.
pub const DEFLATE_PRECODE_LENS_PERMUTATION: [u8; DEFLATE_NUM_PRECODE_SYMS] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Extra bits table for precode symbols 0..18.
pub const DEFLATE_EXTRA_PRECODE_BITS: [u8; DEFLATE_NUM_PRECODE_SYMS] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7,
];

/// Extra bits table for match lengths (slots 257..285).
pub const DEFLATE_EXTRA_LENGTH_BITS: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Extra bits table for match offsets (slots 0..29).
pub const DEFLATE_EXTRA_OFFSET_BITS: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

// MARK: - Huffman Code Generation

/// Reverses the bits of a canonical codeword of specified length.
#[inline(always)]
pub fn reverse_codeword(codeword: u32, len: u8) -> u32 {
    if len == 0 {
        0
    } else {
        codeword.reverse_bits() >> (32 - len)
    }
}

/// Sifts down a subtree element in 1-based index max-heap.
#[inline]
fn heapify_subtree(a: &mut [u32], length: usize, subtree_idx: usize) {
    let mut parent_idx = subtree_idx;
    let v = a[parent_idx - 1];
    while parent_idx * 2 <= length {
        let mut child_idx = parent_idx * 2;
        if child_idx < length && a[child_idx] > a[child_idx - 1] {
            child_idx += 1;
        }
        if v >= a[child_idx - 1] {
            break;
        }
        a[parent_idx - 1] = a[child_idx - 1];
        parent_idx = child_idx;
    }
    a[parent_idx - 1] = v;
}

/// In-place heapsort for packed symbol-frequency slots.
fn heap_sort_slots(a: &mut [u32]) {
    let length = a.len();
    if length < 2 {
        return;
    }
    for subtree_idx in (1..=length / 2).rev() {
        heapify_subtree(a, length, subtree_idx);
    }
    let mut cur_len = length;
    while cur_len >= 2 {
        a.swap(0, cur_len - 1);
        cur_len -= 1;
        heapify_subtree(a, cur_len, 1);
    }
}

/// Sorts symbols primarily by frequency and secondarily by symbol index.
///
/// Symbols with zero frequency are assigned length 0, and non-zero frequency symbols
/// are packed as `(freq << 10) | symbol` into `symout`.
fn sort_symbols(num_syms: usize, freqs: &[u32], lens: &mut [u8], symout: &mut [u32]) -> usize {
    let num_counters = num_syms.min(DEFLATE_MAX_NUM_SYMS);
    let mut counters = [0usize; DEFLATE_MAX_NUM_SYMS];

    for sym in 0..num_syms {
        let bucket = min(freqs[sym] as usize, num_counters - 1);
        counters[bucket] += 1;
    }

    let mut num_used_syms = 0;
    for i in 1..num_counters {
        let count = counters[i];
        counters[i] = num_used_syms;
        num_used_syms += count;
    }

    for sym in 0..num_syms {
        let freq = freqs[sym];
        if freq != 0 {
            let bucket = min(freq as usize, num_counters - 1);
            let pos = counters[bucket];
            counters[bucket] += 1;
            symout[pos] = ((freq & 0x3F_FFFF) << NUM_SYMBOL_BITS) | (sym as u32 & SYMBOL_MASK);
        } else {
            lens[sym] = 0;
        }
    }

    if num_counters > 1 {
        let high_start = counters[num_counters - 2];
        let high_end = counters[num_counters - 1];
        if high_end > high_start {
            heap_sort_slots(&mut symout[high_start..high_end]);
        }
    }

    num_used_syms
}

/// Builds non-leaf nodes of a Huffman tree in-place using two-queue single-pass merging.
fn build_tree(a: &mut [u32], sym_count: usize) {
    if sym_count < 2 {
        return;
    }
    let last_idx = sym_count - 1;
    let mut i = 0;
    let mut b = 0;
    let mut e = 0;

    while e < last_idx {
        let new_freq: u32;

        if i < last_idx && (b == e || (a[i + 1] & FREQ_MASK) <= (a[b] & FREQ_MASK)) {
            new_freq = (a[i] & FREQ_MASK).wrapping_add(a[i + 1] & FREQ_MASK) & FREQ_MASK;
            i += 2;
        } else if b + 2 <= e && (i > last_idx || (a[b + 1] & FREQ_MASK) < (a[i] & FREQ_MASK)) {
            new_freq = (a[b] & FREQ_MASK).wrapping_add(a[b + 1] & FREQ_MASK) & FREQ_MASK;
            a[b] = ((e as u32) << NUM_SYMBOL_BITS) | (a[b] & SYMBOL_MASK);
            a[b + 1] = ((e as u32) << NUM_SYMBOL_BITS) | (a[b + 1] & SYMBOL_MASK);
            b += 2;
        } else {
            new_freq = (a[i] & FREQ_MASK).wrapping_add(a[b] & FREQ_MASK) & FREQ_MASK;
            a[b] = ((e as u32) << NUM_SYMBOL_BITS) | (a[b] & SYMBOL_MASK);
            i += 1;
            b += 1;
        }
        a[e] = new_freq | (a[e] & SYMBOL_MASK);
        e += 1;
    }
}

/// Computes length counts using top-down topological reverse bit-width folding with Kraft conservation.
fn compute_length_counts(
    a: &mut [u32],
    root_idx: usize,
    len_counts: &mut [u32],
    max_codeword_len: usize,
) {
    len_counts[..=max_codeword_len].fill(0);
    len_counts[1] = 2;

    a[root_idx] &= SYMBOL_MASK;

    if root_idx == 0 {
        return;
    }

    for node in (0..root_idx).rev() {
        let parent = (a[node] >> NUM_SYMBOL_BITS) as usize;
        let parent_depth = (a[parent] >> NUM_SYMBOL_BITS) as usize;
        let mut depth = parent_depth + 1;

        a[node] = (a[node] & SYMBOL_MASK) | ((depth as u32) << NUM_SYMBOL_BITS);

        if depth >= max_codeword_len {
            depth = max_codeword_len;
            loop {
                depth -= 1;
                if len_counts[depth] > 0 {
                    break;
                }
            }
        }

        len_counts[depth] -= 1;
        len_counts[depth + 1] += 2;
    }
}

/// Assigns canonical Huffman codewords and reverses their bit representations.
fn gen_codewords(
    a: &[u32],
    lens: &mut [u8],
    len_counts: &[u32],
    max_codeword_len: usize,
    num_syms: usize,
    codewords: &mut [u32],
) {
    let mut next_codewords = [0u32; DEFLATE_MAX_CODEWORD_LEN + 1];
    let mut i = 0;

    for len in (1..=max_codeword_len).rev() {
        let mut count = len_counts[len];
        while count > 0 {
            let sym = (a[i] & SYMBOL_MASK) as usize;
            lens[sym] = len as u8;
            i += 1;
            count -= 1;
        }
    }

    next_codewords[0] = 0;
    next_codewords[1] = 0;
    for len in 2..=max_codeword_len {
        next_codewords[len] = (next_codewords[len - 1] + len_counts[len - 1]) << 1;
    }

    for sym in 0..num_syms {
        let l = lens[sym];
        if l > 0 {
            let code = next_codewords[l as usize];
            next_codewords[l as usize] += 1;
            codewords[sym] = reverse_codeword(code, l);
        } else {
            codewords[sym] = 0;
        }
    }
}

/// Given an alphabet and the frequency of each symbol, constructs a length-limited canonical Huffman code.
///
/// Ensures compliance with DEFLATE RFC 1951 and libdeflate invariants:
/// - Frequencies and symbols are packed in 32-bit words for cache locality.
/// - When `< 2` symbols are used, assigns 2 codewords of length 1 (codeword `0` for sym 0, `1` for the other).
/// - Maximum codeword length is bounded by `max_codeword_len`.
pub fn deflate_make_huffman_code(
    num_syms: usize,
    max_codeword_len: usize,
    freqs: &[u32],
    lens: &mut [u8],
    codewords: &mut [u32],
) {
    assert!((2..=(1 << NUM_SYMBOL_BITS)).contains(&num_syms));
    assert!((1..=DEFLATE_MAX_CODEWORD_LEN).contains(&max_codeword_len));
    assert!(freqs.len() >= num_syms);
    assert!(lens.len() >= num_syms);
    assert!(codewords.len() >= num_syms);

    lens[..num_syms].fill(0);
    codewords[..num_syms].fill(0);

    let max_allowed_freq_sum: u64 = (1u64 << (32 - NUM_SYMBOL_BITS)) - 1;
    let total_sum: u64 = freqs[..num_syms].iter().map(|&f| f as u64).sum();

    let mut scaled_freqs = [0u32; DEFLATE_MAX_NUM_SYMS];
    let effective_freqs: &[u32] = if total_sum > max_allowed_freq_sum {
        let target_sum = max_allowed_freq_sum / 2;
        for (i, &f) in freqs[..num_syms].iter().enumerate() {
            if f > 0 {
                scaled_freqs[i] = (((f as u64) * target_sum / total_sum).max(1)) as u32;
            }
        }
        &scaled_freqs[..num_syms]
    } else {
        &freqs[..num_syms]
    };

    let mut working_slots = [0u32; DEFLATE_MAX_NUM_SYMS];
    let slot_slice = &mut working_slots[..num_syms];

    let num_used_syms = sort_symbols(num_syms, effective_freqs, lens, slot_slice);

    if num_used_syms < 2 {
        let sym = if num_used_syms == 1 {
            (slot_slice[0] & SYMBOL_MASK) as usize
        } else {
            0
        };
        let nonzero_idx = if sym != 0 { sym } else { 1 };

        codewords[0] = 0;
        lens[0] = 1;
        codewords[nonzero_idx] = 1;
        lens[nonzero_idx] = 1;
        return;
    }

    build_tree(slot_slice, num_used_syms);

    let mut len_counts = [0u32; DEFLATE_MAX_CODEWORD_LEN + 1];
    compute_length_counts(
        slot_slice,
        num_used_syms - 2,
        &mut len_counts,
        max_codeword_len,
    );

    gen_codewords(
        slot_slice,
        lens,
        &len_counts,
        max_codeword_len,
        num_syms,
        codewords,
    );
}

// MARK: - Precode RLE Encoder

/// Computes precode items (RLE tokens and extra bits) from a sequence of codeword lengths.
///
/// Encodes runs of 0s (symbols 17 and 18) and runs of non-zero lengths (symbol 16).
/// Each item packs symbol `0..18` in bits `0..4` and extra bits in bits `5..31`.
pub fn compute_precode_items(
    lens: &[u8],
    precode_freqs: &mut [u32; DEFLATE_NUM_PRECODE_SYMS],
    precode_items: &mut Vec<u32>,
) -> usize {
    precode_freqs.fill(0);
    precode_items.clear();

    let num_lens = lens.len();
    if num_lens == 0 {
        return 0;
    }

    let mut run_start = 0;
    while run_start < num_lens {
        let len = lens[run_start];
        let mut run_end = run_start + 1;
        while run_end < num_lens && lens[run_end] == len {
            run_end += 1;
        }

        let mut run_len = run_end - run_start;

        if len == 0 {
            while run_len >= 11 {
                let extra_bits = min(run_len - 11, 0x7F) as u32;
                precode_freqs[18] += 1;
                precode_items.push(18 | (extra_bits << 5));
                run_len -= 11 + (extra_bits as usize);
            }
            if run_len >= 3 {
                let extra_bits = min(run_len - 3, 0x7) as u32;
                precode_freqs[17] += 1;
                precode_items.push(17 | (extra_bits << 5));
                run_len -= 3 + (extra_bits as usize);
            }
        } else if run_len >= 4 {
            precode_freqs[len as usize] += 1;
            precode_items.push(len as u32);
            run_len -= 1;
            while run_len >= 3 {
                let extra_bits = min(run_len - 3, 0x3) as u32;
                precode_freqs[16] += 1;
                precode_items.push(16 | (extra_bits << 5));
                run_len -= 3 + (extra_bits as usize);
            }
        }

        while run_len > 0 {
            precode_freqs[len as usize] += 1;
            precode_items.push(len as u32);
            run_len -= 1;
        }

        run_start = run_end;
    }

    precode_items.len()
}

/// Counts how many precode lengths must be explicitly output according to the canonical permutation.
#[inline]
pub fn compute_num_explicit_precode_lens(precode_lens: &[u8; DEFLATE_NUM_PRECODE_SYMS]) -> usize {
    let mut num_explicit = DEFLATE_NUM_PRECODE_SYMS;
    while num_explicit > 4 {
        let sym = DEFLATE_PRECODE_LENS_PERMUTATION[num_explicit - 1] as usize;
        if precode_lens[sym] != 0 {
            break;
        }
        num_explicit -= 1;
    }
    num_explicit
}

/// Structured precomputed dynamic Huffman header information.
#[derive(Debug, Clone)]
pub struct PrecodeEncodedHeader {
    pub num_litlen_syms: usize,
    pub num_offset_syms: usize,
    pub num_explicit_lens: usize,
    pub precode_freqs: [u32; DEFLATE_NUM_PRECODE_SYMS],
    pub precode_lens: [u8; DEFLATE_NUM_PRECODE_SYMS],
    pub precode_codewords: [u32; DEFLATE_NUM_PRECODE_SYMS],
    pub items: Vec<u32>,
}

/// Helper struct for encoding dynamic block precode headers.
pub struct PrecodeEncoder;

impl PrecodeEncoder {
    /// Precomputes precode Huffman tree and items for dynamic header generation.
    pub fn encode_header(litlen_lens: &[u8], offset_lens: &[u8]) -> PrecodeEncodedHeader {
        let mut num_litlen = litlen_lens.len();
        while num_litlen > 257 && litlen_lens[num_litlen - 1] == 0 {
            num_litlen -= 1;
        }

        let mut num_offset = offset_lens.len();
        while num_offset > 1 && offset_lens[num_offset - 1] == 0 {
            num_offset -= 1;
        }

        let mut combined_lens = Vec::with_capacity(num_litlen + num_offset);
        combined_lens.extend_from_slice(&litlen_lens[..num_litlen]);
        combined_lens.extend_from_slice(&offset_lens[..num_offset]);

        let mut precode_freqs = [0u32; DEFLATE_NUM_PRECODE_SYMS];
        let mut items = Vec::with_capacity(combined_lens.len());
        compute_precode_items(&combined_lens, &mut precode_freqs, &mut items);

        let mut precode_lens = [0u8; DEFLATE_NUM_PRECODE_SYMS];
        let mut precode_codewords = [0u32; DEFLATE_NUM_PRECODE_SYMS];
        deflate_make_huffman_code(
            DEFLATE_NUM_PRECODE_SYMS,
            MAX_PRE_CODEWORD_LEN,
            &precode_freqs,
            &mut precode_lens,
            &mut precode_codewords,
        );

        let num_explicit_lens = compute_num_explicit_precode_lens(&precode_lens);

        PrecodeEncodedHeader {
            num_litlen_syms: num_litlen,
            num_offset_syms: num_offset,
            num_explicit_lens,
            precode_freqs,
            precode_lens,
            precode_codewords,
            items,
        }
    }
}

// MARK: - Fast Bitstream Emitter

/// Bitstream writing error conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastBitWriterError {
    /// Output buffer capacity exceeded.
    BufferOverflow,
}

impl std::fmt::Display for FastBitWriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferOverflow => write!(f, "FastBitWriter buffer overflow"),
        }
    }
}

impl std::error::Error for FastBitWriterError {}

/// Ultra-fast 64-bit bitstream emitter for DEFLATE compression.
///
/// Features:
/// - 64-bit internal bitbuffer with branchless full-word unaligned flushing.
/// - Batch 4-Literal emission optimization when codeword lengths are bounded $\le 14$ bits.
/// - Deterministic zero-copy byte alignment and overflow protection.
#[derive(Debug)]
pub struct FastBitWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
    bitbuf: u64,
    bitcount: u32,
    overflow: bool,
}

impl<'a> FastBitWriter<'a> {
    /// Creates a new bitwriter over a mutable output slice.
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            pos: 0,
            bitbuf: 0,
            bitcount: 0,
            overflow: false,
        }
    }

    /// Appends `n` bits (`0 <= n <= 64`) to the internal bitbuffer.
    /// Caller must ensure `bitcount + n <= 64` (usually by calling `flush_bits()` first).
    #[inline(always)]
    pub fn add_bits(&mut self, bits: u64, n: u32) {
        if n == 0 {
            return;
        }
        debug_assert!(n <= 64);
        debug_assert!(self.bitcount + n <= 64);
        let mask = if n == 64 { u64::MAX } else { (1u64 << n) - 1 };
        self.bitbuf |= (bits & mask) << self.bitcount;
        self.bitcount += n;
    }

    /// Flushes complete bytes from the 64-bit bitbuffer to the output slice.
    ///
    /// Leaves at most 7 unwritten bits in `bitbuf`.
    #[inline(always)]
    pub fn flush_bits(&mut self) {
        if self.pos + 8 <= self.buf.len() {
            let bytes = self.bitbuf.to_le_bytes();
            self.buf[self.pos..self.pos + 8].copy_from_slice(&bytes);
            let num_bytes = (self.bitcount >> 3) as usize;
            self.pos += num_bytes;
            self.bitbuf >>= self.bitcount & !7;
            self.bitcount &= 7;
        } else {
            while self.bitcount >= 8 {
                if self.pos < self.buf.len() {
                    self.buf[self.pos] = (self.bitbuf & 0xFF) as u8;
                    self.pos += 1;
                    self.bitbuf >>= 8;
                    self.bitcount -= 8;
                } else {
                    self.overflow = true;
                    break;
                }
            }
        }
    }

    /// Emits four consecutive literals in a single word batch without intermediate flushing.
    ///
    /// Valid when max literal codeword length $\le 14$, since $7 + 4 \times 14 = 63 \le 64$.
    #[inline(always)]
    pub fn emit_literals_4x(&mut self, lits: [u8; 4], codewords: &[u32], lens: &[u8]) {
        let c0 = codewords[lits[0] as usize] as u64;
        let len0 = lens[lits[0] as usize] as u32;
        let c1 = codewords[lits[1] as usize] as u64;
        let len1 = lens[lits[1] as usize] as u32;
        let c2 = codewords[lits[2] as usize] as u64;
        let len2 = lens[lits[2] as usize] as u32;
        let c3 = codewords[lits[3] as usize] as u64;
        let len3 = lens[lits[3] as usize] as u32;

        self.add_bits(c0, len0);
        self.add_bits(c1, len1);
        self.add_bits(c2, len2);
        self.add_bits(c3, len3);
        self.flush_bits();
    }

    /// Emits a single literal and flushes complete bytes.
    #[inline(always)]
    pub fn emit_literal(&mut self, lit: u8, codeword: u32, len: u8) {
        let _ = lit;
        self.add_bits(codeword as u64, len as u32);
        self.flush_bits();
    }

    /// Aligns bitstream to the next byte boundary by zero-padding unwritten bits.
    #[inline]
    pub fn align_to_byte(&mut self) {
        if (self.bitcount & 7) != 0 {
            let pad = 8 - (self.bitcount & 7);
            self.bitcount += pad;
        }
        self.flush_bits();
    }

    /// Finalizes the bitstream, writing any trailing partial byte.
    ///
    /// Returns the total number of bytes written or `FastBitWriterError::BufferOverflow`.
    pub fn finish(mut self) -> Result<usize, FastBitWriterError> {
        self.flush_bits();
        if self.bitcount > 0 {
            if self.pos < self.buf.len() {
                self.buf[self.pos] = (self.bitbuf & 0xFF) as u8;
                self.pos += 1;
                self.bitcount = 0;
                self.bitbuf = 0;
            } else {
                self.overflow = true;
            }
        }
        if self.overflow {
            Err(FastBitWriterError::BufferOverflow)
        } else {
            Ok(self.pos)
        }
    }

    /// Returns the number of bytes currently written to the output buffer.
    #[inline]
    pub fn bytes_written(&self) -> usize {
        self.pos
    }

    /// Returns the number of pending unwritten bits (0..7 after flush).
    #[inline]
    pub fn bits_buffered(&self) -> u32 {
        self.bitcount
    }

    /// Returns true if the writer encountered buffer overflow.
    #[inline]
    pub fn is_overflow(&self) -> bool {
        self.overflow
    }
}

/// Dynamic vector-backed 64-bit bitstream emitter.
#[derive(Debug, Clone, Default)]
pub struct FastBitWriterVec {
    buffer: Vec<u8>,
    bitbuf: u64,
    bitcount: u32,
}

impl FastBitWriterVec {
    /// Creates a new vector-backed bitwriter with default capacity.
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    /// Creates a new vector-backed bitwriter with specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            bitbuf: 0,
            bitcount: 0,
        }
    }

    /// Appends `n` bits (`0 <= n <= 64`) to the bitbuffer.
    #[inline(always)]
    pub fn add_bits(&mut self, bits: u64, n: u32) {
        if n == 0 {
            return;
        }
        debug_assert!(n <= 64);
        debug_assert!(self.bitcount + n <= 64);
        let mask = if n == 64 { u64::MAX } else { (1u64 << n) - 1 };
        self.bitbuf |= (bits & mask) << self.bitcount;
        self.bitcount += n;
    }

    /// Flushes complete bytes from the bitbuffer into the vector.
    #[inline(always)]
    pub fn flush_bits(&mut self) {
        while self.bitcount >= 8 {
            self.buffer.push((self.bitbuf & 0xFF) as u8);
            self.bitbuf >>= 8;
            self.bitcount -= 8;
        }
    }

    /// Emits four consecutive literals in a single word batch without intermediate flushing.
    #[inline(always)]
    pub fn emit_literals_4x(&mut self, lits: [u8; 4], codewords: &[u32], lens: &[u8]) {
        let c0 = codewords[lits[0] as usize] as u64;
        let len0 = lens[lits[0] as usize] as u32;
        let c1 = codewords[lits[1] as usize] as u64;
        let len1 = lens[lits[1] as usize] as u32;
        let c2 = codewords[lits[2] as usize] as u64;
        let len2 = lens[lits[2] as usize] as u32;
        let c3 = codewords[lits[3] as usize] as u64;
        let len3 = lens[lits[3] as usize] as u32;

        self.add_bits(c0, len0);
        self.add_bits(c1, len1);
        self.add_bits(c2, len2);
        self.add_bits(c3, len3);
        self.flush_bits();
    }

    /// Emits a single literal.
    #[inline(always)]
    pub fn emit_literal(&mut self, lit: u8, codeword: u32, len: u8) {
        let _ = lit;
        self.add_bits(codeword as u64, len as u32);
        self.flush_bits();
    }

    /// Aligns to byte boundary.
    #[inline]
    pub fn align_to_byte(&mut self) {
        if (self.bitcount & 7) != 0 {
            let pad = 8 - (self.bitcount & 7);
            self.bitcount += pad;
        }
        self.flush_bits();
    }

    /// Finalizes the bitstream and unwraps the inner byte vector.
    pub fn finish(mut self) -> Vec<u8> {
        self.flush_bits();
        if self.bitcount > 0 {
            self.buffer.push((self.bitbuf & 0xFF) as u8);
            self.bitcount = 0;
            self.bitbuf = 0;
        }
        self.buffer
    }

    /// Returns a slice of bytes written so far.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }

    /// Drains and returns all flushed bytes from the inner buffer while keeping unwritten bits in `bitbuf`.
    #[inline]
    pub fn take_flushed_bytes(&mut self) -> Vec<u8> {
        self.flush_bits();
        std::mem::take(&mut self.buffer)
    }
}
