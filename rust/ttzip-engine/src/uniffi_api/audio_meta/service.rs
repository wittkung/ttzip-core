// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Service and Pipeline Implementations for Audio Probing, Metadata, Waveforms, and Decoding.

use std::path::Path;
use std::sync::Arc;

use super::decoder::{
    decode_stream_packets_from_bytes, extract_metadata_from_bytes, generate_waveform_from_bytes,
    probe_stream_info_from_bytes, read_file_bytes,
};
use super::types::{
    UniFFIAudioError, UniFFIAudioMetadata, UniFFIAudioPacket, UniFFIAudioStreamInfo,
    UniFFIAudioWaveform,
};

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Probes technical stream parameters from in-memory audio bytes without full decoding.
#[uniffi::export]
pub fn uniffi_probe_audio_bytes(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIAudioStreamInfo, UniFFIAudioError> {
    probe_stream_info_from_bytes(&data, file_name.as_deref())
}

/// Extracts comprehensive metadata tags and embedded cover art from in-memory audio bytes.
#[uniffi::export]
pub fn uniffi_extract_audio_metadata(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIAudioMetadata, UniFFIAudioError> {
    extract_metadata_from_bytes(&data, file_name.as_deref())
}

/// Computes normalized acoustic waveform envelope amplitudes from in-memory audio bytes.
#[uniffi::export]
pub fn uniffi_generate_audio_waveform(
    data: Vec<u8>,
    bucket_count: u32,
    file_name: Option<String>,
) -> Result<UniFFIAudioWaveform, UniFFIAudioError> {
    generate_waveform_from_bytes(&data, bucket_count, file_name.as_deref())
}

/// Decodes audio packets into interleaved float PCM sample chunks for streaming playback.
#[uniffi::export]
pub fn uniffi_decode_audio_stream(
    data: Vec<u8>,
    max_packets: Option<u32>,
    file_name: Option<String>,
) -> Result<Vec<UniFFIAudioPacket>, UniFFIAudioError> {
    decode_stream_packets_from_bytes(&data, max_packets, file_name.as_deref())
}

// ============================================================================
// Stateful UniFFI Service Object
// ============================================================================

/// High-performance Mozilla UniFFI audio engine service exposing probing, metadata, and playback streaming.
#[derive(uniffi::Object, Default)]
pub struct UniFFIAudioService {}

#[uniffi::export]
impl UniFFIAudioService {
    /// Constructs a new thread-safe audio service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Probes technical stream info from an in-memory byte buffer.
    pub fn probe_bytes(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIAudioStreamInfo, UniFFIAudioError> {
        probe_stream_info_from_bytes(&data, file_name.as_deref())
    }

    /// Probes technical stream info from a local audio file on disk.
    pub fn probe_file(&self, file_path: String) -> Result<UniFFIAudioStreamInfo, UniFFIAudioError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        probe_stream_info_from_bytes(&bytes, name.as_deref())
    }

    /// Extracts metadata tags and embedded cover art from an in-memory byte buffer.
    pub fn extract_metadata(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIAudioMetadata, UniFFIAudioError> {
        extract_metadata_from_bytes(&data, file_name.as_deref())
    }

    /// Extracts metadata tags and embedded cover art from a local audio file on disk.
    pub fn extract_metadata_from_file(
        &self,
        file_path: String,
    ) -> Result<UniFFIAudioMetadata, UniFFIAudioError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        extract_metadata_from_bytes(&bytes, name.as_deref())
    }

    /// Generates normalized waveform amplitudes from an in-memory byte buffer.
    pub fn generate_waveform(
        &self,
        data: Vec<u8>,
        bucket_count: u32,
        file_name: Option<String>,
    ) -> Result<UniFFIAudioWaveform, UniFFIAudioError> {
        generate_waveform_from_bytes(&data, bucket_count, file_name.as_deref())
    }

    /// Generates normalized waveform amplitudes from a local audio file on disk.
    pub fn generate_waveform_from_file(
        &self,
        file_path: String,
        bucket_count: u32,
    ) -> Result<UniFFIAudioWaveform, UniFFIAudioError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        generate_waveform_from_bytes(&bytes, bucket_count, name.as_deref())
    }

    /// Decodes chunked PCM sample packets from an in-memory byte buffer.
    pub fn decode_packets(
        &self,
        data: Vec<u8>,
        max_packets: Option<u32>,
        file_name: Option<String>,
    ) -> Result<Vec<UniFFIAudioPacket>, UniFFIAudioError> {
        decode_stream_packets_from_bytes(&data, max_packets, file_name.as_deref())
    }

    /// Decodes chunked PCM sample packets from a local audio file on disk.
    pub fn decode_packets_from_file(
        &self,
        file_path: String,
        max_packets: Option<u32>,
    ) -> Result<Vec<UniFFIAudioPacket>, UniFFIAudioError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        decode_stream_packets_from_bytes(&bytes, max_packets, name.as_deref())
    }
}
