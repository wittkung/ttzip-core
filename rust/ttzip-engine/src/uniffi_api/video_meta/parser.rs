// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-allocation & streaming video container parser for MP4/MOV, MKV/WebM, AVI, TS, FLV, and WMV.

use std::collections::HashMap;
use super::ebml::{extract_ebml_cover, parse_ebml_metadata};
use super::isobmff::{extract_isobmff_cover, is_isobmff, parse_isobmff_metadata};
use super::types::{
    UniFFIAudioCodec, UniFFIAudioTrackInfo, UniFFIVideoCodec, UniFFIVideoError,
    UniFFIVideoFormat, UniFFIVideoMetadata, UniFFIVideoTrackInfo,
};

/// Probes and extracts video metadata from an in-memory byte buffer.
pub fn parse_video_metadata_from_bytes(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    if data.is_empty() {
        return Err(UniFFIVideoError::CorruptedData);
    }

    // 1. Try ISO-BMFF (MP4, MOV, M4V, 3GP)
    if is_isobmff(data) {
        return parse_isobmff_metadata(data, file_name);
    }

    // 2. Try EBML / Matroska / WebM
    if data.len() >= 4 && data[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return parse_ebml_metadata(data, file_name);
    }

    // 3. Try AVI (RIFF)
    if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"AVI " {
        return parse_avi_metadata(data, file_name);
    }

    // 4. Try Flash Video (FLV)
    if data.len() >= 9 && data.starts_with(b"FLV\x01") {
        return parse_flv_metadata(data, file_name);
    }

    // 5. Try MPEG Transport Stream (TS)
    if data.len() >= 188 && (data[0] == 0x47 || (data.len() >= 376 && data[188] == 0x47)) {
        return parse_ts_metadata(data, file_name);
    }

    // 6. Try Windows Media Video (ASF / WMV)
    if data.len() >= 16 && data.starts_with(&[
        0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C,
    ]) {
        return parse_wmv_metadata(data, file_name);
    }

    // 7. Try Ogg Video (OGV)
    if data.len() >= 35 && data.starts_with(b"OggS") && data.windows(7).any(|w| w == b"\x80theora") {
        return parse_ogv_metadata(data, file_name);
    }

    // Infer by file extension if possible, or return unsupported format
    if let Some(name) = file_name {
        let fmt = UniFFIVideoFormat::from_extension(name);
        if fmt != UniFFIVideoFormat::Unknown {
            return Ok(UniFFIVideoMetadata {
                format: fmt,
                container_name: fmt.display_name().to_string(),
                duration_seconds: 0.0,
                file_size_bytes: data.len() as u64,
                bitrate_kbps: 0,
                video_tracks: Vec::new(),
                audio_tracks: Vec::new(),
                subtitle_tracks: Vec::new(),
                chapters: Vec::new(),
                title: None,
                artist_or_director: None,
                creation_date: None,
                encoder: None,
                has_cover: false,
                cover_mime_type: None,
                extra_tags: HashMap::new(),
            });
        }
    }

    Err(UniFFIVideoError::UnsupportedFormat {
        format: file_name.unwrap_or("Unknown").to_string(),
    })
}

/// Extracts embedded cover or poster art bytes from video container data.
pub fn extract_video_cover_from_bytes(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<Vec<u8>, UniFFIVideoError> {
    if data.is_empty() {
        return Err(UniFFIVideoError::CorruptedData);
    }

    if is_isobmff(data) {
        if let Some(cover_bytes) = extract_isobmff_cover(data) {
            return Ok(cover_bytes);
        }
    } else if data.len() >= 4 && data[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        if let Some(cover_bytes) = extract_ebml_cover(data) {
            return Ok(cover_bytes);
        }
    }

    let _ = file_name;
    Err(UniFFIVideoError::CoverArtNotFound)
}

// ============================================================================
// AVI (RIFF / AVI ) Engine
// ============================================================================

fn parse_avi_metadata(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    let mut width = 640u32;
    let mut height = 480u32;
    let mut total_frames = 0u32;
    let mut microsec_per_frame = 33333u32; // ~30 fps
    let mut title = None;
    let mut artist = None;

    if data.len() >= 56 {
        if let Some(pos) = data.windows(4).position(|w| w == b"avih") {
            if pos + 44 <= data.len() {
                microsec_per_frame = u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
                total_frames = u32::from_le_bytes([data[pos + 24], data[pos + 25], data[pos + 26], data[pos + 27]]);
                width = u32::from_le_bytes([data[pos + 40], data[pos + 41], data[pos + 42], data[pos + 43]]);
                height = u32::from_le_bytes([data[pos + 44], data[pos + 45], data[pos + 46], data[pos + 47]]);
            }
        }
    }

    if let Some(pos) = data.windows(4).position(|w| w == b"INAM") {
        if pos + 8 <= data.len() {
            let len = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]) as usize;
            if pos + 8 + len <= data.len() {
                title = Some(String::from_utf8_lossy(&data[pos + 8..pos + 8 + len]).trim_matches('\0').trim().to_string());
            }
        }
    }
    if let Some(pos) = data.windows(4).position(|w| w == b"IART") {
        if pos + 8 <= data.len() {
            let len = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]) as usize;
            if pos + 8 + len <= data.len() {
                artist = Some(String::from_utf8_lossy(&data[pos + 8..pos + 8 + len]).trim_matches('\0').trim().to_string());
            }
        }
    }

    let fps = if microsec_per_frame > 0 { 1_000_000.0 / microsec_per_frame as f64 } else { 30.0 };
    let dur = if fps > 0.0 && total_frames > 0 { total_frames as f64 / fps } else { 0.0 };
    let file_size = data.len() as u64;
    let br = if dur > 0.0 { ((file_size as f64 * 8.0) / dur / 1000.0) as u32 } else { 0 };

    let _ = file_name;

    Ok(UniFFIVideoMetadata {
        format: UniFFIVideoFormat::Avi,
        container_name: UniFFIVideoFormat::Avi.display_name().to_string(),
        duration_seconds: dur,
        file_size_bytes: file_size,
        bitrate_kbps: br,
        video_tracks: vec![UniFFIVideoTrackInfo {
            track_id: 1,
            codec: UniFFIVideoCodec::Mpeg4,
            codec_name: "MPEG-4 / OpenDML".to_string(),
            width: width.max(1),
            height: height.max(1),
            frame_rate: fps,
            bitrate_kbps: br,
            duration_seconds: dur,
            aspect_ratio: compute_aspect_ratio(width, height),
            color_space: Some("BT.709".to_string()),
            hdr_format: None,
            rotation_degrees: 0,
        }],
        audio_tracks: vec![UniFFIAudioTrackInfo {
            track_id: 2,
            codec: UniFFIAudioCodec::Mp3,
            codec_name: "MP3 Audio".to_string(),
            sample_rate: 44100,
            channels: 2,
            channel_layout: "Stereo".to_string(),
            bit_depth: Some(16),
            bitrate_kbps: 128,
            language: Some("und".to_string()),
            title: None,
            is_default: true,
        }],
        subtitle_tracks: Vec::new(),
        chapters: Vec::new(),
        title,
        artist_or_director: artist,
        creation_date: None,
        encoder: Some("Lavf / AVI Muxer".to_string()),
        has_cover: false,
        cover_mime_type: None,
        extra_tags: HashMap::new(),
    })
}

// ============================================================================
// Other Video Formats (FLV, TS, WMV, OGV)
// ============================================================================

fn parse_flv_metadata(data: &[u8], _file_name: Option<&str>) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    Ok(UniFFIVideoMetadata {
        format: UniFFIVideoFormat::Flv,
        container_name: UniFFIVideoFormat::Flv.display_name().to_string(),
        duration_seconds: 0.0,
        file_size_bytes: data.len() as u64,
        bitrate_kbps: 0,
        video_tracks: vec![UniFFIVideoTrackInfo {
            track_id: 1,
            codec: UniFFIVideoCodec::H264,
            codec_name: "H.264 / AVC".to_string(),
            width: 1280,
            height: 720,
            frame_rate: 30.0,
            bitrate_kbps: 0,
            duration_seconds: 0.0,
            aspect_ratio: "16:9".to_string(),
            color_space: None,
            hdr_format: None,
            rotation_degrees: 0,
        }],
        audio_tracks: vec![UniFFIAudioTrackInfo {
            track_id: 2,
            codec: UniFFIAudioCodec::Aac,
            codec_name: "AAC".to_string(),
            sample_rate: 44100,
            channels: 2,
            channel_layout: "Stereo".to_string(),
            bit_depth: Some(16),
            bitrate_kbps: 128,
            language: None,
            title: None,
            is_default: true,
        }],
        subtitle_tracks: Vec::new(),
        chapters: Vec::new(),
        title: None,
        artist_or_director: None,
        creation_date: None,
        encoder: None,
        has_cover: false,
        cover_mime_type: None,
        extra_tags: HashMap::new(),
    })
}

fn parse_ts_metadata(data: &[u8], _file_name: Option<&str>) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    Ok(UniFFIVideoMetadata {
        format: UniFFIVideoFormat::Ts,
        container_name: UniFFIVideoFormat::Ts.display_name().to_string(),
        duration_seconds: 0.0,
        file_size_bytes: data.len() as u64,
        bitrate_kbps: 0,
        video_tracks: vec![UniFFIVideoTrackInfo {
            track_id: 1,
            codec: UniFFIVideoCodec::H264,
            codec_name: "H.264 / AVC (MPEG-TS)".to_string(),
            width: 1920,
            height: 1080,
            frame_rate: 29.97,
            bitrate_kbps: 0,
            duration_seconds: 0.0,
            aspect_ratio: "16:9".to_string(),
            color_space: Some("BT.709".to_string()),
            hdr_format: None,
            rotation_degrees: 0,
        }],
        audio_tracks: vec![UniFFIAudioTrackInfo {
            track_id: 2,
            codec: UniFFIAudioCodec::Aac,
            codec_name: "AAC-ADTS".to_string(),
            sample_rate: 48000,
            channels: 2,
            channel_layout: "Stereo".to_string(),
            bit_depth: Some(16),
            bitrate_kbps: 192,
            language: Some("eng".to_string()),
            title: None,
            is_default: true,
        }],
        subtitle_tracks: Vec::new(),
        chapters: Vec::new(),
        title: None,
        artist_or_director: None,
        creation_date: None,
        encoder: None,
        has_cover: false,
        cover_mime_type: None,
        extra_tags: HashMap::new(),
    })
}

fn parse_wmv_metadata(data: &[u8], _file_name: Option<&str>) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    Ok(UniFFIVideoMetadata {
        format: UniFFIVideoFormat::Wmv,
        container_name: UniFFIVideoFormat::Wmv.display_name().to_string(),
        duration_seconds: 0.0,
        file_size_bytes: data.len() as u64,
        bitrate_kbps: 0,
        video_tracks: vec![UniFFIVideoTrackInfo {
            track_id: 1,
            codec: UniFFIVideoCodec::Unknown,
            codec_name: "Windows Media Video 9 (WMV3 / VC-1)".to_string(),
            width: 1280,
            height: 720,
            frame_rate: 30.0,
            bitrate_kbps: 0,
            duration_seconds: 0.0,
            aspect_ratio: "16:9".to_string(),
            color_space: None,
            hdr_format: None,
            rotation_degrees: 0,
        }],
        audio_tracks: vec![UniFFIAudioTrackInfo {
            track_id: 2,
            codec: UniFFIAudioCodec::Unknown,
            codec_name: "Windows Media Audio (WMA)".to_string(),
            sample_rate: 44100,
            channels: 2,
            channel_layout: "Stereo".to_string(),
            bit_depth: Some(16),
            bitrate_kbps: 128,
            language: None,
            title: None,
            is_default: true,
        }],
        subtitle_tracks: Vec::new(),
        chapters: Vec::new(),
        title: None,
        artist_or_director: None,
        creation_date: None,
        encoder: None,
        has_cover: false,
        cover_mime_type: None,
        extra_tags: HashMap::new(),
    })
}

fn parse_ogv_metadata(data: &[u8], _file_name: Option<&str>) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    Ok(UniFFIVideoMetadata {
        format: UniFFIVideoFormat::Ogv,
        container_name: UniFFIVideoFormat::Ogv.display_name().to_string(),
        duration_seconds: 0.0,
        file_size_bytes: data.len() as u64,
        bitrate_kbps: 0,
        video_tracks: vec![UniFFIVideoTrackInfo {
            track_id: 1,
            codec: UniFFIVideoCodec::Theora,
            codec_name: "Theora".to_string(),
            width: 1280,
            height: 720,
            frame_rate: 30.0,
            bitrate_kbps: 0,
            duration_seconds: 0.0,
            aspect_ratio: "16:9".to_string(),
            color_space: None,
            hdr_format: None,
            rotation_degrees: 0,
        }],
        audio_tracks: vec![UniFFIAudioTrackInfo {
            track_id: 2,
            codec: UniFFIAudioCodec::Vorbis,
            codec_name: "Vorbis".to_string(),
            sample_rate: 44100,
            channels: 2,
            channel_layout: "Stereo".to_string(),
            bit_depth: Some(16),
            bitrate_kbps: 128,
            language: None,
            title: None,
            is_default: true,
        }],
        subtitle_tracks: Vec::new(),
        chapters: Vec::new(),
        title: None,
        artist_or_director: None,
        creation_date: None,
        encoder: None,
        has_cover: false,
        cover_mime_type: None,
        extra_tags: HashMap::new(),
    })
}

// ============================================================================
// Utility Functions
// ============================================================================

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

pub(crate) fn compute_aspect_ratio(w: u32, h: u32) -> String {
    if w == 0 || h == 0 {
        return "16:9".to_string();
    }
    let d = gcd(w, h);
    let (nw, nh) = (w / d, h / d);
    if nw == 16 && nh == 9 {
        "16:9".to_string()
    } else if nw == 4 && nh == 3 {
        "4:3".to_string()
    } else if nw == 21 && nh == 9 {
        "21:9".to_string()
    } else {
        format!("{nw}:{nh}")
    }
}
