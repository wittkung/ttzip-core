// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust video demuxing, metadata extraction, and embedded artwork retrieval microkernel.
//!
//! Provides zero-unsafe multi-format container parsers (MP4, QuickTime MOV, Matroska MKV,
//! WebM, and RIFF AVI), track descriptor extraction, FPS/duration normalization, and chapter extraction.

mod avi;
mod demuxer;
mod mkv;
mod mp4;
pub mod types;


#[cfg(test)]
mod tests;

pub use avi::TTZipAviDemuxer;
pub use demuxer::TTZipVideoDemuxer;
pub use mkv::TTZipMkvDemuxer;
pub use mp4::TTZipMp4Demuxer;
pub use types::{
    AudioCodec, AudioTrackInfo, ChapterInfo, SubtitleTrackInfo, VideoCodec, VideoError, VideoFormat,
    VideoMetadata, VideoResult, VideoTrackInfo,
};
