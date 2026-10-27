// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Solid Stream Decoder and Selective Extraction Engine.

pub mod archive;
pub(crate) mod bcj2;
#[cfg(test)]
mod chain_regression;
pub(crate) mod codec_pump;
pub mod mach_rss;
pub mod payload;
pub mod solid_stream;
pub mod stream;

pub use archive::{SevenZArchive, SevenZReader};
pub use mach_rss::get_current_rss_bytes;
pub use payload::{
    decode_7z_folder_streaming, decode_7z_solid_payload, decode_7z_solid_streaming,
    extract_single_entry_bounded,
};
pub use solid_stream::{
    SolidEarlyExitExtractor, SolidExtractionStats, SolidFileRange, SolidFolderIndex,
    SolidFolderTable, SOLID_MICRO_BUFFER_CHUNK_SIZE,
};
pub use stream::{extract_entry_bytes_stream, extract_entry_bytes_stream_bounded};
