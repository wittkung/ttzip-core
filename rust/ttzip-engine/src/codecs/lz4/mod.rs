// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance LZ4 block, frame descriptor, sequence matching, and streaming codecs.

pub mod block;
pub mod constants;
pub mod decompress;
pub mod dict;
pub mod frame;
pub mod hash;
pub mod hc_opt;
pub mod lut;
pub mod matchfinder;
pub mod partial;

pub use block::*;
pub use constants::*;
pub use decompress::{
    copy_small_offset_ptr, lz4_decompress_custom_to_vec, lz4_decompress_safe_custom, wild_copy_16,
    wild_copy_32, wild_copy_8, LZ4_FAST_LOOP_MARGIN, LZ4_MAX_TOKEN_LITERAL_LEN,
    LZ4_MAX_TOKEN_MATCH_LEN, LZ4_MIN_MATCH,
};
pub use dict::*;
pub use frame::*;
pub use hash::*;
pub use hc_opt::*;
pub use lut::*;
pub use matchfinder::*;
pub use partial::*;

