// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust high-performance multi-format image decoder.
//!
//! Provides zero-unsafe format auto-detection and accelerated decoding
//! for JPEG, PNG, WebP, QOI, PPM/PNM, Farbfeld, BMP, and other formats.

use std::io::Cursor;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_image::image::Image;

/// Unified supported image colorspace enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageColorSpace {
    Rgb,
    Rgba,
    Bgr,
    Bgra,
    Luma,
    LumaA,
    YCbCr,
    Unknown,
}

impl From<ColorSpace> for ImageColorSpace {
    fn from(cs: ColorSpace) -> Self {
        match cs {
            ColorSpace::RGB => ImageColorSpace::Rgb,
            ColorSpace::RGBA => ImageColorSpace::Rgba,
            ColorSpace::BGR => ImageColorSpace::Bgr,
            ColorSpace::BGRA => ImageColorSpace::Bgra,
            ColorSpace::Luma => ImageColorSpace::Luma,
            ColorSpace::LumaA => ImageColorSpace::LumaA,
            ColorSpace::YCbCr => ImageColorSpace::YCbCr,
            _ => ImageColorSpace::Unknown,
        }
    }
}

/// Unified supported image bit depth representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageBitDepth {
    U8,
    U16,
    F32,
}

impl From<BitDepth> for ImageBitDepth {
    fn from(depth: BitDepth) -> Self {
        match depth {
            BitDepth::Eight => ImageBitDepth::U8,
            BitDepth::Sixteen => ImageBitDepth::U16,
            BitDepth::Float32 => ImageBitDepth::F32,
            _ => ImageBitDepth::U8,
        }
    }
}

/// Detected image file format based on magic bytes inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
    Qoi,
    Ppm,
    Farbfeld,
    Bmp,
    Unknown,
}

/// Image decoding and processing error definitions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("Empty or truncated image data stream")]
    EmptyData,

    #[error("Unsupported or unrecognized image format")]
    UnsupportedFormat,

    #[error("Image decoding failed: {0}")]
    DecodeFailed(String),

    #[error("Color conversion failed: {0}")]
    ConversionFailed(String),

    #[error("Invalid image dimensions ({0}x{1})")]
    InvalidDimensions(u32, u32),

    #[error("Invalid viewport crop boundary: rect ({0},{1},{2},{3}) exceeds image ({4}x{5})")]
    InvalidViewport(u32, u32, u32, u32, u32, u32),

    #[error("Buffer length mismatch: expected {expected} bytes, found {found} bytes")]
    BufferMismatch { expected: usize, found: usize },

    #[error("Exif thumbnail not found in image metadata")]
    ExifThumbnailNotFound,
}

/// Decoded image frame container storing dimensions, pixel format, and raw bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedImageFrame {
    pub width: u32,
    pub height: u32,
    pub colorspace: ImageColorSpace,
    pub bit_depth: ImageBitDepth,
    pub bytes: Vec<u8>,
}

impl DecodedImageFrame {
    /// Creates a new decoded image frame container with sanity checks.
    pub fn new(
        width: u32,
        height: u32,
        colorspace: ImageColorSpace,
        bit_depth: ImageBitDepth,
        bytes: Vec<u8>,
    ) -> Result<Self, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::InvalidDimensions(width, height));
        }
        let channels = colorspace.channels();
        let bytes_per_sample = bit_depth.bytes_per_sample();
        let expected_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|px| px.checked_mul(channels))
            .and_then(|b| b.checked_mul(bytes_per_sample))
            .ok_or(ImageError::InvalidDimensions(width, height))?;

        if bytes.len() < expected_len {
            return Err(ImageError::BufferMismatch {
                expected: expected_len,
                found: bytes.len(),
            });
        }

        Ok(Self {
            width,
            height,
            colorspace,
            bit_depth,
            bytes,
        })
    }

    /// Number of color channels in this frame.
    #[inline]
    pub fn channels(&self) -> usize {
        self.colorspace.channels()
    }

    /// Number of bytes per pixel in this frame.
    #[inline]
    pub fn bytes_per_pixel(&self) -> usize {
        self.colorspace.channels() * self.bit_depth.bytes_per_sample()
    }

    /// Number of bytes per scanline row.
    #[inline]
    pub fn stride(&self) -> usize {
        self.width as usize * self.bytes_per_pixel()
    }
}

impl ImageColorSpace {
    /// Returns the number of channels associated with this colorspace.
    #[inline]
    pub const fn channels(&self) -> usize {
        match self {
            ImageColorSpace::Luma => 1,
            ImageColorSpace::LumaA => 2,
            ImageColorSpace::Rgb | ImageColorSpace::Bgr | ImageColorSpace::YCbCr => 3,
            ImageColorSpace::Rgba | ImageColorSpace::Bgra => 4,
            ImageColorSpace::Unknown => 4,
        }
    }
}

impl ImageBitDepth {
    /// Returns the number of bytes per sample channel.
    #[inline]
    pub const fn bytes_per_sample(&self) -> usize {
        match self {
            ImageBitDepth::U8 => 1,
            ImageBitDepth::U16 => 2,
            ImageBitDepth::F32 => 4,
        }
    }
}

/// Pure Safe Rust Multi-Format Image Decoder.
#[derive(Debug, Clone, Copy, Default)]
pub struct TTZipImageDecoder;

impl TTZipImageDecoder {
    /// Detects image format from magic bytes without full decoding.
    #[must_use]
    pub fn detect_format(data: &[u8]) -> ImageFormat {
        if data.len() < 3 {
            return ImageFormat::Unknown;
        }

        // JPEG: FF D8 FF
        if data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
            return ImageFormat::Jpeg;
        }

        // PNG: 89 50 4E 47 0D 0A 1A 0A
        if data.len() >= 8 && data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return ImageFormat::Png;
        }

        // WebP: RIFF .... WEBP
        if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
            return ImageFormat::WebP;
        }

        // QOI: "qoif"
        if data.starts_with(b"qoif") {
            return ImageFormat::Qoi;
        }

        // Farbfeld: "farbfeld"
        if data.starts_with(b"farbfeld") {
            return ImageFormat::Farbfeld;
        }

        // BMP: "BM"
        if data.starts_with(b"BM") {
            return ImageFormat::Bmp;
        }

        // Netpbm PNM/PPM: P1 through P6
        if data[0] == b'P' && matches!(data[1], b'1'..=b'6') {
            let next_byte = data[2];
            if next_byte == b'\n' || next_byte == b'\r' || next_byte == b' ' || next_byte == b'\t'
            {
                return ImageFormat::Ppm;
            }
        }

        ImageFormat::Unknown
    }

    /// Decodes an image from an in-memory byte buffer into its native `DecodedImageFrame`.
    pub fn decode(data: &[u8]) -> Result<DecodedImageFrame, ImageError> {
        if data.is_empty() {
            return Err(ImageError::EmptyData);
        }

        let format = Self::detect_format(data);
        match format {
            ImageFormat::WebP => Self::decode_webp(data),
            ImageFormat::Jpeg
            | ImageFormat::Png
            | ImageFormat::Qoi
            | ImageFormat::Ppm
            | ImageFormat::Farbfeld
            | ImageFormat::Bmp => Self::decode_zune(data),
            ImageFormat::Unknown => {
                // Try decoding with zune first, then webp fallback
                Self::decode_zune(data).or_else(|_| Self::decode_webp(data))
            }
        }
    }

    /// Decodes an image from an in-memory buffer directly into standardized RGBA8 format.
    pub fn decode_rgba8(data: &[u8]) -> Result<DecodedImageFrame, ImageError> {
        let frame = Self::decode(data)?;
        crate::image::colorspace::ColorSpacePipeline::convert_frame_to_rgba8(&frame)
    }

    /// Decodes an image from an in-memory buffer directly into Apple Metal BGRA8Unorm format.
    pub fn decode_bgra8(data: &[u8]) -> Result<DecodedImageFrame, ImageError> {
        let frame = Self::decode(data)?;
        crate::image::colorspace::ColorSpacePipeline::convert_frame_to_bgra8(&frame)
    }

    /// Internal WebP decoding via image-webp crate.
    fn decode_webp(data: &[u8]) -> Result<DecodedImageFrame, ImageError> {
        let mut decoder = image_webp::WebPDecoder::new(Cursor::new(data))
            .map_err(|e| ImageError::DecodeFailed(format!("WebP decoder init error: {e:?}")))?;

        let (w, h) = decoder.dimensions();
        if w == 0 || h == 0 {
            return Err(ImageError::InvalidDimensions(w, h));
        }

        let has_alpha = decoder.has_alpha();
        let buf_size = decoder
            .output_buffer_size()
            .ok_or_else(|| ImageError::DecodeFailed("Invalid WebP buffer size".into()))?;

        let mut raw = vec![0u8; buf_size];
        decoder
            .read_image(&mut raw)
            .map_err(|e| ImageError::DecodeFailed(format!("WebP read error: {e:?}")))?;

        let colorspace = if has_alpha {
            ImageColorSpace::Rgba
        } else {
            ImageColorSpace::Rgb
        };

        DecodedImageFrame::new(w, h, colorspace, ImageBitDepth::U8, raw)
    }

    /// Internal multi-format decoding via zune-image crate.
    fn decode_zune(data: &[u8]) -> Result<DecodedImageFrame, ImageError> {
        let img = Image::read(data, DecoderOptions::default())
            .map_err(|e| ImageError::DecodeFailed(format!("zune-image read error: {e:?}")))?;

        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return Err(ImageError::InvalidDimensions(w as u32, h as u32));
        }

        let cs: ImageColorSpace = img.colorspace().into();

        let frames = img.flatten_to_u8();
        let raw = frames.into_iter().next().ok_or_else(|| {
            ImageError::DecodeFailed("No decoded image frames produced".into())
        })?;

        DecodedImageFrame::new(w as u32, h as u32, cs, ImageBitDepth::U8, raw)
    }
}
