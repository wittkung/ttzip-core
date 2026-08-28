// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Solid Stream selective entry decoding with Bounded Memory and Early Termination.

use super::payload::extract_single_entry_bounded;
use crate::sevenz::header::{SevenZHeaderInfo, SevenZSeekIndex};
use crate::types::TTZipStatus;

pub use super::payload::extract_single_entry_bounded as extract_entry_bytes_stream_bounded;

/// Backwards-compatible stream extraction with default unbounded budget (0 = disabled budget limit).
pub fn extract_entry_bytes_stream(
    mapped: &[u8],
    info: &SevenZHeaderInfo,
    seek_index: &SevenZSeekIndex,
    entry_idx: usize,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    extract_single_entry_bounded(mapped, info, seek_index, entry_idx, password, 0)
}
