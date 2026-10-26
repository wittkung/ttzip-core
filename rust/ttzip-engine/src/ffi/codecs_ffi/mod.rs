// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! C-ABI FFI exports for single-format codecs.
//!
//! Every exported entry point contains a `std::panic::catch_unwind` exception barrier
//! to guarantee that internal panics never unwind across foreign language boundaries.

mod brotli;
mod bzip2;
mod deflate;
mod fast_blocks;
mod lzfse;
mod lzma2;
mod snappy;
mod zstd;

pub use brotli::*;
pub use bzip2::*;
pub use deflate::*;
pub use fast_blocks::*;
pub use lzfse::*;
pub use lzma2::*;
pub use snappy::*;
pub use zstd::*;
