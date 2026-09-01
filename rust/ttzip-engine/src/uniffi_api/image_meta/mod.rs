// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for Image Decoding, Thumbnailing, and Viewport Rendering.
//!
//! Provides high-throughput zero-copy image metadata extraction, SIMD-accelerated RGBA8
//! decoding, multi-scale thumbnail generation, and deep-zoom viewport tile sampling for Swift 6.

pub mod decoder;
pub mod types;

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

pub use decoder::{decode_image_bytes, extract_thumbnail_bytes, probe_image_bytes, sample_viewport_bytes};
pub use types::{
    UniFFIImageFrame, UniFFIImageInfo, UniFFIThumbnailResult, UniFFIViewportCropParams,
    UniFFIViewportTile,
};
use crate::uniffi_api::types::TTZipError;

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Decodes an image from in-memory bytes into unified RGBA8 format.
#[uniffi::export]
pub fn uniffi_decode_image(data: Vec<u8>, max_dimension: Option<u32>) -> Result<UniFFIImageFrame, TTZipError> {
    decode_image_bytes(&data, max_dimension)
}

/// Generates a high-quality downsampled thumbnail from in-memory image bytes.
#[uniffi::export]
pub fn uniffi_extract_thumbnail(
    data: Vec<u8>,
    max_width: u32,
    max_height: u32,
    filter_type: Option<String>,
) -> Result<UniFFIThumbnailResult, TTZipError> {
    extract_thumbnail_bytes(&data, max_width, max_height, filter_type.as_deref())
}

/// Samples a cropped viewport tile from an in-memory image buffer.
#[uniffi::export]
pub fn uniffi_sample_viewport(
    data: Vec<u8>,
    params: UniFFIViewportCropParams,
) -> Result<UniFFIViewportTile, TTZipError> {
    sample_viewport_bytes(&data, &params)
}

/// Probes image format, dimensions, color space, and EXIF tags without full pixel decompression.
#[uniffi::export]
pub fn uniffi_probe_image_info(data: Vec<u8>, file_name: Option<String>) -> Result<UniFFIImageInfo, TTZipError> {
    probe_image_bytes(&data, file_name.as_deref())
}

// ============================================================================
// Service Object
// ============================================================================

/// Stateful Mozilla UniFFI image processing service exposing decoding, thumbnailing, and viewport pipelines.
#[derive(uniffi::Object, Default)]
pub struct UniFFIImageService {}

#[uniffi::export]
impl UniFFIImageService {
    /// Constructs a new image rendering and inspection service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Probes image format and metadata from an in-memory byte buffer.
    pub fn probe_info(&self, data: Vec<u8>, file_name: Option<String>) -> Result<UniFFIImageInfo, TTZipError> {
        probe_image_bytes(&data, file_name.as_deref())
    }

    /// Probes image format and metadata from a local filesystem path.
    pub fn probe_info_from_file(&self, file_path: String) -> Result<UniFFIImageInfo, TTZipError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        probe_image_bytes(&bytes, name.as_deref())
    }

    /// Decodes an image from an in-memory byte buffer into RGBA8 pixels.
    pub fn decode_image(&self, data: Vec<u8>, max_dimension: Option<u32>) -> Result<UniFFIImageFrame, TTZipError> {
        decode_image_bytes(&data, max_dimension)
    }

    /// Decodes an image from a local filesystem path into RGBA8 pixels.
    pub fn decode_image_from_file(
        &self,
        file_path: String,
        max_dimension: Option<u32>,
    ) -> Result<UniFFIImageFrame, TTZipError> {
        let bytes = read_file_bytes(&file_path)?;
        decode_image_bytes(&bytes, max_dimension)
    }

    /// Generates a downsampled thumbnail from an in-memory byte buffer.
    pub fn extract_thumbnail(
        &self,
        data: Vec<u8>,
        max_width: u32,
        max_height: u32,
        filter_type: Option<String>,
    ) -> Result<UniFFIThumbnailResult, TTZipError> {
        extract_thumbnail_bytes(&data, max_width, max_height, filter_type.as_deref())
    }

    /// Generates a downsampled thumbnail from a local filesystem path.
    pub fn extract_thumbnail_from_file(
        &self,
        file_path: String,
        max_width: u32,
        max_height: u32,
        filter_type: Option<String>,
    ) -> Result<UniFFIThumbnailResult, TTZipError> {
        let bytes = read_file_bytes(&file_path)?;
        extract_thumbnail_bytes(&bytes, max_width, max_height, filter_type.as_deref())
    }

    /// Samples a cropped and scaled viewport tile from an in-memory byte buffer.
    pub fn sample_viewport(
        &self,
        data: Vec<u8>,
        params: UniFFIViewportCropParams,
    ) -> Result<UniFFIViewportTile, TTZipError> {
        sample_viewport_bytes(&data, &params)
    }

    /// Samples a cropped and scaled viewport tile from a local filesystem path.
    pub fn sample_viewport_from_file(
        &self,
        file_path: String,
        params: UniFFIViewportCropParams,
    ) -> Result<UniFFIViewportTile, TTZipError> {
        let bytes = read_file_bytes(&file_path)?;
        sample_viewport_bytes(&bytes, &params)
    }
}

// ============================================================================
// Internal Helpers
// ============================================================================

pub(crate) fn read_file_bytes(path_str: &str) -> Result<Vec<u8>, TTZipError> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(TTZipError::FileNotFound {
            path: path_str.to_string(),
        });
    }
    let file = File::open(path).map_err(|e| TTZipError::IoError {
        message: format!("Failed to open file '{path_str}': {e}"),
    })?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(|e| TTZipError::IoError {
        message: format!("Failed to memory map file '{path_str}': {e}"),
    })?;
    Ok(mmap.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bmp(width: u32, height: u32) -> Vec<u8> {
        let row_bytes = (width * 3 + 3) & !3;
        let img_size = row_bytes * height;
        let file_size = 54 + img_size;

        let mut bmp = Vec::with_capacity(file_size as usize);
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&file_size.to_le_bytes());
        bmp.extend_from_slice(&[0, 0, 0, 0]); // Reserved
        bmp.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel data
        bmp.extend_from_slice(&40u32.to_le_bytes()); // Header size (BITMAPINFOHEADER)
        bmp.extend_from_slice(&(width as i32).to_le_bytes());
        bmp.extend_from_slice(&(height as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes()); // Planes
        bmp.extend_from_slice(&24u16.to_le_bytes()); // 24-bit RGB
        bmp.extend_from_slice(&0u32.to_le_bytes()); // Compression (BI_RGB)
        bmp.extend_from_slice(&img_size.to_le_bytes());
        bmp.extend_from_slice(&2835u32.to_le_bytes()); // X pixels per meter
        bmp.extend_from_slice(&2835u32.to_le_bytes()); // Y pixels per meter
        bmp.extend_from_slice(&0u32.to_le_bytes()); // Colors used
        bmp.extend_from_slice(&0u32.to_le_bytes()); // Important colors

        for y in 0..height {
            for x in 0..width {
                let r = ((x * 255) / width.max(1)) as u8;
                let g = ((y * 255) / height.max(1)) as u8;
                let b = 128u8;
                bmp.extend_from_slice(&[b, g, r]); // BMP stores BGR
            }
            let padding = (row_bytes - (width * 3)) as usize;
            bmp.resize(bmp.len() + padding, 0);
        }
        bmp
    }

    #[test]
    fn test_image_probe_and_decode() {
        let bmp_data = create_test_bmp(8, 8);

        // 1. Test probe info
        let info = uniffi_probe_image_info(bmp_data.clone(), Some("sample.bmp".to_string()))
            .expect("Probe BMP failed");
        assert_eq!(info.width, 8);
        assert_eq!(info.height, 8);
        assert_eq!(info.format_name, "BMP");

        // 2. Test decode image
        let frame = uniffi_decode_image(bmp_data.clone(), None)
            .expect("Decode BMP failed");
        assert_eq!(frame.width, 8);
        assert_eq!(frame.height, 8);
        assert_eq!(frame.stride, 32);
        assert_eq!(frame.rgba_bytes.len(), 8 * 8 * 4);

        // 3. Test thumbnail extraction
        let thumb = uniffi_extract_thumbnail(bmp_data.clone(), 4, 4, Some("bilinear".to_string()))
            .expect("Extract thumbnail failed");
        assert_eq!(thumb.width, 4);
        assert_eq!(thumb.height, 4);
        assert_eq!(thumb.stride, 16);
        assert_eq!(thumb.rgba_bytes.len(), 4 * 4 * 4);
        assert!(thumb.duration_ms >= 0.0);

        // 4. Test viewport tile sampling
        let crop_params = UniFFIViewportCropParams {
            crop_x: 2,
            crop_y: 2,
            crop_width: 4,
            crop_height: 4,
            target_width: 4,
            target_height: 4,
        };
        let tile = uniffi_sample_viewport(bmp_data, crop_params)
            .expect("Sample viewport failed");
        assert_eq!(tile.tile_x, 2);
        assert_eq!(tile.tile_y, 2);
        assert_eq!(tile.tile_width, 4);
        assert_eq!(tile.tile_height, 4);
        assert_eq!(tile.stride, 16);
        assert_eq!(tile.rgba_bytes.len(), 16 * 4);
    }

    #[test]
    fn test_service_lifecycle() {
        let service = UniFFIImageService::new();
        let bmp_data = create_test_bmp(16, 16);

        let info = service.probe_info(bmp_data.clone(), None).expect("Probe failed");
        assert_eq!(info.width, 16);
        assert_eq!(info.height, 16);

        let frame = service.decode_image(bmp_data.clone(), Some(8)).expect("Decode failed");
        assert_eq!(frame.width, 8);
        assert_eq!(frame.height, 8);

        let thumb = service.extract_thumbnail(bmp_data.clone(), 6, 6, None).expect("Thumbnail failed");
        assert_eq!(thumb.width, 6);
        assert_eq!(thumb.height, 6);

        let params = UniFFIViewportCropParams {
            crop_x: 4,
            crop_y: 4,
            crop_width: 8,
            crop_height: 8,
            target_width: 4,
            target_height: 4,
        };
        let tile = service.sample_viewport(bmp_data, params).expect("Viewport failed");
        assert_eq!(tile.tile_width, 4);
        assert_eq!(tile.tile_height, 4);
    }
}
