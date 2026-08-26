// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Media container track, chapter, and embedded subtitle demuxer subsystem.
//!
//! Provides zero-copy parsing of Matroska (MKV), WebM, and ISO BMFF (MP4/MOV/M4A)
//! containers for multi-track extraction, chapter navigation, and font/cover extraction.

#![forbid(unsafe_code)]

pub mod mkv;
pub mod mp4;
pub mod types;

#[cfg(test)]
mod tests;

use crate::types::status::TTZipStatus;
pub use mkv::parse_mkv_demux;
pub use mp4::parse_mp4_demux;
pub use types::{
    MediaAttachment, MediaChapter, MediaDemuxSummary, MediaTrackInfo, MediaTrackType,
};

/// EBML container header signature bytes.
const EBML_MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

/// Demuxes tracks, chapters, and embedded attachments from an in-memory media buffer.
///
/// Automatically probes and inspects container format signatures (MKV, WebM, MP4, MOV).
///
/// # Errors
/// Returns `TTZipStatus::ErrInvalidParam` if buffer is empty or too short.
/// Returns `TTZipStatus::ErrCorruptHeader` if header is not recognized or corrupted.
pub fn demux_media_tracks_from_slice(data: &[u8]) -> Result<MediaDemuxSummary, TTZipStatus> {
    if data.len() < 8 {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    if data.starts_with(&EBML_MAGIC) {
        parse_mkv_demux(data)
    } else if &data[4..8] == b"ftyp"
        || &data[4..8] == b"moov"
        || &data[4..8] == b"mdat"
        || &data[4..8] == b"free"
        || &data[4..8] == b"skip"
        || &data[4..8] == b"wide"
    {
        parse_mp4_demux(data)
    } else {
        Err(TTZipStatus::ErrCorruptHeader)
    }
}
