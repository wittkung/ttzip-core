// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for Audio Metadata, Cover Art, Waveform Envelopes, and PCM Streaming.
//!
//! Provides zero-disk-I/O acoustic metadata probing, ID3/FLAC/Vorbis tag inspection,
//! time-domain waveform analysis, and chunked PCM sample decoding directly to Swift 6.

pub(crate) mod decoder;
pub(crate) mod service;
pub mod types;

pub use decoder::{
    decode_stream_packets_from_bytes, extract_metadata_from_bytes, generate_waveform_from_bytes,
    probe_stream_info_from_bytes,
};
pub use service::{
    uniffi_decode_audio_stream, uniffi_extract_audio_metadata, uniffi_generate_audio_waveform,
    uniffi_probe_audio_bytes, UniFFIAudioService,
};
pub use types::{
    UniFFIAudioCoverArt, UniFFIAudioError, UniFFIAudioMetadata, UniFFIAudioPacket,
    UniFFIAudioStreamInfo, UniFFIAudioWaveform,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Generates a valid uncompressed 16-bit stereo PCM WAV test fixture in memory.
    fn make_test_wav(sample_rate: u32, num_channels: u16, num_samples_per_chan: usize) -> Vec<u8> {
        let bits_per_sample = 16u16;
        let block_align = num_channels * (bits_per_sample / 8);
        let byte_rate = sample_rate * block_align as u32;
        let data_len = num_samples_per_chan * block_align as usize;
        let riff_len = 36 + data_len;

        let mut wav = Vec::with_capacity(44 + data_len);
        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(riff_len as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");

        // fmt chunk
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // subchunk1 size
        wav.extend_from_slice(&1u16.to_le_bytes());  // audio format (PCM = 1)
        wav.extend_from_slice(&num_channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());

        // data chunk
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data_len as u32).to_le_bytes());

        // Generate synthetic sine wave samples
        for i in 0..num_samples_per_chan {
            let t = i as f32 / sample_rate as f32;
            let val = (t * 440.0 * 2.0 * std::f32::consts::PI).sin();
            let sample = (val * 24000.0) as i16;
            let bytes = sample.to_le_bytes();
            for _ in 0..num_channels {
                wav.extend_from_slice(&bytes);
            }
        }

        wav
    }

    #[test]
    fn test_audio_probe_wav() {
        let wav = make_test_wav(48000, 2, 4800);
        let info = uniffi_probe_audio_bytes(wav, Some("test.wav".to_string())).expect("probe wav");
        assert_eq!(info.sample_rate, 48000);
        assert_eq!(info.channels, 2);
        assert_eq!(info.channel_layout, "stereo");
        assert_eq!(info.bits_per_sample, Some(16));
        assert!(info.duration_seconds > 0.09 && info.duration_seconds < 0.11);
    }

    #[test]
    fn test_audio_metadata_extraction() {
        let wav = make_test_wav(44100, 2, 4410);
        let meta = uniffi_extract_audio_metadata(wav, Some("song.wav".to_string())).expect("extract meta");
        assert_eq!(meta.container_format, "wav");
        assert_eq!(meta.stream_info.sample_rate, 44100);
        assert_eq!(meta.stream_info.channels, 2);
        assert!(meta.file_size_bytes > 0);
    }

    #[test]
    fn test_audio_waveform_generation() {
        let wav = make_test_wav(44100, 2, 8820);
        let waveform = uniffi_generate_audio_waveform(wav, 64, Some("track.wav".to_string())).expect("waveform");
        assert_eq!(waveform.bucket_count, 64);
        assert_eq!(waveform.amplitudes.len(), 64);
        assert_eq!(waveform.rms_amplitudes.len(), 64);
        assert_eq!(waveform.sample_rate, 44100);
        assert_eq!(waveform.channels, 2);

        for &amp in &waveform.amplitudes {
            assert!((0.0..=1.0).contains(&amp));
        }
    }

    #[test]
    fn test_audio_stream_decoding() {
        let wav = make_test_wav(44100, 2, 4410);
        let packets = uniffi_decode_audio_stream(wav, Some(5), Some("track.wav".to_string())).expect("decode stream");
        assert!(!packets.is_empty());
        let first = &packets[0];
        assert_eq!(first.channels, 2);
        assert_eq!(first.sample_rate, 44100);
        assert!(!first.pcm_f32_samples.is_empty());
        assert_eq!(first.pcm_f32_samples.len(), first.frame_count as usize * 2);
    }

    #[test]
    fn test_uniffi_audio_service_facade() {
        let service = UniFFIAudioService::new();
        let wav = make_test_wav(48000, 1, 2400);

        let probe = service.probe_bytes(wav.clone(), Some("mono.wav".to_string())).expect("probe");
        assert_eq!(probe.sample_rate, 48000);
        assert_eq!(probe.channels, 1);
        assert_eq!(probe.channel_layout, "mono");

        let meta = service.extract_metadata(wav.clone(), Some("mono.wav".to_string())).expect("meta");
        assert_eq!(meta.container_format, "wav");

        let waveform = service.generate_waveform(wav.clone(), 32, Some("mono.wav".to_string())).expect("waveform");
        assert_eq!(waveform.amplitudes.len(), 32);

        let packets = service.decode_packets(wav, Some(10), Some("mono.wav".to_string())).expect("packets");
        assert!(!packets.is_empty());
    }

    #[test]
    fn test_audio_error_handling() {
        let empty_data = Vec::new();
        let err = uniffi_probe_audio_bytes(empty_data, None);
        assert!(err.is_err());

        let invalid_data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        let err = uniffi_probe_audio_bytes(invalid_data, None);
        assert!(err.is_err());
    }
}
