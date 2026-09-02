// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed domain models, track descriptors, and error types for video demuxing.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Recognized container formats for multimedia video streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoFormat {
    Mp4,
    Mov,
    Mkv,
    Webm,
    Avi,
    Unknown,
}

impl VideoFormat {
    /// Returns static string representation of the video container format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "MP4",
            Self::Mov => "QuickTime MOV",
            Self::Mkv => "Matroska MKV",
            Self::Webm => "WebM",
            Self::Avi => "Audio Video Interleave (AVI)",
            Self::Unknown => "Unknown",
        }
    }

    /// Returns standard MIME type for the container format.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Mp4 => "video/mp4",
            Self::Mov => "video/quicktime",
            Self::Mkv => "video/x-matroska",
            Self::Webm => "video/webm",
            Self::Avi => "video/x-msvideo",
            Self::Unknown => "application/octet-stream",
        }
    }

    /// Returns true if the format is a known and supported container type.
    #[must_use]
    pub const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// Recognized video compression codec standards and profiles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    #[serde(rename = "H265_HEVC")]
    H265_HEVC,
    VP8,
    VP9,
    AV1,
    ProRes,
    Mpeg4,
    Unknown(String),
}

impl VideoCodec {
    /// Returns readable name of the video codec.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::H264 => "H.264 / AVC",
            Self::H265_HEVC => "H.265 / HEVC",
            Self::VP8 => "VP8",
            Self::VP9 => "VP9",
            Self::AV1 => "AV1",
            Self::ProRes => "Apple ProRes",
            Self::Mpeg4 => "MPEG-4 Visual",
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// Maps FourCC or sample entry code to a `VideoCodec`.
    #[must_use]
    pub fn from_fourcc(fourcc: &[u8; 4]) -> Self {
        match fourcc {
            b"avc1" | b"avc2" | b"avc3" | b"avc4" | b"H264" | b"h264" | b"X264" | b"x264" => {
                Self::H264
            }
            b"hvc1" | b"hev1" | b"H265" | b"h265" | b"HEVC" | b"hevc" => Self::H265_HEVC,
            b"vp08" | b"VP80" | b"vp80" | b"VP8 " => Self::VP8,
            b"vp09" | b"VP90" | b"vp90" | b"VP9 " => Self::VP9,
            b"av01" | b"AV01" | b"av1 " | b"AV1 " => Self::AV1,
            b"apcn" | b"apch" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" | b"prh1" | b"prh2" => {
                Self::ProRes
            }
            b"mp4v" | b"DIVX" | b"divx" | b"XVID" | b"xvid" | b"DX50" | b"dx50" | b"FMP4" => {
                Self::Mpeg4
            }
            other => {
                let s = String::from_utf8_lossy(other).trim().to_string();
                Self::Unknown(if s.is_empty() {
                    format!("0x{:02X}{:02X}{:02X}{:02X}", other[0], other[1], other[2], other[3])
                } else {
                    s
                })
            }
        }
    }

    /// Maps Matroska / WebM CodecID to a `VideoCodec`.
    #[must_use]
    pub fn from_mkv_codec_id(id: &str) -> Self {
        let trimmed = id.trim();
        match trimmed {
            "V_MPEG4/ISO/AVC" => Self::H264,
            "V_MPEGH/ISO/HEVC" => Self::H265_HEVC,
            "V_VP8" => Self::VP8,
            "V_VP9" => Self::VP9,
            "V_AV1" => Self::AV1,
            "V_PRORES" => Self::ProRes,
            "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/SP" | "V_MS/VFW/FOURCC" => Self::Mpeg4,
            other => {
                if let Some(rest) = other.strip_prefix("V_") {
                    Self::Unknown(rest.to_string())
                } else {
                    Self::Unknown(other.to_string())
                }
            }
        }
    }
}

/// Recognized audio compression codec standards and profiles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioCodec {
    Aac,
    Mp3,
    Opus,
    Vorbis,
    Flac,
    Pcm,
    Ac3,
    Eac3,
    Unknown(String),
}

impl AudioCodec {
    /// Returns readable name of the audio codec.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Aac => "AAC (Advanced Audio Coding)",
            Self::Mp3 => "MPEG-1/2 Audio Layer III (MP3)",
            Self::Opus => "Opus",
            Self::Vorbis => "Ogg Vorbis",
            Self::Flac => "FLAC (Free Lossless Audio Codec)",
            Self::Pcm => "PCM (Uncompressed Linear)",
            Self::Ac3 => "Dolby Digital (AC-3)",
            Self::Eac3 => "Dolby Digital Plus (E-AC-3)",
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// Maps MP4 / MOV sample entry FourCC to an `AudioCodec`.
    #[must_use]
    pub fn from_mp4_fourcc(fourcc: &[u8; 4]) -> Self {
        match fourcc {
            b"mp4a" | b"aac " | b"AACL" | b"AACH" => Self::Aac,
            b".mp3" | b"mp3 " | b"MP3 " | b".MP3" => Self::Mp3,
            b"Opus" | b"opus" => Self::Opus,
            b"vorb" | b"VORB" => Self::Vorbis,
            b"fLaC" | b"flac" => Self::Flac,
            b"sowt" | b"twos" | b"in24" | b"in32" | b"fl32" | b"fl64" | b"lpcm" | b"raw " => {
                Self::Pcm
            }
            b"ac-3" | b"sac3" | b"AC-3" => Self::Ac3,
            b"ec-3" | b"sec3" | b"EC-3" | b"EAC3" => Self::Eac3,
            other => {
                let s = String::from_utf8_lossy(other).trim().to_string();
                Self::Unknown(if s.is_empty() {
                    format!("0x{:02X}{:02X}{:02X}{:02X}", other[0], other[1], other[2], other[3])
                } else {
                    s
                })
            }
        }
    }

    /// Maps Matroska / WebM CodecID to an `AudioCodec`.
    #[must_use]
    pub fn from_mkv_codec_id(id: &str) -> Self {
        let trimmed = id.trim();
        match trimmed {
            "A_AAC" | "A_AAC/MPEG2/LC" | "A_AAC/MPEG4/LC" | "A_AAC/MPEG4/LTP" => Self::Aac,
            "A_MPEG/L3" | "A_MPEG/L2" | "A_MPEG/L1" => Self::Mp3,
            "A_OPUS" => Self::Opus,
            "A_VORBIS" => Self::Vorbis,
            "A_FLAC" => Self::Flac,
            "A_PCM/INT/LIT" | "A_PCM/INT/BIG" | "A_PCM/FLOAT/IEEE" => Self::Pcm,
            "A_AC3" => Self::Ac3,
            "A_EAC3" => Self::Eac3,
            other => {
                if let Some(rest) = other.strip_prefix("A_") {
                    Self::Unknown(rest.to_string())
                } else {
                    Self::Unknown(other.to_string())
                }
            }
        }
    }

    /// Maps RIFF WAVE format tag to an `AudioCodec`.
    #[must_use]
    pub fn from_avi_tag(tag: u16) -> Self {
        match tag {
            0x0001 => Self::Pcm,
            0x0055 => Self::Mp3,
            0x00FF | 0x1600 | 0x706D => Self::Aac,
            0x2000 => Self::Ac3,
            0x2001 => Self::Eac3,
            0xF1AC => Self::Flac,
            0x674F => Self::Vorbis,
            0x704F => Self::Opus,
            other => Self::Unknown(format!("WAVE_TAG_0x{other:04X}")),
        }
    }
}

/// Metadata describing an elementary video track within a media container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoTrackInfo {
    /// Zero-based or container-assigned track identifier.
    pub track_id: u32,
    /// Video compression codec standard.
    pub codec: VideoCodec,
    /// Display or encoded frame width in pixels.
    pub width: u32,
    /// Display or encoded frame height in pixels.
    pub height: u32,
    /// Video frame rate in frames per second (e.g. 24.0, 29.97, 60.0).
    pub fps: f64,
    /// Nominal or average bitrate in kilobits per second, if available.
    pub bitrate_kbps: Option<u32>,
}

impl VideoTrackInfo {
    /// Creates a new `VideoTrackInfo` record.
    #[must_use]
    pub const fn new(
        track_id: u32,
        codec: VideoCodec,
        width: u32,
        height: u32,
        fps: f64,
        bitrate_kbps: Option<u32>,
    ) -> Self {
        Self {
            track_id,
            codec,
            width,
            height,
            fps,
            bitrate_kbps,
        }
    }
}

/// Metadata describing an audio stream track within a media container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTrackInfo {
    /// Container-assigned track identifier.
    pub track_id: u32,
    /// Audio compression codec standard.
    pub codec: AudioCodec,
    /// Channel count (e.g. 1 = Mono, 2 = Stereo, 6 = 5.1 Surround).
    pub channels: u32,
    /// Audio sample rate in Hertz (e.g. 44100, 48000).
    pub sample_rate: u32,
    /// Optional ISO 639-2 / BCP-47 language tag (e.g. "eng", "zho", "jpn").
    pub language: Option<String>,
}

impl AudioTrackInfo {
    /// Creates a new `AudioTrackInfo` record.
    #[must_use]
    pub const fn new(
        track_id: u32,
        codec: AudioCodec,
        channels: u32,
        sample_rate: u32,
        language: Option<String>,
    ) -> Self {
        Self {
            track_id,
            codec,
            channels,
            sample_rate,
            language,
        }
    }
}

/// Metadata describing an embedded subtitle or caption track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleTrackInfo {
    /// Container-assigned track identifier.
    pub track_id: u32,
    /// Subtitle formatting format (e.g. "vtt", "ass", "srt", "mov_text").
    pub format: String,
    /// Optional ISO 639-2 language tag (e.g. "eng", "fra").
    pub language: Option<String>,
    /// Optional title or description (e.g. "English [SDH]", "Director Commentary").
    pub title: Option<String>,
}

impl SubtitleTrackInfo {
    /// Creates a new `SubtitleTrackInfo` record.
    #[must_use]
    pub fn new(
        track_id: u32,
        format: impl Into<String>,
        language: Option<String>,
        title: Option<String>,
    ) -> Self {
        Self {
            track_id,
            format: format.into(),
            language,
            title,
        }
    }
}

/// Timeline chapter boundary and display title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterInfo {
    /// Chapter start position in milliseconds from container beginning.
    pub start_ms: u64,
    /// Chapter end position in milliseconds from container beginning.
    pub end_ms: u64,
    /// Chapter title or label (e.g. "Chapter 1: The Beginning").
    pub title: String,
}

impl ChapterInfo {
    /// Creates a new `ChapterInfo` boundary record.
    #[must_use]
    pub fn new(start_ms: u64, end_ms: u64, title: impl Into<String>) -> Self {
        Self {
            start_ms,
            end_ms,
            title: title.into(),
        }
    }
}

/// Comprehensive normalized metadata for a video container stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoMetadata {
    /// Identified video container format.
    pub format: VideoFormat,
    /// Total playback duration in milliseconds.
    pub duration_ms: u64,
    /// Ordered list of elementary video tracks.
    pub video_tracks: Vec<VideoTrackInfo>,
    /// Ordered list of audio stream tracks.
    pub audio_tracks: Vec<AudioTrackInfo>,
    /// Ordered list of subtitle / timed text tracks.
    pub subtitle_tracks: Vec<SubtitleTrackInfo>,
    /// Ordered list of timeline chapter markers.
    pub chapters: Vec<ChapterInfo>,
    /// Whether the media container carries embedded cover artwork.
    pub has_cover: bool,
}

impl VideoMetadata {
    /// Returns reference to the primary video track, if present.
    #[must_use]
    pub fn primary_video_track(&self) -> Option<&VideoTrackInfo> {
        self.video_tracks.first()
    }

    /// Returns reference to the primary audio track, if present.
    #[must_use]
    pub fn primary_audio_track(&self) -> Option<&AudioTrackInfo> {
        self.audio_tracks.first()
    }

    /// Returns display resolution (width, height) of the primary video track.
    #[must_use]
    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.primary_video_track().map(|v| (v.width, v.height))
    }

    /// Returns frame rate of the primary video track in frames per second.
    #[must_use]
    pub fn fps(&self) -> Option<f64> {
        self.primary_video_track().map(|v| v.fps)
    }
}

/// Unified error conditions during video container parsing and demuxing.
#[derive(Debug, Error)]
pub enum VideoError {
    /// Parsing error caused by malformed or unexpected binary payload.
    #[error("Invalid video data: {0}")]
    InvalidData(String),

    /// Premature end-of-file while reading container structures.
    #[error("Unexpected end of video stream: {0}")]
    UnexpectedEof(String),

    /// Unrecognized or unsupported video container format.
    #[error("Unsupported video container format: {0}")]
    UnsupportedFormat(String),

    /// Specific atom or box framing parsing failure with byte offset.
    #[error("Corrupted container atom '{atom}' at offset {offset}: {reason}")]
    CorruptedAtom {
        atom: String,
        offset: usize,
        reason: String,
    },

    /// Underlying standard I/O error.
    #[error("Video I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result alias for video demuxing operations.
pub type VideoResult<T> = Result<T, VideoError>;
