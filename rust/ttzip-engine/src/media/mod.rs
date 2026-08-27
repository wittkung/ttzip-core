// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multimedia core player and virtual streaming subsystem.
//!
//! Provides the core player state machine, track switching, timeline navigation,
//! and virtual chunk stream pipeline integration for zero-copy media playback.

pub mod player;
pub mod types;

#[cfg(test)]
mod tests;

pub use player::TTZipMediaPlayer;
pub use types::{
    AudioTrack, PlaybackTimeInfo, PlayerEvent, PlayerState, SubtitleTrack, VideoDimension,
    VideoTrack,
};
