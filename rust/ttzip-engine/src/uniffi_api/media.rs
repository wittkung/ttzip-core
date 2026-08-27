// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Multimedia Player Microkernel Scaffolding.
//!
//! Exposes strongly-typed media player controls, timeline tracking, and track selection
//! directly to Swift, Kotlin, and Python.

use std::sync::Arc;

use super::types::TTZipError;
use crate::archive::nested_vfs::VirtualFileStream;
use crate::media::player::TTZipMediaPlayer;
use crate::media::types::{
    AudioTrack, PlaybackTimeInfo, PlayerState, SubtitleTrack, VideoDimension, VideoTrack,
};

/// High-level playback state exposed across UniFFI boundary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum UniFFIPlayerState {
    Idle, Loading, Playing, Paused, Buffering, Seeking, Completed, Stopped, Error,
}

impl From<PlayerState> for UniFFIPlayerState {
    fn from(s: PlayerState) -> Self {
        match s {
            PlayerState::Idle => Self::Idle, PlayerState::Loading => Self::Loading,
            PlayerState::Playing => Self::Playing, PlayerState::Paused => Self::Paused,
            PlayerState::Buffering => Self::Buffering, PlayerState::Seeking => Self::Seeking,
            PlayerState::Completed => Self::Completed, PlayerState::Stopped => Self::Stopped,
            PlayerState::Error => Self::Error,
        }
    }
}
impl From<UniFFIPlayerState> for PlayerState {
    fn from(s: UniFFIPlayerState) -> Self {
        match s {
            UniFFIPlayerState::Idle => Self::Idle, UniFFIPlayerState::Loading => Self::Loading,
            UniFFIPlayerState::Playing => Self::Playing, UniFFIPlayerState::Paused => Self::Paused,
            UniFFIPlayerState::Buffering => Self::Buffering, UniFFIPlayerState::Seeking => Self::Seeking,
            UniFFIPlayerState::Completed => Self::Completed, UniFFIPlayerState::Stopped => Self::Stopped,
            UniFFIPlayerState::Error => Self::Error,
        }
    }
}

/// Structured playback timeline progress information exposed across UniFFI boundary.
#[derive(Copy, Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIPlaybackTimeInfo {
    pub current_ms: u64,
    pub duration_ms: u64,
    pub position_ratio: f64,
}

impl From<PlaybackTimeInfo> for UniFFIPlaybackTimeInfo {
    fn from(t: PlaybackTimeInfo) -> Self {
        Self { current_ms: t.current_ms, duration_ms: t.duration_ms, position_ratio: t.position_ratio }
    }
}
impl From<UniFFIPlaybackTimeInfo> for PlaybackTimeInfo {
    fn from(t: UniFFIPlaybackTimeInfo) -> Self {
        Self { current_ms: t.current_ms, duration_ms: t.duration_ms, position_ratio: t.position_ratio }
    }
}

/// Audio track metadata exposed across UniFFI boundary.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIAudioTrack {
    pub id: u32,
    pub name: String,
    pub language: Option<String>,
    pub channels: Option<u16>,
    pub sample_rate: Option<u32>,
    pub codec: String,
    pub is_selected: bool,
}

impl From<AudioTrack> for UniFFIAudioTrack {
    fn from(a: AudioTrack) -> Self {
        Self { id: a.id, name: a.name, language: a.language, channels: a.channels, sample_rate: a.sample_rate, codec: a.codec, is_selected: a.is_selected }
    }
}
impl From<UniFFIAudioTrack> for AudioTrack {
    fn from(a: UniFFIAudioTrack) -> Self {
        Self { id: a.id, name: a.name, language: a.language, channels: a.channels, sample_rate: a.sample_rate, codec: a.codec, is_selected: a.is_selected }
    }
}

/// Subtitle track metadata exposed across UniFFI boundary.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFISubtitleTrack {
    pub id: u32,
    pub name: String,
    pub language: Option<String>,
    pub format: String,
    pub is_selected: bool,
    pub is_external: bool,
}

impl From<SubtitleTrack> for UniFFISubtitleTrack {
    fn from(s: SubtitleTrack) -> Self {
        Self { id: s.id, name: s.name, language: s.language, format: s.format, is_selected: s.is_selected, is_external: s.is_external }
    }
}
impl From<UniFFISubtitleTrack> for SubtitleTrack {
    fn from(s: UniFFISubtitleTrack) -> Self {
        Self { id: s.id, name: s.name, language: s.language, format: s.format, is_selected: s.is_selected, is_external: s.is_external }
    }
}

/// Video track metadata exposed across UniFFI boundary.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIVideoTrack {
    pub id: u32,
    pub name: String,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub is_selected: bool,
}

impl From<VideoTrack> for UniFFIVideoTrack {
    fn from(v: VideoTrack) -> Self {
        Self { id: v.id, name: v.name, codec: v.codec, width: v.width, height: v.height, fps: v.fps, is_selected: v.is_selected }
    }
}
impl From<UniFFIVideoTrack> for VideoTrack {
    fn from(v: UniFFIVideoTrack) -> Self {
        Self { id: v.id, name: v.name, codec: v.codec, width: v.width, height: v.height, fps: v.fps, is_selected: v.is_selected }
    }
}

/// Video display frame dimension record exposed across UniFFI boundary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, uniffi::Record)]
pub struct UniFFIVideoDimension {
    pub width: u32,
    pub height: u32,
}

impl From<VideoDimension> for UniFFIVideoDimension {
    fn from(d: VideoDimension) -> Self { Self { width: d.width, height: d.height } }
}
impl From<UniFFIVideoDimension> for VideoDimension {
    fn from(d: UniFFIVideoDimension) -> Self { Self { width: d.width, height: d.height } }
}

/// Cross-language UniFFI MediaPlayer controller object.
#[derive(uniffi::Object)]
pub struct UniFFITTZipMediaPlayer {
    inner: TTZipMediaPlayer,
}

#[uniffi::export]
impl UniFFITTZipMediaPlayer {
    /// Creates a new media player object.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self { inner: TTZipMediaPlayer::new() })
    }

    /// Mounts a virtual stream pipeline.
    pub fn mount_virtual_stream(&self, stream: Arc<VirtualFileStream>, duration_ms: u64) -> Result<(), TTZipError> {
        self.inner.mount_virtual_stream(stream, duration_ms)
    }

    /// Mounts in-memory media payload bytes.
    pub fn mount_bytes(&self, data: Vec<u8>, duration_ms: u64) -> Result<(), TTZipError> {
        self.inner.mount_bytes(data, duration_ms)
    }

    /// Starts or resumes playback.
    pub fn play(&self) -> Result<(), TTZipError> { self.inner.play() }

    /// Pauses active playback.
    pub fn pause(&self) -> Result<(), TTZipError> { self.inner.pause() }

    /// Stops playback and resets timeline offset.
    pub fn stop(&self) -> Result<(), TTZipError> { self.inner.stop() }

    /// Seeks playback to target millisecond position.
    pub fn seek_to(&self, position_ms: u64) -> Result<u64, TTZipError> { self.inner.seek_to(position_ms) }

    /// Returns current player state.
    pub fn get_state(&self) -> UniFFIPlayerState { self.inner.get_state().into() }

    /// Returns current timeline progress.
    pub fn get_time_info(&self) -> UniFFIPlaybackTimeInfo { self.inner.get_time_info().into() }

    /// Sets playback volume in range [0.0, 1.0].
    pub fn set_volume(&self, volume: f32) { self.inner.set_volume(volume); }

    /// Returns configured volume level.
    pub fn get_volume(&self) -> f32 { self.inner.get_volume() }

    /// Returns effective volume level.
    pub fn effective_volume(&self) -> f32 { self.inner.effective_volume() }

    /// Sets mute flag.
    pub fn set_muted(&self, muted: bool) { self.inner.set_muted(muted); }

    /// Returns mute status.
    pub fn is_muted(&self) -> bool { self.inner.is_muted() }

    /// Sets playback speed multiplier.
    pub fn set_playback_rate(&self, rate: f32) -> Result<(), TTZipError> { self.inner.set_playback_rate(rate) }

    /// Returns playback speed multiplier.
    pub fn get_playback_rate(&self) -> f32 { self.inner.get_playback_rate() }

    /// Returns all available audio tracks.
    pub fn get_audio_tracks(&self) -> Vec<UniFFIAudioTrack> {
        self.inner.get_audio_tracks().into_iter().map(Into::into).collect()
    }

    /// Returns all available subtitle tracks.
    pub fn get_subtitle_tracks(&self) -> Vec<UniFFISubtitleTrack> {
        self.inner.get_subtitle_tracks().into_iter().map(Into::into).collect()
    }

    /// Returns all available video tracks.
    pub fn get_video_tracks(&self) -> Vec<UniFFIVideoTrack> {
        self.inner.get_video_tracks().into_iter().map(Into::into).collect()
    }

    /// Selects active audio track by ID.
    pub fn select_audio_track(&self, track_id: u32) -> Result<(), TTZipError> { self.inner.select_audio_track(track_id) }

    /// Selects active subtitle track by ID or disables subtitles if `None`.
    pub fn select_subtitle_track(&self, track_id: Option<u32>) -> Result<(), TTZipError> { self.inner.select_subtitle_track(track_id) }

    /// Updates playback progress offset.
    pub fn update_playback_time(&self, current_ms: u64) { self.inner.update_playback_time(current_ms); }

    /// Sets media duration in milliseconds.
    pub fn set_duration(&self, duration_ms: u64) { self.inner.set_duration(duration_ms); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_media_player_e2e() {
        let player = UniFFITTZipMediaPlayer::new();
        assert_eq!(player.get_state(), UniFFIPlayerState::Idle);

        player.mount_bytes(vec![1, 2, 3, 4], 45_000).expect("mount ok");
        assert_eq!(player.get_state(), UniFFIPlayerState::Paused);
        assert_eq!(player.get_time_info().duration_ms, 45_000);

        player.play().expect("play ok");
        assert_eq!(player.get_state(), UniFFIPlayerState::Playing);

        player.pause().expect("pause ok");
        assert_eq!(player.get_state(), UniFFIPlayerState::Paused);

        let pos = player.seek_to(15_000).expect("seek ok");
        assert_eq!(pos, 15_000);
        assert_eq!(player.get_time_info().current_ms, 15_000);

        player.set_volume(0.6);
        assert_eq!(player.get_volume(), 0.6);

        player.set_muted(true);
        assert!(player.is_muted());
        assert_eq!(player.effective_volume(), 0.0);

        player.set_muted(false);
        assert!(!player.is_muted());
        assert_eq!(player.effective_volume(), 0.6);
    }
}
