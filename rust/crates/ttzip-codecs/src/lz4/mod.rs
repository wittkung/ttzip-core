// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Safe, high-performance LZ4 block and frame codecs backed by native C `liblz4`.

pub mod block;
pub mod constants;
pub mod frame;

pub use block::*;
pub use constants::*;
pub use frame::*;
