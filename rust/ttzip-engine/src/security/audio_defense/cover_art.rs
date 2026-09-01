// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio Cover Art and Embedded Visual Metadata Quota Guard.
//!
//! Enforces deterministic memory bounds and format validation on embedded artwork
//! (e.g. ID3v2 APIC/PIC, FLAC PICTURE metadata, MP4 covr atoms), preventing
//! memory exhaustion from gigabyte-sized synthetic images and malformed payloads.

use super::{
    AudioDefenseError, DEFAULT_MAX_COVER_ART_COUNT, DEFAULT_MAX_SINGLE_COVER_ART_SIZE,
    DEFAULT_MAX_TOTAL_COVER_ART_SIZE,
};

/// Supported image formats for embedded audio cover artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverArtFormat {
    /// JPEG image format (magic: FF D8 FF).
    Jpeg,
    /// PNG image format (magic: 89 50 4E 47 0D 0A 1A 0A).
    Png,
    /// GIF image format (magic: GIF87a / GIF89a).
    Gif,
    /// WebP image format (magic: RIFF....WEBP).
    Webp,
    /// Unknown or unsupported image format.
    Unknown,
}

impl CoverArtFormat {
    /// Returns the canonical MIME type string for the detected format.
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Unknown => "application/octet-stream",
        }
    }
}

/// Metadata summary describing an inspected embedded cover art image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArtInfo {
    /// Detected image container format.
    pub format: CoverArtFormat,
    /// Byte size of the image payload.
    pub size: usize,
    /// Canonical MIME type string.
    pub mime_type: &'static str,
}

/// Defensive quota manager tracking embedded artwork count and memory volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArtQuotaGuard {
    max_single_size: usize,
    max_total_size: usize,
    max_count: usize,
    current_total_size: usize,
    current_count: usize,
}

impl Default for CoverArtQuotaGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverArtQuotaGuard {
    /// Creates a new guard initialized with standard security boundaries.
    pub const fn new() -> Self {
        Self {
            max_single_size: DEFAULT_MAX_SINGLE_COVER_ART_SIZE,
            max_total_size: DEFAULT_MAX_TOTAL_COVER_ART_SIZE,
            max_count: DEFAULT_MAX_COVER_ART_COUNT,
            current_total_size: 0,
            current_count: 0,
        }
    }

    /// Creates a guard with customized quota thresholds.
    pub const fn with_quotas(
        max_single_size: usize,
        max_total_size: usize,
        max_count: usize,
    ) -> Self {
        Self {
            max_single_size,
            max_total_size,
            max_count,
            current_total_size: 0,
            current_count: 0,
        }
    }

    /// Detects image container format by inspecting file magic signatures.
    pub fn detect_format(data: &[u8]) -> CoverArtFormat {
        if data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
            return CoverArtFormat::Jpeg;
        }

        if data.len() >= 8 && data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            return CoverArtFormat::Png;
        }

        if data.len() >= 6 && (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) {
            return CoverArtFormat::Gif;
        }

        if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
            return CoverArtFormat::Webp;
        }

        CoverArtFormat::Unknown
    }

    /// Inspects and registers an extracted cover art payload against safety quotas.
    pub fn inspect_and_register(&mut self, data: &[u8]) -> Result<CoverArtInfo, AudioDefenseError> {
        let size = data.len();

        // 1. Minimum sanity check
        if size < 4 {
            return Err(AudioDefenseError::CoverArtMalformed {
                reason: format!("Cover art payload too short ({size} bytes)"),
            });
        }

        // 2. Single item size limit
        if size > self.max_single_size {
            return Err(AudioDefenseError::CoverArtSizeExceeded {
                size,
                max_size: self.max_single_size,
            });
        }

        // 3. Image count quota
        if self.current_count >= self.max_count {
            return Err(AudioDefenseError::CoverArtCountExceeded {
                count: self.current_count + 1,
                max_count: self.max_count,
            });
        }

        // 4. Cumulative total size quota
        let next_total = self.current_total_size.saturating_add(size);
        if next_total > self.max_total_size {
            return Err(AudioDefenseError::TotalCoverArtQuotaExceeded {
                total_size: next_total,
                max_quota: self.max_total_size,
            });
        }

        // 5. Magic signature format validation
        let format = Self::detect_format(data);
        if format == CoverArtFormat::Unknown {
            return Err(AudioDefenseError::CoverArtMalformed {
                reason: "Unrecognized or corrupted image magic header".to_string(),
            });
        }

        // 6. Commit reservation
        self.current_count += 1;
        self.current_total_size = next_total;

        Ok(CoverArtInfo {
            format,
            size,
            mime_type: format.mime_type(),
        })
    }

    /// Returns the count of registered cover art images.
    #[inline]
    pub const fn current_count(&self) -> usize {
        self.current_count
    }

    /// Returns cumulative byte size of registered cover art images.
    #[inline]
    pub const fn current_total_size(&self) -> usize {
        self.current_total_size
    }

    /// Resets all registered counters to zero.
    pub fn reset(&mut self) {
        self.current_count = 0;
        self.current_total_size = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMMY_JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
    const DUMMY_PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
    const DUMMY_GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00";
    const DUMMY_WEBP: &[u8] = b"RIFF\x20\x00\x00\x00WEBPVP8 \x14\x00\x00\x00";

    #[test]
    fn test_format_detection() {
        assert_eq!(CoverArtQuotaGuard::detect_format(DUMMY_JPEG), CoverArtFormat::Jpeg);
        assert_eq!(CoverArtQuotaGuard::detect_format(DUMMY_PNG), CoverArtFormat::Png);
        assert_eq!(CoverArtQuotaGuard::detect_format(DUMMY_GIF), CoverArtFormat::Gif);
        assert_eq!(CoverArtQuotaGuard::detect_format(DUMMY_WEBP), CoverArtFormat::Webp);
        assert_eq!(CoverArtQuotaGuard::detect_format(b"corrupted"), CoverArtFormat::Unknown);
    }

    #[test]
    fn test_single_size_quota_enforcement() {
        let mut guard = CoverArtQuotaGuard::with_quotas(1024, 4096, 4);

        // 1000 bytes valid JPEG
        let mut valid_img = vec![0u8; 1000];
        valid_img[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        assert!(guard.inspect_and_register(&valid_img).is_ok());

        // 1025 bytes exceeds single limit
        let mut large_img = vec![0u8; 1025];
        large_img[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        let err = guard.inspect_and_register(&large_img).unwrap_err();
        assert_eq!(
            err,
            AudioDefenseError::CoverArtSizeExceeded {
                size: 1025,
                max_size: 1024
            }
        );
    }

    #[test]
    fn test_cumulative_total_quota_and_count_limits() {
        let mut guard = CoverArtQuotaGuard::with_quotas(1024, 2000, 2);

        let mut img1 = vec![0u8; 900];
        img1[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        assert!(guard.inspect_and_register(&img1).is_ok());

        let mut img2 = vec![0u8; 900];
        img2[0..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert!(guard.inspect_and_register(&img2).is_ok());

        // Exceeds count limit of 2
        let mut img3 = vec![0u8; 100];
        img3[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        let count_err = guard.inspect_and_register(&img3).unwrap_err();
        assert!(matches!(count_err, AudioDefenseError::CoverArtCountExceeded { .. }));

        // Test cumulative size fuse
        guard.reset();
        let mut big1 = vec![0u8; 1024];
        big1[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        assert!(guard.inspect_and_register(&big1).is_ok());

        let mut big2 = vec![0u8; 1000];
        big2[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
        let quota_err = guard.inspect_and_register(&big2).unwrap_err();
        assert_eq!(
            quota_err,
            AudioDefenseError::TotalCoverArtQuotaExceeded {
                total_size: 2024,
                max_quota: 2000
            }
        );
    }

    #[test]
    fn test_malformed_magic_rejection() {
        let mut guard = CoverArtQuotaGuard::new();
        let bad_payload = vec![0x12, 0x34, 0x56, 0x78, 0x90];
        assert!(guard.inspect_and_register(&bad_payload).is_err());
    }
}
