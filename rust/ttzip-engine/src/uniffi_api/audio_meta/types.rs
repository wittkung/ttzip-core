// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records and Types for Audio Metadata, Cover Art, Waveforms, and PCM Decoding.

use std::collections::HashMap;

/// Strongly-typed audio operation error enum mapped directly to Swift `throws UniFFIAudioError`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum UniFFIAudioError {
    /// Failure during audio packet decoding or PCM transformation.
    #[error("Audio decode error: {message}")]
    DecodeError { message: String },

    /// The container or codec format is not supported or recognized.
    #[error("Unsupported audio format: {format}")]
    UnsupportedFormat { format: String },

    /// File system or stream I/O failure.
    #[error("I/O error during audio operation: {message}")]
    IoError { message: String },

    /// Supplied parameter is out of valid bounds or invalid.
    #[error("Invalid audio parameter: {parameter}")]
    InvalidParameter { parameter: String },

    /// The audio bitstream is prematurely truncated or invalid.
    #[error("Audio stream corrupted or truncated")]
    CorruptedStream,

    /// Audio operation was explicitly cancelled by caller.
    #[error("Audio operation cancelled")]
    Cancelled,
}

impl UniFFIAudioError {
    /// Constructs a decode error variant with descriptive message.
    pub fn decode_err(msg: impl std::fmt::Display) -> Self {
        Self::DecodeError {
            message: msg.to_string(),
        }
    }

    /// Constructs an I/O error variant with context.
    pub fn io_err(msg: impl std::fmt::Display) -> Self {
        Self::IoError {
            message: msg.to_string(),
        }
    }
}

/// Embedded picture/album artwork metadata extracted from audio tags.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIAudioCoverArt {
    /// MIME type of the cover image (e.g. "image/jpeg", "image/png").
    pub mime_type: String,
    /// Image width in pixels if known.
    pub width: Option<u32>,
    /// Image height in pixels if known.
    pub height: Option<u32>,
    /// Raw picture image bytes (JPEG / PNG / WebP).
    pub data: Vec<u8>,
    /// Picture description or tag type (e.g. "Front Cover", "Back Cover", "Icon").
    pub description: Option<String>,
}

/// Technical stream properties of the primary audio track.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIAudioStreamInfo {
    /// Short codec identifier (e.g. "mp3", "aac", "flac", "wav", "vorbis", "alac", "opus").
    pub codec_name: String,
    /// Detailed codec description or profile (e.g. "MPEG-1 Layer 3", "AAC-LC", "FLAC 16-bit").
    pub codec_long_name: String,
    /// Sample rate in Hertz (e.g. 44100, 48000, 96000).
    pub sample_rate: u32,
    /// Number of audio channels (e.g. 1 for mono, 2 for stereo, 6 for 5.1 surround).
    pub channels: u32,
    /// Channel layout descriptor (e.g. "mono", "stereo", "5.1").
    pub channel_layout: String,
    /// Bits per sample if fixed uncompressed/lossless (e.g. 16, 24, 32).
    pub bits_per_sample: Option<u32>,
    /// Nominal or average bitrate in bits per second (e.g. 320000).
    pub bit_rate: Option<u64>,
    /// Total duration of the primary audio stream in seconds.
    pub duration_seconds: f64,
    /// Total audio frame or sample count if available.
    pub total_frames: Option<u64>,
}

/// Comprehensive high-level acoustic and tag metadata record.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIAudioMetadata {
    /// Track title.
    pub title: Option<String>,
    /// Primary artist or performer.
    pub artist: Option<String>,
    /// Album title.
    pub album: Option<String>,
    /// Album artist or compilation creator.
    pub album_artist: Option<String>,
    /// Track number in album sequence.
    pub track_number: Option<u32>,
    /// Total number of tracks in album.
    pub track_total: Option<u32>,
    /// Disc number in multi-disc set.
    pub disc_number: Option<u32>,
    /// Total number of discs in set.
    pub disc_total: Option<u32>,
    /// Release year or date string.
    pub year: Option<String>,
    /// Music genre classification.
    pub genre: Option<String>,
    /// Musical composer.
    pub composer: Option<String>,
    /// Song lyrics text if present.
    pub lyrics: Option<String>,
    /// Legal copyright notice.
    pub copyright: Option<String>,
    /// Embedded cover art image if present.
    pub cover_art: Option<UniFFIAudioCoverArt>,
    /// Technical properties of the primary audio stream.
    pub stream_info: UniFFIAudioStreamInfo,
    /// Total size of the audio file in bytes.
    pub file_size_bytes: u64,
    /// Container format name (e.g. "mp3", "m4a", "flac", "wav", "ogg", "aiff").
    pub container_format: String,
    /// Additional unstructured key-value tag pairs.
    pub extra_tags: HashMap<String, String>,
}

/// Normalized acoustic peak and RMS waveform amplitude envelope.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIAudioWaveform {
    /// Array of normalized peak amplitude values `[0.0, 1.0]`.
    pub amplitudes: Vec<f32>,
    /// Total number of sample buckets in the waveform array.
    pub bucket_count: u32,
    /// Total duration of the analyzed audio in seconds.
    pub duration_seconds: f64,
    /// Audio sample rate in Hertz.
    pub sample_rate: u32,
    /// Number of audio channels analyzed.
    pub channels: u32,
    /// Array of normalized RMS energy amplitude values `[0.0, 1.0]`.
    pub rms_amplitudes: Vec<f32>,
}

/// Decoded chunk packet of floating-point PCM audio samples for streaming playback.
#[derive(Clone, Debug, PartialEq, Default, uniffi::Record)]
pub struct UniFFIAudioPacket {
    /// Presentation timestamp in milliseconds from stream origin.
    pub pts_ms: u64,
    /// Duration of this audio packet in milliseconds.
    pub duration_ms: u64,
    /// Number of interleaved audio channels.
    pub channels: u32,
    /// Sample rate in Hertz.
    pub sample_rate: u32,
    /// Interleaved 32-bit floating point PCM audio samples normalized `[-1.0, 1.0]`.
    pub pcm_f32_samples: Vec<f32>,
    /// Number of audio frames in this packet (`pcm_f32_samples.len() / channels`).
    pub frame_count: u32,
    /// Whether this is the final packet of the audio stream (EOF marker).
    pub is_eof: bool,
}
