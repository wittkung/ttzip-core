// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Video Metadata, Track Topology, and Poster Extraction Module.

pub mod ebml;
pub mod isobmff;
pub mod parser;
pub mod service;
pub mod types;

#[cfg(test)]
mod tests;

pub use parser::{extract_video_cover_from_bytes, parse_video_metadata_from_bytes};
pub use service::*;
pub use types::*;
