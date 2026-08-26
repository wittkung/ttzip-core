// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Image Pipeline and Thumbnail Generation Bindings.

use super::types::TTZipError;
use crate::standards::image_pipeline::{
    decode_image_rgba, generate_thumbnail, DecodedImageRgba, ThumbnailFilter,
};

/// High-performance thumbnail downsampling filter algorithms exposed to Swift.
#[derive(Copy, Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ThumbnailSamplingFilter {
    Nearest,
    Bilinear,
    Lanczos3,
}

impl From<ThumbnailSamplingFilter> for ThumbnailFilter {
    fn from(f: ThumbnailSamplingFilter) -> Self {
        match f {
            ThumbnailSamplingFilter::Nearest => ThumbnailFilter::Nearest,
            ThumbnailSamplingFilter::Bilinear => ThumbnailFilter::Bilinear,
            ThumbnailSamplingFilter::Lanczos3 => ThumbnailFilter::Lanczos3,
        }
    }
}

/// Decoded RGBA8 image pixel buffer record exposed to Swift.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DecodedImageRecord {
    pub width: u32,
    pub height: u32,
    pub rgba_bytes: Vec<u8>,
}

impl From<DecodedImageRgba> for DecodedImageRecord {
    fn from(img: DecodedImageRgba) -> Self {
        Self {
            width: img.width,
            height: img.height,
            rgba_bytes: img.data,
        }
    }
}

/// Decodes an image from an in-memory buffer (JPEG, PNG, WebP, BMP, PSD, QOI, HDR) into RGBA8 pixels.
#[uniffi::export]
pub fn decode_image_rgba_from_memory(data: Vec<u8>) -> Result<DecodedImageRecord, TTZipError> {
    decode_image_rgba(&data)
        .map(Into::into)
        .map_err(|e| TTZipError::IoError {
            message: format!("Image decoding failed: {e}"),
        })
}

/// Generates a high-quality downsampled thumbnail from an in-memory image buffer.
#[uniffi::export]
pub fn generate_thumbnail_from_memory(
    data: Vec<u8>,
    max_width: u32,
    max_height: u32,
    filter: ThumbnailSamplingFilter,
) -> Result<DecodedImageRecord, TTZipError> {
    generate_thumbnail(&data, max_width, max_height, filter.into())
        .map(Into::into)
        .map_err(|e| TTZipError::IoError {
            message: format!("Thumbnail generation failed: {e}"),
        })
}
