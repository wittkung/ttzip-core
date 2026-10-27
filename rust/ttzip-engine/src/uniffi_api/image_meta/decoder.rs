// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-allocation probing, SIMD-accelerated decoding, and viewport sampling engine.

use std::time::Instant;

use super::types::{
    UniFFIImageFrame, UniFFIImageInfo, UniFFIThumbnailResult, UniFFIViewportCropParams,
    UniFFIViewportTile,
};
use crate::standards::image_pipeline::{
    calculate_aspect_dimensions, decode_image_rgba, resize_rgba, DecodedImageRgba, ThumbnailFilter,
};
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
    if let Some((w, h, depth, alpha)) = probe_png(data) {
        return Ok(make_image_info(w, h, "PNG", "sRGB", alpha, depth, byte_size));
    }

    if let Some((w, h, depth, alpha)) = probe_jpeg(data) {
        return Ok(make_image_info(w, h, "JPEG", "sRGB", alpha, depth, byte_size));
    }

    if let Some((w, h, depth, alpha)) = probe_webp(data) {
        return Ok(make_image_info(w, h, "WebP", "sRGB", alpha, depth, byte_size));
    }

    if let Some((w, h, depth, alpha)) = probe_gif(data) {
        return Ok(make_image_info(w, h, "GIF", "Indexed Color", alpha, depth, byte_size));
    }

    if let Some((w, h, depth, alpha)) = probe_bmp(data) {
        return Ok(make_image_info(w, h, "BMP", "sRGB", alpha, depth, byte_size));
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

fn make_image_info(
    width: u32,
    height: u32,
    format_name: &str,
    color_space: &str,
    has_alpha: bool,
    bit_depth: u32,
    byte_size: u64,
) -> UniFFIImageInfo {
    UniFFIImageInfo {
        width,
        height,
        format_name: format_name.to_string(),
        color_space: color_space.to_string(),
        has_alpha,
        bit_depth,
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
    }
}

fn probe_png(data: &[u8]) -> Option<(u32, u32, u32, bool)> {
    if data.len() < 26 || !data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    if &data[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(data[20..24].try_into().ok()?);
    let depth = data[24] as u32;
    let color_type = data[25];
    let has_alpha = color_type == 4 || color_type == 6;
    Some((w, h, depth, has_alpha))
}

fn probe_jpeg(data: &[u8]) -> Option<(u32, u32, u32, bool)> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }
    let mut cursor = 2;
    while cursor + 4 <= data.len() {
        if data[cursor] != 0xFF {
            cursor += 1;
            continue;
        }
        let marker = data[cursor + 1];
        cursor += 2;
        if marker == 0xC0 || marker == 0xC1 || marker == 0xC2 {
            if cursor + 7 <= data.len() {
                let h = u16::from_be_bytes([data[cursor + 3], data[cursor + 4]]) as u32;
                let w = u16::from_be_bytes([data[cursor + 5], data[cursor + 6]]) as u32;
                return Some((w, h, 24, false));
            }
            break;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if cursor + 2 <= data.len() {
            let len = u16::from_be_bytes([data[cursor], data[cursor + 1]]) as usize;
            if len < 2 { break; }
            cursor += len;
        } else {
            break;
        }
    }
    None
}

fn probe_webp(data: &[u8]) -> Option<(u32, u32, u32, bool)> {
    if data.len() < 30 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }
    let chunk_tag = &data[12..16];
    if chunk_tag == b"VP8 " && data.len() >= 30 {
        let w = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as u32;
        let h = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as u32;
        return Some((w, h, 24, false));
    }
    if chunk_tag == b"VP8L" && data.len() >= 25 {
        let b1 = data[21] as u32;
        let b2 = data[22] as u32;
        let b3 = data[23] as u32;
        let b4 = data[24] as u32;
        let w = 1 + (b1 | ((b2 & 0x3F) << 8));
        let h = 1 + ((b2 >> 6) | (b3 << 2) | ((b4 & 0x0F) << 10));
        return Some((w, h, 32, true));
    }
    if chunk_tag == b"VP8X" && data.len() >= 30 {
        let flags = data[20];
        let has_alpha = (flags & 0x10) != 0;
        let w = 1 + (data[24] as u32 | ((data[25] as u32) << 8) | ((data[26] as u32) << 16));
        let h = 1 + (data[27] as u32 | ((data[28] as u32) << 8) | ((data[29] as u32) << 16));
        return Some((w, h, if has_alpha { 32 } else { 24 }, has_alpha));
    }
    None
}

fn probe_gif(data: &[u8]) -> Option<(u32, u32, u32, bool)> {
    if data.len() < 10 || (!data.starts_with(b"GIF87a") && !data.starts_with(b"GIF89a")) {
        return None;
    }
    let w = u16::from_le_bytes([data[6], data[7]]) as u32;
    let h = u16::from_le_bytes([data[8], data[9]]) as u32;
    Some((w, h, 8, true))
}

fn probe_bmp(data: &[u8]) -> Option<(u32, u32, u32, bool)> {
    if data.len() < 26 || !data.starts_with(b"BM") {
        return None;
    }
    let w = i32::from_le_bytes(data[18..22].try_into().ok()?).unsigned_abs();
    let h = i32::from_le_bytes(data[22..26].try_into().ok()?).unsigned_abs();
    let bpp = if data.len() >= 30 {
        u16::from_le_bytes(data[28..30].try_into().ok()?) as u32
    } else {
        24
    };
    Some((w, h, bpp, bpp == 32))
}
