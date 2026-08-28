// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Solid Stream Decoder and Selective Extraction Engine.

pub mod archive;
pub mod payload;
pub mod stream;

pub use archive::SevenZArchive;
pub use payload::{
    decode_7z_folder_streaming, decode_7z_solid_payload, decode_7z_solid_streaming,
    extract_single_entry_bounded, get_current_rss_bytes,
};
pub use stream::{extract_entry_bytes_stream, extract_entry_bytes_stream_bounded};
