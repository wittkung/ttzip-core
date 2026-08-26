// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.

//! SIMD-accelerated image decoding and high-quality thumbnail generation pipeline.

use std::io::Cursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_image::core_filters::colorspace::ColorspaceConv;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

/// Unified decoded RGBA8 image container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImageRgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Thumbnail resampling filter algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThumbnailFilter {
    Nearest,
    #[default]
    Bilinear,
    Lanczos3,
}

/// Image decoding and processing errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImagePipelineError {
    #[error("Empty or invalid image data stream")]
    EmptyData,
    #[error("Unsupported or unrecognized image format")]
    UnsupportedFormat,
    #[error("Image decoding failed: {0}")]
    DecodeFailed(String),
    #[error("Color conversion failed: {0}")]
    ConversionFailed(String),
    #[error("Invalid thumbnail target dimensions ({0}x{1})")]
    InvalidDimensions(u32, u32),
}

/// Decodes an image from an in-memory byte buffer into a unified RGBA8 buffer.
pub fn decode_image_rgba(data: &[u8]) -> Result<DecodedImageRgba, ImagePipelineError> {
    if data.is_empty() {
        return Err(ImagePipelineError::EmptyData);
    }
    if is_webp_header(data) {
        return decode_webp_rgba(data);
    }
    match decode_with_zune(data) {
        Ok(img) => Ok(img),
        Err(e) => {
            if is_webp_header(data) || data.starts_with(b"RIFF") {
                decode_webp_rgba(data)
            } else {
                Err(e)
            }
        }
    }
}

fn is_webp_header(data: &[u8]) -> bool {
    data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP"
}

fn decode_with_zune(data: &[u8]) -> Result<DecodedImageRgba, ImagePipelineError> {
    let mut img = Image::read(data, DecoderOptions::default())
        .map_err(|e| ImagePipelineError::DecodeFailed(format!("{e:?}")))?;

    if img.colorspace() != ColorSpace::RGBA {
        ColorspaceConv::new(ColorSpace::RGBA)
            .execute(&mut img)
            .map_err(|e| ImagePipelineError::ConversionFailed(format!("{e:?}")))?;
    }

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(ImagePipelineError::DecodeFailed("Zero dimension image".into()));
    }

    let frames = img.flatten_to_u8();
    let raw = frames.into_iter().next().ok_or_else(|| {
        ImagePipelineError::DecodeFailed("Missing decoded image frame".into())
    })?;

    let expected_len = w * h * 4;
    if raw.len() < expected_len {
        return Err(ImagePipelineError::DecodeFailed(format!(
            "Incomplete frame buffer: got {} bytes, expected {expected_len}",
            raw.len()
        )));
    }

    Ok(DecodedImageRgba {
        width: w as u32,
        height: h as u32,
        data: if raw.len() == expected_len { raw } else { raw[..expected_len].to_vec() },
    })
}

fn decode_webp_rgba(data: &[u8]) -> Result<DecodedImageRgba, ImagePipelineError> {
    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(data))
        .map_err(|e| ImagePipelineError::DecodeFailed(format!("{e:?}")))?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err(ImagePipelineError::DecodeFailed("Zero dimension WebP".into()));
    }
    let has_alpha = decoder.has_alpha();
    let buf_size = decoder.output_buffer_size().ok_or_else(|| {
        ImagePipelineError::DecodeFailed("Invalid WebP buffer size".into())
    })?;
    let mut raw = vec![0u8; buf_size];
    decoder.read_image(&mut raw)
        .map_err(|e| ImagePipelineError::DecodeFailed(format!("{e:?}")))?;

    let rgba_data = if has_alpha && raw.len() == (w * h * 4) as usize {
        raw
    } else {
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for chunk in raw.chunks_exact(if has_alpha { 4 } else { 3 }) {
            if has_alpha {
                out.extend_from_slice(chunk);
            } else {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
        }
        out
    };

    Ok(DecodedImageRgba { width: w, height: h, data: rgba_data })
}

/// Generates a high-quality downsampled thumbnail preserving aspect ratio within bounding box.
pub fn generate_thumbnail(
    data: &[u8],
    max_width: u32,
    max_height: u32,
    filter: ThumbnailFilter,
) -> Result<DecodedImageRgba, ImagePipelineError> {
    if max_width == 0 || max_height == 0 {
        return Err(ImagePipelineError::InvalidDimensions(max_width, max_height));
    }
    let src = decode_image_rgba(data)?;
    let (dst_w, dst_h) = calculate_aspect_dimensions(src.width, src.height, max_width, max_height);
    Ok(resize_rgba(&src, dst_w, dst_h, filter))
}

/// Computes bounding-box constrained dimensions maintaining aspect ratio.
#[inline]
pub fn calculate_aspect_dimensions(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 { return (1, 1); }
    let ratio = (max_w as f64 / src_w as f64).min(max_h as f64 / src_h as f64);
    let dst_w = ((src_w as f64 * ratio).round() as u32).max(1);
    let dst_h = ((src_h as f64 * ratio).round() as u32).max(1);
    (dst_w, dst_h)
}

/// Resizes an RGBA8 image to target dimensions using specified sampling filter.
pub fn resize_rgba(
    src: &DecodedImageRgba,
    dst_w: u32,
    dst_h: u32,
    filter: ThumbnailFilter,
) -> DecodedImageRgba {
    if src.width == dst_w && src.height == dst_h {
        return src.clone();
    }
    match filter {
        ThumbnailFilter::Nearest => resize_nearest(src, dst_w, dst_h),
        ThumbnailFilter::Bilinear => resize_bilinear(src, dst_w, dst_h),
        ThumbnailFilter::Lanczos3 => resize_lanczos3(src, dst_w, dst_h),
    }
}

fn resize_nearest(src: &DecodedImageRgba, dst_w: u32, dst_h: u32) -> DecodedImageRgba {
    let (sw, sh) = (src.width as usize, src.height as usize);
    let (dw, dh) = (dst_w as usize, dst_h as usize);
    let mut out = vec![0u8; dw * dh * 4];
    for dy in 0..dh {
        let sy = (dy * sh / dh).min(sh - 1);
        let row_src = sy * sw * 4;
        let row_dst = dy * dw * 4;
        for dx in 0..dw {
            let sx = (dx * sw / dw).min(sw - 1);
            let s_idx = row_src + sx * 4;
            let d_idx = row_dst + dx * 4;
            out[d_idx..d_idx + 4].copy_from_slice(&src.data[s_idx..s_idx + 4]);
        }
    }
    DecodedImageRgba { width: dst_w, height: dst_h, data: out }
}

fn resize_bilinear(src: &DecodedImageRgba, dst_w: u32, dst_h: u32) -> DecodedImageRgba {
    let (sw, sh) = (src.width as usize, src.height as usize);
    let (dw, dh) = (dst_w as usize, dst_h as usize);
    let mut out = vec![0u8; dw * dh * 4];
    let (x_scale, y_scale) = (sw as f32 / dw as f32, sh as f32 / dh as f32);

    for dy in 0..dh {
        let gy = ((dy as f32 + 0.5) * y_scale - 0.5).clamp(0.0, (sh - 1) as f32);
        let y0 = gy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let fy = gy - y0 as f32;
        let (row0, row1, dst_row) = (y0 * sw * 4, y1 * sw * 4, dy * dw * 4);

        for dx in 0..dw {
            let gx = ((dx as f32 + 0.5) * x_scale - 0.5).clamp(0.0, (sw - 1) as f32);
            let x0 = gx.floor() as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let fx = gx - x0 as f32;

            let (w00, w10) = ((1.0 - fx) * (1.0 - fy), fx * (1.0 - fy));
            let (w01, w11) = ((1.0 - fx) * fy, fx * fy);
            let (i00, i10, i01, i11) = (row0 + x0 * 4, row0 + x1 * 4, row1 + x0 * 4, row1 + x1 * 4);
            let d_idx = dst_row + dx * 4;

            for c in 0..4 {
                let v = w00 * src.data[i00 + c] as f32 + w10 * src.data[i10 + c] as f32
                    + w01 * src.data[i01 + c] as f32 + w11 * src.data[i11 + c] as f32;
                out[d_idx + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    DecodedImageRgba { width: dst_w, height: dst_h, data: out }
}

fn lanczos3_kernel(x: f32) -> f32 {
    if x.abs() < 1e-6 { return 1.0; }
    if x.abs() >= 3.0 { return 0.0; }
    let px = std::f32::consts::PI * x;
    (px.sin() * (px / 3.0).sin()) / (px * px / 3.0)
}

fn resize_lanczos3(src: &DecodedImageRgba, dst_w: u32, dst_h: u32) -> DecodedImageRgba {
    let (sw, sh) = (src.width as usize, src.height as usize);
    let (dw, dh) = (dst_w as usize, dst_h as usize);
    let mut intermediate = vec![0.0f32; dw * sh * 4];
    let x_scale = dw as f32 / sw as f32;
    let x_support = if x_scale < 1.0 { 3.0 / x_scale } else { 3.0 };

    for y in 0..sh {
        let (src_row, inter_row) = (y * sw * 4, y * dw * 4);
        for dx in 0..dw {
            let center = (dx as f32 + 0.5) / x_scale - 0.5;
            let start = (center - x_support).floor().max(0.0) as usize;
            let end = (center + x_support).ceil().min((sw - 1) as f32) as usize;
            let (mut r, mut g, mut b, mut a, mut tw) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);

            for sx in start..=end {
                let dist = (sx as f32 - center) * (if x_scale < 1.0 { x_scale } else { 1.0 });
                let w = lanczos3_kernel(dist);
                if w.abs() > 1e-7 {
                    let s = src_row + sx * 4;
                    r += src.data[s] as f32 * w; g += src.data[s + 1] as f32 * w;
                    b += src.data[s + 2] as f32 * w; a += src.data[s + 3] as f32 * w;
                    tw += w;
                }
            }
            let inv = if tw.abs() > 1e-6 { 1.0 / tw } else { 1.0 };
            let d = inter_row + dx * 4;
            intermediate[d] = r * inv; intermediate[d + 1] = g * inv;
            intermediate[d + 2] = b * inv; intermediate[d + 3] = a * inv;
        }
    }

    let mut out = vec![0u8; dw * dh * 4];
    let y_scale = dh as f32 / sh as f32;
    let y_support = if y_scale < 1.0 { 3.0 / y_scale } else { 3.0 };

    for dy in 0..dh {
        let center = (dy as f32 + 0.5) / y_scale - 0.5;
        let start = (center - y_support).floor().max(0.0) as usize;
        let end = (center + y_support).ceil().min((sh - 1) as f32) as usize;
        let dst_row = dy * dw * 4;

        for dx in 0..dw {
            let (mut r, mut g, mut b, mut a, mut tw) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in start..=end {
                let dist = (sy as f32 - center) * (if y_scale < 1.0 { y_scale } else { 1.0 });
                let w = lanczos3_kernel(dist);
                if w.abs() > 1e-7 {
                    let s = sy * dw * 4 + dx * 4;
                    r += intermediate[s] * w; g += intermediate[s + 1] * w;
                    b += intermediate[s + 2] * w; a += intermediate[s + 3] * w;
                    tw += w;
                }
            }
            let inv = if tw.abs() > 1e-6 { 1.0 / tw } else { 1.0 };
            let d = dst_row + dx * 4;
            out[d] = (r * inv).round().clamp(0.0, 255.0) as u8;
            out[d + 1] = (g * inv).round().clamp(0.0, 255.0) as u8;
            out[d + 2] = (b * inv).round().clamp(0.0, 255.0) as u8;
            out[d + 3] = (a * inv).round().clamp(0.0, 255.0) as u8;
        }
    }
    DecodedImageRgba { width: dst_w, height: dst_h, data: out }
}
