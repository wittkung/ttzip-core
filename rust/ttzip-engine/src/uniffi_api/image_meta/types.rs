// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records and Types for Image Decoding, Metadata, and Viewport Rendering.

/// Comprehensive image metadata and EXIF introspection record.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIImageInfo {
    /// Pixel width of the image.
    pub width: u32,
    /// Pixel height of the image.
    pub height: u32,
    /// Detected format classification (e.g. "PNG", "JPEG", "WebP", "GIF", "BMP", "TIFF", "ICO", "PSD", "QOI", "HDR").
    pub format_name: String,
    /// Color space descriptor (e.g. "sRGB", "Display P3", "Adobe RGB", "Grayscale", "Indexed Color").
    pub color_space: String,
    /// Whether the image contains an active alpha transparency channel.
    pub has_alpha: bool,
    /// Bits per pixel/channel depth (e.g. 8, 16, 24, 32).
    pub bit_depth: u32,
    /// EXIF orientation tag (1..=8, default 1 for normal orientation).
    pub orientation: u32,
    /// Total number of animation frames (1 for static images).
    pub frame_count: u32,
    /// Camera manufacturer name if available in EXIF tags.
    pub camera_make: Option<String>,
    /// Camera model name if available in EXIF tags.
    pub camera_model: Option<String>,
    /// Lens model specification if available in EXIF tags.
    pub lens_model: Option<String>,
    /// ISO speed rating if available in EXIF tags.
    pub iso_speed: Option<u32>,
    /// Lens aperture f-number if available in EXIF tags.
    pub f_number: Option<f64>,
    /// Exposure time in seconds if available in EXIF tags.
    pub exposure_time_secs: Option<f64>,
    /// Focal length in millimeters if available in EXIF tags.
    pub focal_length_mm: Option<f64>,
    /// Original capture date/time string if available in EXIF tags.
    pub date_time_original: Option<String>,
    /// Embedded ICC color profile name if available.
    pub icc_profile_name: Option<String>,
    /// Total byte size of the raw compressed image buffer.
    pub byte_size: u64,
}

/// Decoded full-frame RGBA8 pixel buffer with dimensions and row stride.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIImageFrame {
    /// Pixel width of the decoded image.
    pub width: u32,
    /// Pixel height of the decoded image.
    pub height: u32,
    /// Row stride in bytes (typically `width * 4` for 32-bit RGBA).
    pub stride: u32,
    /// Flat RGBA8 pixel byte buffer in row-major order.
    pub rgba_bytes: Vec<u8>,
    /// Target color space identifier (e.g. "sRGB").
    pub color_space: String,
    /// Animation frame duration in milliseconds, if applicable.
    pub duration_ms: Option<u32>,
}

/// High-performance downsampled thumbnail generation result with execution metrics.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFIThumbnailResult {
    /// Pixel width of the generated thumbnail.
    pub width: u32,
    /// Pixel height of the generated thumbnail.
    pub height: u32,
    /// Row stride in bytes (`width * 4`).
    pub stride: u32,
    /// Flat RGBA8 pixel byte buffer.
    pub rgba_bytes: Vec<u8>,
    /// Effective scale factor relative to original dimensions.
    pub scale_factor: f64,
    /// Wall-clock thumbnail generation latency in milliseconds.
    pub duration_ms: f64,
}

/// Sampled sub-region tile for high-resolution deep zoom viewports.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIViewportTile {
    /// Origin X coordinate of the crop rectangle in original image coordinates.
    pub tile_x: u32,
    /// Origin Y coordinate of the crop rectangle in original image coordinates.
    pub tile_y: u32,
    /// Rendered pixel width of the output tile.
    pub tile_width: u32,
    /// Rendered pixel height of the output tile.
    pub tile_height: u32,
    /// Row stride in bytes (`tile_width * 4`).
    pub stride: u32,
    /// Flat RGBA8 pixel byte buffer.
    pub rgba_bytes: Vec<u8>,
    /// Level of detail (LOD) pyramid level (0 = 1:1 full resolution, 1 = 1/2, 2 = 1/4, etc.).
    pub lod_level: u32,
}

/// Viewport sampling crop and dimension configuration record.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIViewportCropParams {
    pub crop_x: u32,
    pub crop_y: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    pub target_width: u32,
    pub target_height: u32,
}
