// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Dynamic viewport sampling and pyramid tile generator for ultra-high-resolution images.
//!
//! Enables zero-allocation streaming sub-region cropping, multi-scale zooming,
//! and fast tile grid generation for deep zoom viewports.

use crate::image::decoder::{
    DecodedImageFrame, ImageBitDepth, ImageError,
};

/// 2D rectangular sub-region inside an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ViewportRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ViewportRect {
    /// Creates a new viewport rectangle.
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Clamps this viewport rectangle to the source image dimensions.
    #[must_use]
    pub fn clamp_to(&self, max_w: u32, max_h: u32) -> Self {
        let x = self.x.min(max_w);
        let y = self.y.min(max_h);
        let width = self.width.min(max_w.saturating_sub(x));
        let height = self.height.min(max_h.saturating_sub(y));
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Resampling filter algorithm for viewport scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportFilter {
    Nearest,
    #[default]
    Bilinear,
    Lanczos3,
    AreaAverage,
}

/// Dynamic Viewport Sampler and Tile Generator.
#[derive(Debug, Clone, Copy, Default)]
pub struct ViewportSampler;

impl ViewportSampler {
    /// Samples and scales a sub-rectangle from a source frame with specified scale factor.
    pub fn sample_viewport(
        source: &DecodedImageFrame,
        rect: &ViewportRect,
        scale: f32,
        filter: ViewportFilter,
    ) -> Result<DecodedImageFrame, ImageError> {
        if scale <= 0.0 || !scale.is_finite() {
            return Err(ImageError::DecodeFailed(format!(
                "Invalid scale factor: {scale}"
            )));
        }

        let clamped = rect.clamp_to(source.width, source.height);
        if clamped.width == 0 || clamped.height == 0 {
            return Err(ImageError::InvalidViewport(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                source.width,
                source.height,
            ));
        }

        let dst_w = ((clamped.width as f32 * scale).round() as u32).max(1);
        let dst_h = ((clamped.height as f32 * scale).round() as u32).max(1);

        Self::sample_viewport_to_size(source, &clamped, dst_w, dst_h, filter)
    }

    /// Samples a sub-rectangle from a source frame directly into explicit target dimensions.
    pub fn sample_viewport_to_size(
        source: &DecodedImageFrame,
        rect: &ViewportRect,
        dst_width: u32,
        dst_height: u32,
        filter: ViewportFilter,
    ) -> Result<DecodedImageFrame, ImageError> {
        if dst_width == 0 || dst_height == 0 {
            return Err(ImageError::InvalidDimensions(dst_width, dst_height));
        }

        let clamped = rect.clamp_to(source.width, source.height);
        if clamped.width == 0 || clamped.height == 0 {
            return Err(ImageError::InvalidViewport(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                source.width,
                source.height,
            ));
        }

        // Fast path 1: 1:1 scale identical crop without resampling
        if clamped.width == dst_width && clamped.height == dst_height {
            return Ok(crop_exact(source, &clamped));
        }

        let channels = source.channels();
        let out_bytes = match filter {
            ViewportFilter::Nearest => {
                resample_nearest(source, &clamped, dst_width, dst_height, channels)
            }
            ViewportFilter::Bilinear => {
                resample_bilinear(source, &clamped, dst_width, dst_height, channels)
            }
            ViewportFilter::Lanczos3 => {
                resample_lanczos3(source, &clamped, dst_width, dst_height, channels)
            }
            ViewportFilter::AreaAverage => {
                resample_area_average(source, &clamped, dst_width, dst_height, channels)
            }
        };

        DecodedImageFrame::new(
            dst_width,
            dst_height,
            source.colorspace,
            ImageBitDepth::U8,
            out_bytes,
        )
    }

    /// Generates a specific tile at `(tile_col, tile_row)` for deep zoom viewing.
    pub fn generate_tile(
        source: &DecodedImageFrame,
        tile_col: u32,
        tile_row: u32,
        tile_size: u32,
        scale: f32,
        filter: ViewportFilter,
    ) -> Result<DecodedImageFrame, ImageError> {
        if tile_size == 0 || scale <= 0.0 {
            return Err(ImageError::InvalidDimensions(tile_size, tile_size));
        }

        let src_tile_extent = (tile_size as f32 / scale).round() as u32;
        let x = tile_col * src_tile_extent;
        let y = tile_row * src_tile_extent;

        if x >= source.width || y >= source.height {
            return Err(ImageError::InvalidViewport(
                x,
                y,
                src_tile_extent,
                src_tile_extent,
                source.width,
                source.height,
            ));
        }

        let rect = ViewportRect::new(x, y, src_tile_extent, src_tile_extent);
        let clamped = rect.clamp_to(source.width, source.height);

        let dst_w = ((clamped.width as f32 * scale).round() as u32).min(tile_size).max(1);
        let dst_h = ((clamped.height as f32 * scale).round() as u32).min(tile_size).max(1);

        Self::sample_viewport_to_size(source, &clamped, dst_w, dst_h, filter)
    }

    /// Computes all tile boundary boxes across the source image given a tile size.
    #[must_use]
    pub fn compute_tile_grid(
        image_width: u32,
        image_height: u32,
        tile_size: u32,
    ) -> Vec<ViewportRect> {
        if image_width == 0 || image_height == 0 || tile_size == 0 {
            return Vec::new();
        }

        let cols = image_width.div_ceil(tile_size);
        let rows = image_height.div_ceil(tile_size);
        let mut tiles = Vec::with_capacity((cols * rows) as usize);

        for row in 0..rows {
            let y = row * tile_size;
            let h = tile_size.min(image_height - y);
            for col in 0..cols {
                let x = col * tile_size;
                let w = tile_size.min(image_width - x);
                tiles.push(ViewportRect::new(x, y, w, h));
            }
        }

        tiles
    }
}

/// Direct 1:1 sub-rectangle memory crop.
fn crop_exact(src: &DecodedImageFrame, rect: &ViewportRect) -> DecodedImageFrame {
    let channels = src.channels();
    let sw = src.width as usize;
    let rw = rect.width as usize;
    let rh = rect.height as usize;
    let rx = rect.x as usize;
    let ry = rect.y as usize;

    let mut out = vec![0u8; rw * rh * channels];
    let src_stride = sw * channels;
    let dst_stride = rw * channels;
    let row_copy_len = rw * channels;

    for row in 0..rh {
        let src_offset = (ry + row) * src_stride + rx * channels;
        let dst_offset = row * dst_stride;
        out[dst_offset..dst_offset + row_copy_len]
            .copy_from_slice(&src.bytes[src_offset..src_offset + row_copy_len]);
    }

    DecodedImageFrame {
        width: rect.width,
        height: rect.height,
        colorspace: src.colorspace,
        bit_depth: src.bit_depth,
        bytes: out,
    }
}

/// Nearest-neighbor sub-region resampler.
fn resample_nearest(
    src: &DecodedImageFrame,
    rect: &ViewportRect,
    dw: u32,
    dh: u32,
    channels: usize,
) -> Vec<u8> {
    let (sw, dw_sz, dh_sz) = (src.width as usize, dw as usize, dh as usize);
    let (rx, ry, rw, rh) = (
        rect.x as usize,
        rect.y as usize,
        rect.width as usize,
        rect.height as usize,
    );

    let mut out = vec![0u8; dw_sz * dh_sz * channels];
    for dy in 0..dh_sz {
        let sy = ry + (dy * rh / dh_sz).min(rh - 1);
        let src_row = sy * sw * channels;
        let dst_row = dy * dw_sz * channels;

        for dx in 0..dw_sz {
            let sx = rx + (dx * rw / dw_sz).min(rw - 1);
            let s_idx = src_row + sx * channels;
            let d_idx = dst_row + dx * channels;
            out[d_idx..d_idx + channels].copy_from_slice(&src.bytes[s_idx..s_idx + channels]);
        }
    }
    out
}

/// High-quality Bilinear sub-region resampler.
fn resample_bilinear(
    src: &DecodedImageFrame,
    rect: &ViewportRect,
    dw: u32,
    dh: u32,
    channels: usize,
) -> Vec<u8> {
    let (sw, dw_sz, dh_sz) = (src.width as usize, dw as usize, dh as usize);
    let (rx, ry, rw, rh) = (
        rect.x as usize,
        rect.y as usize,
        rect.width as usize,
        rect.height as usize,
    );

    let mut out = vec![0u8; dw_sz * dh_sz * channels];
    let x_scale = rw as f32 / dw as f32;
    let y_scale = rh as f32 / dh as f32;

    for dy in 0..dh_sz {
        let gy = ry as f32 + ((dy as f32 + 0.5) * y_scale - 0.5).clamp(0.0, (rh - 1) as f32);
        let y0 = (gy.floor() as usize).min(ry + rh - 1);
        let y1 = (y0 + 1).min(ry + rh - 1);
        let fy = gy - gy.floor();

        let row0 = y0 * sw * channels;
        let row1 = y1 * sw * channels;
        let dst_row = dy * dw_sz * channels;

        for dx in 0..dw_sz {
            let gx = rx as f32 + ((dx as f32 + 0.5) * x_scale - 0.5).clamp(0.0, (rw - 1) as f32);
            let x0 = (gx.floor() as usize).min(rx + rw - 1);
            let x1 = (x0 + 1).min(rx + rw - 1);
            let fx = gx - gx.floor();

            let w00 = (1.0 - fx) * (1.0 - fy);
            let w10 = fx * (1.0 - fy);
            let w01 = (1.0 - fx) * fy;
            let w11 = fx * fy;

            let i00 = row0 + x0 * channels;
            let i10 = row0 + x1 * channels;
            let i01 = row1 + x0 * channels;
            let i11 = row1 + x1 * channels;
            let d_idx = dst_row + dx * channels;

            for c in 0..channels {
                let v = w00 * src.bytes[i00 + c] as f32
                    + w10 * src.bytes[i10 + c] as f32
                    + w01 * src.bytes[i01 + c] as f32
                    + w11 * src.bytes[i11 + c] as f32;
                out[d_idx + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Area-averaging sub-region resampler for clean downscaling.
fn resample_area_average(
    src: &DecodedImageFrame,
    rect: &ViewportRect,
    dw: u32,
    dh: u32,
    channels: usize,
) -> Vec<u8> {
    let (sw, dw_sz, dh_sz) = (src.width as usize, dw as usize, dh as usize);
    let (rx, ry, rw, rh) = (
        rect.x as usize,
        rect.y as usize,
        rect.width as usize,
        rect.height as usize,
    );

    let mut out = vec![0u8; dw_sz * dh_sz * channels];
    let x_ratio = rw as f32 / dw as f32;
    let y_ratio = rh as f32 / dh as f32;

    for dy in 0..dh_sz {
        let sy_start = ry + (dy as f32 * y_ratio).floor() as usize;
        let sy_end = (ry + (((dy + 1) as f32 * y_ratio).ceil() as usize)).min(ry + rh);
        let dst_row_idx = dy * dw_sz * channels;

        for dx in 0..dw_sz {
            let sx_start = rx + (dx as f32 * x_ratio).floor() as usize;
            let sx_end = (rx + (((dx + 1) as f32 * x_ratio).ceil() as usize)).min(rx + rw);
            let dst_px_idx = dst_row_idx + dx * channels;

            let mut accum = [0u32; 4];
            let mut count = 0u32;

            for y in sy_start..sy_end {
                let src_row_idx = y * sw * channels;
                for x in sx_start..sx_end {
                    let src_px_idx = src_row_idx + x * channels;
                    for c in 0..channels {
                        accum[c] += src.bytes[src_px_idx + c] as u32;
                    }
                    count += 1;
                }
            }

            if count > 0 {
                let half_count = count / 2;
                for c in 0..channels {
                    out[dst_px_idx + c] = ((accum[c] + half_count) / count).min(255) as u8;
                }
            }
        }
    }
    out
}

#[inline]
fn lanczos3_weight(x: f32) -> f32 {
    if x.abs() < 1e-5 {
        return 1.0;
    }
    if x.abs() >= 3.0 {
        return 0.0;
    }
    let px = std::f32::consts::PI * x;
    (px.sin() * (px / 3.0).sin()) / (px * px / 3.0)
}

/// Lanczos-3 2-pass high-fidelity viewport resampler.
fn resample_lanczos3(
    src: &DecodedImageFrame,
    rect: &ViewportRect,
    dw: u32,
    dh: u32,
    channels: usize,
) -> Vec<u8> {
    let (sw, dw_sz, dh_sz) = (src.width as usize, dw as usize, dh as usize);
    let (rx, ry, rw, rh) = (
        rect.x as usize,
        rect.y as usize,
        rect.width as usize,
        rect.height as usize,
    );

    // Pass 1: Horizontal resampling to intermediate buffer
    let mut intermediate = vec![0.0f32; dw_sz * rh * channels];
    let x_scale = dw as f32 / rw as f32;
    let x_support = if x_scale < 1.0 {
        3.0 / x_scale
    } else {
        3.0
    };

    for y in 0..rh {
        let src_row = (ry + y) * sw * channels;
        let inter_row = y * dw_sz * channels;

        for dx in 0..dw_sz {
            let center = rx as f32 + (dx as f32 + 0.5) / x_scale - 0.5;
            let start = (center - x_support).floor().max(rx as f32) as usize;
            let end = (center + x_support).ceil().min((rx + rw - 1) as f32) as usize;

            let mut weights_sum = 0.0f32;
            let mut accum = [0.0f32; 4];

            for sx in start..=end {
                let diff = (sx as f32 - center) * (if x_scale < 1.0 { x_scale } else { 1.0 });
                let w = lanczos3_weight(diff);
                if w.abs() > 1e-6 {
                    weights_sum += w;
                    let src_idx = src_row + sx * channels;
                    for c in 0..channels {
                        accum[c] += src.bytes[src_idx + c] as f32 * w;
                    }
                }
            }

            let inter_idx = inter_row + dx * channels;
            if weights_sum.abs() > 1e-6 {
                for c in 0..channels {
                    intermediate[inter_idx + c] = accum[c] / weights_sum;
                }
            }
        }
    }

    // Pass 2: Vertical resampling from intermediate buffer to final output
    let mut out = vec![0u8; dw_sz * dh_sz * channels];
    let y_scale = dh as f32 / rh as f32;
    let y_support = if y_scale < 1.0 {
        3.0 / y_scale
    } else {
        3.0
    };

    for dy in 0..dh_sz {
        let center = (dy as f32 + 0.5) / y_scale - 0.5;
        let start = (center - y_support).floor().max(0.0) as usize;
        let end = (center + y_support).ceil().min((rh - 1) as f32) as usize;
        let dst_row = dy * dw_sz * channels;

        for dx in 0..dw_sz {
            let mut weights_sum = 0.0f32;
            let mut accum = [0.0f32; 4];

            for sy in start..=end {
                let diff = (sy as f32 - center) * (if y_scale < 1.0 { y_scale } else { 1.0 });
                let w = lanczos3_weight(diff);
                if w.abs() > 1e-6 {
                    weights_sum += w;
                    let inter_idx = (sy * dw_sz + dx) * channels;
                    for c in 0..channels {
                        accum[c] += intermediate[inter_idx + c] * w;
                    }
                }
            }

            let d_idx = dst_row + dx * channels;
            if weights_sum.abs() > 1e-6 {
                for c in 0..channels {
                    let v = (accum[c] / weights_sum).round().clamp(0.0, 255.0);
                    out[d_idx + c] = v as u8;
                }
            }
        }
    }

    out
}
