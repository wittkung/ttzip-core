// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records, Enums, and Error Types for Video Metadata and Cover Art.

use std::collections::HashMap;

/// Supported video container and format classifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, uniffi::Enum)]
pub enum UniFFIVideoFormat {
    /// MPEG-4 Part 14 container (.mp4).
    #[default]
    Mp4,
    /// Apple MPEG-4 video format (.m4v).
    M4v,
    /// Apple QuickTime Movie container (.mov, .qt).
    Mov,
    /// Matroska Multimedia Container (.mkv).
    Mkv,
    /// WebM open media format (.webm).
    Webm,
    /// Audio Video Interleave (.avi).
    Avi,
    /// Windows Media Video (.wmv, .asf).
    Wmv,
    /// Flash Video format (.flv).
    Flv,
    /// MPEG Transport Stream (.ts, .m2ts).
    Ts,
    /// Ogg Theora video container (.ogv).
    Ogv,
    /// Unrecognized or generic video format.
    Unknown,
}

impl UniFFIVideoFormat {
    /// Human-readable format name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Mp4 => "MPEG-4 Part 14 Video (MP4)",
            Self::M4v => "Apple MPEG-4 Video (M4V)",
            Self::Mov => "Apple QuickTime Movie (MOV)",
            Self::Mkv => "Matroska Video (MKV)",
            Self::Webm => "WebM Video (WebM)",
            Self::Avi => "Audio Video Interleave (AVI)",
            Self::Wmv => "Windows Media Video (WMV)",
            Self::Flv => "Flash Video (FLV)",
            Self::Ts => "MPEG Transport Stream (TS)",
            Self::Ogv => "Ogg Theora Video (OGV)",
            Self::Unknown => "Unknown Video Format",
        }
    }

    /// Standard MIME type string.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Mp4 => "video/mp4",
            Self::M4v => "video/x-m4v",
            Self::Mov => "video/quicktime",
            Self::Mkv => "video/x-matroska",
            Self::Webm => "video/webm",
            Self::Avi => "video/x-msvideo",
            Self::Wmv => "video/x-ms-wmv",
            Self::Flv => "video/x-flv",
            Self::Ts => "video/mp2t",
            Self::Ogv => "video/ogg",
            Self::Unknown => "application/octet-stream",
        }
    }

    /// Infers video format from a file extension or filename.
    pub fn from_extension(ext_or_name: &str) -> Self {
        let ext = ext_or_name
            .rsplit('.')
            .next()
            .unwrap_or(ext_or_name)
            .to_ascii_lowercase();
        match ext.as_str() {
            "mp4" => Self::Mp4,
            "m4v" => Self::M4v,
            "mov" | "qt" => Self::Mov,
            "mkv" => Self::Mkv,
            "webm" => Self::Webm,
            "avi" => Self::Avi,
            "wmv" | "asf" => Self::Wmv,
            "flv" => Self::Flv,
            "ts" | "m2ts" | "mts" => Self::Ts,
            "ogv" => Self::Ogv,
            _ => Self::Unknown,
        }
    }
}

/// Video track codec classifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, uniffi::Enum)]
pub enum UniFFIVideoCodec {
    /// Advanced Video Coding (AVC / H.264).
    #[default]
    H264,
    /// High Efficiency Video Coding (HEVC / H.265).
    Hevc,
    /// AOMedia Video 1 (AV1).
    Av1,
    /// Google VP9.
    Vp9,
    /// Google VP8.
    Vp8,
    /// Apple ProRes family.
    ProRes,
    /// Xiph Theora.
    Theora,
    /// MPEG-4 Part 2 Visual (DivX / Xvid).
    Mpeg4,
    /// MPEG-2 Video.
    Mpeg2,
    /// Unrecognized video codec.
    Unknown,
}

impl UniFFIVideoCodec {
    /// Human-readable codec name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::H264 => "H.264 / AVC",
            Self::Hevc => "H.265 / HEVC",
            Self::Av1 => "AV1",
            Self::Vp9 => "VP9",
            Self::Vp8 => "VP8",
            Self::ProRes => "Apple ProRes",
            Self::Theora => "Theora",
            Self::Mpeg4 => "MPEG-4 Part 2",
            Self::Mpeg2 => "MPEG-2",
            Self::Unknown => "Unknown Codec",
        }
    }
}

/// Audio track codec classifications within video containers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, uniffi::Enum)]
pub enum UniFFIAudioCodec {
    /// Advanced Audio Coding (AAC).
    #[default]
    Aac,
    /// Dolby Digital (AC-3).
    Ac3,
    /// Dolby Digital Plus (Enhanced AC-3 / E-AC-3).
    Eac3,
    /// Opus Audio Codec.
    Opus,
    /// Free Lossless Audio Codec (FLAC).
    Flac,
    /// Ogg Vorbis.
    Vorbis,
    /// MPEG-1 Audio Layer III (MP3).
    Mp3,
    /// Apple Lossless Audio Codec (ALAC).
    Alac,
    /// Uncompressed Linear PCM.
    Pcm,
    /// Unrecognized audio codec.
    Unknown,
}

impl UniFFIAudioCodec {
    /// Human-readable audio codec name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Aac => "AAC",
            Self::Ac3 => "Dolby Digital (AC-3)",
            Self::Eac3 => "Dolby Digital Plus (E-AC-3)",
            Self::Opus => "Opus",
            Self::Flac => "FLAC",
            Self::Vorbis => "Vorbis",
            Self::Mp3 => "MP3",
            Self::Alac => "Apple Lossless (ALAC)",
            Self::Pcm => "Linear PCM",
            Self::Unknown => "Unknown Audio Codec",
        }
    }
}

/// Technical stream properties of an individual video track.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIVideoTrackInfo {
    /// 1-based index or container track ID.
    pub track_id: u32,
    /// Strongly-typed video codec enumeration.
    pub codec: UniFFIVideoCodec,
    /// Detailed or raw codec descriptor string (e.g. "avc1", "hev1", "vp09.00").
    pub codec_name: String,
    /// Video frame width in pixels.
    pub width: u32,
    /// Video frame height in pixels.
    pub height: u32,
    /// Frame rate in frames per second (e.g. 23.976, 29.97, 60.0).
    pub frame_rate: f64,
    /// Average video bitrate in kilobits per second.
    pub bitrate_kbps: u32,
    /// Duration of this video track in seconds.
    pub duration_seconds: f64,
    /// Aspect ratio string representation (e.g. "16:9", "4:3", "2.39:1").
    pub aspect_ratio: String,
    /// Color primaries or color space (e.g. "BT.709", "BT.2020", "Display P3").
    pub color_space: Option<String>,
    /// High Dynamic Range format if present (e.g. "HDR10", "Dolby Vision", "HLG").
    pub hdr_format: Option<String>,
    /// Display rotation in clockwise degrees (0, 90, 180, 270).
    pub rotation_degrees: u32,
}

/// Technical stream properties of an individual audio track within the video.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIAudioTrackInfo {
    /// 1-based index or container track ID.
    pub track_id: u32,
    /// Strongly-typed audio codec enumeration.
    pub codec: UniFFIAudioCodec,
    /// Detailed or raw codec descriptor string (e.g. "mp4a.40.2", "opus", "ac-3").
    pub codec_name: String,
    /// Audio sample rate in Hertz (e.g. 44100, 48000).
    pub sample_rate: u32,
    /// Number of audio channels (e.g. 2 for stereo, 6 for 5.1 surround).
    pub channels: u32,
    /// Audio channel layout descriptor (e.g. "Stereo", "5.1", "7.1.4").
    pub channel_layout: String,
    /// Audio sample bit depth if applicable (e.g. 16, 24).
    pub bit_depth: Option<u32>,
    /// Average audio bitrate in kilobits per second.
    pub bitrate_kbps: u32,
    /// ISO 639-2 language code (e.g. "eng", "zho", "jpn").
    pub language: Option<String>,
    /// Descriptive track name or title (e.g. "Director's Commentary").
    pub title: Option<String>,
    /// Whether this track is designated as the default audio track.
    pub is_default: bool,
}

/// Information regarding an embedded subtitle or timed text track.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFISubtitleTrackInfo {
    /// 1-based index or container track ID.
    pub track_id: u32,
    /// Subtitle format/codec (e.g. "SubRip (SRT)", "ASS/SSA", "VobSub", "tx3g").
    pub format: String,
    /// ISO 639-2 language code (e.g. "eng", "spa", "fra").
    pub language: Option<String>,
    /// Subtitle track display name or description.
    pub title: Option<String>,
    /// Whether this subtitle track is marked for forced display.
    pub is_forced: bool,
    /// Whether this subtitle track is marked as default.
    pub is_default: bool,
    /// Whether this subtitle track contains SDH (Subtitles for Deaf and Hard of Hearing).
    pub is_sdh: bool,
}

/// Chapter navigation marker in the video timeline.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIChapterInfo {
    /// 1-based sequential chapter index.
    pub chapter_id: u32,
    /// Descriptive chapter title.
    pub title: String,
    /// Chapter start timestamp in seconds from video origin.
    pub start_time_seconds: f64,
    /// Chapter end timestamp in seconds.
    pub end_time_seconds: f64,
}

/// Comprehensive high-level video container and media stream metadata record.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIVideoMetadata {
    /// Identified video container format.
    pub format: UniFFIVideoFormat,
    /// Human-readable container name (e.g. "MPEG-4 Part 14 Video (MP4)").
    pub container_name: String,
    /// Total duration of the video container in seconds.
    pub duration_seconds: f64,
    /// Total byte size of the video file/stream.
    pub file_size_bytes: u64,
    /// Total overall average bitrate in kilobits per second.
    pub bitrate_kbps: u32,
    /// List of video streams/tracks found in container.
    pub video_tracks: Vec<UniFFIVideoTrackInfo>,
    /// List of audio streams/tracks found in container.
    pub audio_tracks: Vec<UniFFIAudioTrackInfo>,
    /// List of embedded subtitle tracks found in container.
    pub subtitle_tracks: Vec<UniFFISubtitleTrackInfo>,
    /// List of chapter markers in chronological order.
    pub chapters: Vec<UniFFIChapterInfo>,
    /// Media title if present in tags.
    pub title: Option<String>,
    /// Director, artist, or author credit if present.
    pub artist_or_director: Option<String>,
    /// Release date or creation timestamp string.
    pub creation_date: Option<String>,
    /// Encoding application or multiplexer tool.
    pub encoder: Option<String>,
    /// Whether embedded cover / poster art is available.
    pub has_cover: bool,
    /// MIME type of the embedded cover art if present (e.g. "image/jpeg", "image/png").
    pub cover_mime_type: Option<String>,
    /// Additional unstructured key-value tag metadata.
    pub extra_tags: HashMap<String, String>,
}

/// Strongly-typed video metadata errors mapped directly to Swift `throws UniFFIVideoError`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum UniFFIVideoError {
    /// The container or codec format is not supported.
    #[error("Unsupported video format: {format}")]
    UnsupportedFormat { format: String },

    /// Failure encountered while parsing video container atoms or headers.
    #[error("Video parse error: {message}")]
    ParseError { message: String },

    /// File system or stream I/O failure.
    #[error("I/O error during video operation: {message}")]
    IoError { message: String },

    /// The video bitstream or container header is corrupted or prematurely truncated.
    #[error("Video stream corrupted or truncated")]
    CorruptedData,

    /// Specified track ID was not found in container.
    #[error("Track not found: {track_id}")]
    TrackNotFound { track_id: u32 },

    /// No embedded poster or cover art was found in video container.
    #[error("Cover art not found in video container")]
    CoverArtNotFound,

    /// Supplied parameter is invalid or out of bounds.
    #[error("Invalid video parameter: {parameter}")]
    InvalidParameter { parameter: String },

    /// Video operation was explicitly cancelled.
    #[error("Video operation cancelled")]
    Cancelled,
}

impl UniFFIVideoError {
    /// Constructs a parse error variant with formatted message.
    pub fn parse_err(msg: impl std::fmt::Display) -> Self {
        Self::ParseError {
            message: msg.to_string(),
        }
    }

    /// Constructs an I/O error variant.
    pub fn io_err(msg: impl std::fmt::Display) -> Self {
        Self::IoError {
            message: msg.to_string(),
        }
    }
}
