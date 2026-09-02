// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Video Dimension, Frame Memory Ceiling, and Arithmetic Overflow Guard.
//!
//! Enforces deterministic geometric bounds on decoded video frames:
//! - Resolution boundaries: 1 <= width <= 8,192 px and 1 <= height <= 8,192 px (8K UHD max).
//! - Single uncompressed frame memory ceiling: <= 256 MiB.
//! - Checked arithmetic on planar strides, chroma subsampling, and aspect ratio calculations.

use super::{
    VideoDefenseError, DEFAULT_MAX_VIDEO_DIMENSION, DEFAULT_MAX_VIDEO_FRAME_MEMORY,
    DEFAULT_MIN_VIDEO_DIMENSION,
};

/// Common uncompressed video pixel and chroma formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VideoPixelFormat {
    /// Planar YUV 4:2:0 (8-bit, 12 bits per pixel / 1.5 bytes).
    #[default]
    Yuv420p,
    /// Semi-planar YUV 4:2:0 with interleaved UV (NV12, 12 bits per pixel / 1.5 bytes).
    Nv12,
    /// Semi-planar YUV 4:2:0 with interleaved VU (NV21, 12 bits per pixel / 1.5 bytes).
    Nv21,
    /// Planar YUV 4:2:2 (8-bit, 16 bits per pixel / 2 bytes).
    Yuv422p,
    /// Planar YUV 4:4:4 (8-bit, 24 bits per pixel / 3 bytes).
    Yuv444p,
    /// 10-bit Little-Endian Planar YUV 4:2:0 (15 bits per pixel, stored as 3 bytes per pixel).
    Yuv420p10le,
    /// Packed RGB 24-bit (3 bytes per pixel).
    Rgb24,
    /// Packed BGR 24-bit (3 bytes per pixel).
    Bgr24,
    /// Packed RGBA 32-bit (4 bytes per pixel).
    Rgba32,
    /// Packed BGRA 32-bit (4 bytes per pixel).
    Bgra32,
}

impl VideoPixelFormat {
    /// Returns the bits per pixel (bpp) as an exact numerator and denominator.
    pub const fn bits_per_pixel(self) -> (u32, u32) {
        match self {
            Self::Yuv420p | Self::Nv12 | Self::Nv21 => (12, 1),
            Self::Yuv422p => (16, 1),
            Self::Yuv444p | Self::Rgb24 | Self::Bgr24 => (24, 1),
            Self::Yuv420p10le => (24, 1), // 2 bytes per luma/chroma sample with 4:2:0 subsampling
            Self::Rgba32 | Self::Bgra32 => (32, 1),
        }
    }
}

/// Inspection summary report for a validated video geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoDimensionReport {
    /// Validated video width in pixels.
    pub width: u32,
    /// Validated video height in pixels.
    pub height: u32,
    /// Target pixel format.
    pub pixel_format: VideoPixelFormat,
    /// Estimated memory required for a single uncompressed frame in bytes.
    pub estimated_frame_bytes: usize,
    /// Display aspect ratio (width / height).
    pub aspect_ratio: f64,
}

/// Defensive guard validating video resolution, frame memory bounds, and arithmetic safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoDimensionGuard {
    min_dimension: u32,
    max_dimension: u32,
    max_frame_memory: usize,
}

impl Default for VideoDimensionGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoDimensionGuard {
    /// Creates a new guard with default security boundaries (1..=8192 px, <= 256MB frame memory).
    pub const fn new() -> Self {
        Self {
            min_dimension: DEFAULT_MIN_VIDEO_DIMENSION,
            max_dimension: DEFAULT_MAX_VIDEO_DIMENSION,
            max_frame_memory: DEFAULT_MAX_VIDEO_FRAME_MEMORY,
        }
    }

    /// Creates a new guard with custom dimension boundaries and frame memory ceiling.
    pub const fn with_bounds(min_dim: u32, max_dim: u32, max_frame_mem: usize) -> Self {
        Self {
            min_dimension: min_dim,
            max_dimension: max_dim,
            max_frame_memory: max_frame_mem,
        }
    }

    /// Returns the minimum allowable dimension.
    #[inline]
    pub const fn min_dimension(&self) -> u32 {
        self.min_dimension
    }

    /// Returns the maximum allowable dimension.
    #[inline]
    pub const fn max_dimension(&self) -> u32 {
        self.max_dimension
    }

    /// Returns the maximum allowable single-frame memory in bytes.
    #[inline]
    pub const fn max_frame_memory(&self) -> usize {
        self.max_frame_memory
    }

    /// Validates width and height boundaries against configured bounds.
    pub fn validate_dimensions(&self, width: u32, height: u32) -> Result<(), VideoDefenseError> {
        if width == 0 {
            return Err(VideoDefenseError::InvalidDimensionZero { axis: "width" });
        }
        if height == 0 {
            return Err(VideoDefenseError::InvalidDimensionZero { axis: "height" });
        }

        if width < self.min_dimension || width > self.max_dimension {
            return Err(VideoDefenseError::DimensionLimitExceeded {
                axis: "width",
                value: width,
                min: self.min_dimension,
                max: self.max_dimension,
            });
        }

        if height < self.min_dimension || height > self.max_dimension {
            return Err(VideoDefenseError::DimensionLimitExceeded {
                axis: "height",
                value: height,
                min: self.min_dimension,
                max: self.max_dimension,
            });
        }

        Ok(())
    }

    /// Computes display aspect ratio safely with divide-by-zero protection.
    pub fn calculate_aspect_ratio(
        &self,
        width: u32,
        height: u32,
    ) -> Result<f64, VideoDefenseError> {
        self.validate_dimensions(width, height)?;
        Ok(width as f64 / height as f64)
    }

    /// Estimates the uncompressed frame size in bytes for the specified pixel format and alignment.
    pub fn estimate_frame_size(
        &self,
        width: u32,
        height: u32,
        pixel_format: VideoPixelFormat,
    ) -> Result<usize, VideoDefenseError> {
        self.validate_dimensions(width, height)?;

        let w = width as usize;
        let h = height as usize;

        let frame_bytes = match pixel_format {
            VideoPixelFormat::Yuv420p | VideoPixelFormat::Nv12 | VideoPixelFormat::Nv21 => {
                // Y plane: W * H
                let y_size = w
                    .checked_mul(h)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?;
                // UV plane(s): (W/2) * (H/2) * 2 = (W * H) / 2
                let uv_size = y_size
                    .checked_div(2)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?;
                y_size
                    .checked_add(uv_size)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?
            }
            VideoPixelFormat::Yuv422p => {
                // Y plane: W * H, U plane: (W/2) * H, V plane: (W/2) * H -> Total: 2 * W * H
                let y_size = w
                    .checked_mul(h)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?;
                y_size
                    .checked_mul(2)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?
            }
            VideoPixelFormat::Yuv444p | VideoPixelFormat::Rgb24 | VideoPixelFormat::Bgr24 => {
                // 3 bytes per pixel
                let pixels = w
                    .checked_mul(h)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?;
                pixels
                    .checked_mul(3)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?
            }
            VideoPixelFormat::Yuv420p10le => {
                // 10-bit YUV420 stored as 16-bit (2 bytes per sample) -> 3 bytes per pixel
                let pixels = w
                    .checked_mul(h)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?;
                pixels
                    .checked_mul(3)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?
            }
            VideoPixelFormat::Rgba32 | VideoPixelFormat::Bgra32 => {
                // 4 bytes per pixel
                let pixels = w
                    .checked_mul(h)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?;
                pixels
                    .checked_mul(4)
                    .ok_or(VideoDefenseError::DimensionArithmeticOverflow)?
            }
        };

        if frame_bytes > self.max_frame_memory {
            return Err(VideoDefenseError::FrameMemoryExceeded {
                width,
                height,
                estimated_bytes: frame_bytes,
                max_bytes: self.max_frame_memory,
            });
        }

        Ok(frame_bytes)
    }

    /// Performs full geometric and memory inspection, producing a `VideoDimensionReport`.
    pub fn inspect(
        &self,
        width: u32,
        height: u32,
        pixel_format: VideoPixelFormat,
    ) -> Result<VideoDimensionReport, VideoDefenseError> {
        let estimated_frame_bytes = self.estimate_frame_size(width, height, pixel_format)?;
        let aspect_ratio = self.calculate_aspect_ratio(width, height)?;

        Ok(VideoDimensionReport {
            width,
            height,
            pixel_format,
            estimated_frame_bytes,
            aspect_ratio,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_standard_dimensions() {
        let guard = VideoDimensionGuard::new();
        let report = guard.inspect(1920, 1080, VideoPixelFormat::Yuv420p).unwrap();
        assert_eq!(report.width, 1920);
        assert_eq!(report.height, 1080);
        assert_eq!(report.estimated_frame_bytes, 1920 * 1080 * 3 / 2);
        assert!((report.aspect_ratio - (16.0 / 9.0)).abs() < 0.001);
    }

    #[test]
    fn test_4k_and_8k_boundaries() {
        let guard = VideoDimensionGuard::new();
        // 4K UHD 3840x2160 RGBA32 (3840 * 2160 * 4 = 33,177,600 bytes = ~31.6MB <= 256MB)
        let report_4k = guard.inspect(3840, 2160, VideoPixelFormat::Rgba32).unwrap();
        assert_eq!(report_4k.estimated_frame_bytes, 3840 * 2160 * 4);

        // 8K UHD 7680x4320 YUV420p (7680 * 4320 * 1.5 = 49,766,400 bytes <= 256MB)
        let report_8k = guard.inspect(7680, 4320, VideoPixelFormat::Yuv420p).unwrap();
        assert_eq!(report_8k.estimated_frame_bytes, 7680 * 4320 * 3 / 2);

        // Exact maximum boundary 8192x8192 RGBA32: 8192 * 8192 * 4 = 268,435,456 bytes (256 MiB exact)
        let report_max = guard.inspect(8192, 8192, VideoPixelFormat::Rgba32).unwrap();
        assert_eq!(report_max.estimated_frame_bytes, 256 * 1024 * 1024);
    }

    #[test]
    fn test_zero_dimensions() {
        let guard = VideoDimensionGuard::new();
        let err_w = guard.validate_dimensions(0, 1080).unwrap_err();
        assert_eq!(err_w, VideoDefenseError::InvalidDimensionZero { axis: "width" });

        let err_h = guard.validate_dimensions(1920, 0).unwrap_err();
        assert_eq!(err_h, VideoDefenseError::InvalidDimensionZero { axis: "height" });
    }

    #[test]
    fn test_dimension_limit_exceeded() {
        let guard = VideoDimensionGuard::new();
        let err = guard.validate_dimensions(8193, 1080).unwrap_err();
        assert_eq!(
            err,
            VideoDefenseError::DimensionLimitExceeded {
                axis: "width",
                value: 8193,
                min: 1,
                max: 8192
            }
        );
    }

    #[test]
    fn test_frame_memory_exceeded() {
        let guard = VideoDimensionGuard::with_bounds(1, 16384, 64 * 1024 * 1024); // 64MB frame budget
        // 8192x8192 RGBA32 requires 256MB > 64MB budget
        let err = guard.estimate_frame_size(8192, 8192, VideoPixelFormat::Rgba32).unwrap_err();
        match err {
            VideoDefenseError::FrameMemoryExceeded { width, height, estimated_bytes, max_bytes } => {
                assert_eq!(width, 8192);
                assert_eq!(height, 8192);
                assert_eq!(estimated_bytes, 256 * 1024 * 1024);
                assert_eq!(max_bytes, 64 * 1024 * 1024);
            }
            _ => panic!("Expected FrameMemoryExceeded"),
        }
    }
}
