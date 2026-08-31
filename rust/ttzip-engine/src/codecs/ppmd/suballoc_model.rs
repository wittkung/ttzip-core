// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PPMd7 / PPMd8 Sub-Allocator driven statistical modeling engine and range coder integration.

use crate::types::TTZipStatus;
use super::models::{PpmdContext, PpmdState};
use super::see::{SeeEntry, SeeEstimator};
use super::suballoc::SubAllocBumpArena;
use super::variant::{PpmdRestoreMethod, PpmdVariant};

/// PPMd7 / PPMd8 Sub-Allocator driven statistical modeling engine.
pub struct PpmdSubAllocModel {
    pub arena: SubAllocBumpArena,
    pub max_order: u32,
    pub max_context_ref: u32,
    pub see: SeeEstimator,
    pub restart_count: usize,
    pub cutoff_count: usize,
    pub total_freed_by_cutoff: usize,
}

/// Cumulative frequency range and location for a PPMd state symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmdSymbolRange {
    pub state_idx: usize,
    pub state: PpmdState,
    pub low_cum_freq: u32,
    pub high_cum_freq: u32,
}

/// Aggregated symbol statistics and escape frequency for a context scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpmdContextScan {
    pub ranges: Vec<PpmdSymbolRange>,
    pub cum_freq: u32,
    pub esc_freq: u32,
}

impl PpmdSubAllocModel {
    /// Creates a new sub-allocator PPMd model instance for specified budget, order, and variant.
    pub fn new(size: usize, order: u32, variant: PpmdVariant) -> Result<Self, TTZipStatus> {
        let mut arena = SubAllocBumpArena::new(size, variant)?;
        let clamped_order = order.clamp(2, 32);
        arena.max_order = clamped_order;
        let root_ref = arena.root_context_ref;
        Ok(Self {
            arena,
            max_order: clamped_order,
            max_context_ref: root_ref,
            see: SeeEstimator::new(),
            restart_count: 0,
            cutoff_count: 0,
            total_freed_by_cutoff: 0,
        })
    }

    #[inline]
    fn scan_context_stats(
        &self,
        ctx: &PpmdContext,
        excluded: &[bool; 256],
    ) -> PpmdContextScan {
        let mut ranges = Vec::with_capacity(ctx.num_stats as usize);
        let mut cum_freq = 0u32;

        if ctx.num_stats == 1 {
            let state = ctx.one_state();
            if !excluded[state.symbol() as usize] {
                let high = state.freq() as u32;
                ranges.push(PpmdSymbolRange {
                    state_idx: 0,
                    state,
                    low_cum_freq: 0,
                    high_cum_freq: high,
                });
                cum_freq = high;
            }
        } else {
            for idx in 0..ctx.num_stats as usize {
                if let Ok(state) = self.arena.read_state(ctx.stats_ref, idx) {
                    if !excluded[state.symbol() as usize] {
                        let low = cum_freq;
                        let high = low + state.freq() as u32;
                        cum_freq = high;
                        ranges.push(PpmdSymbolRange {
                            state_idx: idx,
                            state,
                            low_cum_freq: low,
                            high_cum_freq: high,
                        });
                    }
                }
            }
        }

        let esc_freq = if ctx.num_stats > 1 {
            let total_masked = ctx.num_stats as usize - ranges.len();
            let mut esc_entry = SeeEntry::default();
            let esc = esc_entry.make_esc_freq().max(1);
            if total_masked > 0 {
                esc + (total_masked as u32)
            } else {
                esc
            }
        } else {
            1
        };

        PpmdContextScan {
            ranges,
            cum_freq,
            esc_freq,
        }
    }

    /// Encodes a single byte symbol through PPMd context tree escape cascade.
    pub fn encode_symbol(
        &mut self,
        symbol: u8,
        rc: &mut crate::codecs::ppmd::PpmdRangeEncoder,
    ) -> Result<(), TTZipStatus> {
        let mut excluded = [false; 256];
        let mut curr_ctx_ref = self.max_context_ref;
        let mut encoded = false;

        while curr_ctx_ref != 0 {
            if let Ok(ctx) = self.arena.read_context(curr_ctx_ref) {
                let scan = self.scan_context_stats(&ctx, &excluded);
                if !scan.ranges.is_empty() {
                    let total_freq = scan.cum_freq + scan.esc_freq;
                    let mut found = false;
                    for range in &scan.ranges {
                        if range.state.symbol() == symbol {
                            rc.encode(range.low_cum_freq, range.high_cum_freq, total_freq);
                            encoded = true;
                            found = true;
                            break;
                        }
                    }
                    if found {
                        break;
                    }
                    rc.encode(scan.cum_freq, total_freq, total_freq);
                    for range in &scan.ranges {
                        excluded[range.state.symbol() as usize] = true;
                    }
                }
                curr_ctx_ref = ctx.suffix_ref;
            } else {
                break;
            }
        }

        if !encoded {
            let mut unexcluded = Vec::with_capacity(256);
            for s in 0..=255u8 {
                if !excluded[s as usize] {
                    unexcluded.push(s);
                }
            }
            if let Some(pos) = unexcluded.iter().position(|&s| s == symbol) {
                let total = unexcluded.len() as u32;
                rc.encode(pos as u32, (pos + 1) as u32, total);
            }
        }

        self.update_model_on_symbol(symbol)?;
        Ok(())
    }

    /// Decodes a single byte symbol through PPMd context tree escape cascade.
    pub fn decode_symbol(
        &mut self,
        rc: &mut crate::codecs::ppmd::PpmdRangeDecoder,
    ) -> Result<u8, TTZipStatus> {
        let mut excluded = [false; 256];
        let mut curr_ctx_ref = self.max_context_ref;
        let mut decoded_symbol: Option<u8> = None;

        while curr_ctx_ref != 0 {
            if let Ok(ctx) = self.arena.read_context(curr_ctx_ref) {
                let scan = self.scan_context_stats(&ctx, &excluded);
                if !scan.ranges.is_empty() {
                    let total_freq = scan.cum_freq + scan.esc_freq;
                    let count = rc.get_threshold(total_freq)?;
                    if count < scan.cum_freq {
                        for range in &scan.ranges {
                            if count >= range.low_cum_freq && count < range.high_cum_freq {
                                rc.decode(range.low_cum_freq, range.high_cum_freq, total_freq)?;
                                decoded_symbol = Some(range.state.symbol());
                                break;
                            }
                        }
                        if decoded_symbol.is_some() {
                            break;
                        }
                    } else {
                        rc.decode(scan.cum_freq, total_freq, total_freq)?;
                        for range in &scan.ranges {
                            excluded[range.state.symbol() as usize] = true;
                        }
                    }
                }
                curr_ctx_ref = ctx.suffix_ref;
            } else {
                break;
            }
        }

        let symbol = if let Some(sym) = decoded_symbol {
            sym
        } else {
            let mut unexcluded = Vec::with_capacity(256);
            for s in 0..=255u8 {
                if !excluded[s as usize] {
                    unexcluded.push(s);
                }
            }
            if unexcluded.is_empty() {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let total = unexcluded.len() as u32;
            let threshold = rc.get_threshold(total)?;
            let idx = threshold.min(total - 1) as usize;
            rc.decode(idx as u32, (idx + 1) as u32, total)?;
            unexcluded[idx]
        };

        self.update_model_on_symbol(symbol)?;
        Ok(symbol)
    }

    fn update_model_on_symbol(&mut self, symbol: u8) -> Result<(), TTZipStatus> {
        let p_ref = self.max_context_ref;
        let alloc_res = (|| -> Result<u32, TTZipStatus> {
            let c_ref = self.arena.alloc_context()?;
            let s_ref = match self.arena.alloc_units_for_states(2) {
                Ok(s) => s,
                Err(e) => {
                    self.arena.free_context(c_ref);
                    return Err(e);
                }
            };
            self.arena.write_state(s_ref, 0, &PpmdState::new(symbol, 2, 0))?;
            self.arena.write_state(s_ref, 1, &PpmdState::new(symbol.wrapping_add(1), 1, 0))?;
            self.arena.write_context(c_ref, &PpmdContext::new_full(2, 3, s_ref, p_ref))?;
            if p_ref > 0 {
                if let Ok(mut p_ctx) = self.arena.read_context(p_ref) {
                    if p_ctx.num_stats == 1 {
                        let mut st = p_ctx.one_state();
                        st.set_successor_ref(c_ref);
                        p_ctx.set_one_state(&st);
                        let _ = self.arena.write_context(p_ref, &p_ctx);
                    } else if p_ctx.num_stats > 1 {
                        if let Ok(mut st) = self.arena.read_state(p_ctx.stats_ref, 0) {
                            st.set_successor_ref(c_ref);
                            let _ = self.arena.write_state(p_ctx.stats_ref, 0, &st);
                        }
                    }
                }
            }
            Ok(c_ref)
        })();

        match alloc_res {
            Ok(c_ref) => {
                self.max_context_ref = c_ref;
            }
            Err(TTZipStatus::ErrOutOfMemory) => match self.arena.restore_method {
                PpmdRestoreMethod::Restart => {
                    self.arena.restart_model()?;
                    self.max_context_ref = self.arena.root_context_ref;
                    self.see.reset();
                    self.restart_count += 1;
                }
                PpmdRestoreMethod::CutOff => {
                    self.total_freed_by_cutoff += self
                        .arena
                        .cutoff_prune(self.arena.root_context_ref, self.max_order)?;
                    self.cutoff_count += 1;
                    self.max_context_ref = self.arena.root_context_ref;
                }
            },
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Compresses a slice of bytes into an owned byte vector using this model.
    pub fn compress(&mut self, src: &[u8]) -> Result<Vec<u8>, TTZipStatus> {
        let mut rc = crate::codecs::ppmd::PpmdRangeEncoder::new();
        for &b in src {
            self.encode_symbol(b, &mut rc)?;
        }
        Ok(rc.finish())
    }

    /// Decompresses an exact count of bytes into an owned byte vector using this model.
    pub fn decompress(&mut self, src: &[u8], dst_len: usize) -> Result<Vec<u8>, TTZipStatus> {
        let mut rc = crate::codecs::ppmd::PpmdRangeDecoder::new(src)?;
        let mut dst = Vec::with_capacity(dst_len);
        for _ in 0..dst_len {
            dst.push(self.decode_symbol(&mut rc)?);
        }
        Ok(dst)
    }
}
