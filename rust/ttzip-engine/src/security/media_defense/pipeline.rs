// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified Video Security Pipeline Coordinating All 6 Defense Layers.
//!
//! Coordinates multi-stage defense-in-depth inspection across video stream headers,
//! container structures, geometric dimensions, demuxer streaming, subtitles, and sensitive allocations:
//! 1. Container Atom/Box Hierarchy Safety ([`AtomDepthGuard`])
//! 2. Video Resolution and Frame Memory Guard ([`VideoDimensionGuard`])
//! 3. Demuxer Stream and PTS Monotonicity Circuit Breaker ([`DemuxerLoopGuard`])
//! 4. Subtitle Script and Vector Drawing Sandbox ([`SubtitleScriptSandboxGuard`])
//! 5. Systemic Task Memory Watchdog ([`VideoMemoryBudgetGuard`])
//! 6. Zeroize-on-Drop Sensitive Frame Buffers ([`SensitiveVideoBuffer`])

use super::{
    AtomDepthGuard, AtomInspectionSummary, DemuxerLoopGuard, DemuxerLoopTracker, SanitizedSubtitle,
    SensitiveVideoBuffer, SubtitleScriptSandboxGuard, VideoDefenseError, VideoDimensionGuard,
    VideoDimensionReport, VideoMemoryBudgetGuard, VideoMemoryReservation, VideoPixelFormat,
    VideoSubtitleFormat,
};

/// Recognised video container stream format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VideoContainerFormat {
    /// ISO Base Media File Format (MP4, QuickTime MOV, M4V, 3GP).
    #[default]
    Mp4,
    /// Matroska Multimedia Container / WebM (.mkv, .webm).
    Mkv,
    /// Audio Video Interleave (.avi) RIFF container.
    Avi,
    /// Unknown or unsupported container format.
    Unknown,
}

/// Comprehensive inspection report produced by the video security pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoSecurityReport {
    /// Detected container format.
    pub format: VideoContainerFormat,
    /// Container payload byte length.
    pub payload_size: usize,
    /// Atom hierarchy inspection summary (for MP4/ISOBMFF containers).
    pub atom_summary: Option<AtomInspectionSummary>,
    /// Validated video dimensions if probed from container headers.
    pub dimension_report: Option<VideoDimensionReport>,
    /// Resident memory bytes reserved for container parsing.
    pub memory_reserved_bytes: usize,
}

/// Unified 6-Layer Video Media Security Pipeline.
#[derive(Debug, Clone, Default)]
pub struct VideoSecurityPipeline {
    /// Atom / Box nesting depth and 64-bit largesize guard.
    pub atom_guard: AtomDepthGuard,
    /// Video dimension and frame memory guard.
    pub dimension_guard: VideoDimensionGuard,
    /// Demuxer loop and PTS monotonicity guard.
    pub demuxer_guard: DemuxerLoopGuard,
    /// Subtitle active script and ASS drawing sandbox guard.
    pub subtitle_guard: SubtitleScriptSandboxGuard,
    /// Task resident memory budget watchdog.
    pub memory_watchdog: VideoMemoryBudgetGuard,
}

impl VideoSecurityPipeline {
    /// Creates a customized video defense pipeline.
    pub fn new(
        atom_guard: AtomDepthGuard,
        dimension_guard: VideoDimensionGuard,
        demuxer_guard: DemuxerLoopGuard,
        subtitle_guard: SubtitleScriptSandboxGuard,
        memory_watchdog: VideoMemoryBudgetGuard,
    ) -> Self {
        Self {
            atom_guard,
            dimension_guard,
            demuxer_guard,
            subtitle_guard,
            memory_watchdog,
        }
    }

    /// Detects container format from the initial magic bytes of the stream.
    pub fn detect_container_format(data: &[u8]) -> VideoContainerFormat {
        if data.len() >= 12 {
            // Check RIFF AVI
            if &data[0..4] == b"RIFF" && &data[8..12] == b"AVI " {
                return VideoContainerFormat::Avi;
            }
        }

        if data.len() >= 4 {
            // Check Matroska EBML header magic: [0x1A, 0x45, 0xDF, 0xA3]
            if &data[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
                return VideoContainerFormat::Mkv;
            }

            // Check MP4 / QuickTime box headers: e.g. ftyp, moov, mdat, free, skip, wide
            if data.len() >= 8 {
                let box_type = &data[4..8];
                let is_mp4_box = matches!(
                    box_type,
                    b"ftyp"
                        | b"moov"
                        | b"mdat"
                        | b"free"
                        | b"skip"
                        | b"wide"
                        | b"pnot"
                        | b"prfl"
                        | b"pdin"
                );
                if is_mp4_box {
                    return VideoContainerFormat::Mp4;
                }
            }
        }

        VideoContainerFormat::Unknown
    }

    /// Probes and validates container headers, enforcing atom depth and memory quotas.
    pub fn inspect_container_header(
        &mut self,
        data: &[u8],
    ) -> Result<(VideoSecurityReport, VideoMemoryReservation), VideoDefenseError> {
        let reservation = self.memory_watchdog.reserve(data.len())?;
        let format = Self::detect_container_format(data);

        let atom_summary = match format {
            VideoContainerFormat::Mp4 => {
                let summary = self.atom_guard.scan_container_atoms(data)?;
                Some(summary)
            }
            VideoContainerFormat::Mkv => {
                // Validate EBML header integrity
                if data.len() < 4 || &data[0..4] != [0x1A, 0x45, 0xDF, 0xA3] {
                    return Err(VideoDefenseError::MalformedContainerHeader {
                        reason: "Invalid or missing EBML header in Matroska stream".to_string(),
                    });
                }
                None
            }
            VideoContainerFormat::Avi => {
                // Validate RIFF AVI header integrity
                if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"AVI " {
                    return Err(VideoDefenseError::MalformedContainerHeader {
                        reason: "Invalid or missing RIFF AVI header chunk".to_string(),
                    });
                }
                None
            }
            VideoContainerFormat::Unknown => None,
        };

        let report = VideoSecurityReport {
            format,
            payload_size: data.len(),
            atom_summary,
            dimension_report: None,
            memory_reserved_bytes: reservation.bytes(),
        };

        Ok((report, reservation))
    }

    /// Validates video resolution and computes single-frame memory requirements.
    pub fn validate_video_dimensions(
        &self,
        width: u32,
        height: u32,
        pixel_format: VideoPixelFormat,
    ) -> Result<VideoDimensionReport, VideoDefenseError> {
        self.dimension_guard.inspect(width, height, pixel_format)
    }

    /// Sanitizes subtitle tracks, disarming scripts, external protocols, and path traversals.
    pub fn sanitize_subtitles(
        &self,
        text: &str,
        format: VideoSubtitleFormat,
    ) -> Result<SanitizedSubtitle, VideoDefenseError> {
        self.subtitle_guard.sanitize(text, format)
    }

    /// Spawns a stateful Demuxer Loop Tracker for monitoring playback stream health.
    pub fn create_demuxer_tracker(&self) -> DemuxerLoopTracker {
        self.demuxer_guard.create_tracker()
    }

    /// Reserves memory against the task resident memory budget.
    pub fn reserve_memory(
        &self,
        bytes: usize,
    ) -> Result<VideoMemoryReservation, VideoDefenseError> {
        self.memory_watchdog.reserve(bytes)
    }

    /// Safely validates dimensions, reserves memory quota, and allocates a zeroized sensitive video frame.
    pub fn allocate_sensitive_frame(
        &self,
        width: u32,
        height: u32,
        pixel_format: VideoPixelFormat,
    ) -> Result<(SensitiveVideoBuffer, VideoMemoryReservation), VideoDefenseError> {
        let frame_bytes = self
            .dimension_guard
            .estimate_frame_size(width, height, pixel_format)?;
        let reservation = self.memory_watchdog.reserve(frame_bytes)?;
        let buffer = SensitiveVideoBuffer::zeroed(frame_bytes);
        Ok((buffer, reservation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_container_detection() {
        // MP4 header with ftyp box
        let mut mp4_data = Vec::new();
        mp4_data.extend_from_slice(&24u32.to_be_bytes());
        mp4_data.extend_from_slice(b"ftyp");
        mp4_data.extend_from_slice(b"isom\0\0\0\0isommp42");
        assert_eq!(
            VideoSecurityPipeline::detect_container_format(&mp4_data),
            VideoContainerFormat::Mp4
        );

        // MKV EBML header
        let mkv_data = [0x1A, 0x45, 0xDF, 0xA3, 0x93, 0x42, 0x86, 0x81, 0x01];
        assert_eq!(
            VideoSecurityPipeline::detect_container_format(&mkv_data),
            VideoContainerFormat::Mkv
        );

        // AVI header
        let avi_data = b"RIFF\x20\x00\x00\x00AVI LIST\x10\x00\x00\x00hdrl";
        assert_eq!(
            VideoSecurityPipeline::detect_container_format(avi_data),
            VideoContainerFormat::Avi
        );
    }

    #[test]
    fn test_pipeline_end_to_end_frame_allocation() {
        let pipeline = VideoSecurityPipeline::default();
        let (frame_buf, reservation) = pipeline
            .allocate_sensitive_frame(1920, 1080, VideoPixelFormat::Yuv420p)
            .unwrap();

        let expected_len = 1920 * 1080 * 3 / 2;
        assert_eq!(frame_buf.len(), expected_len);
        assert_eq!(reservation.bytes(), expected_len);
        assert!(frame_buf.iter().all(|&b| b == 0));
    }
}
