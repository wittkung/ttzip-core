// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 12-State Finite State Machine and 3-Dimensional Probability Model for LZMA/LZMA2.
//!
//! Provides the canonical 12-state transition engine (`LzmaState`) and the
//! multi-dimensional probability table (`LzmaProbTable`) indexed by `pos_state (pb)`,
//! `state (12)`, and `literal_context (lc, lp)`.

use crate::codecs::lzma::range_coder::PROB_INIT_VAL;
use crate::types::TTZipStatus;

/// Number of primary states in the LZMA finite state machine.
pub const NUM_STATES: usize = 12;

/// Maximum number of position states (2^4 = 16).
pub const NUM_POS_STATES_MAX: usize = 16;

/// Number of length-to-position context states.
pub const NUM_LEN_TO_POS_STATES: usize = 4;

/// Number of slot bits for position encoding (64 slots).
pub const NUM_POS_SLOTS: usize = 64;

/// Number of align bits (4 bits = 16 alignment slots).
pub const NUM_ALIGN_BITS: usize = 4;
/// Size of the position alignment table.
pub const ALIGN_TABLE_SIZE: usize = 1 << NUM_ALIGN_BITS;

/// Full distance decoding context count.
pub const NUM_FULL_DISTANCES: usize = 128;
/// End position model index.
pub const END_POS_MODEL_INDEX: usize = 14;
/// Size of position decoders array (128 - 14 = 114).
pub const NUM_POS_DECODERS: usize = NUM_FULL_DISTANCES - END_POS_MODEL_INDEX;

/// Size of a single literal context sub-table (0x300 = 768 entries).
pub const LITERAL_SUB_TABLE_SIZE: usize = 0x300;

/// 12-State Finite State Machine for LZMA/LZMA2 sequence tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum LzmaState {
    /// State 0: Literal after Literal
    LitLit = 0,
    /// State 1: Match after Literal
    MatchLit = 1,
    /// State 2: Rep after Literal
    RepLit = 2,
    /// State 3: ShortRep after Literal
    ShortRepLit = 3,
    /// State 4: Match after Match
    MatchMatch = 4,
    /// State 5: Rep after Match
    RepMatch = 5,
    /// State 6: ShortRep after Match
    ShortRepMatch = 6,
    /// State 7: Literal after Match
    LitMatch = 7,
    /// State 8: Literal after Rep
    LitRep = 8,
    /// State 9: Literal after ShortRep
    LitShortRep = 9,
    /// State 10: Literal after Match & Literal
    LitMatchLit = 10,
    /// State 11: Literal after Rep & Literal
    LitRepLit = 11,
}

// Aliases for State0 ~ State11 numeric naming
pub use LzmaState::LitLit as State0;
pub use LzmaState::MatchLit as State1;
pub use LzmaState::RepLit as State2;
pub use LzmaState::ShortRepLit as State3;
pub use LzmaState::MatchMatch as State4;
pub use LzmaState::RepMatch as State5;
pub use LzmaState::ShortRepMatch as State6;
pub use LzmaState::LitMatch as State7;
pub use LzmaState::LitRep as State8;
pub use LzmaState::LitShortRep as State9;
pub use LzmaState::LitMatchLit as State10;
pub use LzmaState::LitRepLit as State11;

impl LzmaState {
    /// Total number of states (12).
    pub const COUNT: usize = NUM_STATES;

    /// Attempts to construct an `LzmaState` from a raw numeric `u8` value.
    #[inline(always)]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::LitLit),
            1 => Some(Self::MatchLit),
            2 => Some(Self::RepLit),
            3 => Some(Self::ShortRepLit),
            4 => Some(Self::MatchMatch),
            5 => Some(Self::RepMatch),
            6 => Some(Self::ShortRepMatch),
            7 => Some(Self::LitMatch),
            8 => Some(Self::LitRep),
            9 => Some(Self::LitShortRep),
            10 => Some(Self::LitMatchLit),
            11 => Some(Self::LitRepLit),
            _ => None,
        }
    }

    /// Returns the zero-based numeric index (0..=11) of this state.
    #[inline(always)]
    pub const fn as_usize(self) -> usize {
        self as usize
    }

    /// Returns the raw `u8` representation of this state.
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns `true` if the previous operation was a literal symbol (state < 7).
    ///
    /// In LZMA decoding, `is_literal() == true` instructs the decoder to use pure literal bit-tree,
    /// while `is_literal() == false` (states 7..=11) requires match-byte guided literal decoding.
    #[inline(always)]
    pub const fn is_literal(self) -> bool {
        (self as u8) < 7
    }

    /// Updates the state after emitting a literal byte.
    ///
    /// Mathematical mapping: `(state < 4) ? 0 : ((state < 10) ? (state - 3) : (state - 6))`
    #[inline(always)]
    #[must_use]
    pub const fn update_literal(self) -> Self {
        match self {
            Self::LitLit | Self::MatchLit | Self::RepLit | Self::ShortRepLit => Self::LitLit,
            Self::MatchMatch => Self::MatchLit,
            Self::RepMatch => Self::RepLit,
            Self::ShortRepMatch => Self::ShortRepLit,
            Self::LitMatch => Self::MatchMatch,
            Self::LitRep => Self::RepMatch,
            Self::LitShortRep => Self::ShortRepMatch,
            Self::LitMatchLit => Self::MatchMatch,
            Self::LitRepLit => Self::RepMatch,
        }
    }

    /// Updates the state after emitting a standard LZMA match.
    ///
    /// Mathematical mapping: `(state < 7) ? 7 : 10`
    #[inline(always)]
    #[must_use]
    pub const fn update_match(self) -> Self {
        if (self as u8) < 7 {
            Self::LitMatch
        } else {
            Self::LitMatchLit
        }
    }

    /// Updates the state after emitting a long repeat match (Rep0, Rep1, Rep2, Rep3).
    ///
    /// Mathematical mapping: `(state < 7) ? 8 : 11`
    #[inline(always)]
    #[must_use]
    pub const fn update_rep(self) -> Self {
        if (self as u8) < 7 {
            Self::LitRep
        } else {
            Self::LitRepLit
        }
    }

    /// Updates the state after emitting a 1-byte short repeat match.
    ///
    /// Mathematical mapping: `(state < 7) ? 9 : 11`
    #[inline(always)]
    #[must_use]
    pub const fn update_short_rep(self) -> Self {
        if (self as u8) < 7 {
            Self::LitShortRep
        } else {
            Self::LitRepLit
        }
    }
}

impl Default for LzmaState {
    #[inline(always)]
    fn default() -> Self {
        Self::LitLit
    }
}

impl std::fmt::Display for LzmaState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State{}({:?})", self.as_u8(), self)
    }
}

/// LZMA Literal and Position Model Properties (`lc`, `lp`, `pb`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralProperties {
    /// Literal context bits (0..=8, default 3).
    pub lc: u32,
    /// Literal position bits (0..=4, default 0).
    pub lp: u32,
    /// Position state bits (0..=4, default 2).
    pub pb: u32,
}

impl Default for LiteralProperties {
    #[inline]
    fn default() -> Self {
        Self {
            lc: 3,
            lp: 0,
            pb: 2,
        }
    }
}

impl LiteralProperties {
    /// Creates and validates a new `LiteralProperties` instance.
    ///
    /// # Errors
    /// Returns `TTZipStatus::ErrCorruptHeader` if properties violate LZMA specification limits
    /// (`lc <= 8`, `lp <= 4`, `pb <= 4`, `lc + lp <= 12`).
    pub fn new(lc: u32, lp: u32, pb: u32) -> Result<Self, TTZipStatus> {
        if lc > 8 || lp > 4 || pb > 4 || (lc + lp) > 12 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(Self { lc, lp, pb })
    }

    /// Parses LZMA 1-byte packed property `d = (pb * 5 + lp) * 9 + lc`.
    pub fn from_byte(b: u8) -> Result<Self, TTZipStatus> {
        let mut val = b as u32;
        if val >= (9 * 5 * 5) {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let lc = val % 9;
        val /= 9;
        let lp = val % 5;
        let pb = val / 5;
        Self::new(lc, lp, pb)
    }

    /// Computes the 1-byte packed property byte.
    #[inline]
    pub const fn to_byte(&self) -> u8 {
        ((self.pb * 5 + self.lp) * 9 + self.lc) as u8
    }

    /// Computes the position state index (`0..16`) for a given uncompressed byte position.
    #[inline(always)]
    pub const fn pos_state(&self, pos: usize) -> usize {
        pos & ((1 << self.pb) - 1)
    }

    /// Computes the literal context sub-table index for a given position and previous byte.
    #[inline(always)]
    pub const fn literal_context_index(&self, pos: usize, prev_byte: u8) -> usize {
        let pos_masked = pos & ((1 << self.lp) - 1);
        let prev_shifted = (prev_byte as usize) >> (8 - self.lc);
        (pos_masked << self.lc) + prev_shifted
    }

    /// Returns the total number of literal contexts (`1 << (lc + lp)`).
    #[inline(always)]
    pub const fn num_literal_contexts(&self) -> usize {
        1 << (self.lc + self.lp)
    }

    /// Returns the total length of the flattened literal probabilities array.
    #[inline(always)]
    pub const fn literal_probs_len(&self) -> usize {
        LITERAL_SUB_TABLE_SIZE << (self.lc + self.lp)
    }
}

/// Length coder probability sub-tree model.
#[derive(Debug, Clone)]
pub struct LenCoderProbs {
    /// Choice bit 1: low vs (mid or high).
    pub choice1: u16,
    /// Choice bit 2: mid vs high.
    pub choice2: u16,
    /// Low length bit trees (8 entries each for 16 pos states).
    pub low: [[u16; 8]; NUM_POS_STATES_MAX],
    /// Mid length bit trees (8 entries each for 16 pos states).
    pub mid: [[u16; 8]; NUM_POS_STATES_MAX],
    /// High length bit tree (256 entries).
    pub high: [u16; 256],
}

impl Default for LenCoderProbs {
    fn default() -> Self {
        Self::new()
    }
}

impl LenCoderProbs {
    /// Creates a new length coder probability model initialized to `PROB_INIT_VAL`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            choice1: PROB_INIT_VAL,
            choice2: PROB_INIT_VAL,
            low: [[PROB_INIT_VAL; 8]; NUM_POS_STATES_MAX],
            mid: [[PROB_INIT_VAL; 8]; NUM_POS_STATES_MAX],
            high: [PROB_INIT_VAL; 256],
        }
    }

    /// Resets all probabilities to `PROB_INIT_VAL`.
    pub fn reset(&mut self) {
        self.choice1 = PROB_INIT_VAL;
        self.choice2 = PROB_INIT_VAL;
        self.low = [[PROB_INIT_VAL; 8]; NUM_POS_STATES_MAX];
        self.mid = [[PROB_INIT_VAL; 8]; NUM_POS_STATES_MAX];
        self.high = [PROB_INIT_VAL; 256];
    }
}

/// Full 3-Dimensional Multi-Context Probability Table for LZMA / LZMA2.
#[derive(Debug, Clone)]
pub struct LzmaProbTable {
    /// Literal model configuration properties.
    pub props: LiteralProperties,
    /// Match vs Literal decision probabilities `[state][pos_state]`.
    pub is_match: [[u16; NUM_POS_STATES_MAX]; NUM_STATES],
    /// Repetition match decision probabilities `[state]`.
    pub is_rep: [u16; NUM_STATES],
    /// Rep0 match decision probabilities `[state]`.
    pub is_rep_g0: [u16; NUM_STATES],
    /// Rep1 match decision probabilities `[state]`.
    pub is_rep_g1: [u16; NUM_STATES],
    /// Rep2 match decision probabilities `[state]`.
    pub is_rep_g2: [u16; NUM_STATES],
    /// Long Rep0 match decision probabilities `[state][pos_state]`.
    pub is_rep0_long: [[u16; NUM_POS_STATES_MAX]; NUM_STATES],
    /// Position slot probabilities `[len_to_pos_state][slot]`.
    pub pos_slot: [[u16; NUM_POS_SLOTS]; NUM_LEN_TO_POS_STATES],
    /// Position direct decoders table (114 entries).
    pub pos_decoders: [u16; NUM_POS_DECODERS],
    /// Position alignment table (16 entries).
    pub pos_align: [u16; ALIGN_TABLE_SIZE],
    /// Match length probability model.
    pub len_coder: LenCoderProbs,
    /// Repetition match length probability model.
    pub rep_len_coder: LenCoderProbs,
    /// Dynamic literal probability contexts table.
    pub literal_probs: Vec<u16>,
}

impl LzmaProbTable {
    /// Creates a new `LzmaProbTable` initialized with given literal properties.
    #[must_use]
    pub fn new(props: LiteralProperties) -> Self {
        let lit_len = props.literal_probs_len();
        Self {
            props,
            is_match: [[PROB_INIT_VAL; NUM_POS_STATES_MAX]; NUM_STATES],
            is_rep: [PROB_INIT_VAL; NUM_STATES],
            is_rep_g0: [PROB_INIT_VAL; NUM_STATES],
            is_rep_g1: [PROB_INIT_VAL; NUM_STATES],
            is_rep_g2: [PROB_INIT_VAL; NUM_STATES],
            is_rep0_long: [[PROB_INIT_VAL; NUM_POS_STATES_MAX]; NUM_STATES],
            pos_slot: [[PROB_INIT_VAL; NUM_POS_SLOTS]; NUM_LEN_TO_POS_STATES],
            pos_decoders: [PROB_INIT_VAL; NUM_POS_DECODERS],
            pos_align: [PROB_INIT_VAL; ALIGN_TABLE_SIZE],
            len_coder: LenCoderProbs::new(),
            rep_len_coder: LenCoderProbs::new(),
            literal_probs: vec![PROB_INIT_VAL; lit_len],
        }
    }

    /// Resets all probability contexts in the table back to 50% likelihood (`PROB_INIT_VAL = 1024`).
    pub fn reset(&mut self) {
        self.is_match = [[PROB_INIT_VAL; NUM_POS_STATES_MAX]; NUM_STATES];
        self.is_rep = [PROB_INIT_VAL; NUM_STATES];
        self.is_rep_g0 = [PROB_INIT_VAL; NUM_STATES];
        self.is_rep_g1 = [PROB_INIT_VAL; NUM_STATES];
        self.is_rep_g2 = [PROB_INIT_VAL; NUM_STATES];
        self.is_rep0_long = [[PROB_INIT_VAL; NUM_POS_STATES_MAX]; NUM_STATES];
        self.pos_slot = [[PROB_INIT_VAL; NUM_POS_SLOTS]; NUM_LEN_TO_POS_STATES];
        self.pos_decoders = [PROB_INIT_VAL; NUM_POS_DECODERS];
        self.pos_align = [PROB_INIT_VAL; ALIGN_TABLE_SIZE];
        self.len_coder.reset();
        self.rep_len_coder.reset();
        self.literal_probs.fill(PROB_INIT_VAL);
    }

    /// Returns a mutable slice to the 0x300 literal probability sub-table for `(pos, prev_byte)`.
    #[inline(always)]
    pub fn literal_sub_table_mut(&mut self, pos: usize, prev_byte: u8) -> &mut [u16] {
        let ctx_idx = self.props.literal_context_index(pos, prev_byte);
        let start = ctx_idx * LITERAL_SUB_TABLE_SIZE;
        &mut self.literal_probs[start..start + LITERAL_SUB_TABLE_SIZE]
    }

    /// Returns an immutable slice to the 0x300 literal probability sub-table for `(pos, prev_byte)`.
    #[inline(always)]
    pub fn literal_sub_table(&self, pos: usize, prev_byte: u8) -> &[u16] {
        let ctx_idx = self.props.literal_context_index(pos, prev_byte);
        let start = ctx_idx * LITERAL_SUB_TABLE_SIZE;
        &self.literal_probs[start..start + LITERAL_SUB_TABLE_SIZE]
    }
}
