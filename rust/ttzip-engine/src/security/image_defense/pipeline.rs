// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified multi-stage image defense pipeline orchestrating all 6 security layers.

use super::{
    ExifInspectionSummary, ExifSafetyGuard, IccInspectionSummary, IccProfileGuard,
    ImageDefenseError, ImageDimensions, MalformedChunkGuard, MemoryBudgetWatchdog,
    MemoryReservation, PixelBombGuard, SanitizedChunkReport,
};

/// Comprehensive report produced by the defense pipeline prior to full decoding.
#[derive(Debug)]
pub struct ImageInspectionReport {
    pub dimensions: ImageDimensions,
    pub chunk_report: SanitizedChunkReport,
    pub exif_summary: Option<ExifInspectionSummary>,
    pub icc_summary: Option<IccInspectionSummary>,
    pub memory_reservation: MemoryReservation,
}

/// 6-Layer Defense-in-Depth pipeline coordinating zero-trust image validation.
#[derive(Debug, Clone, Default)]
pub struct ImageSecurityPipeline {
    pub pixel_bomb_guard: PixelBombGuard,
    pub exif_guard: ExifSafetyGuard,
    pub icc_guard: IccProfileGuard,
    pub watchdog: MemoryBudgetWatchdog,
}

impl ImageSecurityPipeline {
    /// Creates a custom pipeline instance with specified guards and watchdog.
    pub fn new(
        pixel_bomb_guard: PixelBombGuard,
        exif_guard: ExifSafetyGuard,
        icc_guard: IccProfileGuard,
        watchdog: MemoryBudgetWatchdog,
    ) -> Self {
        Self {
            pixel_bomb_guard,
            exif_guard,
            icc_guard,
            watchdog,
        }
    }

    /// Executes the full 6-stage defense inspection across the raw image payload.
    pub fn verify_image_stream(
        &self,
        data: &[u8],
    ) -> Result<ImageInspectionReport, ImageDefenseError> {
        // Stage 1: Malformed chunk and stream truncation validation
        let chunk_report = MalformedChunkGuard::inspect_and_validate(data)?;

        // Stage 2: Zero-allocation dimension probe & Pixel Bomb expansion ratio fuse
        let dimensions = self.pixel_bomb_guard.inspect_and_validate(data)?;

        // Stage 3: EXIF metadata safety scan (if stream contains EXIF TIFF markers)
        let exif_summary = if let Some(exif_offset) = Self::find_exif_offset(data) {
            let summary = self.exif_guard.inspect(&data[exif_offset..])?;
            Some(summary)
        } else {
            None
        };

        // Stage 4: ICC color profile poisoning scan (if stream contains ICC chunk)
        let icc_summary = if let Some(icc_slice) = Self::find_icc_payload(data) {
            let summary = self.icc_guard.inspect(icc_slice)?;
            Some(summary)
        } else {
            None
        };

        // Stage 5: Memory Watchdog quota reservation for uncompressed buffer
        let uncompressed_bytes = (dimensions.width as usize)
            .saturating_mul(dimensions.height as usize)
            .saturating_mul(dimensions.channels as usize);
        let memory_reservation = self.watchdog.reserve(uncompressed_bytes)?;

        // Stage 6: Return safety report holding active memory reservation
        Ok(ImageInspectionReport {
            dimensions,
            chunk_report,
            exif_summary,
            icc_summary,
            memory_reservation,
        })
    }

    fn find_exif_offset(data: &[u8]) -> Option<usize> {
        // JPEG APP1 Exif marker
        if data.starts_with(&[0xFF, 0xD8]) {
            let mut pos = 2;
            while pos + 4 <= data.len() {
                if data[pos] != 0xFF {
                    pos += 1;
                    continue;
                }
                let marker = data[pos + 1];
                pos += 2;
                if marker == 0xE1 && pos + 8 <= data.len() {
                    // Check for Exif\0\0
                    if &data[pos + 2..pos + 8] == b"Exif\0\0" {
                        return Some(pos + 2);
                    }
                }
                if marker == 0xD9 || marker == 0xDA {
                    break;
                }
                if pos + 2 > data.len() {
                    break;
                }
                let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
                pos += len;
            }
        }

        // Direct TIFF / EXIF header
        if data.starts_with(b"Exif\0\0") {
            return Some(0);
        }
        if data.starts_with(b"II\x2A\x00") || data.starts_with(b"MM\x00\x2A") {
            return Some(0);
        }

        None
    }

    fn find_icc_payload(data: &[u8]) -> Option<&[u8]> {
        // PNG iCCP chunk
        if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            let mut offset = 8;
            while offset + 12 <= data.len() {
                let chunk_len = u32::from_be_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;
                let chunk_type = &data[offset + 4..offset + 8];
                if chunk_type == b"iCCP" && offset + 8 + chunk_len <= data.len() {
                    return Some(&data[offset + 8..offset + 8 + chunk_len]);
                }
                offset = offset.saturating_add(chunk_len + 12);
            }
        }

        // Raw ICC profile
        if data.len() >= 128 && data.get(36..40) == Some(b"acsp") {
            return Some(data);
        }

        None
    }
}
