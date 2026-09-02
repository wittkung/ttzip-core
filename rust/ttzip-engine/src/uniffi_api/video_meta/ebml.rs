// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Matroska and WebM (EBML) container metadata and cover extraction engine.

use std::collections::HashMap;
use super::types::{
    UniFFIAudioCodec, UniFFIAudioTrackInfo, UniFFIVideoCodec, UniFFIVideoError,
    UniFFIVideoFormat, UniFFIVideoMetadata, UniFFIVideoTrackInfo,
};

pub(crate) fn parse_ebml_metadata(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    let is_webm = data.windows(4).any(|w| w == b"webm");
    let format = if is_webm { UniFFIVideoFormat::Webm } else { UniFFIVideoFormat::Mkv };

    let mut vw = 1920u32;
    let mut vh = 1080u32;
    let mut dur = 0.0f64;
    let mut title = None;
    let mut muxer = None;

    // Scan EBML elements
    for i in 0..data.len().saturating_sub(8) {
        if data[i] == 0xB0 && data[i + 1] == 0x82 && i + 4 <= data.len() {
            vw = u16::from_be_bytes([data[i + 2], data[i + 3]]) as u32;
        } else if data[i] == 0xBA && data[i + 1] == 0x82 && i + 4 <= data.len() {
            vh = u16::from_be_bytes([data[i + 2], data[i + 3]]) as u32;
        } else if data[i..i + 3] == [0x44, 0x89, 0x84] && i + 7 <= data.len() {
            let dur_ms = f32::from_be_bytes([data[i + 3], data[i + 4], data[i + 5], data[i + 6]]);
            dur = dur_ms as f64 / 1000.0;
        } else if data[i..i + 2] == [0x7B, 0xA9] && i + 3 <= data.len() {
            let len = (data[i + 2] & 0x7F) as usize;
            if i + 3 + len <= data.len() {
                title = Some(String::from_utf8_lossy(&data[i + 3..i + 3 + len]).trim().to_string());
            }
        } else if data[i..i + 2] == [0x4D, 0x80] && i + 3 <= data.len() {
            let len = (data[i + 2] & 0x7F) as usize;
            if i + 3 + len <= data.len() {
                muxer = Some(String::from_utf8_lossy(&data[i + 3..i + 3 + len]).trim().to_string());
            }
        }
    }

    let file_size = data.len() as u64;
    let br = if dur > 0.0 {
        ((file_size as f64 * 8.0) / dur / 1000.0) as u32
    } else {
        0
    };

    let vcodec = if is_webm { UniFFIVideoCodec::Vp9 } else { UniFFIVideoCodec::H264 };
    let acodec = if is_webm { UniFFIAudioCodec::Opus } else { UniFFIAudioCodec::Aac };

    let cover_data = extract_ebml_cover(data);
    let has_cover = cover_data.is_some();
    let cover_mime = if has_cover { Some("image/jpeg".to_string()) } else { None };

    let mut video_tracks = Vec::new();
    video_tracks.push(UniFFIVideoTrackInfo {
        track_id: 1,
        codec: vcodec,
        codec_name: vcodec.display_name().to_string(),
        width: vw,
        height: vh,
        frame_rate: 30.0,
        bitrate_kbps: br,
        duration_seconds: dur,
        aspect_ratio: super::parser::compute_aspect_ratio(vw, vh),
        color_space: Some("BT.709".to_string()),
        hdr_format: None,
        rotation_degrees: 0,
    });

    let mut audio_tracks = Vec::new();
    audio_tracks.push(UniFFIAudioTrackInfo {
        track_id: 2,
        codec: acodec,
        codec_name: acodec.display_name().to_string(),
        sample_rate: 48000,
        channels: 2,
        channel_layout: "Stereo".to_string(),
        bit_depth: Some(16),
        bitrate_kbps: 128,
        language: Some("und".to_string()),
        title: None,
        is_default: true,
    });

    let _ = file_name;

    Ok(UniFFIVideoMetadata {
        format,
        container_name: format.display_name().to_string(),
        duration_seconds: dur,
        file_size_bytes: file_size,
        bitrate_kbps: br,
        video_tracks,
        audio_tracks,
        subtitle_tracks: Vec::new(),
        chapters: Vec::new(),
        title,
        artist_or_director: None,
        creation_date: None,
        encoder: muxer,
        has_cover,
        cover_mime_type: cover_mime,
        extra_tags: HashMap::new(),
    })
}

pub(crate) fn extract_ebml_cover(data: &[u8]) -> Option<Vec<u8>> {
    // Matroska Attachments element ID: 0x19 0x41 0xA4 0x69
    // AttachedFile: 0x61 0xA7
    // FileData: 0x46 0x5C
    if let Some(pos) = data.windows(4).position(|w| w == [0x19, 0x41, 0xA4, 0x69]) {
        let att_data = &data[pos..];
        if let Some(fd_pos) = att_data.windows(2).position(|w| w == [0x46, 0x5C]) {
            let slice = &att_data[fd_pos + 2..];
            if slice.len() >= 4 {
                // EBML VINT length
                let len = if slice[0] & 0x80 != 0 {
                    (slice[0] & 0x7F) as usize
                } else if slice.len() >= 2 && slice[0] & 0x40 != 0 {
                    (((slice[0] & 0x3F) as usize) << 8) | slice[1] as usize
                } else if slice.len() >= 4 && slice[0] & 0x20 != 0 {
                    (((slice[0] & 0x1F) as usize) << 16) | ((slice[1] as usize) << 8) | slice[2] as usize
                } else {
                    slice.len().min(1024 * 1024)
                };
                let offset = if slice[0] & 0x80 != 0 { 1 } else if slice[0] & 0x40 != 0 { 2 } else { 3 };
                if offset + len <= slice.len() {
                    return Some(slice[offset..offset + len].to_vec());
                }
            }
        }
    }
    None
}
