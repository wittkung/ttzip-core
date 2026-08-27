// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multimedia core playback engine and virtual chunk stream pipeline.

use std::sync::Arc;
use parking_lot::RwLock;

use super::types::{AudioTrack, PlaybackTimeInfo, PlayerState, SubtitleTrack, VideoTrack};
use crate::archive::nested_vfs::{calculate_chunk_size, VirtualChunkedStream, VirtualFileStream};
use crate::standards::demuxer::demux_media_tracks_two_pass;
use crate::standards::demuxer::types::MediaTrackType;
use crate::uniffi_api::types::TTZipError;

/// High-performance thread-safe multimedia playback microkernel.
pub struct TTZipMediaPlayer {
    state: RwLock<PlayerState>,
    time_info: RwLock<PlaybackTimeInfo>,
    volume: RwLock<f32>,
    muted: RwLock<bool>,
    previous_volume: RwLock<f32>,
    playback_rate: RwLock<f32>,
    audio_tracks: RwLock<Vec<AudioTrack>>,
    subtitle_tracks: RwLock<Vec<SubtitleTrack>>,
    video_tracks: RwLock<Vec<VideoTrack>>,
    active_stream: RwLock<Option<Arc<VirtualFileStream>>>,
}

impl Default for TTZipMediaPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl TTZipMediaPlayer {
    /// Creates a new uninitialized media player instance in `Idle` state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RwLock::new(PlayerState::Idle),
            time_info: RwLock::new(PlaybackTimeInfo::zero()),
            volume: RwLock::new(1.0),
            muted: RwLock::new(false),
            previous_volume: RwLock::new(1.0),
            playback_rate: RwLock::new(1.0),
            audio_tracks: RwLock::new(Vec::new()),
            subtitle_tracks: RwLock::new(Vec::new()),
            video_tracks: RwLock::new(Vec::new()),
            active_stream: RwLock::new(None),
        }
    }

    /// Mounts a virtual file stream pipeline and extracts container tracks if available.
    pub fn mount_virtual_stream(&self, stream: Arc<VirtualFileStream>, duration_ms: u64) -> Result<(), TTZipError> {
        let total_size = stream.size();
        let head_len = total_size.min(64 * 1024) as u32;
        let head = stream.read_exact_at(0, head_len).unwrap_or_default();
        let tail = if total_size > 64 * 1024 {
            stream.read_exact_at(total_size - 64 * 1024, 64 * 1024).ok()
        } else {
            None
        };

        let mut effective_duration = duration_ms;
        if let Ok(summary) = demux_media_tracks_two_pass(&head, tail.as_deref()) {
            if let Some(demux_dur) = summary.duration_ms {
                if demux_dur > 0 {
                    effective_duration = demux_dur;
                }
            }
            self.populate_tracks_from_demux(summary.tracks);
        }

        *self.active_stream.write() = Some(stream);
        *self.time_info.write() = PlaybackTimeInfo::new(0, effective_duration);
        *self.state.write() = PlayerState::Paused;
        Ok(())
    }

    /// Mounts an in-memory chunk stream.
    pub fn mount_virtual_chunked_stream(&self, chunked: VirtualChunkedStream, duration_ms: u64) -> Result<(), TTZipError> {
        self.mount_virtual_stream(Arc::new(VirtualFileStream::new(chunked)), duration_ms)
    }

    /// Mounts raw memory buffer into bounded chunk stream.
    pub fn mount_bytes(&self, data: Vec<u8>, duration_ms: u64) -> Result<(), TTZipError> {
        let total_size = data.len() as u64;
        let chunk_size = calculate_chunk_size(total_size);
        let arc_data = Arc::new(data);
        let loader = Arc::new(move |offset: u64, len: usize| {
            let off = offset as usize;
            if off >= arc_data.len() {
                return Ok(Vec::new());
            }
            let end = (off + len).min(arc_data.len());
            Ok(arc_data[off..end].to_vec())
        });
        self.mount_virtual_chunked_stream(VirtualChunkedStream::new(total_size, chunk_size, loader), duration_ms)
    }

    /// Starts or resumes playback.
    pub fn play(&self) -> Result<(), TTZipError> {
        let mut state = self.state.write();
        match *state {
            PlayerState::Idle => {
                if self.active_stream.read().is_none() {
                    return Err(TTZipError::IoError { message: "Cannot play without active media stream".to_string() });
                }
                *state = PlayerState::Playing;
            }
            PlayerState::Paused | PlayerState::Buffering | PlayerState::Seeking | PlayerState::Stopped => {
                *state = PlayerState::Playing;
            }
            PlayerState::Completed => {
                let duration = self.time_info.read().duration_ms;
                *self.time_info.write() = PlaybackTimeInfo::new(0, duration);
                *state = PlayerState::Playing;
            }
            PlayerState::Playing => {}
            PlayerState::Loading | PlayerState::Error => {
                return Err(TTZipError::IoError { message: format!("Cannot play from state {:?}", *state) });
            }
        }
        Ok(())
    }

    /// Pauses active playback.
    pub fn pause(&self) -> Result<(), TTZipError> {
        let mut state = self.state.write();
        if matches!(*state, PlayerState::Playing | PlayerState::Buffering | PlayerState::Seeking) {
            *state = PlayerState::Paused;
        }
        Ok(())
    }

    /// Stops playback and resets timeline position.
    pub fn stop(&self) -> Result<(), TTZipError> {
        *self.state.write() = PlayerState::Stopped;
        let duration = self.time_info.read().duration_ms;
        *self.time_info.write() = PlaybackTimeInfo::new(0, duration);
        Ok(())
    }

    /// Seeks playback to a target millisecond offset.
    pub fn seek_to(&self, position_ms: u64) -> Result<u64, TTZipError> {
        let duration = self.time_info.read().duration_ms;
        let clamped = position_ms.min(duration);
        *self.time_info.write() = PlaybackTimeInfo::new(clamped, duration);
        let mut state = self.state.write();
        if *state == PlayerState::Completed && clamped < duration {
            *state = PlayerState::Paused;
        }
        Ok(clamped)
    }

    /// Sets playback volume in range [0.0, 1.0].
    pub fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        if clamped > 0.0 && *self.muted.read() {
            *self.muted.write() = false;
        }
        *self.volume.write() = clamped;
        *self.previous_volume.write() = clamped;
    }

    /// Returns the current configured volume level.
    #[must_use]
    pub fn get_volume(&self) -> f32 { *self.volume.read() }

    /// Returns the effective output volume accounting for mute status.
    #[must_use]
    pub fn effective_volume(&self) -> f32 {
        if *self.muted.read() { 0.0 } else { *self.volume.read() }
    }

    /// Sets or clears audio mute.
    pub fn set_muted(&self, muted: bool) { *self.muted.write() = muted; }

    /// Returns true if audio is currently muted.
    #[must_use]
    pub fn is_muted(&self) -> bool { *self.muted.read() }

    /// Sets the playback speed multiplier.
    pub fn set_playback_rate(&self, rate: f32) -> Result<(), TTZipError> {
        if rate <= 0.0 || rate > 16.0 {
            return Err(TTZipError::IoError { message: format!("Invalid playback rate: {rate}") });
        }
        *self.playback_rate.write() = rate;
        Ok(())
    }

    /// Returns current playback speed multiplier.
    #[must_use]
    pub fn get_playback_rate(&self) -> f32 { *self.playback_rate.read() }

    /// Returns current playback state.
    #[must_use]
    pub fn get_state(&self) -> PlayerState { *self.state.read() }

    /// Returns current playback timeline position info.
    #[must_use]
    pub fn get_time_info(&self) -> PlaybackTimeInfo { *self.time_info.read() }

    /// Updates playback progress and automatically triggers `Completed` upon reaching end.
    pub fn update_playback_time(&self, current_ms: u64) {
        let duration = self.time_info.read().duration_ms;
        let clamped = current_ms.min(duration);
        *self.time_info.write() = PlaybackTimeInfo::new(clamped, duration);
        if duration > 0 && clamped >= duration {
            *self.state.write() = PlayerState::Completed;
        }
    }

    /// Configures total media duration in milliseconds.
    pub fn set_duration(&self, duration_ms: u64) {
        let current = self.time_info.read().current_ms;
        *self.time_info.write() = PlaybackTimeInfo::new(current, duration_ms);
    }

    /// Replaces active track descriptors directly.
    pub fn set_tracks(&self, audio: Vec<AudioTrack>, subs: Vec<SubtitleTrack>, video: Vec<VideoTrack>) {
        *self.audio_tracks.write() = audio;
        *self.subtitle_tracks.write() = subs;
        *self.video_tracks.write() = video;
    }

    /// Returns all available audio tracks.
    #[must_use]
    pub fn get_audio_tracks(&self) -> Vec<AudioTrack> { self.audio_tracks.read().clone() }

    /// Returns all available subtitle tracks.
    #[must_use]
    pub fn get_subtitle_tracks(&self) -> Vec<SubtitleTrack> { self.subtitle_tracks.read().clone() }

    /// Returns all available video tracks.
    #[must_use]
    pub fn get_video_tracks(&self) -> Vec<VideoTrack> { self.video_tracks.read().clone() }

    /// Selects an audio track by ID, deselecting others.
    pub fn select_audio_track(&self, track_id: u32) -> Result<(), TTZipError> {
        let mut tracks = self.audio_tracks.write();
        if !tracks.iter().any(|t| t.id == track_id) {
            return Err(TTZipError::IoError { message: format!("Audio track {track_id} not found") });
        }
        for track in tracks.iter_mut() { track.is_selected = track.id == track_id; }
        Ok(())
    }

    /// Selects a subtitle track by ID or disables subtitles if `None`.
    pub fn select_subtitle_track(&self, track_id: Option<u32>) -> Result<(), TTZipError> {
        let mut tracks = self.subtitle_tracks.write();
        if let Some(id) = track_id {
            if !tracks.iter().any(|t| t.id == id) {
                return Err(TTZipError::IoError { message: format!("Subtitle track {id} not found") });
            }
            for track in tracks.iter_mut() { track.is_selected = track.id == id; }
        } else {
            for track in tracks.iter_mut() { track.is_selected = false; }
        }
        Ok(())
    }

    /// Returns the currently active audio track if any.
    #[must_use]
    pub fn selected_audio_track(&self) -> Option<AudioTrack> {
        self.audio_tracks.read().iter().find(|t| t.is_selected).cloned()
    }

    /// Returns the currently active subtitle track if any.
    #[must_use]
    pub fn selected_subtitle_track(&self) -> Option<SubtitleTrack> {
        self.subtitle_tracks.read().iter().find(|t| t.is_selected).cloned()
    }

    /// Returns reference to the currently mounted virtual file stream.
    #[must_use]
    pub fn active_stream(&self) -> Option<Arc<VirtualFileStream>> {
        self.active_stream.read().clone()
    }

    fn populate_tracks_from_demux(&self, demux_tracks: Vec<crate::standards::demuxer::types::MediaTrackInfo>) {
        let mut audio = Vec::new();
        let mut video = Vec::new();
        let mut subs = Vec::new();
        for t in demux_tracks {
            match t.track_type {
                MediaTrackType::Audio => {
                    let mut trk = AudioTrack::new(t.track_id, t.title.unwrap_or_else(|| format!("Audio {}", t.track_id)), t.codec);
                    trk.language = t.language;
                    trk.channels = t.channels;
                    trk.sample_rate = t.sample_rate;
                    trk.is_selected = t.is_default || audio.is_empty();
                    audio.push(trk);
                }
                MediaTrackType::Video => {
                    let mut trk = VideoTrack::new(t.track_id, t.title.unwrap_or_else(|| format!("Video {}", t.track_id)), t.codec, t.width.unwrap_or(0), t.height.unwrap_or(0), 0.0);
                    trk.is_selected = t.is_default || video.is_empty();
                    video.push(trk);
                }
                MediaTrackType::Subtitle => {
                    let mut trk = SubtitleTrack::new(t.track_id, t.title.unwrap_or_else(|| format!("Subtitle {}", t.track_id)), t.codec);
                    trk.language = t.language;
                    trk.is_selected = t.is_default;
                    subs.push(trk);
                }
            }
        }
        self.set_tracks(audio, subs, video);
    }
}
