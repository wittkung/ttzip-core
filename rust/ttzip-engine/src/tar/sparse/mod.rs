// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust TAR sparse file extent mapping, header decoding, and stream processing.

pub mod map;
pub mod writer;

pub use map::*;
pub use writer::*;

/// TAR sparse format specification version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TarSparseFormat {
    /// GNU Sparse 0.1 format (PAX extended header `GNU.sparse.map`).
    #[default]
    Gnu0_1,
    /// GNU Sparse 1.0 format (PAX `GNU.sparse.major=1`, map stored in payload).
    Gnu1_0,
}


