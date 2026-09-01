// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified Audio Defense Pipeline Orchestrating All 6 Security Layers.
//!
//! Coordinates multi-stage defense-in-depth inspection across raw audio payloads:
//! 1. Container and ID3 Tag Safety Inspection ([`Id3TagSafetyGuard`])
//! 2. Channel Count and Sample Rate Validation ([`AudioChannelRateGuard`])
//! 3. Embedded Cover Art Quota Control ([`CoverArtQuotaGuard`])
//! 4. Memory Watchdog Quota Reservation ([`AudioMemoryBudgetGuard`])
//! 5. Packet / Frame Decoding Circuit Breaker ([`FrameLoopTimeoutGuard`])
//! 6. Zeroize-on-Drop Sensitive Audio Allocation ([`SensitiveAudioBuffer`])

use super::{
    AudioChannelRateGuard, AudioDefenseError, AudioMemoryBudgetGuard, AudioMemoryReservation,
    CoverArtInfo, CoverArtQuotaGuard, FrameLoopTimeoutGuard, FrameLoopTracker, Id3InspectionSummary,
    Id3TagSafetyGuard,
};

/// Comprehensive report produced by the audio defense pipeline after inspecting stream headers.
#[derive(Debug)]
pub struct AudioSecurityReport {
    /// ID3 metadata header summary, if present in the container stream.
    pub id3_summary: Option<Id3InspectionSummary>,
    /// Inspected channel count, if probed from container headers.
    pub channels: Option<u16>,
    /// Inspected sample rate in Hz, if probed from container headers.
    pub sample_rate: Option<u32>,
    /// Number of embedded artwork items registered so far.
    pub cover_art_count: usize,
    /// Active RAII resident memory reservation guard.
    pub memory_reservation: Option<AudioMemoryReservation>,
}

/// Unified 6-Layer Defense-in-Depth pipeline coordinating zero-trust audio stream validation.
#[derive(Debug, Clone, Default)]
pub struct AudioSecurityPipeline {
    /// Channel and sample rate boundary guard.
    pub channel_rate_guard: AudioChannelRateGuard,
    /// Embedded artwork quota guard.
    pub cover_art_guard: CoverArtQuotaGuard,
    /// Frame decode error circuit breaker.
    pub frame_loop_guard: FrameLoopTimeoutGuard,
    /// ID3v2 tag and syncsafe safety guard.
    pub id3_guard: Id3TagSafetyGuard,
    /// Task resident memory watchdog.
    pub memory_watchdog: AudioMemoryBudgetGuard,
}

impl AudioSecurityPipeline {
    /// Creates a customized audio defense pipeline.
    pub fn new(
        channel_rate_guard: AudioChannelRateGuard,
        cover_art_guard: CoverArtQuotaGuard,
        frame_loop_guard: FrameLoopTimeoutGuard,
        id3_guard: Id3TagSafetyGuard,
        memory_watchdog: AudioMemoryBudgetGuard,
    ) -> Self {
        Self {
            channel_rate_guard,
            cover_art_guard,
            frame_loop_guard,
            id3_guard,
            memory_watchdog,
        }
    }

    /// Spawns a stateful frame loop tracker for monitoring stream decode health.
    pub fn create_frame_tracker(&self) -> FrameLoopTracker {
        self.frame_loop_guard.create_tracker()
    }

    /// Reserves memory against the task resident memory budget.
    pub fn reserve_memory(
        &self,
        bytes: usize,
    ) -> Result<AudioMemoryReservation, AudioDefenseError> {
        self.memory_watchdog.reserve(bytes)
    }

    /// Validates channel count and sample rate parameters.
    pub fn validate_channel_rate(
        &self,
        channels: u16,
        sample_rate: u32,
    ) -> Result<(), AudioDefenseError> {
        self.channel_rate_guard.validate(channels, sample_rate)
    }

    /// Inspects and registers an embedded cover art item against safety quotas.
    pub fn inspect_cover_art(&mut self, data: &[u8]) -> Result<CoverArtInfo, AudioDefenseError> {
        self.cover_art_guard.inspect_and_register(data)
    }

    /// Executes multi-stage defense inspection across the stream header.
    pub fn inspect_stream_header(
        &self,
        data: &[u8],
    ) -> Result<AudioSecurityReport, AudioDefenseError> {
        if data.len() < 4 {
            return Err(AudioDefenseError::GeneralDefenseError(
                "Audio stream buffer too small for header inspection".to_string(),
            ));
        }

        // Stage 1: ID3v2 Header & Syncsafe Inspection
        let (id3_summary, audio_offset) = if data.starts_with(b"ID3") {
            let summary = self.id3_guard.inspect_header(data)?;
            let offset = summary.total_tag_size.min(data.len());
            (Some(summary), offset)
        } else {
            (None, 0)
        };

        let stream_slice = &data[audio_offset..];
        let mut channels = None;
        let mut sample_rate = None;

        // Stage 2: Container format specific probing
        if stream_slice.starts_with(b"RIFF") && stream_slice.len() >= 28 && stream_slice.get(8..12) == Some(b"WAVE") {
            // WAV Container
            if let Some((ch, sr)) = Self::probe_wav_header(stream_slice) {
                self.channel_rate_guard.validate(ch, sr)?;
                channels = Some(ch);
                sample_rate = Some(sr);
            }
        } else if stream_slice.starts_with(b"FORM") && stream_slice.len() >= 30 && (stream_slice.get(8..12) == Some(b"AIFF") || stream_slice.get(8..12) == Some(b"AIFC")) {
            // AIFF Container
            if let Some((ch, sr)) = Self::probe_aiff_header(stream_slice) {
                self.channel_rate_guard.validate(ch, sr)?;
                channels = Some(ch);
                sample_rate = Some(sr);
            }
        } else if stream_slice.starts_with(b"fLaC") && stream_slice.len() >= 22 {
            // FLAC Container
            if let Some((ch, sr)) = Self::probe_flac_header(stream_slice) {
                self.channel_rate_guard.validate(ch, sr)?;
                channels = Some(ch);
                sample_rate = Some(sr);
            }
        }

        // Stage 3: Initial minimal memory reservation for stream buffer header
        let initial_reservation = self.memory_watchdog.reserve(data.len().min(64 * 1024))?;

        Ok(AudioSecurityReport {
            id3_summary,
            channels,
            sample_rate,
            cover_art_count: self.cover_art_guard.current_count(),
            memory_reservation: Some(initial_reservation),
        })
    }

    /// Probes WAV PCM format chunk for channels and sample rate.
    fn probe_wav_header(data: &[u8]) -> Option<(u16, u32)> {
        let mut offset = 12;
        while offset + 8 <= data.len() {
            let chunk_id = &data[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;
            offset += 8;

            if chunk_id == b"fmt " && offset + 8 <= data.len() {
                let channels = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
                let sample_rate = u32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]);
                return Some((channels, sample_rate));
            }

            offset = offset.saturating_add(chunk_size);
            if !chunk_size.is_multiple_of(2) {
                offset = offset.saturating_add(1);
            }
        }
        None
    }

    /// Probes AIFF COMM chunk for channels and sample rate.
    fn probe_aiff_header(data: &[u8]) -> Option<(u16, u32)> {
        let mut offset = 12;
        while offset + 8 <= data.len() {
            let chunk_id = &data[offset..offset + 4];
            let chunk_size = u32::from_be_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;
            offset += 8;

            if chunk_id == b"COMM" && offset + 18 <= data.len() {
                let channels = u16::from_be_bytes([data[offset], data[offset + 1]]);
                // AIFF sample rate is 80-bit IEEE 754 float at offset + 8..18
                let exponent = u16::from_be_bytes([data[offset + 8], data[offset + 9]]) as i32 - 16383;
                let mantissa = u64::from_be_bytes([
                    data[offset + 10], data[offset + 11], data[offset + 12], data[offset + 13],
                    data[offset + 14], data[offset + 15], data[offset + 16], data[offset + 17],
                ]);
                let sample_rate = if (0..=31).contains(&exponent) {
                    (mantissa >> (63 - exponent)) as u32
                } else {
                    44_100
                };
                return Some((channels, sample_rate));
            }

            offset = offset.saturating_add(chunk_size);
            if !chunk_size.is_multiple_of(2) {
                offset = offset.saturating_add(1);
            }
        }
        None
    }

    /// Probes FLAC STREAMINFO metadata block (first block after fLaC magic).
    fn probe_flac_header(data: &[u8]) -> Option<(u16, u32)> {
        // STREAMINFO block header starts at offset 4: 1 byte flag/type + 3 bytes length = 4 bytes
        // STREAMINFO payload starts at offset 8
        if data.len() < 26 {
            return None;
        }

        let block_type = data[4] & 0x7F;
        if block_type != 0 {
            return None;
        }

        // STREAMINFO bytes:
        // offset 14..17: sample rate (20 bits), channels - 1 (3 bits), bits per sample - 1 (5 bits)
        let sr_ch_bits = ((data[18] as u32) << 16) | ((data[19] as u32) << 8) | (data[20] as u32);
        let sample_rate = sr_ch_bits >> 4;
        let channels = (((sr_ch_bits >> 1) & 0x07) as u16) + 1;

        Some((channels, sample_rate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_wav_header_inspection() {
        let pipeline = AudioSecurityPipeline::default();

        // Construct minimal valid WAV header: 44.1kHz stereo
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&36u32.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        wav.extend_from_slice(&1u16.to_le_bytes());  // PCM format = 1
        wav.extend_from_slice(&2u16.to_le_bytes());  // channels = 2
        wav.extend_from_slice(&44100u32.to_le_bytes()); // sample rate = 44100
        wav.extend_from_slice(&(44100u32 * 4).to_le_bytes()); // byte rate
        wav.extend_from_slice(&4u16.to_le_bytes());  // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample = 16
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&0u32.to_le_bytes());

        let report = pipeline.inspect_stream_header(&wav).unwrap();
        assert_eq!(report.channels, Some(2));
        assert_eq!(report.sample_rate, Some(44_100));
        assert!(report.id3_summary.is_none());
        assert!(report.memory_reservation.is_some());
    }

    #[test]
    fn test_pipeline_id3_and_wav_compound() {
        let pipeline = AudioSecurityPipeline::default();

        // 1. ID3v2 header
        let mut data = vec![b'I', b'D', b'3', 3, 0, 0];
        let id3_size = Id3TagSafetyGuard::encode_syncsafe_u32(16).unwrap();
        data.extend_from_slice(&id3_size);
        data.extend_from_slice(&[0u8; 16]); // ID3 body

        // 2. Append WAV header
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&36u32.to_le_bytes());
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&48000u32.to_le_bytes());
        data.extend_from_slice(&(48000u32 * 4).to_le_bytes());
        data.extend_from_slice(&4u16.to_le_bytes());
        data.extend_from_slice(&16u16.to_le_bytes());

        let report = pipeline.inspect_stream_header(&data).unwrap();
        assert!(report.id3_summary.is_some());
        assert_eq!(report.channels, Some(2));
        assert_eq!(report.sample_rate, Some(48_000));
    }

    #[test]
    fn test_pipeline_invalid_channel_rejection() {
        let pipeline = AudioSecurityPipeline::default();

        let mut bad_wav = Vec::new();
        bad_wav.extend_from_slice(b"RIFF");
        bad_wav.extend_from_slice(&36u32.to_le_bytes());
        bad_wav.extend_from_slice(b"WAVE");
        bad_wav.extend_from_slice(b"fmt ");
        bad_wav.extend_from_slice(&16u32.to_le_bytes());
        bad_wav.extend_from_slice(&1u16.to_le_bytes());
        bad_wav.extend_from_slice(&0u16.to_le_bytes()); // 0 channels (invalid)
        bad_wav.extend_from_slice(&44100u32.to_le_bytes());
        bad_wav.extend_from_slice(&0u32.to_le_bytes());
        bad_wav.extend_from_slice(&0u16.to_le_bytes());
        bad_wav.extend_from_slice(&16u16.to_le_bytes());

        let err = pipeline.inspect_stream_header(&bad_wav).unwrap_err();
        assert!(matches!(err, AudioDefenseError::InvalidChannelCount { .. }));
    }
}
