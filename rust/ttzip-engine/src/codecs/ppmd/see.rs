// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! SEE (Secondary Escape Estimation) for PPMd.
//!
//! Provides the 16-bin quantization estimator to adaptively estimate the escape
//! probability and eliminate order-fallback entropy penalties.

use super::models::{PPMD_PERIOD_BITS, SEE_NUM_BINS, SEE_NUM_CLASSES};

/// Quantized probability entry for secondary escape estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeeEntry {
    pub summ: u16,
    pub shift: u8,
    pub count: u8,
}

impl Default for SeeEntry {
    fn default() -> Self {
        Self {
            summ: 0,
            shift: PPMD_PERIOD_BITS.saturating_sub(4),
            count: 4,
        }
    }
}

impl SeeEntry {
    #[inline]
    pub const fn new(summ: u16, shift: u8, count: u8) -> Self {
        Self { summ, shift, count }
    }

    #[inline]
    pub fn make_esc_freq(&mut self) -> u32 {
        let r = (self.summ >> self.shift) as u32;
        self.summ = self.summ.wrapping_sub(r as u16);
        if r == 0 {
            1
        } else {
            r
        }
    }

    #[inline]
    pub fn update(&mut self) {
        if self.shift < PPMD_PERIOD_BITS {
            self.count = self.count.saturating_sub(1);
            if self.count == 0 {
                self.summ = self.summ.wrapping_shl(1);
                self.count = (3 << self.shift) as u8;
                self.shift += 1;
            }
        }
    }
}

/// SEE Probability Estimator maintaining 16 quantization bins across context classes.
#[derive(Debug, Clone)]
pub struct SeeEstimator {
    pub table: [[SeeEntry; SEE_NUM_BINS]; SEE_NUM_CLASSES],
    pub dummy: SeeEntry,
}

impl Default for SeeEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl SeeEstimator {
    pub fn new() -> Self {
        let mut est = Self {
            table: [[SeeEntry::default(); SEE_NUM_BINS]; SEE_NUM_CLASSES],
            dummy: SeeEntry::new(0, PPMD_PERIOD_BITS, 64),
        };
        est.reset();
        est
    }

    pub fn reset(&mut self) {
        for i in 0..SEE_NUM_CLASSES {
            for k in 0..SEE_NUM_BINS {
                let shift = PPMD_PERIOD_BITS - 4;
                let summ = (5 * i as u16 + 10) << shift;
                self.table[i][k] = SeeEntry::new(summ, shift, 4);
            }
        }
        self.dummy = SeeEntry::new(0, PPMD_PERIOD_BITS, 64);
    }

    #[inline]
    pub fn quantize_bin(
        non_masked: usize,
        suffix_diff: usize,
        summ_freq: u16,
        num_stats: u16,
        num_masked: usize,
        hi_bits_flag: u8,
    ) -> usize {
        let bit0 = (non_masked < suffix_diff) as usize;
        let bit1 = ((summ_freq < 11 * num_stats) as usize) << 1;
        let bit2 = ((num_masked > non_masked) as usize) << 2;
        let bit3 = ((hi_bits_flag & 8) != 0) as usize * 8;
        (bit0 | bit1 | bit2 | bit3).min(SEE_NUM_BINS - 1)
    }

    #[inline]
    pub fn get_entry_mut(&mut self, class_idx: usize, bin_idx: usize) -> &mut SeeEntry {
        if class_idx < SEE_NUM_CLASSES && bin_idx < SEE_NUM_BINS {
            &mut self.table[class_idx][bin_idx]
        } else {
            &mut self.dummy
        }
    }
}
