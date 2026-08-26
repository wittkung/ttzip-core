// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed domain models for metadata probe results across all media formats.

use std::collections::HashMap;

/// Top-level media classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaType {
    Unknown,
    Image,
    Audio,
    Video,
    Document,
    Font,
    Model3D,
    Archive,
}

impl MediaType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Image => "Image",
            Self::Audio => "Audio",
            Self::Video => "Video",
            Self::Document => "Document",
            Self::Font => "Font",
            Self::Model3D => "3D Model",
            Self::Archive => "Archive",
        }
    }
}

/// Image metadata probe result.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageProbeResult {
    pub width: u32,
    pub height: u32,
    pub orientation: u32, // 1..8 (EXIF standard)
    pub bit_depth: u32,
    pub color_space: Option<String>,
    pub has_alpha: bool,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length_mm: Option<f64>,
    pub f_number: Option<f64>,
    pub exposure_time_secs: Option<f64>,
    pub iso_speed: Option<u32>,
    pub date_time_original: Option<String>,
    pub icc_profile_name: Option<String>,
}

/// Audio metadata probe result.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioProbeResult {
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub channels: u32,
    pub bit_depth: u32,
    pub bitrate_kbps: u32,
    pub codec: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

/// Video metadata probe result.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoProbeResult {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub bitrate_kbps: u32,
    pub orientation_degrees: u32, // 0, 90, 180, 270
}

/// Font metadata probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontProbeResult {
    pub font_family: Option<String>,
    pub font_subfamily: Option<String>,
    pub postscript_name: Option<String>,
    pub units_per_em: u32,
    pub num_glyphs: u32,
    pub is_variable: bool,
    pub format_flavor: String,
}

/// 3D Model metadata probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model3DProbeResult {
    pub format_name: String,
    pub triangle_count: Option<u64>,
    pub vertex_count: Option<u64>,
    pub generator_version: Option<String>,
}

/// Document metadata probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentProbeResult {
    pub format_name: String,
    pub version: Option<String>,
    pub page_count: Option<u32>,
    pub title: Option<String>,
    pub author: Option<String>,
}

/// Unified file metadata probe outcome containing strong-typed media properties
/// and an inspector attributes dictionary.
#[derive(Debug, Clone, PartialEq)]
pub struct UnifiedMetadataProbe {
    pub media_type: MediaType,
    pub format_name: String,
    pub mime_type: String,
    pub file_size: u64,
    pub is_container: bool,
    pub image: Option<ImageProbeResult>,
    pub audio: Option<AudioProbeResult>,
    pub video: Option<VideoProbeResult>,
    pub font: Option<FontProbeResult>,
    pub model_3d: Option<Model3DProbeResult>,
    pub document: Option<DocumentProbeResult>,
    pub attributes: HashMap<String, String>,
}

impl UnifiedMetadataProbe {
    /// Creates an empty/unknown fallback probe record.
    #[must_use]
    pub fn unknown(file_size: u64) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert("Media Type".to_string(), "Unknown".to_string());
        attributes.insert("Format".to_string(), "Binary Data".to_string());
        attributes.insert("MIME Type".to_string(), "application/octet-stream".to_string());
        attributes.insert("File Size".to_string(), format!("{file_size} bytes"));

        Self {
            media_type: MediaType::Unknown,
            format_name: "Binary Data".to_string(),
            mime_type: "application/octet-stream".to_string(),
            file_size,
            is_container: false,
            image: None,
            audio: None,
            video: None,
            font: None,
            model_3d: None,
            document: None,
            attributes,
        }
    }
}
