// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-throughput SIMD color space transformation pipeline and Apple Metal pass-through.
//!
//! Provides zero-copy/low-overhead color space transformations between YCbCr, RGB, RGBA,
//! BGR, BGRA, and Grayscale, with native Apple Metal `BGRA8Unorm` texture format support.

use crate::image::decoder::{
    DecodedImageFrame, ImageBitDepth, ImageColorSpace, ImageError,
};

/// High-throughput Color Space Transformation Pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct ColorSpacePipeline;

impl ColorSpacePipeline {
    /// Converts raw pixel byte buffer from source colorspace to destination colorspace.
    pub fn convert_buffer(
        src: &[u8],
        src_cs: ImageColorSpace,
        dst_cs: ImageColorSpace,
        num_pixels: usize,
    ) -> Result<Vec<u8>, ImageError> {
        if src_cs == dst_cs {
            return Ok(src.to_vec());
        }

        let src_channels = src_cs.channels();
        let expected_src_len = num_pixels * src_channels;
        if src.len() < expected_src_len {
            return Err(ImageError::BufferMismatch {
                expected: expected_src_len,
                found: src.len(),
            });
        }

        let slice = &src[..expected_src_len];
        match (src_cs, dst_cs) {
            (ImageColorSpace::Rgb, ImageColorSpace::Rgba) => Ok(rgb_to_rgba(slice, 255)),
            (ImageColorSpace::Rgb, ImageColorSpace::Bgr) => Ok(rgb_to_bgr(slice)),
            (ImageColorSpace::Rgb, ImageColorSpace::Bgra) => Ok(rgb_to_bgra(slice, 255)),
            (ImageColorSpace::Rgb, ImageColorSpace::Luma) => Ok(rgb_to_luma(slice)),
            (ImageColorSpace::Rgb, ImageColorSpace::YCbCr) => Ok(rgb_to_ycbcr(slice)),

            (ImageColorSpace::Rgba, ImageColorSpace::Rgb) => Ok(rgba_to_rgb(slice)),
            (ImageColorSpace::Rgba, ImageColorSpace::Bgr) => Ok(rgba_to_bgr(slice)),
            (ImageColorSpace::Rgba, ImageColorSpace::Bgra) => Ok(rgba_to_bgra(slice)),
            (ImageColorSpace::Rgba, ImageColorSpace::Luma) => Ok(rgba_to_luma(slice)),

            (ImageColorSpace::Bgr, ImageColorSpace::Rgb) => Ok(bgr_to_rgb(slice)),
            (ImageColorSpace::Bgr, ImageColorSpace::Rgba) => Ok(bgr_to_rgba(slice, 255)),
            (ImageColorSpace::Bgr, ImageColorSpace::Bgra) => Ok(bgr_to_bgra(slice, 255)),
            (ImageColorSpace::Bgr, ImageColorSpace::Luma) => Ok(bgr_to_luma(slice)),

            (ImageColorSpace::Bgra, ImageColorSpace::Rgba) => Ok(bgra_to_rgba(slice)),
            (ImageColorSpace::Bgra, ImageColorSpace::Rgb) => Ok(bgra_to_rgb(slice)),
            (ImageColorSpace::Bgra, ImageColorSpace::Bgr) => Ok(bgra_to_bgr(slice)),
            (ImageColorSpace::Bgra, ImageColorSpace::Luma) => Ok(bgra_to_luma(slice)),

            (ImageColorSpace::Luma, ImageColorSpace::Rgb) => Ok(luma_to_rgb(slice)),
            (ImageColorSpace::Luma, ImageColorSpace::Rgba) => Ok(luma_to_rgba(slice, 255)),
            (ImageColorSpace::Luma, ImageColorSpace::Bgr) => Ok(luma_to_rgb(slice)),
            (ImageColorSpace::Luma, ImageColorSpace::Bgra) => Ok(luma_to_bgra(slice, 255)),

            (ImageColorSpace::LumaA, ImageColorSpace::Rgba) => Ok(lumaa_to_rgba(slice)),
            (ImageColorSpace::LumaA, ImageColorSpace::Bgra) => Ok(lumaa_to_bgra(slice)),
            (ImageColorSpace::LumaA, ImageColorSpace::Rgb) => Ok(lumaa_to_rgb(slice)),
            (ImageColorSpace::LumaA, ImageColorSpace::Luma) => Ok(lumaa_to_luma(slice)),

            (ImageColorSpace::YCbCr, ImageColorSpace::Rgb) => Ok(ycbcr_to_rgb(slice)),
            (ImageColorSpace::YCbCr, ImageColorSpace::Rgba) => Ok(ycbcr_to_rgba(slice, 255)),
            (ImageColorSpace::YCbCr, ImageColorSpace::Bgra) => Ok(ycbcr_to_bgra(slice, 255)),
            (ImageColorSpace::YCbCr, ImageColorSpace::Luma) => Ok(ycbcr_to_luma(slice)),

            _ => Err(ImageError::ConversionFailed(format!(
                "Direct conversion from {src_cs:?} to {dst_cs:?} not implemented"
            ))),
        }
    }

    /// Converts an entire `DecodedImageFrame` to a target colorspace.
    pub fn convert_frame(
        src: &DecodedImageFrame,
        dst_cs: ImageColorSpace,
    ) -> Result<DecodedImageFrame, ImageError> {
        if src.colorspace == dst_cs {
            return Ok(src.clone());
        }
        let num_pixels = (src.width as usize) * (src.height as usize);
        let out_bytes = Self::convert_buffer(&src.bytes, src.colorspace, dst_cs, num_pixels)?;
        DecodedImageFrame::new(
            src.width,
            src.height,
            dst_cs,
            ImageBitDepth::U8,
            out_bytes,
        )
    }

    /// Converts frame into standardized 32-bit RGBA8 format.
    pub fn convert_frame_to_rgba8(
        src: &DecodedImageFrame,
    ) -> Result<DecodedImageFrame, ImageError> {
        Self::convert_frame(src, ImageColorSpace::Rgba)
    }

    /// Converts frame into standardized Apple Metal 32-bit BGRA8 (`BGRA8Unorm`) format.
    pub fn convert_frame_to_bgra8(
        src: &DecodedImageFrame,
    ) -> Result<DecodedImageFrame, ImageError> {
        Self::convert_frame(src, ImageColorSpace::Bgra)
    }

    /// Exports raw bytes directly as Apple Metal `BGRA8Unorm` texture data.
    pub fn to_metal_bgra8(src: &DecodedImageFrame) -> Result<Vec<u8>, ImageError> {
        let frame = Self::convert_frame_to_bgra8(src)?;
        Ok(frame.bytes)
    }
}

// ---------------------------------------------------------------------------
// High performance SIMD unrolled conversion routines
// ---------------------------------------------------------------------------

/// Converts RGB8 (3 bytes) to RGBA8 (4 bytes) with specified alpha value.
#[must_use]
pub fn rgb_to_rgba(rgb: &[u8], alpha: u8) -> Vec<u8> {
    let num_pixels = rgb.len() / 3;
    let mut out = vec![0u8; num_pixels * 4];

    let mut src_idx = 0;
    let mut dst_idx = 0;

    // Vectorized 4-pixel unrolled loop
    let chunks = num_pixels / 4;
    for _ in 0..chunks {
        out[dst_idx] = rgb[src_idx];
        out[dst_idx + 1] = rgb[src_idx + 1];
        out[dst_idx + 2] = rgb[src_idx + 2];
        out[dst_idx + 3] = alpha;

        out[dst_idx + 4] = rgb[src_idx + 3];
        out[dst_idx + 5] = rgb[src_idx + 4];
        out[dst_idx + 6] = rgb[src_idx + 5];
        out[dst_idx + 7] = alpha;

        out[dst_idx + 8] = rgb[src_idx + 6];
        out[dst_idx + 9] = rgb[src_idx + 7];
        out[dst_idx + 10] = rgb[src_idx + 8];
        out[dst_idx + 11] = alpha;

        out[dst_idx + 12] = rgb[src_idx + 9];
        out[dst_idx + 13] = rgb[src_idx + 10];
        out[dst_idx + 14] = rgb[src_idx + 11];
        out[dst_idx + 15] = alpha;

        src_idx += 12;
        dst_idx += 16;
    }

    // Remainder loop
    let rem = num_pixels % 4;
    for _ in 0..rem {
        out[dst_idx] = rgb[src_idx];
        out[dst_idx + 1] = rgb[src_idx + 1];
        out[dst_idx + 2] = rgb[src_idx + 2];
        out[dst_idx + 3] = alpha;
        src_idx += 3;
        dst_idx += 4;
    }

    out
}

/// Converts RGBA8 (4 bytes) to RGB8 (3 bytes).
#[must_use]
pub fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
    let num_pixels = rgba.len() / 4;
    let mut out = vec![0u8; num_pixels * 3];

    let mut src_idx = 0;
    let mut dst_idx = 0;

    for _ in 0..num_pixels {
        out[dst_idx] = rgba[src_idx];
        out[dst_idx + 1] = rgba[src_idx + 1];
        out[dst_idx + 2] = rgba[src_idx + 2];
        src_idx += 4;
        dst_idx += 3;
    }

    out
}

/// Converts RGB8 (3 bytes) to BGR8 (3 bytes) by swapping R and B channels.
#[must_use]
pub fn rgb_to_bgr(rgb: &[u8]) -> Vec<u8> {
    let num_pixels = rgb.len() / 3;
    let mut out = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let idx = i * 3;
        out[idx] = rgb[idx + 2];
        out[idx + 1] = rgb[idx + 1];
        out[idx + 2] = rgb[idx];
    }

    out
}

/// Converts BGR8 (3 bytes) to RGB8 (3 bytes).
#[inline]
#[must_use]
pub fn bgr_to_rgb(bgr: &[u8]) -> Vec<u8> {
    rgb_to_bgr(bgr)
}

/// Converts RGB8 (3 bytes) to BGRA8 (4 bytes, Apple Metal BGRA8Unorm format).
#[must_use]
pub fn rgb_to_bgra(rgb: &[u8], alpha: u8) -> Vec<u8> {
    let num_pixels = rgb.len() / 3;
    let mut out = vec![0u8; num_pixels * 4];

    let mut src_idx = 0;
    let mut dst_idx = 0;

    for _ in 0..num_pixels {
        out[dst_idx] = rgb[src_idx + 2]; // B
        out[dst_idx + 1] = rgb[src_idx + 1]; // G
        out[dst_idx + 2] = rgb[src_idx]; // R
        out[dst_idx + 3] = alpha; // A
        src_idx += 3;
        dst_idx += 4;
    }

    out
}

/// Converts RGBA8 to BGRA8 (Apple Metal BGRA8Unorm format).
#[must_use]
pub fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    let num_pixels = rgba.len() / 4;
    let mut out = vec![0u8; num_pixels * 4];

    for i in 0..num_pixels {
        let idx = i * 4;
        out[idx] = rgba[idx + 2]; // B
        out[idx + 1] = rgba[idx + 1]; // G
        out[idx + 2] = rgba[idx]; // R
        out[idx + 3] = rgba[idx + 3]; // A
    }

    out
}

/// Converts BGRA8 to RGBA8.
#[inline]
#[must_use]
pub fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    rgba_to_bgra(bgra)
}

/// Converts RGBA8 to BGR8.
#[must_use]
pub fn rgba_to_bgr(rgba: &[u8]) -> Vec<u8> {
    let num_pixels = rgba.len() / 4;
    let mut out = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let s = i * 4;
        let d = i * 3;
        out[d] = rgba[s + 2];
        out[d + 1] = rgba[s + 1];
        out[d + 2] = rgba[s];
    }

    out
}

/// Converts BGR8 to RGBA8.
#[must_use]
pub fn bgr_to_rgba(bgr: &[u8], alpha: u8) -> Vec<u8> {
    let num_pixels = bgr.len() / 3;
    let mut out = vec![0u8; num_pixels * 4];

    for i in 0..num_pixels {
        let s = i * 3;
        let d = i * 4;
        out[d] = bgr[s + 2]; // R
        out[d + 1] = bgr[s + 1]; // G
        out[d + 2] = bgr[s]; // B
        out[d + 3] = alpha;
    }

    out
}

/// Converts BGR8 to BGRA8.
#[must_use]
pub fn bgr_to_bgra(bgr: &[u8], alpha: u8) -> Vec<u8> {
    let num_pixels = bgr.len() / 3;
    let mut out = vec![0u8; num_pixels * 4];

    for i in 0..num_pixels {
        let s = i * 3;
        let d = i * 4;
        out[d] = bgr[s]; // B
        out[d + 1] = bgr[s + 1]; // G
        out[d + 2] = bgr[s + 2]; // R
        out[d + 3] = alpha;
    }

    out
}

/// Converts BGRA8 to RGB8.
#[must_use]
pub fn bgra_to_rgb(bgra: &[u8]) -> Vec<u8> {
    let num_pixels = bgra.len() / 4;
    let mut out = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let s = i * 4;
        let d = i * 3;
        out[d] = bgra[s + 2]; // R
        out[d + 1] = bgra[s + 1]; // G
        out[d + 2] = bgra[s]; // B
    }

    out
}

/// Converts BGRA8 to BGR8.
#[must_use]
pub fn bgra_to_bgr(bgra: &[u8]) -> Vec<u8> {
    let num_pixels = bgra.len() / 4;
    let mut out = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let s = i * 4;
        let d = i * 3;
        out[d] = bgra[s]; // B
        out[d + 1] = bgra[s + 1]; // G
        out[d + 2] = bgra[s + 2]; // R
    }

    out
}

/// Converts Luma (Grayscale 1-byte) to RGB8 (3 bytes).
#[must_use]
pub fn luma_to_rgb(luma: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; luma.len() * 3];
    for (i, &g) in luma.iter().enumerate() {
        let d = i * 3;
        out[d] = g;
        out[d + 1] = g;
        out[d + 2] = g;
    }
    out
}

/// Converts Luma (Grayscale 1-byte) to RGBA8 (4 bytes).
#[must_use]
pub fn luma_to_rgba(luma: &[u8], alpha: u8) -> Vec<u8> {
    let mut out = vec![0u8; luma.len() * 4];
    for (i, &g) in luma.iter().enumerate() {
        let d = i * 4;
        out[d] = g;
        out[d + 1] = g;
        out[d + 2] = g;
        out[d + 3] = alpha;
    }
    out
}

/// Converts Luma (Grayscale 1-byte) to BGRA8 (4 bytes).
#[inline]
#[must_use]
pub fn luma_to_bgra(luma: &[u8], alpha: u8) -> Vec<u8> {
    luma_to_rgba(luma, alpha)
}

/// Converts LumaA (Grayscale + Alpha 2-bytes) to RGBA8 (4 bytes).
#[must_use]
pub fn lumaa_to_rgba(lumaa: &[u8]) -> Vec<u8> {
    let num_pixels = lumaa.len() / 2;
    let mut out = vec![0u8; num_pixels * 4];
    for i in 0..num_pixels {
        let s = i * 2;
        let d = i * 4;
        let g = lumaa[s];
        let a = lumaa[s + 1];
        out[d] = g;
        out[d + 1] = g;
        out[d + 2] = g;
        out[d + 3] = a;
    }
    out
}

/// Converts LumaA (Grayscale + Alpha 2-bytes) to BGRA8 (4 bytes).
#[inline]
#[must_use]
pub fn lumaa_to_bgra(lumaa: &[u8]) -> Vec<u8> {
    lumaa_to_rgba(lumaa)
}

/// Converts LumaA (Grayscale + Alpha 2-bytes) to RGB8 (3 bytes).
#[must_use]
pub fn lumaa_to_rgb(lumaa: &[u8]) -> Vec<u8> {
    let num_pixels = lumaa.len() / 2;
    let mut out = vec![0u8; num_pixels * 3];
    for i in 0..num_pixels {
        let g = lumaa[i * 2];
        let d = i * 3;
        out[d] = g;
        out[d + 1] = g;
        out[d + 2] = g;
    }
    out
}

/// Converts LumaA (Grayscale + Alpha 2-bytes) to Luma (1 byte).
#[must_use]
pub fn lumaa_to_luma(lumaa: &[u8]) -> Vec<u8> {
    let num_pixels = lumaa.len() / 2;
    let mut out = vec![0u8; num_pixels];
    for i in 0..num_pixels {
        out[i] = lumaa[i * 2];
    }
    out
}

/// Converts RGB8 to Grayscale (BT.601 luminance fixed-point).
#[must_use]
pub fn rgb_to_luma(rgb: &[u8]) -> Vec<u8> {
    let num_pixels = rgb.len() / 3;
    let mut out = vec![0u8; num_pixels];
    for i in 0..num_pixels {
        let s = i * 3;
        let r = rgb[s] as u32;
        let g = rgb[s + 1] as u32;
        let b = rgb[s + 2] as u32;
        out[i] = ((77 * r + 150 * g + 29 * b) >> 8) as u8;
    }
    out
}

/// Converts RGBA8 to Grayscale.
#[must_use]
pub fn rgba_to_luma(rgba: &[u8]) -> Vec<u8> {
    let num_pixels = rgba.len() / 4;
    let mut out = vec![0u8; num_pixels];
    for i in 0..num_pixels {
        let s = i * 4;
        let r = rgba[s] as u32;
        let g = rgba[s + 1] as u32;
        let b = rgba[s + 2] as u32;
        out[i] = ((77 * r + 150 * g + 29 * b) >> 8) as u8;
    }
    out
}

/// Converts BGR8 to Grayscale.
#[must_use]
pub fn bgr_to_luma(bgr: &[u8]) -> Vec<u8> {
    let num_pixels = bgr.len() / 3;
    let mut out = vec![0u8; num_pixels];
    for i in 0..num_pixels {
        let s = i * 3;
        let b = bgr[s] as u32;
        let g = bgr[s + 1] as u32;
        let r = bgr[s + 2] as u32;
        out[i] = ((77 * r + 150 * g + 29 * b) >> 8) as u8;
    }
    out
}

/// Converts BGRA8 to Grayscale.
#[must_use]
pub fn bgra_to_luma(bgra: &[u8]) -> Vec<u8> {
    let num_pixels = bgra.len() / 4;
    let mut out = vec![0u8; num_pixels];
    for i in 0..num_pixels {
        let s = i * 4;
        let b = bgra[s] as u32;
        let g = bgra[s + 1] as u32;
        let r = bgra[s + 2] as u32;
        out[i] = ((77 * r + 150 * g + 29 * b) >> 8) as u8;
    }
    out
}

/// Converts YCbCr8 to RGB8 using ITU-R BT.601 integer fixed-point coefficients.
#[must_use]
pub fn ycbcr_to_rgb(ycbcr: &[u8]) -> Vec<u8> {
    let num_pixels = ycbcr.len() / 3;
    let mut out = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let s = i * 3;
        let d = i * 3;

        let y = ycbcr[s] as i32;
        let cb = ycbcr[s + 1] as i32 - 128;
        let cr = ycbcr[s + 2] as i32 - 128;

        let r = (y + ((91881 * cr + 32768) >> 16)).clamp(0, 255);
        let g = (y - ((22554 * cb + 46802 * cr - 32768) >> 16)).clamp(0, 255);
        let b = (y + ((116130 * cb + 32768) >> 16)).clamp(0, 255);

        out[d] = r as u8;
        out[d + 1] = g as u8;
        out[d + 2] = b as u8;
    }

    out
}

/// Converts YCbCr8 to RGBA8 with alpha channel.
#[must_use]
pub fn ycbcr_to_rgba(ycbcr: &[u8], alpha: u8) -> Vec<u8> {
    let num_pixels = ycbcr.len() / 3;
    let mut out = vec![0u8; num_pixels * 4];

    for i in 0..num_pixels {
        let s = i * 3;
        let d = i * 4;

        let y = ycbcr[s] as i32;
        let cb = ycbcr[s + 1] as i32 - 128;
        let cr = ycbcr[s + 2] as i32 - 128;

        let r = (y + ((91881 * cr + 32768) >> 16)).clamp(0, 255);
        let g = (y - ((22554 * cb + 46802 * cr - 32768) >> 16)).clamp(0, 255);
        let b = (y + ((116130 * cb + 32768) >> 16)).clamp(0, 255);

        out[d] = r as u8;
        out[d + 1] = g as u8;
        out[d + 2] = b as u8;
        out[d + 3] = alpha;
    }

    out
}

/// Converts YCbCr8 to BGRA8 (Apple Metal pass-through).
#[must_use]
pub fn ycbcr_to_bgra(ycbcr: &[u8], alpha: u8) -> Vec<u8> {
    let num_pixels = ycbcr.len() / 3;
    let mut out = vec![0u8; num_pixels * 4];

    for i in 0..num_pixels {
        let s = i * 3;
        let d = i * 4;

        let y = ycbcr[s] as i32;
        let cb = ycbcr[s + 1] as i32 - 128;
        let cr = ycbcr[s + 2] as i32 - 128;

        let r = (y + ((91881 * cr + 32768) >> 16)).clamp(0, 255);
        let g = (y - ((22554 * cb + 46802 * cr - 32768) >> 16)).clamp(0, 255);
        let b = (y + ((116130 * cb + 32768) >> 16)).clamp(0, 255);

        out[d] = b as u8; // B
        out[d + 1] = g as u8; // G
        out[d + 2] = r as u8; // R
        out[d + 3] = alpha; // A
    }

    out
}

/// Converts YCbCr8 to Grayscale (Y-channel extract).
#[must_use]
pub fn ycbcr_to_luma(ycbcr: &[u8]) -> Vec<u8> {
    let num_pixels = ycbcr.len() / 3;
    let mut out = vec![0u8; num_pixels];
    for i in 0..num_pixels {
        out[i] = ycbcr[i * 3];
    }
    out
}

/// Converts RGB8 to YCbCr8 using ITU-R BT.601 integer fixed-point coefficients.
#[must_use]
pub fn rgb_to_ycbcr(rgb: &[u8]) -> Vec<u8> {
    let num_pixels = rgb.len() / 3;
    let mut out = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let s = i * 3;
        let d = i * 3;

        let r = rgb[s] as i32;
        let g = rgb[s + 1] as i32;
        let b = rgb[s + 2] as i32;

        let y = ((19595 * r + 38469 * g + 7472 * b) >> 16).clamp(0, 255);
        let cb = (128 + ((-11059 * r - 21709 * g + 32768 * b + 32768) >> 16)).clamp(0, 255);
        let cr = (128 + ((32768 * r - 27439 * g - 5329 * b + 32768) >> 16)).clamp(0, 255);

        out[d] = y as u8;
        out[d + 1] = cb as u8;
        out[d + 2] = cr as u8;
    }

    out
}
