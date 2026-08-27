// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed domain models for multimedia core playback and streaming.

use serde::{Deserialize, Serialize};

/// High-level playback state for the media player state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Buffering,
    Seeking,
    Completed,
    Stopped,
    Error,
}

impl PlayerState {
    /// Returns static string representation of the playback state.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Loading => "Loading",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Buffering => "Buffering",
            Self::Seeking => "Seeking",
            Self::Completed => "Completed",
            Self::Stopped => "Stopped",
            Self::Error => "Error",
        }
    }

    /// Returns true if the player is currently active (playing or buffering).
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Playing | Self::Buffering | Self::Seeking)
    }
}

/// Structured playback timeline progress information.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlaybackTimeInfo {
    pub current_ms: u64,
    pub duration_ms: u64,
    pub position_ratio: f64,
}

impl PlaybackTimeInfo {
    /// Creates a new `PlaybackTimeInfo` with normalized position ratio [0.0, 1.0].
    #[must_use]
    pub fn new(current_ms: u64, duration_ms: u64) -> Self {
        let position_ratio = if duration_ms == 0 {
            0.0
        } else {
            (current_ms as f64 / duration_ms as f64).clamp(0.0, 1.0)
        };
        Self { current_ms, duration_ms, position_ratio }
    }

    /// Creates an empty initial time progress record.
    #[must_use]
    pub const fn zero() -> Self {
        Self { current_ms: 0, duration_ms: 0, position_ratio: 0.0 }
    }
}

/// Metadata describing an audio stream track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTrack {
    pub id: u32,
    pub name: String,
    pub language: Option<String>,
    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub codec: String,
    pub is_selected: bool,
}

impl AudioTrack {
    /// Creates a new `AudioTrack` descriptor.
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>, codec: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            language: None,
            channels: None,
            sample_rate: None,
            codec: codec.into(),
            is_selected: false,
        }
    }
}

/// Metadata describing an embedded or external subtitle track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleTrack {
    pub id: u32,
    pub name: String,
    pub language: Option<String>,
    pub format: String,
    pub is_selected: bool,
    pub is_external: bool,
}

impl SubtitleTrack {
    /// Creates a new `SubtitleTrack` descriptor.
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>, format: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            language: None,
            format: format.into(),
            is_selected: false,
            is_external: false,
        }
    }
}

/// Metadata describing a video elementary stream track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoTrack {
    pub id: u32,
    pub name: String,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub is_selected: bool,
}

impl VideoTrack {
    /// Creates a new `VideoTrack` descriptor.
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>, codec: impl Into<String>, width: u32, height: u32, fps: f64) -> Self {
        Self {
            id,
            name: name.into(),
            codec: codec.into(),
            width,
            height,
            fps,
            is_selected: false,
        }
    }
}

/// 2D video display frame dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoDimension {
    pub width: u32,
    pub height: u32,
}

impl VideoDimension {
    /// Creates a new `VideoDimension` instance.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// High-level player events emitted upon state changes or playback milestones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayerEvent {
    StateChanged(PlayerState),
    TimeUpdated(PlaybackTimeInfo),
    AudioTrackChanged(u32),
    SubtitleTrackChanged(Option<u32>),
    VolumeChanged { volume: f32, is_muted: bool },
    RateChanged(f32),
    SeekCompleted(u64),
    StreamMounted { total_size: u64, duration_ms: u64 },
    Error(String),
}
