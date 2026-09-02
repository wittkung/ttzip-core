// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Top-level unified facade and format dispatcher for video stream demuxing.

use super::avi::TTZipAviDemuxer;
use super::mkv::TTZipMkvDemuxer;
use super::mp4::TTZipMp4Demuxer;
use super::types::{VideoError, VideoFormat, VideoMetadata, VideoResult};

/// Top-level facade for video container format probing, metadata extraction, and cover retrieval.
pub struct TTZipVideoDemuxer;

impl TTZipVideoDemuxer {
    /// Identifies the container format of a video stream buffer via magic byte heuristics.
    #[must_use]
    pub fn probe_from_bytes(data: &[u8]) -> VideoFormat {
        if data.len() < 4 {
            return VideoFormat::Unknown;
        }

        // 1. Check MKV / WebM EBML Header (0x1A45DFA3)
        if data.len() >= 4 && &data[0..4] == &[0x1A, 0x45, 0xDF, 0xA3] {
            return TTZipMkvDemuxer::probe_format(data);
        }

        // 2. Check RIFF AVI Header ("RIFF" + "AVI ")
        let avi_fmt = TTZipAviDemuxer::probe_format(data);
        if avi_fmt.is_known() {
            return avi_fmt;
        }

        // 3. Check MP4 / MOV Header (ftyp, moov, mdat, wide)
        let mp4_fmt = TTZipMp4Demuxer::probe_format(data);
        if mp4_fmt.is_known() {
            return mp4_fmt;
        }

        VideoFormat::Unknown
    }

    /// Demuxes the video buffer into normalized tracks, duration, chapters, and cover metadata.
    pub fn demux_from_bytes(data: &[u8]) -> VideoResult<VideoMetadata> {
        let format = Self::probe_from_bytes(data);

        match format {
            VideoFormat::Mp4 | VideoFormat::Mov => {
                let demuxer = TTZipMp4Demuxer::new(data);
                demuxer.demux()
            }
            VideoFormat::Mkv | VideoFormat::Webm => {
                let demuxer = TTZipMkvDemuxer::new(data);
                demuxer.demux()
            }
            VideoFormat::Avi => {
                let demuxer = TTZipAviDemuxer::new(data);
                demuxer.demux()
            }
            VideoFormat::Unknown => {
                // Fallback attempt: try demuxing as MP4 first, then MKV, then AVI
                if let Ok(meta) = TTZipMp4Demuxer::new(data).demux() {
                    return Ok(meta);
                }
                if let Ok(meta) = TTZipMkvDemuxer::new(data).demux() {
                    return Ok(meta);
                }
                if let Ok(meta) = TTZipAviDemuxer::new(data).demux() {
                    return Ok(meta);
                }

                Err(VideoError::UnsupportedFormat(
                    "Unable to identify video container format from byte stream".to_string(),
                ))
            }
        }
    }

    /// Extracts embedded cover artwork payload bytes from the video buffer if present.
    pub fn extract_cover_from_bytes(data: &[u8]) -> VideoResult<Option<Vec<u8>>> {
        let format = Self::probe_from_bytes(data);

        match format {
            VideoFormat::Mp4 | VideoFormat::Mov => {
                let demuxer = TTZipMp4Demuxer::new(data);
                Ok(demuxer.extract_cover())
            }
            VideoFormat::Mkv | VideoFormat::Webm => {
                let demuxer = TTZipMkvDemuxer::new(data);
                Ok(demuxer.extract_cover())
            }
            VideoFormat::Avi => Ok(None),
            VideoFormat::Unknown => {
                if let Some(cover) = TTZipMp4Demuxer::new(data).extract_cover() {
                    return Ok(Some(cover));
                }
                if let Some(cover) = TTZipMkvDemuxer::new(data).extract_cover() {
                    return Ok(Some(cover));
                }
                Ok(None)
            }
        }
    }
}
