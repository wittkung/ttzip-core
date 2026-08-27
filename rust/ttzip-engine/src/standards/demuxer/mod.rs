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
pub use mkv::{demux_mkv_two_pass, parse_mkv_demux};
pub use mp4::{demux_mp4_two_pass, parse_mp4_demux};
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
    demux_media_tracks_two_pass(data, None)
}

/// Performs zero-copy two-pass demuxing using file head and optional tail slices.
///
/// Enables instant probing of multi-gigabyte media containers where indexing structures
/// (such as MP4 `moov` or MKV `SeekHead` / `Cues` / `Chapters` / `Attachments`) are placed
/// at the end of the file.
///
/// # Errors
/// Returns `TTZipStatus::ErrInvalidParam` if head is empty or too short.
/// Returns `TTZipStatus::ErrCorruptHeader` if header is unrecognized or corrupted.
pub fn demux_media_tracks_two_pass(
    head: &[u8],
    tail: Option<&[u8]>,
) -> Result<MediaDemuxSummary, TTZipStatus> {
    if head.len() < 4 {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    if head.starts_with(&EBML_MAGIC) {
        demux_mkv_two_pass(head, tail)
    } else if head.len() >= 8
        && (&head[4..8] == b"ftyp"
            || &head[4..8] == b"moov"
            || &head[4..8] == b"mdat"
            || &head[4..8] == b"free"
            || &head[4..8] == b"skip"
            || &head[4..8] == b"wide")
    {
        demux_mp4_two_pass(head, tail)
    } else {
        Err(TTZipStatus::ErrCorruptHeader)
    }
}
