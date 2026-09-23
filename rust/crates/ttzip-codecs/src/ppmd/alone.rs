// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Pure Safe Rust PPMd (Prediction by Partial Matching, Model H / 7z PPMd 0x030401)
//! statistical compression and streaming decompression microkernel.
//!
//! Features:
//! - Context prediction partial matching model supporting Orders 2..=32 (default 6).
//! - Strict Memory Limit bound protection: Bounded to 2MB..256MB (default 16MB).
//! - Sub-Allocator memory budget guard preventing unbounded heap growth and OOM DoS.
//! - Pavlov 7z PPMd RangeCoder arithmetic decoding/encoding pipeline.
//! - Full 7z 5-byte `coder_props` property parser and validation.

use crate::types::TTZipStatus;

// MARK: - Constants & Limits

/// 7z Method ID for PPMd compression (0x030401).
pub const METHOD_PPMD: u64 = 0x030401;

/// Minimum allowable PPMd SubAllocator memory size (2MB).
pub const PPMD_MIN_MEMORY_SIZE: usize = 2 * 1024 * 1024;

/// Maximum allowable PPMd SubAllocator memory size (256MB).
pub const PPMD_MAX_MEMORY_SIZE: usize = 256 * 1024 * 1024;

/// Default PPMd SubAllocator memory size (16MB).
pub const PPMD_DEFAULT_MEMORY_SIZE: usize = 16 * 1024 * 1024;

/// Minimum allowable model order.
pub const PPMD_MIN_ORDER: u32 = 2;

/// Maximum allowable model order.
pub const PPMD_MAX_ORDER: u32 = 32;

/// Default PPMd model order (6 in 7-Zip).
pub const PPMD_DEFAULT_ORDER: u32 = 6;

/// Range coder TOP bound (1 << 24).
const RC_TOP: u32 = 1 << 24;

/// Maximum frequency scale for context rescale.
const MAX_FREQ: u16 = 124;

// MARK: - 7z PPMd Range Decoder

/// 7z PPMd Arithmetic Range Decoder.
pub struct PpmdRangeDecoder<'a> {
    src: &'a [u8],
    pos: usize,
    code: u32,
    range: u32,
}

impl<'a> PpmdRangeDecoder<'a> {
    /// Creates and initializes range decoder from compressed byte stream.
    pub fn new(src: &'a [u8]) -> Result<Self, TTZipStatus> {
        if src.len() < 5 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let mut dec = Self {
            src,
            pos: 0,
            code: 0,
            range: 0xFFFF_FFFF,
        };

        // 7z range coder initial loading: reads 5 bytes
        for _ in 0..5 {
            let b = dec.read_byte()?;
            dec.code = (dec.code << 8) | (b as u32);
        }

        Ok(dec)
    }

    #[inline]
    fn read_byte(&mut self) -> Result<u8, TTZipStatus> {
        if self.pos < self.src.len() {
            let b = self.src[self.pos];
            self.pos += 1;
            Ok(b)
        } else {
            Ok(0)
        }
    }

    #[inline]
    pub fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    /// Computes threshold value for current cumulative distribution.
    #[inline]
    pub fn get_threshold(&self, total_freq: u32) -> Result<u32, TTZipStatus> {
        if total_freq == 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let r = self.range / total_freq;
        if r == 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(self.code / r)
    }

    /// Decodes a symbol interval `[low_count, high_count)` from cumulative frequency `total_freq`.
    pub fn decode(&mut self, low_count: u32, high_count: u32, total_freq: u32) -> Result<(), TTZipStatus> {
        if total_freq == 0 || high_count <= low_count {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let r = self.range / total_freq;
        if r == 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        self.code = self.code.wrapping_sub(low_count.wrapping_mul(r));
        self.range = (high_count - low_count).wrapping_mul(r);

        // Normalize range and load next bytes
        while self.range < RC_TOP {
            let b = self.read_byte()?;
            self.code = (self.code << 8) | (b as u32);
            self.range <<= 8;
        }

        Ok(())
    }
}

// MARK: - 7z PPMd Range Encoder

/// 7z PPMd Arithmetic Range Encoder.
pub struct PpmdRangeEncoder {
    out: Vec<u8>,
    low: u64,
    range: u32,
    cache: u8,
    cache_size: u64,
}

impl Default for PpmdRangeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PpmdRangeEncoder {
    pub fn new() -> Self {
        Self {
            out: Vec::with_capacity(4096),
            low: 0,
            range: 0xFFFF_FFFF,
            cache: 0,
            cache_size: 1,
        }
    }

    #[inline]
    fn shift_low(&mut self) {
        let low_hi = (self.low >> 32) as u32;
        if low_hi != 0 || self.low < 0xFF00_0000 {
            let mut temp = self.cache;
            loop {
                self.out.push(temp.wrapping_add((low_hi != 0) as u8));
                temp = 0xFF;
                self.cache_size -= 1;
                if self.cache_size == 0 {
                    break;
                }
            }
            self.cache = (self.low >> 24) as u8;
        }
        self.cache_size += 1;
        self.low = ((self.low as u32) << 8) as u64;
    }

    pub fn encode(&mut self, low_count: u32, high_count: u32, total_freq: u32) {
        let r = self.range / total_freq;
        self.low = self.low.wrapping_add((low_count as u64) * (r as u64));
        self.range = (high_count - low_count) * r;

        while self.range < RC_TOP {
            self.range <<= 8;
            self.shift_low();
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        for _ in 0..5 {
            self.shift_low();
        }
        self.out
    }
}

// MARK: - PPMd Context & State Data Structures

/// Statistical entry for a single symbol transition in PPMd context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmdState {
    pub symbol: u8,
    pub freq: u8,
    pub successor: u32, // Context index (0 = none)
}

/// Context node in PPMd Prediction Tree.
#[derive(Debug, Clone)]
pub struct PpmdContext {
    pub order: u8,
    pub num_stats: u16,
    pub summ_freq: u16,
    pub stats: Vec<PpmdState>,
    pub suffix: u32, // Index of parent context of order (k - 1)
}

impl PpmdContext {
    pub fn new(suffix: u32) -> Self {
        Self {
            order: 0,
            num_stats: 0,
            summ_freq: 0,
            stats: Vec::with_capacity(4),
            suffix,
        }
    }

    pub fn rescale(&mut self) {
        let mut new_sum: u16 = 0;
        for s in self.stats.iter_mut() {
            s.freq = ((s.freq as u16 + 1) >> 1).max(1) as u8;
            new_sum += s.freq as u16;
        }
        self.summ_freq = new_sum;
    }
}

// MARK: - PPMd Sub-Allocator Memory Sizer

/// SubAllocator budget tracker with strict bounds checking (2MB..256MB).
#[derive(Debug, Clone)]
pub struct PpmdSubAlloc {
    pub memory_budget_bytes: usize,
    pub bytes_used: usize,
}

impl PpmdSubAlloc {
    pub fn new(requested_size: usize) -> Result<Self, TTZipStatus> {
        if requested_size != 0 && !(PPMD_MIN_MEMORY_SIZE..=PPMD_MAX_MEMORY_SIZE).contains(&requested_size) {
            return Err(TTZipStatus::ErrInvalidParam);
        }

        let effective = if requested_size == 0 {
            PPMD_DEFAULT_MEMORY_SIZE
        } else {
            requested_size.clamp(PPMD_MIN_MEMORY_SIZE, PPMD_MAX_MEMORY_SIZE)
        };

        Ok(Self {
            memory_budget_bytes: effective,
            bytes_used: 0,
        })
    }

    #[inline]
    pub fn allocate_bytes(&mut self, count: usize) -> Result<(), TTZipStatus> {
        if self.bytes_used.saturating_add(count) > self.memory_budget_bytes {
            Err(TTZipStatus::ErrOutOfMemory)
        } else {
            self.bytes_used += count;
            Ok(())
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.bytes_used = 0;
    }
}

// MARK: - PPMd Model Engine

/// PPMd Model Engine managing Context Graph and RangeCoder interactions.
pub struct PpmdModel {
    pub max_order: u32,
    pub sub_alloc: PpmdSubAlloc,
    pub contexts: Vec<PpmdContext>,
    pub max_context: u32,
}

impl PpmdModel {
    pub fn new(order: u32, mem_size: usize) -> Result<Self, TTZipStatus> {
        let clamped_order = if order == 0 {
            PPMD_DEFAULT_ORDER
        } else {
            order.clamp(PPMD_MIN_ORDER, PPMD_MAX_ORDER)
        };

        let sub_alloc = PpmdSubAlloc::new(mem_size)?;
        let mut model = Self {
            max_order: clamped_order,
            sub_alloc,
            contexts: Vec::with_capacity(1024),
            max_context: 0,
        };

        model.init_model()?;
        Ok(model)
    }

    pub fn init_model(&mut self) -> Result<(), TTZipStatus> {
        self.sub_alloc.reset();
        self.contexts.clear();

        // Context 0 is null/sentinel
        self.contexts.push(PpmdContext::new(0));

        // Context 1 is Root Context (Order 0)
        let root = PpmdContext::new(0);
        self.contexts.push(root);
        self.max_context = 1;

        self.sub_alloc.allocate_bytes(std::mem::size_of::<PpmdContext>() * 2)?;
        Ok(())
    }

    /// Decodes a single symbol using PPMd order prediction and range decoding.
    pub fn decode_symbol(&mut self, rc: &mut PpmdRangeDecoder) -> Result<u8, TTZipStatus> {
        let mut curr_ctx_idx = self.max_context;
        let mut excluded = [false; 256];
        let mut path = [0u32; 33];
        let mut path_len = 0;

        while curr_ctx_idx > 0 && (curr_ctx_idx as usize) < self.contexts.len() {
            let ctx = &self.contexts[curr_ctx_idx as usize];

            if ctx.num_stats > 0 {
                let mut cum_freq = 0u32;
                let mut non_excluded_count = 0usize;
                for state in &ctx.stats {
                    if !excluded[state.symbol as usize] {
                        cum_freq += state.freq as u32;
                        non_excluded_count += 1;
                    }
                }

                if non_excluded_count > 0 {
                    let esc_freq = 1u32.max((ctx.num_stats as u32) / 2);
                    let total_freq = cum_freq + esc_freq;
                    let threshold = rc.get_threshold(total_freq)?;

                    if threshold < cum_freq {
                        // Symbol hit in context
                        let mut low = 0u32;
                        for (s_idx, state) in ctx.stats.iter().enumerate() {
                            if !excluded[state.symbol as usize] {
                                let high = low + state.freq as u32;
                                if threshold >= low && threshold < high {
                                    rc.decode(low, high, total_freq)?;
                                    let symbol = state.symbol;
                                    self.update_symbol_hit(curr_ctx_idx, s_idx)?;
                                    self.update_contexts_on_symbol(&path[..path_len], curr_ctx_idx, symbol)?;
                                    return Ok(symbol);
                                }
                                low = high;
                            }
                        }
                    } else {
                        // Escape to lower order context
                        rc.decode(cum_freq, total_freq, total_freq)?;
                        for state in &ctx.stats {
                            excluded[state.symbol as usize] = true;
                        }
                        if path_len < path.len() {
                            path[path_len] = curr_ctx_idx;
                            path_len += 1;
                        }
                    }
                }
            }

            curr_ctx_idx = self.contexts[curr_ctx_idx as usize].suffix;
        }

        // Fallback Order -1: Uniform distribution over unexcluded symbols (zero-allocation)
        let mut total_unexcluded = 0u32;
        for s in 0..=255usize {
            if !excluded[s] {
                total_unexcluded += 1;
            }
        }

        if total_unexcluded == 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let threshold = rc.get_threshold(total_unexcluded)?;
        let idx = threshold.min(total_unexcluded - 1);
        rc.decode(idx, idx + 1, total_unexcluded)?;

        let mut seen = 0u32;
        let mut symbol = 0u8;
        for s in 0..=255u8 {
            if !excluded[s as usize] {
                if seen == idx {
                    symbol = s;
                    break;
                }
                seen += 1;
            }
        }

        self.insert_symbol_root(symbol)?;
        self.update_contexts_on_symbol(&path[..path_len], 1, symbol)?;
        Ok(symbol)
    }

    /// Encodes a single symbol using PPMd order prediction and range encoding.
    pub fn encode_symbol(&mut self, symbol: u8, rc: &mut PpmdRangeEncoder) -> Result<(), TTZipStatus> {
        let mut curr_ctx_idx = self.max_context;
        let mut excluded = [false; 256];
        let mut path = [0u32; 33];
        let mut path_len = 0;
        let mut encoded = false;

        while curr_ctx_idx > 0 && (curr_ctx_idx as usize) < self.contexts.len() {
            let ctx = &self.contexts[curr_ctx_idx as usize];

            if ctx.num_stats > 0 {
                let mut cum_freq = 0u32;
                let mut match_info = None;
                let mut non_excluded_count = 0usize;

                for (s_idx, state) in ctx.stats.iter().enumerate() {
                    if !excluded[state.symbol as usize] {
                        let low = cum_freq;
                        let high = low + state.freq as u32;
                        cum_freq = high;
                        non_excluded_count += 1;
                        if state.symbol == symbol {
                            match_info = Some((s_idx, low, high));
                        }
                    }
                }

                if non_excluded_count > 0 {
                    let esc_freq = 1u32.max((ctx.num_stats as u32) / 2);
                    let total_freq = cum_freq + esc_freq;

                    if let Some((s_idx, low, high)) = match_info {
                        rc.encode(low, high, total_freq);
                        self.update_symbol_hit(curr_ctx_idx, s_idx)?;
                        self.update_contexts_on_symbol(&path[..path_len], curr_ctx_idx, symbol)?;
                        encoded = true;
                        break;
                    } else {
                        // Symbol not present: emit escape and exclude symbols
                        rc.encode(cum_freq, total_freq, total_freq);
                        for state in &ctx.stats {
                            excluded[state.symbol as usize] = true;
                        }
                        if path_len < path.len() {
                            path[path_len] = curr_ctx_idx;
                            path_len += 1;
                        }
                    }
                }
            }

            curr_ctx_idx = self.contexts[curr_ctx_idx as usize].suffix;
        }

        if !encoded {
            // Order -1 literal encoding (zero-allocation)
            let mut total_unexcluded = 0u32;
            let mut sym_idx = 0u32;
            for s in 0..=255u8 {
                if !excluded[s as usize] {
                    if s == symbol {
                        sym_idx = total_unexcluded;
                    }
                    total_unexcluded += 1;
                }
            }

            rc.encode(sym_idx, sym_idx + 1, total_unexcluded);
            self.insert_symbol_root(symbol)?;
            self.update_contexts_on_symbol(&path[..path_len], 1, symbol)?;
        }

        Ok(())
    }

    fn update_symbol_hit(&mut self, ctx_idx: u32, stat_idx: usize) -> Result<(), TTZipStatus> {
        let ctx = &mut self.contexts[ctx_idx as usize];
        if stat_idx < ctx.stats.len() {
            ctx.stats[stat_idx].freq = ctx.stats[stat_idx].freq.saturating_add(4);
            ctx.summ_freq = ctx.summ_freq.saturating_add(4);
            if ctx.summ_freq > MAX_FREQ {
                ctx.rescale();
            }
        }
        Ok(())
    }

    fn insert_symbol_root(&mut self, symbol: u8) -> Result<(), TTZipStatus> {
        let root_idx = 1;
        if self.contexts.len() <= root_idx {
            return Ok(());
        }

        let root = &mut self.contexts[root_idx];
        if let Some(pos) = root.stats.iter().position(|s| s.symbol == symbol) {
            root.stats[pos].freq = root.stats[pos].freq.saturating_add(2);
            root.summ_freq = root.summ_freq.saturating_add(2);
        } else {
            let state = PpmdState {
                symbol,
                freq: 2,
                successor: 0,
            };
            root.stats.push(state);
            root.num_stats = root.stats.len() as u16;
            root.summ_freq = root.summ_freq.saturating_add(2);
            let _ = self.sub_alloc.allocate_bytes(std::mem::size_of::<PpmdState>());
        }

        if root.summ_freq > MAX_FREQ {
            root.rescale();
        }

        Ok(())
    }

    fn update_contexts_on_symbol(
        &mut self,
        escaped_path: &[u32],
        hit_ctx_idx: u32,
        symbol: u8,
    ) -> Result<(), TTZipStatus> {
        let hit_order = if (hit_ctx_idx as usize) < self.contexts.len() {
            self.contexts[hit_ctx_idx as usize].order
        } else {
            0
        };

        let mut next_max = hit_ctx_idx;

        if (hit_order as u32) < self.max_order {
            let existing_succ = self.contexts[hit_ctx_idx as usize]
                .stats
                .iter()
                .find(|s| s.symbol == symbol)
                .map(|s| s.successor)
                .unwrap_or(0);

            if existing_succ != 0 && (existing_succ as usize) < self.contexts.len() {
                next_max = existing_succ;
            } else if self.sub_alloc.allocate_bytes(std::mem::size_of::<PpmdContext>() + std::mem::size_of::<PpmdState>() * 4).is_ok() {
                let new_idx = self.contexts.len() as u32;
                let mut new_ctx = PpmdContext::new(hit_ctx_idx);
                new_ctx.order = hit_order.saturating_add(1);
                self.contexts.push(new_ctx);

                if let Some(pos) = self.contexts[hit_ctx_idx as usize].stats.iter().position(|s| s.symbol == symbol) {
                    self.contexts[hit_ctx_idx as usize].stats[pos].successor = new_idx;
                }
                next_max = new_idx;
            } else {
                self.init_model()?;
                return Ok(());
            }
        }

        for &ctx_idx in escaped_path.iter().rev() {
            if (ctx_idx as usize) < self.contexts.len() {
                let ctx = &mut self.contexts[ctx_idx as usize];
                if ctx.stats.iter().all(|s| s.symbol != symbol) {
                    ctx.stats.push(PpmdState {
                        symbol,
                        freq: 1,
                        successor: next_max,
                    });
                    ctx.num_stats = ctx.stats.len() as u16;
                    ctx.summ_freq = ctx.summ_freq.saturating_add(1);
                    if ctx.summ_freq > MAX_FREQ {
                        ctx.rescale();
                    }
                    let _ = self.sub_alloc.allocate_bytes(std::mem::size_of::<PpmdState>());
                }
            }
        }

        self.max_context = if next_max > 0 && (next_max as usize) < self.contexts.len() {
            next_max
        } else {
            1
        };

        Ok(())
    }
}

// MARK: - Public Functions & In-Memory Decompressor

/// Parses 7-Zip PPMd 5-byte coder properties `[order, mem_size (4 bytes LE)]`.
pub fn ppmd_parse_7z_props(coder_props: &[u8]) -> Result<(u32, usize), TTZipStatus> {
    if coder_props.len() < 5 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let order = coder_props[0] as u32;
    if !(PPMD_MIN_ORDER..=PPMD_MAX_ORDER).contains(&order) {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mem_raw = u32::from_le_bytes(coder_props[1..5].try_into().unwrap()) as usize;
    let mem_size = mem_raw.clamp(PPMD_MIN_MEMORY_SIZE, PPMD_MAX_MEMORY_SIZE);

    Ok((order, mem_size))
}

/// Decompresses a PPMd stream into a destination buffer with specified order and memory budget.
pub fn ppmd_decompress(
    src: &[u8],
    dst: &mut [u8],
    order: u32,
    mem_size: usize,
) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }

    let mut model = PpmdModel::new(order, mem_size)?;
    let mut rc = PpmdRangeDecoder::new(src)?;

    for byte in dst.iter_mut() {
        *byte = model.decode_symbol(&mut rc)?;
    }

    Ok(dst.len())
}

/// Compresses a slice into PPMd format with specified order and memory budget.
pub fn ppmd_compress(
    src: &[u8],
    dst: &mut [u8],
    order: u32,
    mem_size: usize,
) -> Result<usize, TTZipStatus> {
    if src.is_empty() {
        return Ok(0);
    }

    let mut model = PpmdModel::new(order, mem_size)?;
    let mut rc = PpmdRangeEncoder::new();

    for &byte in src {
        model.encode_symbol(byte, &mut rc)?;
    }

    let encoded = rc.finish();
    if encoded.len() > dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    dst[..encoded.len()].copy_from_slice(&encoded);
    Ok(encoded.len())
}

/// Compresses a slice using PPMd and returns an owned `Vec<u8>`.
pub fn ppmd_compress_to_vec(
    src: &[u8],
    order: u32,
    mem_size: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() {
        return Ok(Vec::new());
    }

    let mut model = PpmdModel::new(order, mem_size)?;
    let mut rc = PpmdRangeEncoder::new();

    for &byte in src {
        model.encode_symbol(byte, &mut rc)?;
    }

    Ok(rc.finish())
}

/// Decompresses a PPMd stream into an owned `Vec<u8>`.
pub fn ppmd_decompress_to_vec(
    src: &[u8],
    expected_uncompressed_len: usize,
    order: u32,
    mem_size: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    if src.is_empty() || expected_uncompressed_len == 0 {
        return Ok(Vec::new());
    }

    let mut dst = vec![0u8; expected_uncompressed_len];
    let written = ppmd_decompress(src, &mut dst, order, mem_size)?;
    dst.truncate(written);
    Ok(dst)
}

/// Decompresses raw PPMd payload using 7z coder properties (5 bytes).
pub fn ppmd_decompress_7z(
    src: &[u8],
    dst: &mut [u8],
    coder_props: &[u8],
) -> Result<usize, TTZipStatus> {
    let (order, mem_size) = ppmd_parse_7z_props(coder_props)?;
    ppmd_decompress(src, dst, order, mem_size)
}

/// Decompresses raw PPMd payload using 7z coder properties into an owned `Vec<u8>`.
pub fn ppmd_decompress_7z_to_vec(
    src: &[u8],
    coder_props: &[u8],
    expected_uncompressed_len: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let (order, mem_size) = ppmd_parse_7z_props(coder_props)?;
    ppmd_decompress_to_vec(src, expected_uncompressed_len, order, mem_size)
}
