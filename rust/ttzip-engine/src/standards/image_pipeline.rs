// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Safe Rust image decoding and high-quality thumbnail generation pipeline.

use std::io::{Cursor, Read};
use flate2::read::ZlibDecoder;

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
    if is_png_header(data) {
        return decode_png_rgba(data);
    }
    if is_bmp_header(data) {
        return decode_bmp_rgba(data);
    }
    Err(ImagePipelineError::DecodeFailed("Unsupported or unrecognized image format".into()))
}

#[inline]
fn is_webp_header(data: &[u8]) -> bool {
    data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP"
}

#[inline]
fn is_png_header(data: &[u8]) -> bool {
    data.len() >= 8 && data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
}

#[inline]
fn is_bmp_header(data: &[u8]) -> bool {
    data.len() >= 26 && data.starts_with(b"BM")
}

fn decode_bmp_rgba(data: &[u8]) -> Result<DecodedImageRgba, ImagePipelineError> {
    if data.len() < 54 {
        return Err(ImagePipelineError::DecodeFailed("BMP payload smaller than standard header".into()));
    }
    let pixel_offset = u32::from_le_bytes(
        data[10..14].try_into().map_err(|_| ImagePipelineError::DecodeFailed("Invalid BMP offset".into()))?,
    ) as usize;

    let width = i32::from_le_bytes(
        data[18..22].try_into().map_err(|_| ImagePipelineError::DecodeFailed("Invalid BMP width".into()))?,
    );
    let height = i32::from_le_bytes(
        data[22..26].try_into().map_err(|_| ImagePipelineError::DecodeFailed("Invalid BMP height".into()))?,
    );
    let bpp = u16::from_le_bytes(
        data[28..30].try_into().map_err(|_| ImagePipelineError::DecodeFailed("Invalid BMP bpp".into()))?,
    );

    if width <= 0 || height == 0 {
        return Err(ImagePipelineError::DecodeFailed("Invalid BMP dimensions".into()));
    }
    let w = width as usize;
    let h = height.unsigned_abs() as usize;
    let is_top_down = height < 0;

    if bpp != 24 && bpp != 32 {
        return Err(ImagePipelineError::DecodeFailed(format!("Unsupported BMP bit depth: {bpp}")));
    }

    let bytes_per_pixel = (bpp / 8) as usize;
    let row_stride = (w * bytes_per_pixel).div_ceil(4) * 4;
    let mut out = vec![0u8; w * h * 4];

    for row in 0..h {
        let src_row = if is_top_down { row } else { h - 1 - row };
        let row_start = pixel_offset + src_row * row_stride;
        if row_start + w * bytes_per_pixel > data.len() {
            return Err(ImagePipelineError::DecodeFailed("Unexpected EOF reading BMP scanline".into()));
        }
        let src_slice = &data[row_start..row_start + w * bytes_per_pixel];
        let dst_row_start = row * w * 4;

        if bpp == 24 {
            for col in 0..w {
                let s_idx = col * 3;
                let d_idx = dst_row_start + col * 4;
                out[d_idx] = src_slice[s_idx + 2];     // R
                out[d_idx + 1] = src_slice[s_idx + 1]; // G
                out[d_idx + 2] = src_slice[s_idx];     // B
                out[d_idx + 3] = 255;                  // A
            }
        } else {
            for col in 0..w {
                let s_idx = col * 4;
                let d_idx = dst_row_start + col * 4;
                out[d_idx] = src_slice[s_idx + 2];     // R
                out[d_idx + 1] = src_slice[s_idx + 1]; // G
                out[d_idx + 2] = src_slice[s_idx];     // B
                out[d_idx + 3] = src_slice[s_idx + 3]; // A
            }
        }
    }

    Ok(DecodedImageRgba {
        width: w as u32,
        height: h as u32,
        data: out,
    })
}

fn decode_png_rgba(data: &[u8]) -> Result<DecodedImageRgba, ImagePipelineError> {
    let mut cursor = 8;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut color_type = 6u8; // default to RGBA
    let mut idat_combined = Vec::new();

    while cursor + 8 <= data.len() {
        let length = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
        let chunk_type = &data[cursor + 4..cursor + 8];
        cursor += 8;

        if cursor + length > data.len() {
            return Err(ImagePipelineError::DecodeFailed("Truncated PNG chunk".into()));
        }
        let chunk_data = &data[cursor..cursor + length];
        cursor += length + 4; // skip data + CRC

        match chunk_type {
            b"IHDR" => {
                if chunk_data.len() < 13 {
                    return Err(ImagePipelineError::DecodeFailed("Invalid IHDR chunk length".into()));
                }
                width = u32::from_be_bytes(chunk_data[0..4].try_into().unwrap());
                height = u32::from_be_bytes(chunk_data[4..8].try_into().unwrap());
                let bit_depth = chunk_data[8];
                color_type = chunk_data[9];
                if bit_depth != 8 {
                    return Err(ImagePipelineError::DecodeFailed(format!("Unsupported PNG bit depth: {bit_depth}")));
                }
            }
            b"IDAT" => {
                idat_combined.extend_from_slice(chunk_data);
            }
            b"IEND" => break,
            _ => {}
        }
    }

    if width == 0 || height == 0 {
        return Err(ImagePipelineError::DecodeFailed("Zero dimension PNG".into()));
    }

    let bpp: usize = match color_type {
        0 => 1, // Grayscale
        2 => 3, // RGB
        6 => 4, // RGBA
        _ => return Err(ImagePipelineError::DecodeFailed(format!("Unsupported PNG color type: {color_type}"))),
    };

    let mut decoder = ZlibDecoder::new(&idat_combined[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|e| {
        ImagePipelineError::DecodeFailed(format!("PNG zlib decompression failed: {e}"))
    })?;

    let w = width as usize;
    let h = height as usize;
    let scanline_stride = w * bpp;
    let expected_len = h * (1 + scanline_stride);
    if decompressed.len() < expected_len {
        return Err(ImagePipelineError::DecodeFailed(format!(
            "Incomplete PNG stream: expected {expected_len} bytes, got {}",
            decompressed.len()
        )));
    }

    let mut out = vec![0u8; w * h * 4];
    let mut prev_row = vec![0u8; scanline_stride];
    let mut curr_row = vec![0u8; scanline_stride];

    for row in 0..h {
        let offset = row * (1 + scanline_stride);
        let filter_type = decompressed[offset];
        let raw_line = &decompressed[offset + 1..offset + 1 + scanline_stride];

        unfilter_scanline(filter_type, raw_line, &prev_row, &mut curr_row, bpp);

        let dst_row_offset = row * w * 4;
        match color_type {
            6 => {
                out[dst_row_offset..dst_row_offset + w * 4].copy_from_slice(&curr_row);
            }
            2 => {
                for col in 0..w {
                    let s = col * 3;
                    let d = dst_row_offset + col * 4;
                    out[d] = curr_row[s];
                    out[d + 1] = curr_row[s + 1];
                    out[d + 2] = curr_row[s + 2];
                    out[d + 3] = 255;
                }
            }
            0 => {
                for col in 0..w {
                    let s = col;
                    let d = dst_row_offset + col * 4;
                    let v = curr_row[s];
                    out[d] = v;
                    out[d + 1] = v;
                    out[d + 2] = v;
                    out[d + 3] = 255;
                }
            }
            _ => unreachable!(),
        }

        prev_row.copy_from_slice(&curr_row);
    }

    Ok(DecodedImageRgba {
        width,
        height,
        data: out,
    })
}

fn unfilter_scanline(filter_type: u8, raw: &[u8], prev: &[u8], curr: &mut [u8], bpp: usize) {
    match filter_type {
        0 => curr.copy_from_slice(raw),
        1 => {
            for i in 0..raw.len() {
                let left = if i >= bpp { curr[i - bpp] } else { 0 };
                curr[i] = raw[i].wrapping_add(left);
            }
        }
        2 => {
            for i in 0..raw.len() {
                let up = prev[i];
                curr[i] = raw[i].wrapping_add(up);
            }
        }
        3 => {
            for i in 0..raw.len() {
                let left = if i >= bpp { curr[i - bpp] as u16 } else { 0 };
                let up = prev[i] as u16;
                let avg = ((left + up) / 2) as u8;
                curr[i] = raw[i].wrapping_add(avg);
            }
        }
        4 => {
            for i in 0..raw.len() {
                let left = if i >= bpp { curr[i - bpp] } else { 0 };
                let up = prev[i];
                let up_left = if i >= bpp { prev[i - bpp] } else { 0 };
                curr[i] = raw[i].wrapping_add(paeth_predictor(left, up, up_left));
            }
        }
        _ => curr.copy_from_slice(raw),
    }
}

#[inline]
fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let a_i = a as i32;
    let b_i = b as i32;
    let c_i = c as i32;
    let p = a_i + b_i - c_i;
    let pa = (p - a_i).abs();
    let pb = (p - b_i).abs();
    let pc = (p - c_i).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
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
