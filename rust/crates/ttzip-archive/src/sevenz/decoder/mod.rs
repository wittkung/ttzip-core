// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! 7-Zip Solid Stream Decoder and Selective Extraction Engine.

pub mod archive;
pub mod payload;
pub mod solid_stream;
pub mod stream;

pub use archive::{SevenZArchive, SevenZReader};
pub use payload::{
    decode_7z_folder_streaming, decode_7z_solid_payload, decode_7z_solid_streaming,
    extract_single_entry_bounded, get_current_rss_bytes,
};
pub use solid_stream::{
    SolidEarlyExitExtractor, SolidExtractionStats, SolidFileRange, SolidFolderIndex,
    SolidFolderTable, SOLID_MICRO_BUFFER_CHUNK_SIZE,
};
pub use stream::{extract_entry_bytes_stream, extract_entry_bytes_stream_bounded};
