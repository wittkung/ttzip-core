// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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
