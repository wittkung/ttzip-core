// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Media and File Metadata Probing Scaffolding.
//!
//! Provides typed, Sendable metadata structures and zero-copy probing functions
//! for direct consumption in Swift UI property inspectors and quick look previews.

use std::collections::HashMap;
use std::path::Path;

use super::types::TTZipError;
use crate::standards::metadata_probe::{
    probe_metadata_buffer, probe_metadata_file, AudioProbeResult, DocumentProbeResult, FontProbeResult,
    ImageProbeResult, MediaType, Model3DProbeResult, UnifiedMetadataProbe, VideoProbeResult,
};

/// High-level media categorization exposed to Swift.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FileMediaType {
    Unknown,
    Image,
    Audio,
    Video,
    Document,
    Font,
    Model3D,
    Archive,
}

impl From<MediaType> for FileMediaType {
    fn from(m: MediaType) -> Self {
        match m {
            MediaType::Unknown => FileMediaType::Unknown,
            MediaType::Image => FileMediaType::Image,
            MediaType::Audio => FileMediaType::Audio,
            MediaType::Video => FileMediaType::Video,
            MediaType::Document => FileMediaType::Document,
            MediaType::Font => FileMediaType::Font,
            MediaType::Model3D => FileMediaType::Model3D,
            MediaType::Archive => FileMediaType::Archive,
        }
    }
}

/// Image metadata properties record exposed to Swift.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ImageMetadataRecord {
    pub width: u32,
    pub height: u32,
    pub orientation: u32,
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

impl From<ImageProbeResult> for ImageMetadataRecord {
    fn from(p: ImageProbeResult) -> Self {
        Self {
            width: p.width,
            height: p.height,
            orientation: p.orientation,
            bit_depth: p.bit_depth,
            color_space: p.color_space,
            has_alpha: p.has_alpha,
            camera_make: p.camera_make,
            camera_model: p.camera_model,
            lens_model: p.lens_model,
            focal_length_mm: p.focal_length_mm,
            f_number: p.f_number,
            exposure_time_secs: p.exposure_time_secs,
            iso_speed: p.iso_speed,
            date_time_original: p.date_time_original,
            icc_profile_name: p.icc_profile_name,
        }
    }
}

/// Audio metadata properties record exposed to Swift.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct AudioMetadataRecord {
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

impl From<AudioProbeResult> for AudioMetadataRecord {
    fn from(p: AudioProbeResult) -> Self {
        Self {
            duration_secs: p.duration_secs,
            sample_rate: p.sample_rate,
            channels: p.channels,
            bit_depth: p.bit_depth,
            bitrate_kbps: p.bitrate_kbps,
            codec: p.codec,
            title: p.title,
            artist: p.artist,
            album: p.album,
        }
    }
}

/// Video metadata properties record exposed to Swift.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct VideoMetadataRecord {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub bitrate_kbps: u32,
    pub orientation_degrees: u32,
}

impl From<VideoProbeResult> for VideoMetadataRecord {
    fn from(p: VideoProbeResult) -> Self {
        Self {
            duration_secs: p.duration_secs,
            width: p.width,
            height: p.height,
            frame_rate: p.frame_rate,
            video_codec: p.video_codec,
            audio_codec: p.audio_codec,
            audio_sample_rate: p.audio_sample_rate,
            audio_channels: p.audio_channels,
            bitrate_kbps: p.bitrate_kbps,
            orientation_degrees: p.orientation_degrees,
        }
    }
}

/// Font metadata properties record exposed to Swift.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FontMetadataRecord {
    pub font_family: Option<String>,
    pub font_subfamily: Option<String>,
    pub postscript_name: Option<String>,
    pub units_per_em: u32,
    pub num_glyphs: u32,
    pub is_variable: bool,
    pub format_flavor: String,
}

impl From<FontProbeResult> for FontMetadataRecord {
    fn from(p: FontProbeResult) -> Self {
        Self {
            font_family: p.font_family,
            font_subfamily: p.font_subfamily,
            postscript_name: p.postscript_name,
            units_per_em: p.units_per_em,
            num_glyphs: p.num_glyphs,
            is_variable: p.is_variable,
            format_flavor: p.format_flavor,
        }
    }
}

/// 3D Model metadata properties record exposed to Swift.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Model3DMetadataRecord {
    pub format_name: String,
    pub triangle_count: Option<u64>,
    pub vertex_count: Option<u64>,
    pub generator_version: Option<String>,
}

impl From<Model3DProbeResult> for Model3DMetadataRecord {
    fn from(p: Model3DProbeResult) -> Self {
        Self {
            format_name: p.format_name,
            triangle_count: p.triangle_count,
            vertex_count: p.vertex_count,
            generator_version: p.generator_version,
        }
    }
}

/// Document metadata properties record exposed to Swift.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DocumentMetadataRecord {
    pub format_name: String,
    pub version: Option<String>,
    pub page_count: Option<u32>,
    pub title: Option<String>,
    pub author: Option<String>,
}

impl From<DocumentProbeResult> for DocumentMetadataRecord {
    fn from(p: DocumentProbeResult) -> Self {
        Self {
            format_name: p.format_name,
            version: p.version,
            page_count: p.page_count,
            title: p.title,
            author: p.author,
        }
    }
}

/// Comprehensive metadata record returned to Swift.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FileMetadataRecord {
    pub media_type: FileMediaType,
    pub format_name: String,
    pub mime_type: String,
    pub file_size: u64,
    pub is_container: bool,
    pub image: Option<ImageMetadataRecord>,
    pub audio: Option<AudioMetadataRecord>,
    pub video: Option<VideoMetadataRecord>,
    pub font: Option<FontMetadataRecord>,
    pub model_3d: Option<Model3DMetadataRecord>,
    pub document: Option<DocumentMetadataRecord>,
    pub attributes: HashMap<String, String>,
}

impl From<UnifiedMetadataProbe> for FileMetadataRecord {
    fn from(p: UnifiedMetadataProbe) -> Self {
        Self {
            media_type: p.media_type.into(),
            format_name: p.format_name,
            mime_type: p.mime_type,
            file_size: p.file_size,
            is_container: p.is_container,
            image: p.image.map(Into::into),
            audio: p.audio.map(Into::into),
            video: p.video.map(Into::into),
            font: p.font.map(Into::into),
            model_3d: p.model_3d.map(Into::into),
            document: p.document.map(Into::into),
            attributes: p.attributes,
        }
    }
}

/// Probes full file metadata from a file on disk using zero-copy memory mapping.
#[uniffi::export]
pub fn probe_file_metadata(path: String) -> Result<FileMetadataRecord, TTZipError> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path });
    }

    probe_metadata_file(p)
        .map(Into::into)
        .map_err(|e| TTZipError::IoError {
            message: format!("Failed to probe file metadata: {e}"),
        })
}

/// Probes file metadata from an in-memory byte buffer.
#[uniffi::export]
pub fn probe_buffer_metadata(
    data: Vec<u8>,
    filename_hint: Option<String>,
) -> Result<FileMetadataRecord, TTZipError> {
    let probe = probe_metadata_buffer(&data, filename_hint.as_deref(), None);
    Ok(probe.into())
}
