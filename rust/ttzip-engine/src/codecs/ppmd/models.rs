// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Compact PPMd data models and fundamental quantization constants.
//!
//! Provides the 12-byte `PpmdContext` and 6-byte `PpmdState` structures
//! strictly aligned to 12-byte Units according to 7-Zip PPMd7/PPMd8 specifications.

// MARK: - Constants & Quantization Tables

pub const PPMD_UNIT_SIZE: usize = 12;
pub const PPMD_NUM_INDEXES: usize = 38;
pub const PPMD_MIN_SUBALLOC_SIZE: usize = 2048;
pub const PPMD_MAX_SUBALLOC_SIZE: usize = 256 * 1024 * 1024;
pub const PPMD_DEFAULT_SUBALLOC_SIZE: usize = 16 * 1024 * 1024;
pub const PPMD_MAX_FREQ: u8 = 124;
pub const PPMD_PERIOD_BITS: u8 = 7;
pub const PPMD_INT_BITS: u8 = 7;
pub const PPMD_BIN_SCALE: u16 = 1 << (PPMD_INT_BITS + PPMD_PERIOD_BITS);
pub const SEE_NUM_BINS: usize = 16;
pub const SEE_NUM_CLASSES: usize = 25;

pub const INIT_BIN_ESC: [u16; 8] = [
    0x3CDD, 0x1F3F, 0x59BF, 0x48F3, 0x64A1, 0x5ABC, 0x6632, 0x6051,
];

pub const PPMD_EXP_ESCAPE: [u8; 16] = [
    25, 14, 9, 7, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 2,
];

// MARK: - Compact PPMd Data Structures (12B Context & 6B State)

/// 6-byte compact transition state in PPMd prediction graph.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmdState {
    pub symbol: u8,
    pub freq: u8,
    pub successor_ref: u32,
}

impl PpmdState {
    #[inline]
    pub const fn new(symbol: u8, freq: u8, successor_ref: u32) -> Self {
        Self {
            symbol,
            freq,
            successor_ref,
        }
    }

    #[inline]
    pub fn symbol(&self) -> u8 {
        self.symbol
    }

    #[inline]
    pub fn freq(&self) -> u8 {
        self.freq
    }

    #[inline]
    pub fn successor_ref(&self) -> u32 {
        self.successor_ref
    }

    #[inline]
    pub fn set_symbol(&mut self, symbol: u8) {
        self.symbol = symbol;
    }

    #[inline]
    pub fn set_freq(&mut self, freq: u8) {
        self.freq = freq;
    }

    #[inline]
    pub fn set_successor_ref(&mut self, successor_ref: u32) {
        self.successor_ref = successor_ref;
    }
}

/// 12-byte compact context node in PPMd prediction tree.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmdContext {
    pub num_stats: u16,
    pub summ_freq: u16,
    pub stats_ref: u32,
    pub suffix_ref: u32,
}

impl PpmdContext {
    #[inline]
    pub const fn new(suffix_ref: u32) -> Self {
        Self {
            num_stats: 0,
            summ_freq: 0,
            stats_ref: 0,
            suffix_ref,
        }
    }

    #[inline]
    pub const fn new_full(
        num_stats: u16,
        summ_freq: u16,
        stats_ref: u32,
        suffix_ref: u32,
    ) -> Self {
        Self {
            num_stats,
            summ_freq,
            stats_ref,
            suffix_ref,
        }
    }

    #[inline]
    pub fn one_state(&self) -> PpmdState {
        PpmdState::new(
            (self.summ_freq & 0xFF) as u8,
            ((self.summ_freq >> 8) & 0xFF) as u8,
            self.stats_ref,
        )
    }

    #[inline]
    pub fn set_one_state(&mut self, state: &PpmdState) {
        self.summ_freq = (state.symbol() as u16) | ((state.freq() as u16) << 8);
        self.stats_ref = state.successor_ref();
    }

    #[inline]
    pub fn is_binary(&self) -> bool {
        self.num_stats <= 1
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.suffix_ref == 0
    }
}
