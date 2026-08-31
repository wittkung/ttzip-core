// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Universal streaming filter trait definitions.

use std::io::Read;
use super::kinds::FilterKind;

/// Universal streaming filter decoding trait.
pub trait StreamFilter: Read + Send {
    /// Returns the filter classification kind.
    fn filter_kind(&self) -> FilterKind;

    /// Returns total raw compressed/encoded bytes consumed by this filter.
    fn bytes_consumed(&self) -> u64;

    /// Returns total decoded/uncompressed bytes produced by this filter.
    fn bytes_produced(&self) -> u64;
}
