// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed domain models for media container track and metadata demuxing.

use serde::{Deserialize, Serialize};

/// Type classification for a media container track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaTrackType {
    Audio,
    Video,
    Subtitle,
}

impl MediaTrackType {
    /// Returns a human-readable string representation of the track type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Video => "Video",
            Self::Subtitle => "Subtitle",
        }
    }
}

/// Metadata describing a specific elementary stream/track inside a media container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaTrackInfo {
    pub track_id: u32,
    pub track_type: MediaTrackType,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl MediaTrackInfo {
    /// Creates a new `MediaTrackInfo` with standard defaults.
    #[must_use]
    pub fn new(track_id: u32, track_type: MediaTrackType, codec: impl Into<String>) -> Self {
        Self {
            track_id,
            track_type,
            codec: codec.into(),
            language: None,
            title: None,
            is_default: false,
            channels: None,
            sample_rate: None,
            width: None,
            height: None,
        }
    }
}

/// A chapter entry denoting a structured segment within the media timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaChapter {
    pub start_time_ms: u64,
    pub end_time_ms: Option<u64>,
    pub title: String,
}

impl MediaChapter {
    /// Creates a new `MediaChapter`.
    #[must_use]
    pub fn new(start_time_ms: u64, end_time_ms: Option<u64>, title: impl Into<String>) -> Self {
        Self {
            start_time_ms,
            end_time_ms,
            title: title.into(),
        }
    }
}

/// An embedded binary attachment (such as cover art, fonts, or poster images).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAttachment {
    pub file_name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

impl MediaAttachment {
    /// Creates a new `MediaAttachment`.
    #[must_use]
    pub fn new(file_name: impl Into<String>, mime_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            file_name: file_name.into(),
            mime_type: mime_type.into(),
            data,
        }
    }
}

/// Comprehensive summary of a demuxed media container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaDemuxSummary {
    pub container_format: String,
    pub duration_ms: Option<u64>,
    pub title: Option<String>,
    pub tracks: Vec<MediaTrackInfo>,
    pub chapters: Vec<MediaChapter>,
    pub attachments: Vec<MediaAttachment>,
}

impl MediaDemuxSummary {
    /// Creates an empty demux summary for the specified container format.
    #[must_use]
    pub fn new(container_format: impl Into<String>) -> Self {
        Self {
            container_format: container_format.into(),
            duration_ms: None,
            title: None,
            tracks: Vec::new(),
            chapters: Vec::new(),
            attachments: Vec::new(),
        }
    }

    /// Returns all video tracks.
    #[must_use]
    pub fn video_tracks(&self) -> impl Iterator<Item = &MediaTrackInfo> {
        self.tracks.iter().filter(|t| t.track_type == MediaTrackType::Video)
    }

    /// Returns all audio tracks.
    #[must_use]
    pub fn audio_tracks(&self) -> impl Iterator<Item = &MediaTrackInfo> {
        self.tracks.iter().filter(|t| t.track_type == MediaTrackType::Audio)
    }

    /// Returns all subtitle tracks.
    #[must_use]
    pub fn subtitle_tracks(&self) -> impl Iterator<Item = &MediaTrackInfo> {
        self.tracks.iter().filter(|t| t.track_type == MediaTrackType::Subtitle)
    }
}
