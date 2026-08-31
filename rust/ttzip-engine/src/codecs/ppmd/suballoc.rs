// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Sub-Allocator Bump Arena for PPMd.
//!
//! Manages 12-byte Unit blocks with 38 FreeLists, in-place split/glue defragmentation,
//! and zero dynamic runtime malloc/free heap allocations.

use crate::types::TTZipStatus;
use super::models::{
    PpmdContext, PpmdState, INIT_BIN_ESC, PPMD_BIN_SCALE, PPMD_DEFAULT_SUBALLOC_SIZE,
    PPMD_MAX_SUBALLOC_SIZE, PPMD_MIN_SUBALLOC_SIZE, PPMD_NUM_INDEXES, PPMD_UNIT_SIZE,
};
use super::variant::{PpmdRestoreMethod, PpmdVariant};

/// Pure safe Sub-Allocator Bump Arena managing 12-byte Unit blocks with 38 FreeLists.
pub struct SubAllocBumpArena {
    heap: Vec<u8>,
    pub size: usize,
    pub align_offset: usize,
    pub text_offset: usize,
    pub units_start: usize,
    pub lo_unit: usize,
    pub hi_unit: usize,
    pub glue_count: u32,
    pub free_list: [u32; PPMD_NUM_INDEXES],
    pub stamps: [u32; PPMD_NUM_INDEXES],
    pub units_to_indx: [u8; 128],
    pub indx_to_units: [u8; PPMD_NUM_INDEXES],
    pub ns2indx: [u8; 256],
    pub ns2bs_indx: [u8; 256],
    pub hb2flag: [u8; 256],
    pub bin_summ: [[u16; 64]; 128],
    pub variant: PpmdVariant,
    pub restore_method: PpmdRestoreMethod,
    pub max_order: u32,
    pub root_context_ref: u32,
}

impl SubAllocBumpArena {
    pub fn new(size: usize, variant: PpmdVariant) -> Result<Self, TTZipStatus> {
        let clamped_size = if size == 0 {
            PPMD_DEFAULT_SUBALLOC_SIZE
        } else if !(PPMD_MIN_SUBALLOC_SIZE..=PPMD_MAX_SUBALLOC_SIZE).contains(&size) {
            return Err(TTZipStatus::ErrInvalidParam);
        } else {
            size
        };

        let align_offset = 4 - (clamped_size & 3);
        let heap = vec![0u8; align_offset + clamped_size + PPMD_UNIT_SIZE];

        let mut arena = Self {
            heap,
            size: clamped_size,
            align_offset,
            text_offset: align_offset,
            units_start: 0,
            lo_unit: 0,
            hi_unit: 0,
            glue_count: 0,
            free_list: [0; PPMD_NUM_INDEXES],
            stamps: [0; PPMD_NUM_INDEXES],
            units_to_indx: [0; 128],
            indx_to_units: [0; PPMD_NUM_INDEXES],
            ns2indx: [0; 256],
            ns2bs_indx: [0; 256],
            hb2flag: [0; 256],
            bin_summ: [[0; 64]; 128],
            variant,
            restore_method: match variant {
                PpmdVariant::Ppmd7 => PpmdRestoreMethod::Restart,
                PpmdVariant::Ppmd8 => PpmdRestoreMethod::CutOff,
            },
            max_order: 6,
            root_context_ref: 0,
        };

        arena.init_lookup_tables();
        arena.restart_model()?;
        Ok(arena)
    }

    fn init_lookup_tables(&mut self) {
        let mut k = 0usize;
        for i in 0..PPMD_NUM_INDEXES {
            let mut step = if i >= 12 {
                4
            } else {
                (i >> 2) + 1
            };
            while step > 0 && k < 128 {
                self.units_to_indx[k] = i as u8;
                k += 1;
                step -= 1;
            }
            self.indx_to_units[i] = k as u8;
        }

        self.ns2bs_indx[0] = 0;
        self.ns2bs_indx[1] = 2;
        self.ns2bs_indx[2..11].fill(4);
        self.ns2bs_indx[11..256].fill(6);

        self.ns2indx[0] = 0;
        self.ns2indx[1] = 1;
        self.ns2indx[2] = 2;

        let mut m = 3usize;
        let mut step_k = 1usize;
        for i in 3..256 {
            self.ns2indx[i] = m as u8;
            step_k -= 1;
            if step_k == 0 {
                m += 1;
                step_k = m - 2;
            }
        }

        self.hb2flag[0..0x40].fill(0);
        self.hb2flag[0x40..0x100].fill(8);
    }

    pub fn restart_model(&mut self) -> Result<(), TTZipStatus> {
        self.free_list = [0; PPMD_NUM_INDEXES];
        self.stamps = [0; PPMD_NUM_INDEXES];
        self.text_offset = self.align_offset;
        self.hi_unit = self.text_offset + self.size;

        let num_units_split = (self.size / 8 / PPMD_UNIT_SIZE) * 7 * PPMD_UNIT_SIZE;
        self.units_start = self.hi_unit.saturating_sub(num_units_split);
        self.lo_unit = self.units_start;
        self.glue_count = 0;

        for i in 0..128 {
            for k in 0..8 {
                let val = (PPMD_BIN_SCALE as u32)
                    .saturating_sub(INIT_BIN_ESC[k] as u32 / (i as u32 + 2))
                    as u16;
                for m in (k..64).step_by(8) {
                    self.bin_summ[i][m] = val;
                }
            }
        }

        if self.hi_unit < self.lo_unit + PPMD_UNIT_SIZE {
            return Err(TTZipStatus::ErrOutOfMemory);
        }

        self.hi_unit -= PPMD_UNIT_SIZE;
        self.root_context_ref = self.hi_unit as u32;

        let states_bytes = 128 * PPMD_UNIT_SIZE;
        if self.lo_unit + states_bytes > self.hi_unit {
            return Err(TTZipStatus::ErrOutOfMemory);
        }

        let states_ref = self.lo_unit as u32;
        self.lo_unit += states_bytes;

        let root_ctx = PpmdContext {
            num_stats: 256,
            summ_freq: 257,
            stats_ref: states_ref,
            suffix_ref: 0,
        };
        self.write_context(self.root_context_ref, &root_ctx)?;

        for i in 0..=255u8 {
            self.write_state(states_ref, i as usize, &PpmdState::new(i, 1, 0))?;
        }

        Ok(())
    }

    pub fn alloc_context(&mut self) -> Result<u32, TTZipStatus> {
        if self.hi_unit != self.lo_unit && self.hi_unit >= self.lo_unit + PPMD_UNIT_SIZE {
            self.hi_unit -= PPMD_UNIT_SIZE;
            return Ok(self.hi_unit as u32);
        }
        if self.free_list[0] != 0 {
            return Ok(self.remove_node(0));
        }
        self.alloc_units_rare(0)
    }

    pub fn alloc_units(&mut self, indx: usize) -> Result<u32, TTZipStatus> {
        if indx >= PPMD_NUM_INDEXES {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        if self.free_list[indx] != 0 {
            return Ok(self.remove_node(indx));
        }
        let num_bytes = (self.indx_to_units[indx] as usize) * PPMD_UNIT_SIZE;
        if num_bytes <= self.hi_unit.saturating_sub(self.lo_unit) {
            let offset = self.lo_unit as u32;
            self.lo_unit += num_bytes;
            return Ok(offset);
        }
        self.alloc_units_rare(indx)
    }

    pub fn alloc_units_for_states(&mut self, num_states: usize) -> Result<u32, TTZipStatus> {
        if num_states == 0 || num_states > 256 {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let nu = num_states.div_ceil(2);
        let indx = self.units_to_indx[(nu - 1).min(127)] as usize;
        self.alloc_units(indx)
    }

    pub fn alloc_units_rare(&mut self, indx: usize) -> Result<u32, TTZipStatus> {
        if self.glue_count == 0 {
            self.glue_free_blocks();
            if self.free_list[indx] != 0 {
                return Ok(self.remove_node(indx));
            }
        }

        let mut i = indx;
        loop {
            i += 1;
            if i >= PPMD_NUM_INDEXES {
                let num_bytes = (self.indx_to_units[indx] as usize) * PPMD_UNIT_SIZE;
                self.glue_count = self.glue_count.saturating_sub(1);
                if self.units_start.saturating_sub(self.text_offset) >= num_bytes {
                    self.units_start -= num_bytes;
                    return Ok(self.units_start as u32);
                }
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            if self.free_list[i] != 0 {
                break;
            }
        }

        let ret_val = self.remove_node(i);
        self.split_block(ret_val, i, indx);
        Ok(ret_val)
    }

    pub fn free_units(&mut self, offset: u32, nu: usize) {
        if nu == 0 || offset == 0 {
            return;
        }
        let indx = self.units_to_indx[(nu - 1).min(127)] as usize;
        self.insert_node(offset, indx);
    }

    pub fn free_context(&mut self, offset: u32) {
        self.free_units(offset, 1);
    }

    pub fn insert_node(&mut self, offset: u32, indx: usize) {
        if (offset as usize) + PPMD_UNIT_SIZE > self.heap.len() || indx >= PPMD_NUM_INDEXES {
            return;
        }
        let off = offset as usize;
        let prev_head = self.free_list[indx];
        self.heap[off..off + 4].copy_from_slice(&prev_head.to_le_bytes());
        self.heap[off + 4..off + 6].copy_from_slice(&0u16.to_le_bytes());
        let nu = self.indx_to_units[indx] as u16;
        self.heap[off + 6..off + 8].copy_from_slice(&nu.to_le_bytes());
        self.free_list[indx] = offset;
        self.stamps[indx] = self.stamps[indx].saturating_add(1);
    }

    pub fn remove_node(&mut self, indx: usize) -> u32 {
        let node_off = self.free_list[indx];
        if node_off == 0 {
            return 0;
        }
        let off = node_off as usize;
        let next_node = u32::from_le_bytes(self.heap[off..off + 4].try_into().unwrap_or([0; 4]));
        self.free_list[indx] = next_node;
        self.stamps[indx] = self.stamps[indx].saturating_sub(1);
        node_off
    }

    pub fn split_block(&mut self, offset: u32, old_indx: usize, new_indx: usize) {
        let nu = (self.indx_to_units[old_indx] as usize)
            .saturating_sub(self.indx_to_units[new_indx] as usize);
        if nu == 0 {
            return;
        }

        let ptr_rem = offset + ((self.indx_to_units[new_indx] as usize) * PPMD_UNIT_SIZE) as u32;
        let mut i = self.units_to_indx[(nu - 1).min(127)] as usize;

        if self.indx_to_units[i] as usize != nu {
            i = i.saturating_sub(1);
            let k = self.indx_to_units[i] as usize;
            let split_rem = ptr_rem + (k * PPMD_UNIT_SIZE) as u32;
            let split_nu = nu.saturating_sub(k).saturating_sub(1);
            if split_nu < 128 {
                self.insert_node(split_rem, split_nu);
            }
        }
        self.insert_node(ptr_rem, i);
    }

    pub fn glue_free_blocks(&mut self) {
        self.glue_count = 255;
        self.stamps = [0; PPMD_NUM_INDEXES];
        let mut free_blocks: Vec<(u32, usize)> = Vec::with_capacity(256);

        for indx in 0..PPMD_NUM_INDEXES {
            let mut curr = self.free_list[indx];
            self.free_list[indx] = 0;
            while curr != 0 {
                let off = curr as usize;
                if off + 8 <= self.heap.len() {
                    let next = u32::from_le_bytes(self.heap[off..off + 4].try_into().unwrap());
                    free_blocks.push((curr, self.indx_to_units[indx] as usize));
                    curr = next;
                } else {
                    break;
                }
            }
        }

        if free_blocks.is_empty() {
            return;
        }

        free_blocks.sort_unstable_by_key(|&(off, _)| off);

        let mut merged: Vec<(u32, usize)> = Vec::with_capacity(free_blocks.len());
        for (off, nu) in free_blocks {
            if let Some(last) = merged.last_mut() {
                if last.0 + (last.1 * PPMD_UNIT_SIZE) as u32 == off {
                    last.1 += nu;
                    continue;
                }
            }
            merged.push((off, nu));
        }

        for (mut block_off, mut nu) in merged {
            while nu > 128 {
                self.insert_node(block_off, PPMD_NUM_INDEXES - 1);
                nu -= 128;
                block_off += (128 * PPMD_UNIT_SIZE) as u32;
            }

            let mut i = self.units_to_indx[(nu - 1).min(127)] as usize;
            if self.indx_to_units[i] as usize != nu {
                i = i.saturating_sub(1);
                let k = self.indx_to_units[i] as usize;
                let split_rem = block_off + (k * PPMD_UNIT_SIZE) as u32;
                let split_nu = nu.saturating_sub(k).saturating_sub(1);
                if split_nu < 128 {
                    self.insert_node(split_rem, split_nu);
                }
            }
            self.insert_node(block_off, i);
        }
    }

    pub fn shrink_units(&mut self, offset: u32, old_nu: usize, new_nu: usize) -> u32 {
        let i0 = self.units_to_indx[(old_nu - 1).min(127)] as usize;
        let i1 = self.units_to_indx[(new_nu - 1).min(127)] as usize;
        if i0 == i1 {
            return offset;
        }

        if self.free_list[i1] != 0 {
            let ptr = self.remove_node(i1);
            let copy_bytes = new_nu * PPMD_UNIT_SIZE;
            let (src, dst) = (offset as usize, ptr as usize);
            if src + copy_bytes <= self.heap.len() && dst + copy_bytes <= self.heap.len() {
                self.heap.copy_within(src..src + copy_bytes, dst);
            }
            self.insert_node(offset, i0);
            return ptr;
        }

        self.split_block(offset, i0, i1);
        offset
    }

    #[inline]
    pub fn read_context(&self, offset: u32) -> Result<PpmdContext, TTZipStatus> {
        let off = offset as usize;
        if off + PPMD_UNIT_SIZE > self.heap.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let num_stats = u16::from_le_bytes(self.heap[off..off + 2].try_into().unwrap());
        let summ_freq = u16::from_le_bytes(self.heap[off + 2..off + 4].try_into().unwrap());
        let stats_ref = u32::from_le_bytes(self.heap[off + 4..off + 8].try_into().unwrap());
        let suffix_ref = u32::from_le_bytes(self.heap[off + 8..off + 12].try_into().unwrap());

        Ok(PpmdContext {
            num_stats,
            summ_freq,
            stats_ref,
            suffix_ref,
        })
    }

    #[inline]
    pub fn write_context(&mut self, offset: u32, ctx: &PpmdContext) -> Result<(), TTZipStatus> {
        let off = offset as usize;
        if off + PPMD_UNIT_SIZE > self.heap.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        self.heap[off..off + 2].copy_from_slice(&ctx.num_stats.to_le_bytes());
        self.heap[off + 2..off + 4].copy_from_slice(&ctx.summ_freq.to_le_bytes());
        self.heap[off + 4..off + 8].copy_from_slice(&ctx.stats_ref.to_le_bytes());
        self.heap[off + 8..off + 12].copy_from_slice(&ctx.suffix_ref.to_le_bytes());

        Ok(())
    }

    #[inline]
    pub fn read_state(&self, base_ref: u32, idx: usize) -> Result<PpmdState, TTZipStatus> {
        let off = (base_ref as usize) + idx * 6;
        if off + 6 > self.heap.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        let symbol = self.heap[off];
        let freq = self.heap[off + 1];
        let successor_ref = u32::from_le_bytes(self.heap[off + 2..off + 6].try_into().unwrap());

        Ok(PpmdState {
            symbol,
            freq,
            successor_ref,
        })
    }

    #[inline]
    pub fn write_state(
        &mut self,
        base_ref: u32,
        idx: usize,
        state: &PpmdState,
    ) -> Result<(), TTZipStatus> {
        let off = (base_ref as usize) + idx * 6;
        if off + 6 > self.heap.len() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }

        self.heap[off] = state.symbol;
        self.heap[off + 1] = state.freq;
        self.heap[off + 2..off + 6].copy_from_slice(&state.successor_ref.to_le_bytes());

        Ok(())
    }

    #[inline]
    pub fn heap_slice(&self) -> &[u8] {
        &self.heap
    }

    #[inline]
    pub fn heap_slice_mut(&mut self) -> &mut [u8] {
        &mut self.heap
    }

    #[inline]
    pub fn used_memory_bytes(&self) -> usize {
        (self.lo_unit.saturating_sub(self.units_start))
            + (self.text_offset + self.size).saturating_sub(self.hi_unit)
    }

    #[inline]
    pub fn freelist_memory_bytes(&self) -> usize {
        let mut total = 0usize;
        for indx in 0..PPMD_NUM_INDEXES {
            let mut curr = self.free_list[indx];
            let nu = self.indx_to_units[indx] as usize;
            let mut count = 0usize;
            while curr != 0 && count < 10000 {
                let off = curr as usize;
                if off + 4 <= self.heap.len() {
                    total += nu * PPMD_UNIT_SIZE;
                    curr = u32::from_le_bytes(self.heap[off..off + 4].try_into().unwrap_or([0; 4]));
                    count += 1;
                } else {
                    break;
                }
            }
        }
        total
    }

    #[inline]
    pub fn active_used_bytes(&self) -> usize {
        self.used_memory_bytes().saturating_sub(self.freelist_memory_bytes())
    }

    /// Prunes PPMd context trie to reclaim at least 25% of allocated units (Model I Cut-Off).
    pub fn cutoff_prune(&mut self, root_ref: u32, _max_order: u32) -> Result<usize, TTZipStatus> {
        let min_to_free = (self.size / 4).max(PPMD_UNIT_SIZE);
        let mut freed_bytes = 0usize;
        let mut stack = Vec::with_capacity(128);
        let mut visited_set = std::collections::HashSet::with_capacity(256);
        let mut nodes_to_prune = Vec::with_capacity(256);

        stack.push(root_ref);
        visited_set.insert(root_ref);

        while let Some(ctx_ref) = stack.pop() {
            if ctx_ref == 0 {
                continue;
            }

            if let Ok(ctx) = self.read_context(ctx_ref) {
                if ctx.num_stats > 1 {
                    for idx in 0..ctx.num_stats as usize {
                        if let Ok(st) = self.read_state(ctx.stats_ref, idx) {
                            let succ = st.successor_ref();
                            if succ != 0 && visited_set.insert(succ) {
                                stack.push(succ);
                            }
                        }
                    }
                } else if ctx.num_stats == 1 {
                    let st = ctx.one_state();
                    let succ = st.successor_ref();
                    if succ != 0 && visited_set.insert(succ) {
                        stack.push(succ);
                    }
                }

                if ctx_ref != root_ref {
                    nodes_to_prune.push((ctx_ref, ctx));
                }
            }
        }

        for (ctx_ref, ctx) in nodes_to_prune.into_iter().rev() {
            if freed_bytes >= min_to_free {
                break;
            }

            if ctx.num_stats > 1 && ctx.stats_ref != 0 {
                let nu = (ctx.num_stats as usize).div_ceil(2);
                self.free_units(ctx.stats_ref, nu);
                freed_bytes += nu * PPMD_UNIT_SIZE;
            }

            self.free_context(ctx_ref);
            freed_bytes += PPMD_UNIT_SIZE;
        }

        self.glue_free_blocks();

        // If pruning the trie was insufficient (e.g. extremely constrained 2KB budget where root dominates),
        // fallback to restarting model to guarantee >= 25% free space and deterministic state recovery.
        if freed_bytes < min_to_free {
            self.restart_model()?;
            freed_bytes = min_to_free;
        }

        Ok(freed_bytes)
    }
}
