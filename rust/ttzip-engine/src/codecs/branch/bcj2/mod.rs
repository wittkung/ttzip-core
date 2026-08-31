// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 4-Stream BCJ2 (Branch Conversion v2) Architecture.
//!
//! BCJ2 separates x86 executable bytecode into four dedicated channels:
//! 1. **Stream 0 (`STREAM_MAIN`)**: Literals and opcode headers.
//! 2. **Stream 1 (`STREAM_CALL`)**: 32-bit big-endian absolute targets for CALL (0xE8) instructions.
//! 3. **Stream 2 (`STREAM_JUMP`)**: 32-bit big-endian absolute targets for JMP (0xE9) instructions.
//! 4. **Stream 3 (`STREAM_RC`)**: Status bitstream coded via 258-context binary range coding.

pub mod decoder;
pub mod encoder;
pub mod range;
pub mod range_coder;
pub mod stream;

pub use decoder::{decode_bcj2, Bcj2Decoder};
pub use encoder::{encode_bcj2, Bcj2Encoder, Bcj2Streams};
pub use range::{
    Bcj2RangeDecoder, Bcj2RangeDecoderProbs, Bcj2RangeEncoder, BIT_MODEL_TOTAL, NUM_BCJ2_PROBS,
    NUM_BIT_MODEL_TOTAL_BITS, NUM_MOVE_BITS, PROB_INIT_VAL, TOP_VALUE,
};
pub use range_coder::{RangeDecoder, RangeEncoder};
pub use stream::{Bcj2StreamArbitrator, MicroBuffer, MICRO_BUFFER_SIZE};

use super::StreamFilter;

/// Stream index for main literals and opcode markers.
pub const STREAM_MAIN: usize = 0;
/// Stream index for CALL (0xE8) absolute addresses.
pub const STREAM_CALL: usize = 1;
/// Stream index for JMP (0xE9) absolute addresses.
pub const STREAM_JUMP: usize = 2;
/// Stream index for RangeCoder status bitstream.
pub const STREAM_RC: usize = 3;
/// Total number of streams in BCJ2 topology.
pub const NUM_STREAMS: usize = 4;

impl StreamFilter for Bcj2Encoder {
    #[inline]
    fn num_input_streams(&self) -> usize {
        1
    }

    #[inline]
    fn num_output_streams(&self) -> usize {
        NUM_STREAMS
    }
}

impl StreamFilter for Bcj2Decoder {
    #[inline]
    fn num_input_streams(&self) -> usize {
        NUM_STREAMS
    }

    #[inline]
    fn num_output_streams(&self) -> usize {
        1
    }
}
