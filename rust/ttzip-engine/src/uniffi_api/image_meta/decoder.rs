// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-allocation probing, SIMD-accelerated decoding, and viewport sampling engine.

use std::time::Instant;

use super::types::{
    UniFFIImageFrame, UniFFIImageInfo, UniFFIThumbnailResult, UniFFIViewportCropParams,
    UniFFIViewportTile,
};
use crate::standards::image_pipeline::{
    calculate_aspect_dimensions, decode_image_rgba, resize_rgba, DecodedImageRgba, ThumbnailFilter,
};
use crate::standards::metadata_probe::image as probe;
use crate::uniffi_api::types::TTZipError;

/// Detects image format, geometry, and EXIF metadata without full pixel buffer decompression.
pub fn probe_image_bytes(data: &[u8], file_name: Option<&str>) -> Result<UniFFIImageInfo, TTZipError> {
    if data.is_empty() {
        return Err(TTZipError::IoError {
            message: "Empty image data buffer".to_string(),
        });
    }

    let byte_size = data.len() as u64;

    // 1. Try zero-copy standard header probes
    if let Some(res) = probe::probe_png(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "PNG".to_string(),
            color_space: res.color_space.unwrap_or_else(|| "sRGB".to_string()),
            has_alpha: res.has_alpha,
            bit_depth: res.bit_depth,
            orientation: res.orientation,
            frame_count: 1,
            camera_make: res.camera_make,
            camera_model: res.camera_model,
            lens_model: res.lens_model,
            iso_speed: res.iso_speed,
            f_number: res.f_number,
            exposure_time_secs: res.exposure_time_secs,
            focal_length_mm: res.focal_length_mm,
            date_time_original: res.date_time_original,
            icc_profile_name: res.icc_profile_name,
            byte_size,
        });
    }

    if let Some(res) = probe::probe_jpeg(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "JPEG".to_string(),
            color_space: res.color_space.unwrap_or_else(|| "sRGB".to_string()),
            has_alpha: res.has_alpha,
            bit_depth: res.bit_depth,
            orientation: res.orientation,
            frame_count: 1,
            camera_make: res.camera_make,
            camera_model: res.camera_model,
            lens_model: res.lens_model,
            iso_speed: res.iso_speed,
            f_number: res.f_number,
            exposure_time_secs: res.exposure_time_secs,
            focal_length_mm: res.focal_length_mm,
            date_time_original: res.date_time_original,
            icc_profile_name: res.icc_profile_name,
            byte_size,
        });
    }

    if let Some(res) = probe::probe_webp(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "WebP".to_string(),
            color_space: res.color_space.unwrap_or_else(|| "sRGB".to_string()),
            has_alpha: res.has_alpha,
            bit_depth: res.bit_depth,
            orientation: res.orientation,
            frame_count: 1,
            camera_make: res.camera_make,
            camera_model: res.camera_model,
            lens_model: res.lens_model,
            iso_speed: res.iso_speed,
            f_number: res.f_number,
            exposure_time_secs: res.exposure_time_secs,
            focal_length_mm: res.focal_length_mm,
            date_time_original: res.date_time_original,
            icc_profile_name: res.icc_profile_name,
            byte_size,
        });
    }

    if let Some(res) = probe::probe_gif(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "GIF".to_string(),
            color_space: res.color_space.unwrap_or_else(|| "Indexed Color".to_string()),
            has_alpha: res.has_alpha,
            bit_depth: res.bit_depth,
            orientation: 1,
            frame_count: 1,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            iso_speed: None,
            f_number: None,
            exposure_time_secs: None,
            focal_length_mm: None,
            date_time_original: None,
            icc_profile_name: None,
            byte_size,
        });
    }

    if let Some(res) = probe::probe_bmp(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "BMP".to_string(),
            color_space: res.color_space.unwrap_or_else(|| "sRGB".to_string()),
            has_alpha: res.has_alpha,
            bit_depth: res.bit_depth,
            orientation: 1,
            frame_count: 1,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            iso_speed: None,
            f_number: None,
            exposure_time_secs: None,
            focal_length_mm: None,
            date_time_original: None,
            icc_profile_name: None,
            byte_size,
        });
    }

    if let Some(res) = probe::probe_tiff(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "TIFF".to_string(),
            color_space: res.color_space.unwrap_or_else(|| "sRGB".to_string()),
            has_alpha: res.has_alpha,
            bit_depth: res.bit_depth,
            orientation: res.orientation,
            frame_count: 1,
            camera_make: res.camera_make,
            camera_model: res.camera_model,
            lens_model: res.lens_model,
            iso_speed: res.iso_speed,
            f_number: res.f_number,
            exposure_time_secs: res.exposure_time_secs,
            focal_length_mm: res.focal_length_mm,
            date_time_original: res.date_time_original,
            icc_profile_name: res.icc_profile_name,
            byte_size,
        });
    }

    if let Some(res) = probe::probe_ico(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "ICO".to_string(),
            color_space: "sRGB".to_string(),
            has_alpha: true,
            bit_depth: res.bit_depth,
            orientation: 1,
            frame_count: 1,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            iso_speed: None,
            f_number: None,
            exposure_time_secs: None,
            focal_length_mm: None,
            date_time_original: None,
            icc_profile_name: None,
            byte_size,
        });
    }

    if let Some(res) = probe::probe_psd(data) {
        return Ok(UniFFIImageInfo {
            width: res.width,
            height: res.height,
            format_name: "PSD".to_string(),
            color_space: res.color_space.unwrap_or_else(|| "RGB".to_string()),
            has_alpha: res.has_alpha,
            bit_depth: res.bit_depth,
            orientation: 1,
            frame_count: 1,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            iso_speed: None,
            f_number: None,
            exposure_time_secs: None,
            focal_length_mm: None,
            date_time_original: None,
            icc_profile_name: None,
            byte_size,
        });
    }

    // 2. Magic byte / extension fallback
    let format_name = infer_format_name(data, file_name);
    let decoded = decode_image_rgba(data).map_err(|e| TTZipError::IoError {
        message: format!("Image format probe and fallback decode failed: {e}"),
    })?;

    Ok(UniFFIImageInfo {
        width: decoded.width,
        height: decoded.height,
        format_name,
        color_space: "sRGB".to_string(),
        has_alpha: true,
        bit_depth: 32,
        orientation: 1,
        frame_count: 1,
        camera_make: None,
        camera_model: None,
        lens_model: None,
        iso_speed: None,
        f_number: None,
        exposure_time_secs: None,
        focal_length_mm: None,
        date_time_original: None,
        icc_profile_name: None,
        byte_size,
    })
}

/// Decodes an image from in-memory bytes into unified RGBA8 format with optional dimension cap.
pub fn decode_image_bytes(data: &[u8], max_dimension: Option<u32>) -> Result<UniFFIImageFrame, TTZipError> {
    if data.is_empty() {
        return Err(TTZipError::IoError {
            message: "Empty image data buffer".to_string(),
        });
    }

    let decoded = decode_image_rgba(data).map_err(|e| TTZipError::IoError {
        message: format!("Image decode failed: {e}"),
    })?;

    let final_img = if let Some(max_dim) = max_dimension {
        if max_dim > 0 && (decoded.width > max_dim || decoded.height > max_dim) {
            let (dst_w, dst_h) = calculate_aspect_dimensions(decoded.width, decoded.height, max_dim, max_dim);
            resize_rgba(&decoded, dst_w, dst_h, ThumbnailFilter::Bilinear)
        } else {
            decoded
        }
    } else {
        decoded
    };

    let stride = final_img.width.saturating_mul(4);
    Ok(UniFFIImageFrame {
        width: final_img.width,
        height: final_img.height,
        stride,
        rgba_bytes: final_img.data,
        color_space: "sRGB".to_string(),
        duration_ms: None,
    })
}

/// Generates a downsampled thumbnail preserving aspect ratio within bounding limits.
pub fn extract_thumbnail_bytes(
    data: &[u8],
    max_width: u32,
    max_height: u32,
    filter_type: Option<&str>,
) -> Result<UniFFIThumbnailResult, TTZipError> {
    if data.is_empty() {
        return Err(TTZipError::IoError {
            message: "Empty image data buffer".to_string(),
        });
    }
    if max_width == 0 || max_height == 0 {
        return Err(TTZipError::IoError {
            message: format!("Invalid thumbnail dimensions: {max_width}x{max_height}"),
        });
    }

    let start = Instant::now();
    let filter = parse_filter(filter_type);
    let src = decode_image_rgba(data).map_err(|e| TTZipError::IoError {
        message: format!("Thumbnail extraction failed during source decode: {e}"),
    })?;

    let (dst_w, dst_h) = calculate_aspect_dimensions(src.width, src.height, max_width, max_height);
    let resized = resize_rgba(&src, dst_w, dst_h, filter);
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    let scale_factor = if src.width > 0 && src.height > 0 {
        (dst_w as f64 / src.width as f64).min(dst_h as f64 / src.height as f64)
    } else {
        1.0
    };

    let stride = resized.width.saturating_mul(4);
    Ok(UniFFIThumbnailResult {
        width: resized.width,
        height: resized.height,
        stride,
        rgba_bytes: resized.data,
        scale_factor,
        duration_ms,
    })
}

/// Samples a cropped viewport rectangle from high-resolution image data and scales to target tile size.
pub fn sample_viewport_bytes(
    data: &[u8],
    params: &UniFFIViewportCropParams,
) -> Result<UniFFIViewportTile, TTZipError> {
    if data.is_empty() {
        return Err(TTZipError::IoError {
            message: "Empty image data buffer".to_string(),
        });
    }

    let src = decode_image_rgba(data).map_err(|e| TTZipError::IoError {
        message: format!("Viewport sampling failed during source decode: {e}"),
    })?;

    if src.width == 0 || src.height == 0 {
        return Err(TTZipError::IoError {
            message: "Source image has zero dimensions".to_string(),
        });
    }

    // Clamp crop rectangle safely within source image bounds
    let valid_x = params.crop_x.min(src.width.saturating_sub(1));
    let valid_y = params.crop_y.min(src.height.saturating_sub(1));
    let max_avail_w = src.width.saturating_sub(valid_x).max(1);
    let max_avail_h = src.height.saturating_sub(valid_y).max(1);

    let actual_crop_w = if params.crop_width == 0 {
        max_avail_w
    } else {
        params.crop_width.min(max_avail_w)
    };
    let actual_crop_h = if params.crop_height == 0 {
        max_avail_h
    } else {
        params.crop_height.min(max_avail_h)
    };

    // Extract cropped sub-rectangle buffer
    let mut cropped_data = Vec::with_capacity((actual_crop_w * actual_crop_h * 4) as usize);
    let src_stride = (src.width * 4) as usize;
    let crop_row_bytes = (actual_crop_w * 4) as usize;

    for row in 0..actual_crop_h {
        let sy = (valid_y + row) as usize;
        let start_idx = sy * src_stride + (valid_x * 4) as usize;
        let end_idx = start_idx + crop_row_bytes;
        if end_idx <= src.data.len() {
            cropped_data.extend_from_slice(&src.data[start_idx..end_idx]);
        } else {
            cropped_data.resize(cropped_data.len() + crop_row_bytes, 0);
        }
    }

    let cropped_img = DecodedImageRgba {
        width: actual_crop_w,
        height: actual_crop_h,
        data: cropped_data,
    };

    let dst_w = if params.target_width == 0 { actual_crop_w } else { params.target_width };
    let dst_h = if params.target_height == 0 { actual_crop_h } else { params.target_height };

    let final_tile = if dst_w == actual_crop_w && dst_h == actual_crop_h {
        cropped_img
    } else {
        resize_rgba(&cropped_img, dst_w, dst_h, ThumbnailFilter::Bilinear)
    };

    // Calculate level of detail pyramid level
    let downsample_ratio = (actual_crop_w as f64 / dst_w as f64).max(actual_crop_h as f64 / dst_h as f64);
    let lod_level = if downsample_ratio > 1.0 {
        downsample_ratio.log2().floor() as u32
    } else {
        0
    };

    let stride = final_tile.width.saturating_mul(4);
    Ok(UniFFIViewportTile {
        tile_x: valid_x,
        tile_y: valid_y,
        tile_width: final_tile.width,
        tile_height: final_tile.height,
        stride,
        rgba_bytes: final_tile.data,
        lod_level,
    })
}

fn parse_filter(filter_type: Option<&str>) -> ThumbnailFilter {
    match filter_type.map(|s| s.trim().to_lowercase()).as_deref() {
        Some("nearest") => ThumbnailFilter::Nearest,
        Some("lanczos") | Some("lanczos3") => ThumbnailFilter::Lanczos3,
        _ => ThumbnailFilter::Bilinear,
    }
}

fn infer_format_name(data: &[u8], file_name: Option<&str>) -> String {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "PNG".to_string();
    }
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "JPEG".to_string();
    }
    if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        return "WebP".to_string();
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return "GIF".to_string();
    }
    if data.starts_with(b"BM") {
        return "BMP".to_string();
    }
    if data.starts_with(b"II\x2A\x00") || data.starts_with(b"MM\x00\x2A") {
        return "TIFF".to_string();
    }
    if data.starts_with(b"8BPS") {
        return "PSD".to_string();
    }
    if data.starts_with(b"qoif") {
        return "QOI".to_string();
    }
    if data.starts_with(b"#?RADIANCE") {
        return "HDR".to_string();
    }

    if let Some(name) = file_name {
        let ext = std::path::Path::new(name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_uppercase();
        if !ext.is_empty() {
            return ext;
        }
    }

    "Unknown".to_string()
}
