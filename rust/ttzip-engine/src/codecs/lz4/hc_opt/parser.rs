// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dynamic Programming Optimal Parser and Multi-tier LZ4HC Evaluation Engine.
//!
//! Features:
//! - 4096-window forward graph shortest-path DP optimal parsing (`Lz4HcOptimalParser`)
//! - Multi-tier search strategies: Fast greedy (L1-2), 2-stage lazy (L3-5), 3-stage lazy (L6-8), and full DP (L9-12)
//! - Direct LZ4 sequence serialization with zero-allocation buffers.

use super::dual_table::{Lz4HcDualTable, Lz4Match, LAST_LITERALS, MIN_MATCH};
use super::price::{price_literals, price_sequence, price_sequence_speed};
use crate::codecs::lz4::block::lz4_compress_bound;
use crate::types::TTZipStatus;

/// Forward lookahead window buffer size for dynamic programming optimal parsing (4096 + 4).
pub const LZ4_OPT_NUM: usize = 4096 + 4;

/// Match Forward Limit: no match may start within 12 bytes of block end.
pub const MF_LIMIT: usize = 12;

/// Search depth per compression level (Index 0 is unused, 1..=12).
pub const MAX_SEARCH_DEPTH_TABLE: [usize; 13] = [
    0,     // 0
    8,     // Level 1: Fast HC
    16,    // Level 2
    32,    // Level 3: Lazy-2
    64,    // Level 4
    128,   // Level 5
    256,   // Level 6: Lazy-3
    512,   // Level 7
    1024,  // Level 8
    2048,  // Level 9: DP Optimal (Default HC)
    4096,  // Level 10
    8192,  // Level 11
    16384, // Level 12: Maximum
];

/// Dynamic programming optimal graph node representing the shortest path to a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lz4OptimalNode {
    /// Cumulative bit-cost price to reach this position.
    pub price: usize,
    /// Backward match offset (0 indicates literal step).
    pub off: u32,
    /// Match length or literal count.
    pub len: u32,
    /// Number of trailing literals before this sequence.
    pub lit_len: u32,
}

impl Default for Lz4OptimalNode {
    fn default() -> Self {
        Self {
            price: usize::MAX,
            off: 0,
            len: 0,
            lit_len: 0,
        }
    }
}

/// Compression strategy applied across different LZ4HC levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lz4HcStrategy {
    /// Fast greedy matching (Level 1..=2).
    Fast,
    /// 2-step lazy evaluation lookahead (Level 3..=5).
    Lazy2,
    /// 3-step lazy evaluation lookahead (Level 6..=8).
    Lazy3,
    /// Full dynamic programming forward shortest-path optimal parser (Level 9..=12).
    #[default]
    Optimal,
}

/// Tuning parameters for LZ4HC compression engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lz4HcParams {
    /// Target compression level (1..=12).
    pub level: i32,
    /// Compression strategy.
    pub strategy: Lz4HcStrategy,
    /// Maximum search depth per hash lookup.
    pub search_depth: usize,
    /// Bias compression toward faster decompression by avoiding small offset overlap copies.
    pub favor_dec_speed: bool,
    /// Maximum lookahead window in bytes for DP parsing.
    pub optimal_lookahead: usize,
}

impl Lz4HcParams {
    /// Constructs default configuration for a given compression level (1..=12).
    pub fn for_level(level: i32) -> Self {
        let clvl = level.clamp(1, 12);
        let depth = MAX_SEARCH_DEPTH_TABLE[clvl as usize];
        let strategy = match clvl {
            1..=2 => Lz4HcStrategy::Fast,
            3..=5 => Lz4HcStrategy::Lazy2,
            6..=8 => Lz4HcStrategy::Lazy3,
            _ => Lz4HcStrategy::Optimal,
        };

        Self {
            level: clvl,
            strategy,
            search_depth: depth,
            favor_dec_speed: false,
            optimal_lookahead: LZ4_OPT_NUM - 4,
        }
    }

    /// Sets the `favor_dec_speed` flag.
    pub fn with_favor_dec_speed(mut self, favor: bool) -> Self {
        self.favor_dec_speed = favor;
        self
    }
}

/// Sequence descriptor generated during parsing before emission.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ParsedSequence {
    match_ip: usize,
    lit_len: usize,
    offset: u16,
    match_len: usize,
}


/// LZ4HC Optimal Parser engine with zero per-block heap allocation.
pub struct Lz4HcOptimalParser {
    dual_table: Box<Lz4HcDualTable>,
    opt: Box<[Lz4OptimalNode; LZ4_OPT_NUM]>,
    params: Lz4HcParams,
}

impl Lz4HcOptimalParser {
    /// Creates a new LZ4HC Optimal Parser for the given compression level.
    pub fn new(level: i32) -> Self {
        Self::with_params(Lz4HcParams::for_level(level))
    }

    /// Creates a new LZ4HC Optimal Parser with custom parameters.
    pub fn with_params(params: Lz4HcParams) -> Self {
        let dual_table = Lz4HcDualTable::new();
        let opt_layout = std::alloc::Layout::new::<[Lz4OptimalNode; LZ4_OPT_NUM]>();
        let opt = unsafe {
            let ptr = std::alloc::alloc_zeroed(opt_layout) as *mut [Lz4OptimalNode; LZ4_OPT_NUM];
            if ptr.is_null() {
                std::alloc::handle_alloc_error(opt_layout);
            }
            Box::from_raw(ptr)
        };

        Self {
            dual_table,
            opt,
            params,
        }
    }

    /// Compresses a source block into `dst` using the configured LZ4HC strategy.
    pub fn compress_block(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        if src.is_empty() {
            return Ok(0);
        }
        let bound = lz4_compress_bound(src.len());
        if dst.len() < bound {
            return Err(TTZipStatus::ErrCompressionFailed);
        }

        self.dual_table.reset();

        let src_len = src.len();
        if src_len < MF_LIMIT {
            let mut dst_pos = 0;
            emit_last_literals(dst, &mut dst_pos, src)?;
            return Ok(dst_pos);
        }

        let mut dst_pos = 0;
        let mut anchor = 0;
        let mut ip = 0;
        let match_limit = src_len - LAST_LITERALS;

        while ip + MIN_MATCH <= match_limit {
            match self.params.strategy {
                Lz4HcStrategy::Fast => {
                    let mut matches = [Lz4Match::default(); 8];
                    let count = self.dual_table.insert_and_find_matches(
                        src,
                        ip,
                        self.params.search_depth,
                        self.params.favor_dec_speed,
                        &mut matches,
                    );

                    if count > 0 {
                        let m = matches[count - 1];
                        emit_sequence(
                            dst,
                            &mut dst_pos,
                            &src[anchor..ip],
                            m.offset,
                            m.length as usize,
                        )?;
                        let m_len = m.length as usize;
                        for step in 1..m_len {
                            self.dual_table.insert_pos(src, ip + step);
                        }
                        ip += m_len;
                        anchor = ip;
                    } else {
                        ip += 1;
                    }
                }
                Lz4HcStrategy::Lazy2 => {
                    let mut matches0 = [Lz4Match::default(); 8];
                    let count0 = self.dual_table.insert_and_find_matches(
                        src,
                        ip,
                        self.params.search_depth,
                        self.params.favor_dec_speed,
                        &mut matches0,
                    );

                    if count0 == 0 {
                        ip += 1;
                        continue;
                    }

                    let m0 = matches0[count0 - 1];
                    let mut matches1 = [Lz4Match::default(); 8];
                    let count1 = self.dual_table.insert_and_find_matches(
                        src,
                        ip + 1,
                        self.params.search_depth,
                        self.params.favor_dec_speed,
                        &mut matches1,
                    );

                    let (selected_ip, selected_match) = if count1 > 0
                        && matches1[count1 - 1].length > m0.length + 1
                    {
                        (ip + 1, matches1[count1 - 1])
                    } else {
                        (ip, m0)
                    };

                    emit_sequence(
                        dst,
                        &mut dst_pos,
                        &src[anchor..selected_ip],
                        selected_match.offset,
                        selected_match.length as usize,
                    )?;
                    let m_len = selected_match.length as usize;
                    for step in 1..m_len {
                        self.dual_table.insert_pos(src, selected_ip + step);
                    }
                    ip = selected_ip + m_len;
                    anchor = ip;
                }
                Lz4HcStrategy::Lazy3 => {
                    let mut matches0 = [Lz4Match::default(); 8];
                    let count0 = self.dual_table.insert_and_find_matches(
                        src,
                        ip,
                        self.params.search_depth,
                        self.params.favor_dec_speed,
                        &mut matches0,
                    );

                    if count0 == 0 {
                        ip += 1;
                        continue;
                    }

                    let m0 = matches0[count0 - 1];
                    let mut matches1 = [Lz4Match::default(); 8];
                    let count1 = self.dual_table.insert_and_find_matches(
                        src,
                        ip + 1,
                        self.params.search_depth,
                        self.params.favor_dec_speed,
                        &mut matches1,
                    );

                    let mut matches2 = [Lz4Match::default(); 8];
                    let count2 = if ip + 2 + MIN_MATCH <= match_limit {
                        self.dual_table.insert_and_find_matches(
                            src,
                            ip + 2,
                            self.params.search_depth,
                            self.params.favor_dec_speed,
                            &mut matches2,
                        )
                    } else {
                        0
                    };

                    let (selected_ip, selected_match) = if count2 > 0
                        && matches2[count2 - 1].length > m0.length + 2
                        && (count1 == 0 || matches2[count2 - 1].length > matches1[count1 - 1].length + 1)
                    {
                        (ip + 2, matches2[count2 - 1])
                    } else if count1 > 0 && matches1[count1 - 1].length > m0.length + 1 {
                        (ip + 1, matches1[count1 - 1])
                    } else {
                        (ip, m0)
                    };

                    emit_sequence(
                        dst,
                        &mut dst_pos,
                        &src[anchor..selected_ip],
                        selected_match.offset,
                        selected_match.length as usize,
                    )?;
                    let m_len = selected_match.length as usize;
                    for step in 1..m_len {
                        self.dual_table.insert_pos(src, selected_ip + step);
                    }
                    ip = selected_ip + m_len;
                    anchor = ip;
                }
                Lz4HcStrategy::Optimal => {
                    let mut matches = [Lz4Match::default(); 16];
                    let count = self.dual_table.insert_and_find_matches(
                        src,
                        ip,
                        self.params.search_depth,
                        self.params.favor_dec_speed,
                        &mut matches,
                    );

                    if count == 0 {
                        ip += 1;
                        continue;
                    }

                    let longest = matches[count - 1].length as usize;
                    if longest >= self.params.optimal_lookahead {
                        let m = matches[count - 1];
                        emit_sequence(
                            dst,
                            &mut dst_pos,
                            &src[anchor..ip],
                            m.offset,
                            m.length as usize,
                        )?;
                        let m_len = m.length as usize;
                        for step in 1..m_len {
                            self.dual_table.insert_pos(src, ip + step);
                        }
                        ip += m_len;
                        anchor = ip;
                    } else {
                        let anchor_lit_len = ip - anchor;
                        let sequences =
                            self.run_optimal_dp(src, ip, anchor_lit_len, &matches[..count]);
                        if sequences.is_empty() {
                            ip += 1;
                        } else {
                            for seq in sequences {
                                let match_ip = seq.match_ip;
                                emit_sequence(
                                    dst,
                                    &mut dst_pos,
                                    &src[anchor..match_ip],
                                    seq.offset,
                                    seq.match_len,
                                )?;
                                anchor = match_ip + seq.match_len;
                            }
                            ip = anchor;
                        }
                    }
                }
            }
        }

        if anchor < src_len {
            emit_last_literals(dst, &mut dst_pos, &src[anchor..src_len])?;
        }

        Ok(dst_pos)
    }

    /// Runs shortest-path dynamic programming relaxation across the forward lookahead window.
    fn run_optimal_dp(
        &mut self,
        src: &[u8],
        ip: usize,
        anchor_lit_len: usize,
        first_matches: &[Lz4Match],
    ) -> Vec<ParsedSequence> {
        let max_lookahead = (src.len().saturating_sub(LAST_LITERALS) - ip)
            .min(self.params.optimal_lookahead)
            .min(LZ4_OPT_NUM - 4);

        for node in &mut self.opt[0..=max_lookahead] {
            node.price = usize::MAX;
            node.off = 0;
            node.len = 0;
            node.lit_len = 0;
        }

        self.opt[0] = Lz4OptimalNode {
            price: 0,
            off: 0,
            len: 0,
            lit_len: anchor_lit_len as u32,
        };

        let mut last_pos = 0;

        for m in first_matches {
            let max_ml = (m.length as usize).min(max_lookahead);
            if max_ml < MIN_MATCH {
                continue;
            }
            let step = if max_ml > 32 { (max_ml - 16) / 8 + 1 } else { 1 };
            let mut ml = MIN_MATCH;
            while ml <= max_ml {
                let cost = price_sequence_speed(
                    anchor_lit_len,
                    ml,
                    m.offset,
                    self.params.favor_dec_speed,
                );
                if cost <= self.opt[ml].price {
                    self.opt[ml] = Lz4OptimalNode {
                        price: cost,
                        off: m.offset as u32,
                        len: ml as u32,
                        lit_len: 0,
                    };
                    if ml > last_pos {
                        last_pos = ml;
                    }
                }
                if ml < 16 || ml + step > max_ml {
                    ml += 1;
                } else {
                    ml += step;
                }
            }

            let cost = price_sequence_speed(
                anchor_lit_len,
                max_ml,
                m.offset,
                self.params.favor_dec_speed,
            );
            if cost <= self.opt[max_ml].price {
                self.opt[max_ml] = Lz4OptimalNode {
                    price: cost,
                    off: m.offset as u32,
                    len: max_ml as u32,
                    lit_len: 0,
                };
                if max_ml > last_pos {
                    last_pos = max_ml;
                }
            }
        }

        let mut cur = 1;
        while cur <= last_pos && cur < max_lookahead {
            if self.opt[cur].price < usize::MAX {
                let cur_lit_len = self.opt[cur].lit_len as usize;

                // 1. Literal step to cur + 1
                let extra_cost = if cur_lit_len == 0
                    || cur_lit_len == 14
                    || (cur_lit_len > 14 && (cur_lit_len - 14).is_multiple_of(255))
                {
                    2
                } else {
                    1
                };
                let lit_price = self.opt[cur].price + extra_cost;
                if lit_price <= self.opt[cur + 1].price {
                    self.opt[cur + 1] = Lz4OptimalNode {
                        price: lit_price,
                        off: 0,
                        len: 1,
                        lit_len: (cur_lit_len + 1) as u32,
                    };
                    if cur + 1 > last_pos {
                        last_pos = cur + 1;
                    }
                }

                // 2. Matches at cur_pos
                let cur_pos = ip + cur;
                let mut matches = [Lz4Match::default(); 16];
                let count = self.dual_table.insert_and_find_matches(
                    src,
                    cur_pos,
                    self.params.search_depth,
                    self.params.favor_dec_speed,
                    &mut matches,
                );

                if count > 0 {
                    let base_idx = cur.saturating_sub(cur_lit_len);

                    for m in &matches[..count] {
                        let max_ml = (m.length as usize).min(max_lookahead - cur);
                        if max_ml < MIN_MATCH {
                            continue;
                        }

                        let step = if max_ml > 32 { (max_ml - 16) / 8 + 1 } else { 1 };
                        let mut ml = MIN_MATCH;
                        while ml <= max_ml {
                            let cost = self.opt[base_idx].price
                                + price_sequence_speed(
                                    cur_lit_len,
                                    ml,
                                    m.offset,
                                    self.params.favor_dec_speed,
                                );
                            if cost <= self.opt[cur + ml].price {
                                self.opt[cur + ml] = Lz4OptimalNode {
                                    price: cost,
                                    off: m.offset as u32,
                                    len: ml as u32,
                                    lit_len: 0,
                                };
                                if cur + ml > last_pos {
                                    last_pos = cur + ml;
                                }
                            }
                            if ml < 16 || ml + step > max_ml {
                                ml += 1;
                            } else {
                                ml += step;
                            }
                        }

                        let cost = self.opt[base_idx].price
                            + price_sequence_speed(
                                cur_lit_len,
                                max_ml,
                                m.offset,
                                self.params.favor_dec_speed,
                            );
                        if cost <= self.opt[cur + max_ml].price {
                            self.opt[cur + max_ml] = Lz4OptimalNode {
                                price: cost,
                                off: m.offset as u32,
                                len: max_ml as u32,
                                lit_len: 0,
                            };
                            if cur + max_ml > last_pos {
                                last_pos = cur + max_ml;
                            }
                        }
                    }
                }
            }
            cur += 1;
        }

        let mut curr = last_pos;
        let mut seq_stack = Vec::new();

        while curr > 0 {
            let off = self.opt[curr].off;
            let len = self.opt[curr].len as usize;

            if off > 0 {
                let match_end = curr;
                let match_start = match_end - len;
                let lit_len = self.opt[match_start].lit_len as usize;
                let base_idx = match_start.saturating_sub(lit_len);

                seq_stack.push(ParsedSequence {
                    match_ip: ip + match_start,
                    lit_len,
                    offset: off as u16,
                    match_len: len,
                });

                curr = base_idx;
            } else {
                curr = curr.saturating_sub(1);
            }
        }

        if seq_stack.is_empty() {
            seq_stack.push(ParsedSequence {
                match_ip: ip,
                lit_len: anchor_lit_len,
                offset: first_matches[0].offset,
                match_len: first_matches[0].length as usize,
            });
        }

        seq_stack.reverse();
        seq_stack
    }


}

/// Emits an LZ4 sequence with token, literals, 2-byte match offset, and variable length extensions.
fn emit_sequence(
    dst: &mut [u8],
    dst_pos: &mut usize,
    literals: &[u8],
    match_offset: u16,
    match_len: usize,
) -> Result<(), TTZipStatus> {
    debug_assert!(match_len >= MIN_MATCH);
    debug_assert!(match_offset > 0);

    let lit_len = literals.len();
    let token_lit = lit_len.min(15) as u8;
    let match_extra = match_len - MIN_MATCH;
    let token_match = match_extra.min(15) as u8;
    let token = (token_lit << 4) | token_match;

    let required = price_sequence(lit_len, match_len);
    if *dst_pos + required > dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    dst[*dst_pos] = token;
    *dst_pos += 1;

    if lit_len >= 15 {
        let mut rem = lit_len - 15;
        while rem >= 255 {
            dst[*dst_pos] = 255;
            *dst_pos += 1;
            rem -= 255;
        }
        dst[*dst_pos] = rem as u8;
        *dst_pos += 1;
    }

    if lit_len > 0 {
        dst[*dst_pos..*dst_pos + lit_len].copy_from_slice(literals);
        *dst_pos += lit_len;
    }

    dst[*dst_pos..*dst_pos + 2].copy_from_slice(&match_offset.to_le_bytes());
    *dst_pos += 2;

    if match_extra >= 15 {
        let mut rem = match_extra - 15;
        while rem >= 255 {
            dst[*dst_pos] = 255;
            *dst_pos += 1;
            rem -= 255;
        }
        dst[*dst_pos] = rem as u8;
        *dst_pos += 1;
    }

    Ok(())
}

/// Emits trailing last literals block at the end of an LZ4 block.
fn emit_last_literals(
    dst: &mut [u8],
    dst_pos: &mut usize,
    literals: &[u8],
) -> Result<(), TTZipStatus> {
    let lit_len = literals.len();
    let token_lit = lit_len.min(15) as u8;
    let token = token_lit << 4;

    let required = price_literals(lit_len);
    if *dst_pos + required > dst.len() {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    dst[*dst_pos] = token;
    *dst_pos += 1;

    if lit_len >= 15 {
        let mut rem = lit_len - 15;
        while rem >= 255 {
            dst[*dst_pos] = 255;
            *dst_pos += 1;
            rem -= 255;
        }
        dst[*dst_pos] = rem as u8;
        *dst_pos += 1;
    }

    if lit_len > 0 {
        dst[*dst_pos..*dst_pos + lit_len].copy_from_slice(literals);
        *dst_pos += lit_len;
    }

    Ok(())
}
