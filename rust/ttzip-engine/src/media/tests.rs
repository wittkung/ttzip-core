// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit tests for multimedia core playback engine and state machine.

use std::sync::Arc;

use super::player::TTZipMediaPlayer;
use super::types::{AudioTrack, PlayerState, SubtitleTrack, VideoTrack};
use crate::archive::nested_vfs::{calculate_chunk_size, VirtualChunkedStream, VirtualFileStream};

#[test]
fn test_player_initial_state_and_defaults() {
    let player = TTZipMediaPlayer::new();
    assert_eq!(player.get_state(), PlayerState::Idle);
    assert_eq!(player.get_volume(), 1.0);
    assert_eq!(player.effective_volume(), 1.0);
    assert!(!player.is_muted());
    assert_eq!(player.get_playback_rate(), 1.0);

    let time = player.get_time_info();
    assert_eq!(time.current_ms, 0);
    assert_eq!(time.duration_ms, 0);
    assert_eq!(time.position_ratio, 0.0);
    assert!(player.active_stream().is_none());
}

#[test]
fn test_player_lifecycle_and_state_machine() {
    let player = TTZipMediaPlayer::new();
    let dummy_data = vec![0x1A, 0x45, 0xDF, 0xA3, 0x93, 0x42, 0x82, 0x88, 0x6d, 0x61, 0x74, 0x72, 0x6f, 0x73, 0x6b, 0x61];
    player.mount_bytes(dummy_data, 60_000).expect("mount failed");

    assert_eq!(player.get_state(), PlayerState::Paused);
    assert_eq!(player.get_time_info().duration_ms, 60_000);

    player.play().expect("play failed");
    assert_eq!(player.get_state(), PlayerState::Playing);
    assert!(player.get_state().is_active());

    player.pause().expect("pause failed");
    assert_eq!(player.get_state(), PlayerState::Paused);
    assert!(!player.get_state().is_active());

    player.play().expect("resume failed");
    assert_eq!(player.get_state(), PlayerState::Playing);

    player.update_playback_time(60_000);
    assert_eq!(player.get_state(), PlayerState::Completed);
    assert_eq!(player.get_time_info().position_ratio, 1.0);

    player.seek_to(30_000).expect("seek failed");
    assert_eq!(player.get_state(), PlayerState::Paused);
    assert_eq!(player.get_time_info().current_ms, 30_000);
    assert!((player.get_time_info().position_ratio - 0.5).abs() < f64::EPSILON);

    player.stop().expect("stop failed");
    assert_eq!(player.get_state(), PlayerState::Stopped);
    assert_eq!(player.get_time_info().current_ms, 0);
}

#[test]
fn test_volume_clamping_and_muting() {
    let player = TTZipMediaPlayer::new();

    player.set_volume(0.75);
    assert_eq!(player.get_volume(), 0.75);
    assert_eq!(player.effective_volume(), 0.75);

    player.set_volume(1.5);
    assert_eq!(player.get_volume(), 1.0);

    player.set_volume(-0.5);
    assert_eq!(player.get_volume(), 0.0);

    player.set_volume(0.8);
    player.set_muted(true);
    assert!(player.is_muted());
    assert_eq!(player.get_volume(), 0.8);
    assert_eq!(player.effective_volume(), 0.0);

    player.set_muted(false);
    assert!(!player.is_muted());
    assert_eq!(player.effective_volume(), 0.8);

    player.set_muted(true);
    player.set_volume(0.5);
    assert!(!player.is_muted());
    assert_eq!(player.effective_volume(), 0.5);
}

#[test]
fn test_seek_clamping_and_timeline() {
    let player = TTZipMediaPlayer::new();
    player.set_duration(100_000);

    let pos = player.seek_to(50_000).expect("seek failed");
    assert_eq!(pos, 50_000);
    assert_eq!(player.get_time_info().current_ms, 50_000);
    assert!((player.get_time_info().position_ratio - 0.5).abs() < f64::EPSILON);

    let pos_clamped = player.seek_to(150_000).expect("clamped seek failed");
    assert_eq!(pos_clamped, 100_000);
    assert_eq!(player.get_time_info().current_ms, 100_000);
    assert_eq!(player.get_time_info().position_ratio, 1.0);
}

#[test]
fn test_playback_rate_control() {
    let player = TTZipMediaPlayer::new();

    player.set_playback_rate(1.5).expect("valid rate");
    assert_eq!(player.get_playback_rate(), 1.5);

    player.set_playback_rate(0.5).expect("valid slow rate");
    assert_eq!(player.get_playback_rate(), 0.5);

    assert!(player.set_playback_rate(0.0).is_err());
    assert!(player.set_playback_rate(-1.0).is_err());
    assert!(player.set_playback_rate(20.0).is_err());
}

#[test]
fn test_track_selection_and_management() {
    let player = TTZipMediaPlayer::new();

    let mut a1 = AudioTrack::new(1, "English Stereo", "aac");
    a1.is_selected = true;
    let a2 = AudioTrack::new(2, "Japanese 5.1", "ac3");
    let s1 = SubtitleTrack::new(10, "English", "ass");
    let s2 = SubtitleTrack::new(11, "Chinese", "srt");
    let v1 = VideoTrack::new(100, "Main Video", "h264", 1920, 1080, 60.0);

    player.set_tracks(vec![a1, a2], vec![s1, s2], vec![v1]);

    assert_eq!(player.get_audio_tracks().len(), 2);
    assert_eq!(player.get_subtitle_tracks().len(), 2);
    assert_eq!(player.get_video_tracks().len(), 1);

    assert_eq!(player.selected_audio_track().unwrap().id, 1);
    assert!(player.selected_subtitle_track().is_none());

    player.select_audio_track(2).expect("select audio track 2");
    assert_eq!(player.selected_audio_track().unwrap().id, 2);
    assert!(!player.get_audio_tracks()[0].is_selected);
    assert!(player.get_audio_tracks()[1].is_selected);

    player.select_subtitle_track(Some(10)).expect("select sub 10");
    assert_eq!(player.selected_subtitle_track().unwrap().id, 10);

    player.select_subtitle_track(None).expect("disable subs");
    assert!(player.selected_subtitle_track().is_none());

    assert!(player.select_audio_track(999).is_err());
    assert!(player.select_subtitle_track(Some(999)).is_err());
}

#[test]
fn test_virtual_stream_mounting_and_data_read() {
    let player = TTZipMediaPlayer::new();
    let data = b"TTZip Media Core Virtual Stream Test Payload Data 0123456789".to_vec();
    let total_size = data.len() as u64;
    let chunk_size = calculate_chunk_size(total_size);
    let arc_data = Arc::new(data.clone());
    let loader = Arc::new(move |offset: u64, len: usize| {
        let off = offset as usize;
        if off >= arc_data.len() {
            return Ok(Vec::new());
        }
        let end = (off + len).min(arc_data.len());
        Ok(arc_data[off..end].to_vec())
    });

    let chunked = VirtualChunkedStream::new(total_size, chunk_size, loader);
    let stream = Arc::new(VirtualFileStream::new(chunked));

    player.mount_virtual_stream(Arc::clone(&stream), 12_345).expect("mount ok");
    assert_eq!(player.get_time_info().duration_ms, 12_345);

    let active = player.active_stream().expect("stream present");
    assert_eq!(active.size(), total_size);

    let read_back = active.read_exact_at(0, 10).expect("read exact");
    assert_eq!(&read_back, &data[..10]);
}
