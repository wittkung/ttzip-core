// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust high-throughput audio decoding, metadata extraction, and waveform sampling microkernel.
//!
//! Provides multi-format streaming decoders, tag/artwork extractors, PCM format converters,
//! and dual-track (Peak + RMS) acoustic waveform generators across MP3, FLAC, WAV, AAC, ALAC,
//! OGG Vorbis, and Opus.

use std::path::Path;
use thiserror::Error;

pub mod decoder;
pub mod metadata;
pub mod pcm;
pub mod waveform;

pub use decoder::{AudioStreamInfo, DecodedAudioPacket, TTZipAudioDecoder};
pub use metadata::{AudioCoverArt, AudioMetadata, AudioMetadataExtractor, AudioPictureType};
pub use pcm::AudioPcmConverter;
pub use waveform::{AudioWaveform, AudioWaveformSampler};

#[cfg(test)]
mod tests;

/// Unified error types encountered during audio decoding, metadata extraction, or waveform sampling.
#[derive(Debug, Error)]
pub enum AudioError {
    /// Underlying I/O error when reading audio source.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Audio container format or framing parsing error.
    #[error("Audio format error: {0}")]
    Format(String),

    /// Codec initialization or packet decoding failure.
    #[error("Audio codec error: {0}")]
    Codec(String),

    /// Unsupported audio container format.
    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    /// Unsupported audio codec.
    #[error("Unsupported audio codec: {0}")]
    UnsupportedCodec(String),

    /// Corrupted audio stream or packet payload.
    #[error("Corrupt audio stream: {0}")]
    CorruptStream(String),

    /// Invalid parameter passed to audio API.
    #[error("Invalid audio parameter: {0}")]
    InvalidParameter(String),

    /// Audio stream seek operation failure.
    #[error("Audio seek error: {0}")]
    SeekError(String),

    /// Audio stream reached end of file / stream.
    #[error("End of audio stream")]
    EndOfStream,
}

/// Legacy/Convenience function: Extracts normalized peak amplitudes `[0.0 .. 1.0]` across `bucket_count` buckets.
pub fn extract_waveform_from_bytes(data: &[u8], bucket_count: usize) -> Vec<f32> {
    let buckets = bucket_count.clamp(16, 2048);
    if data.len() < 12 {
        return default_waveform(buckets);
    }
    match AudioWaveformSampler::sample_waveform_from_bytes(data, buckets) {
        Ok(wf) => wf.peaks,
        Err(_) => default_waveform(buckets),
    }
}

/// Legacy/Convenience function: Extracts normalized peak amplitudes `[0.0 .. 1.0]` from an audio file on disk.
pub fn extract_waveform_from_file<P: AsRef<Path>>(
    path: P,
    bucket_count: usize,
) -> std::io::Result<Vec<f32>> {
    let buckets = bucket_count.clamp(16, 2048);
    let p = path.as_ref();
    match AudioWaveformSampler::sample_waveform_from_file(p, buckets) {
        Ok(wf) => Ok(wf.peaks),
        Err(AudioError::Io(e)) => Err(e),
        Err(_) => {
            let bytes = std::fs::read(p)?;
            Ok(extract_waveform_from_bytes(&bytes, buckets))
        }
    }
}

/// Generates a pleasant organic fallback waveform when audio decoding fails.
pub fn default_waveform(count: usize) -> Vec<f32> {
    AudioWaveformSampler::generate_fallback_waveform(count, 0.0).peaks
}
