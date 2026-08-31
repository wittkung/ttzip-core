// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Branch Conversion (BCJ & BCJ2) bytecode pre-filtering codecs.
//!
//! Provides x86, ARM64 (BL & ADRP), and 4-stream BCJ2 executable branch address
//! normalization filters for improving entropy coding and LZMA/LZMA2 compression ratios.

pub mod bcj2;
pub mod bcj2_stream;
pub mod bcj_arm64;
pub mod bcj_x86;

pub use bcj2::{
    decode_bcj2, encode_bcj2, Bcj2Decoder, Bcj2Encoder, Bcj2Streams, NUM_BCJ2_PROBS,
    STREAM_CALL, STREAM_JUMP, STREAM_MAIN, STREAM_RC,
};
pub use bcj2_stream::{
    decode_bcj2_stream, Bcj2ArbitratorStatus, Bcj2RangeState, Bcj2State, Bcj2StreamArbitrator,
    Bcj2StreamId, Bcj2StreamReader, BCJ2_STREAM_BUFFER_SIZE,
};
pub use bcj_arm64::{arm64_decode, arm64_encode, BcjArm64};
pub use bcj_x86::{x86_decode, x86_encode, BcjX86};

/// Common trait for single-stream in-place branch filters (e.g. ARM64 BL/ADRP, x86 CALL/JMP).
pub trait BranchFilter: Send + Sync {
    /// Normalizes relative branch targets to absolute addresses in-place.
    ///
    /// `data`: mutable executable slice.
    /// `ip`: starting instruction pointer offset (typically 0).
    /// Returns the number of bytes processed.
    fn encode(&self, data: &mut [u8], ip: u32) -> usize;

    /// Denormalizes absolute addresses back to relative branch displacements in-place.
    ///
    /// `data`: mutable filtered slice.
    /// `ip`: starting instruction pointer offset (typically 0).
    /// Returns the number of bytes processed.
    fn decode(&self, data: &mut [u8], ip: u32) -> usize;
}

/// Common trait for multi-stream branch filters (e.g. BCJ2 4-Stream).
pub trait StreamFilter: Send + Sync {
    /// Number of input streams required for decoding.
    fn num_input_streams(&self) -> usize;

    /// Number of output streams produced during encoding.
    fn num_output_streams(&self) -> usize;
}
