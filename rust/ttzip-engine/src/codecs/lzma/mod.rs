// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance LZMA1 / LZMA2 codec microkernel.
//!
//! Exposes the pure Rust branchless range coder microkernel, 12-state finite state machine,
//! 3D probability modeling tables, and liblzma RAII Alone decoder.

pub mod alone;
pub mod range_coder;
pub mod state_machine;

pub use alone::{lzma1_decompress, LzmaAloneDecoder};
pub use range_coder::{
    RangeCoderError, RangeDecoder, RangeEncoder, BIT_MODEL_TOTAL, NUM_BIT_MODEL_TOTAL_BITS,
    NUM_MOVE_BITS, PROB_INIT_VAL, TOP_VALUE,
};
pub use state_machine::{
    LenCoderProbs, LiteralProperties, LzmaProbTable, LzmaState, State0, State1, State10, State11,
    State2, State3, State4, State5, State6, State7, State8, State9, ALIGN_TABLE_SIZE,
    END_POS_MODEL_INDEX, LITERAL_SUB_TABLE_SIZE, NUM_ALIGN_BITS, NUM_FULL_DISTANCES,
    NUM_LEN_TO_POS_STATES, NUM_POS_DECODERS, NUM_POS_SLOTS, NUM_POS_STATES_MAX, NUM_STATES,
};
