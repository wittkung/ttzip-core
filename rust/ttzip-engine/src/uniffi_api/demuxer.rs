// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Media Demuxer Scaffolding.
//!
//! Exposes Matroska (MKV), WebM, and MP4 track, chapter, and embedded
//! attachment demuxing capabilities directly to Swift, Kotlin, and Python.

use super::types::TTZipError;
use crate::standards::demuxer::{
    MediaAttachment, MediaChapter, MediaDemuxSummary, MediaTrackInfo, MediaTrackType,
};

/// Type classification for a media container track exposed to Swift.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIMediaTrackType {
    Audio,
    Video,
    Subtitle,
}

impl From<MediaTrackType> for UniFFIMediaTrackType {
    fn from(t: MediaTrackType) -> Self {
        match t { MediaTrackType::Audio => Self::Audio, MediaTrackType::Video => Self::Video, MediaTrackType::Subtitle => Self::Subtitle }
    }
}
impl From<UniFFIMediaTrackType> for MediaTrackType {
    fn from(t: UniFFIMediaTrackType) -> Self {
        match t { UniFFIMediaTrackType::Audio => Self::Audio, UniFFIMediaTrackType::Video => Self::Video, UniFFIMediaTrackType::Subtitle => Self::Subtitle }
    }
}

/// Metadata describing a specific elementary stream/track inside a media container.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIMediaTrackInfo {
    pub track_id: u32,
    pub track_type: UniFFIMediaTrackType,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl From<MediaTrackInfo> for UniFFIMediaTrackInfo {
    fn from(t: MediaTrackInfo) -> Self {
        Self { track_id: t.track_id, track_type: t.track_type.into(), codec: t.codec, language: t.language, title: t.title, is_default: t.is_default, channels: t.channels, sample_rate: t.sample_rate, width: t.width, height: t.height }
    }
}
impl From<UniFFIMediaTrackInfo> for MediaTrackInfo {
    fn from(t: UniFFIMediaTrackInfo) -> Self {
        Self { track_id: t.track_id, track_type: t.track_type.into(), codec: t.codec, language: t.language, title: t.title, is_default: t.is_default, channels: t.channels, sample_rate: t.sample_rate, width: t.width, height: t.height }
    }
}

/// A chapter entry denoting a structured segment within the media timeline.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIMediaChapter {
    pub start_time_ms: u64,
    pub end_time_ms: Option<u64>,
    pub title: String,
}

impl From<MediaChapter> for UniFFIMediaChapter {
    fn from(c: MediaChapter) -> Self {
        Self { start_time_ms: c.start_time_ms, end_time_ms: c.end_time_ms, title: c.title }
    }
}
impl From<UniFFIMediaChapter> for MediaChapter {
    fn from(c: UniFFIMediaChapter) -> Self {
        Self { start_time_ms: c.start_time_ms, end_time_ms: c.end_time_ms, title: c.title }
    }
}

/// An embedded binary attachment (such as cover art, fonts, or poster images).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIMediaAttachment {
    pub file_name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

impl From<MediaAttachment> for UniFFIMediaAttachment {
    fn from(a: MediaAttachment) -> Self {
        Self { file_name: a.file_name, mime_type: a.mime_type, data: a.data }
    }
}
impl From<UniFFIMediaAttachment> for MediaAttachment {
    fn from(a: UniFFIMediaAttachment) -> Self {
        Self { file_name: a.file_name, mime_type: a.mime_type, data: a.data }
    }
}

/// Comprehensive summary of a demuxed media container.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIMediaDemuxSummary {
    pub container_format: String,
    pub duration_ms: Option<u64>,
    pub title: Option<String>,
    pub tracks: Vec<UniFFIMediaTrackInfo>,
    pub chapters: Vec<UniFFIMediaChapter>,
    pub attachments: Vec<UniFFIMediaAttachment>,
}

impl From<MediaDemuxSummary> for UniFFIMediaDemuxSummary {
    fn from(s: MediaDemuxSummary) -> Self {
        Self { container_format: s.container_format, duration_ms: s.duration_ms, title: s.title, tracks: s.tracks.into_iter().map(Into::into).collect(), chapters: s.chapters.into_iter().map(Into::into).collect(), attachments: s.attachments.into_iter().map(Into::into).collect() }
    }
}
impl From<UniFFIMediaDemuxSummary> for MediaDemuxSummary {
    fn from(s: UniFFIMediaDemuxSummary) -> Self {
        Self { container_format: s.container_format, duration_ms: s.duration_ms, title: s.title, tracks: s.tracks.into_iter().map(Into::into).collect(), chapters: s.chapters.into_iter().map(Into::into).collect(), attachments: s.attachments.into_iter().map(Into::into).collect() }
    }
}

/// Demuxes tracks, chapters, and embedded attachments from an in-memory media buffer.
#[uniffi::export]
pub fn demux_media_tracks(data: Vec<u8>) -> Result<UniFFIMediaDemuxSummary, TTZipError> {
    crate::standards::demuxer::demux_media_tracks_from_slice(&data)
        .map(Into::into)
        .map_err(|status| match status {
            crate::types::TTZipStatus::ErrInvalidParam => TTZipError::IoError {
                message: "Input data too short or empty media buffer".to_string(),
            },
            crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
                details: "Unrecognized or corrupted media container header".to_string(),
                offset: 0,
            },
            other => TTZipError::EngineError { code: other as i32 },
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_demux_short_buffer() {
        let short_data = vec![0u8; 4];
        let res = demux_media_tracks(short_data);
        assert!(res.is_err());
    }

    #[test]
    fn test_uniffi_demux_corrupted_header() {
        let dummy = vec![0xFFu8; 32];
        let res = demux_media_tracks(dummy);
        assert!(res.is_err());
    }
}
