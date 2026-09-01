// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit test suite for audio decoding, metadata extraction, PCM conversions, and waveform sampling.

use super::*;
use std::f32::consts::PI;

/// Synthesizes a valid RIFF/WAVE PCM 16-bit mono or multi-channel byte stream with an optional sine envelope.
fn create_mock_wav_pcm16(
    sample_count: usize,
    channels: u16,
    sample_rate: u32,
    duration_envelope: bool,
) -> Vec<u8> {
    let bits_per_sample = 16u16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = (sample_count * channels as usize * 2) as u32;

    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + data_size).to_le_bytes());
    data.extend_from_slice(b"WAVE");

    // fmt subchunk
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&channels.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&byte_rate.to_le_bytes());
    data.extend_from_slice(&block_align.to_le_bytes());
    data.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_size.to_le_bytes());

    for i in 0..sample_count {
        let t = i as f32 / sample_count as f32;
        let env = if duration_envelope {
            (PI * t).sin()
        } else {
            1.0
        };
        for ch in 0..channels {
            let freq = 440.0 + (ch as f32 * 110.0);
            let sample = ((t * freq * 2.0 * PI).sin() * env * 30000.0) as i16;
            data.extend_from_slice(&sample.to_le_bytes());
        }
    }

    data
}

/// Synthesizes a valid RIFF/WAVE PCM 24-bit stereo stream.
fn create_mock_wav_pcm24(sample_count: usize, channels: u16, sample_rate: u32) -> Vec<u8> {
    let bits_per_sample = 24u16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = (sample_count * channels as usize * 3) as u32;

    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + data_size).to_le_bytes());
    data.extend_from_slice(b"WAVE");

    // fmt subchunk
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&channels.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&byte_rate.to_le_bytes());
    data.extend_from_slice(&block_align.to_le_bytes());
    data.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_size.to_le_bytes());

    for i in 0..sample_count {
        let t = i as f32 / sample_count as f32;
        let sample = ((t * 440.0 * 2.0 * PI).sin() * 8_000_000.0) as i32;
        let bytes = sample.to_le_bytes();
        for _ in 0..channels {
            data.push(bytes[0]);
            data.push(bytes[1]);
            data.push(bytes[2]);
        }
    }

    data
}

/// Synthesizes a valid RIFF/WAVE IEEE 32-bit float stereo stream.
fn create_mock_wav_pcm32f(sample_count: usize, channels: u16, sample_rate: u32) -> Vec<u8> {
    let bits_per_sample = 32u16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = (sample_count * channels as usize * 4) as u32;

    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + data_size).to_le_bytes());
    data.extend_from_slice(b"WAVE");

    // fmt subchunk
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&3u16.to_le_bytes()); // IEEE Float
    data.extend_from_slice(&channels.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&byte_rate.to_le_bytes());
    data.extend_from_slice(&block_align.to_le_bytes());
    data.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_size.to_le_bytes());

    for i in 0..sample_count {
        let t = i as f32 / sample_count as f32;
        let sample = (t * 440.0 * 2.0 * PI).sin() * 0.85;
        for _ in 0..channels {
            data.extend_from_slice(&sample.to_le_bytes());
        }
    }

    data
}

/// Synthesizes a WAV file with RIFF `LIST INFO` metadata tags.
fn create_mock_wav_with_info(
    sample_count: usize,
    title: &str,
    artist: &str,
    album: &str,
    year: &str,
) -> Vec<u8> {
    let channels = 1u16;
    let sample_rate = 44100u32;
    let bits_per_sample = 16u16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = (sample_count * channels as usize * 2) as u32;

    // Build LIST INFO chunk
    let mut info_chunk = Vec::new();
    info_chunk.extend_from_slice(b"INFO");

    let tags = [
        (b"INAM", title),
        (b"IART", artist),
        (b"IPRD", album),
        (b"ICRD", year),
    ];

    for (fourcc, val) in tags {
        let val_bytes = val.as_bytes();
        let len = val_bytes.len() as u32 + 1; // including null terminator
        info_chunk.extend_from_slice(fourcc);
        info_chunk.extend_from_slice(&len.to_le_bytes());
        info_chunk.extend_from_slice(val_bytes);
        info_chunk.push(0); // null terminator
        if !len.is_multiple_of(2) {
            info_chunk.push(0); // pad byte
        }
    }

    let mut list_chunk = Vec::new();
    list_chunk.extend_from_slice(b"LIST");
    list_chunk.extend_from_slice(&(info_chunk.len() as u32).to_le_bytes());
    list_chunk.extend_from_slice(&info_chunk);

    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    let total_riff_size = 4 + (8 + 16) + (8 + list_chunk.len() as u32) + (8 + data_size);
    data.extend_from_slice(&total_riff_size.to_le_bytes());
    data.extend_from_slice(b"WAVE");

    // fmt chunk
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&channels.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&byte_rate.to_le_bytes());
    data.extend_from_slice(&block_align.to_le_bytes());
    data.extend_from_slice(&bits_per_sample.to_le_bytes());

    // list chunk (before data)
    data.extend_from_slice(&list_chunk);

    // data chunk
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_size.to_le_bytes());

    for i in 0..sample_count {
        let t = i as f32 / sample_count as f32;
        let sample = ((t * 440.0 * 2.0 * PI).sin() * 30000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

#[test]
fn test_pcm_converter_interleaved_planar_roundtrip() {
    let interleaved = vec![1.0, -1.0, 0.5, -0.5, 0.25, -0.25];
    let planar = AudioPcmConverter::interleaved_to_planar_f32(&interleaved, 2);
    assert_eq!(planar.len(), 2);
    assert_eq!(planar[0], vec![1.0, 0.5, 0.25]);
    assert_eq!(planar[1], vec![-1.0, -0.5, -0.25]);

    let reconstructed = AudioPcmConverter::planar_to_interleaved_f32(&planar);
    assert_eq!(interleaved, reconstructed);
}

#[test]
fn test_pcm_converter_f32_i16_i32_roundtrip() {
    let original = vec![0.0, 0.5, -0.5, 1.0, -1.0];
    let i16_samples = AudioPcmConverter::f32_to_i16(&original);
    assert_eq!(i16_samples[0], 0);
    assert_eq!(i16_samples[3], 32767);
    assert_eq!(i16_samples[4], -32768);

    let restored_f32 = AudioPcmConverter::i16_to_f32(&i16_samples);
    for (orig, rest) in original.iter().zip(restored_f32.iter()) {
        assert!((orig - rest).abs() < 0.001);
    }

    let i32_samples = AudioPcmConverter::f32_to_i32(&original);
    assert_eq!(i32_samples[0], 0);
    assert_eq!(i32_samples[3], 2_147_483_647);
    assert_eq!(i32_samples[4], -2_147_483_648);

    let restored_f32_32 = AudioPcmConverter::i32_to_f32(&i32_samples);
    for (orig, rest) in original.iter().zip(restored_f32_32.iter()) {
        assert!((orig - rest).abs() < 0.00001);
    }
}

#[test]
fn test_pcm_converter_i24_packed() {
    let original = vec![0.0, 0.5, -0.5, 0.9999, -1.0];
    let packed = AudioPcmConverter::f32_to_i24_packed(&original);
    assert_eq!(packed.len(), original.len() * 3);

    let restored = AudioPcmConverter::i24_packed_to_f32(&packed);
    assert_eq!(restored.len(), original.len());
    for (orig, rest) in original.iter().zip(restored.iter()) {
        assert!((orig - rest).abs() < 0.0001);
    }
}

#[test]
fn test_pcm_converter_downmix_to_mono_and_stereo() {
    let stereo = vec![1.0, -1.0, 0.6, 0.4];
    let mono = AudioPcmConverter::downmix_to_mono(&stereo, 2);
    assert_eq!(mono.len(), 2);
    assert_eq!(mono[0], 0.0);
    assert_eq!(mono[1], 0.5);

    let upmixed_stereo = AudioPcmConverter::downmix_to_stereo(&mono, 1);
    assert_eq!(upmixed_stereo.len(), 4);
    assert_eq!(upmixed_stereo, vec![0.0, 0.0, 0.5, 0.5]);
}

#[test]
fn test_pcm_converter_5_1_surround_downmix() {
    // 5.1 frame: L=1.0, R=1.0, C=0.0, LFE=0.0, Ls=0.0, Rs=0.0
    let surround = vec![1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let stereo = AudioPcmConverter::downmix_to_stereo(&surround, 6);
    assert_eq!(stereo.len(), 2);
    assert!(stereo[0] > 0.0 && stereo[0] <= 1.0);
    assert_eq!(stereo[0], stereo[1]);

    let remixed = AudioPcmConverter::remix_channels(&surround, 6, 2);
    assert_eq!(remixed, stereo);
}

#[test]
fn test_audio_decoder_wav_pcm16_streaming() {
    let wav_bytes = create_mock_wav_pcm16(44100, 2, 44100, true);
    let mut decoder = TTZipAudioDecoder::open_from_bytes(&wav_bytes).expect("Failed to open WAV decoder");

    let info = decoder.stream_info();
    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.channels, 2);
    assert_eq!(info.bits_per_sample, Some(16));

    let mut total_decoded_frames = 0;
    while let Some(packet) = decoder.decode_next_packet().expect("Decode error") {
        assert_eq!(packet.channels, 2);
        assert_eq!(packet.sample_rate, 44100);
        assert_eq!(packet.samples_interleaved.len(), packet.frames * 2);
        total_decoded_frames += packet.frames;
    }

    assert_eq!(total_decoded_frames, 44100);
}

#[test]
fn test_audio_decoder_wav_pcm24_and_32f_streaming() {
    let wav24 = create_mock_wav_pcm24(22050, 2, 44100);
    let mut dec24 = TTZipAudioDecoder::open_from_bytes(&wav24).expect("Failed to open 24-bit WAV");
    assert_eq!(dec24.stream_info().bits_per_sample, Some(24));
    let mut f24 = 0;
    while let Some(pkt) = dec24.decode_next_packet().unwrap() {
        f24 += pkt.frames;
    }
    assert_eq!(f24, 22050);

    let wav32f = create_mock_wav_pcm32f(22050, 2, 44100);
    let mut dec32f = TTZipAudioDecoder::open_from_bytes(&wav32f).expect("Failed to open 32f WAV");
    let mut f32 = 0;
    while let Some(pkt) = dec32f.decode_next_packet().unwrap() {
        f32 += pkt.frames;
    }
    assert_eq!(f32, 22050);
}

#[test]
fn test_audio_decoder_seeking_and_reset() {
    let wav_bytes = create_mock_wav_pcm16(44100 * 2, 1, 44100, false); // 2.0s
    let mut decoder = TTZipAudioDecoder::open_from_bytes(&wav_bytes).unwrap();

    let seeked_ts = decoder.seek(1.0).expect("Seek failed");
    assert!((seeked_ts - 1.0).abs() < 0.05);

    if let Some(pkt) = decoder.decode_next_packet().unwrap() {
        assert!(pkt.timestamp_seconds >= 0.95);
    }

    decoder.reset().expect("Reset failed");
    assert_eq!(decoder.current_frame(), 0);
}

#[test]
fn test_audio_metadata_extractor_wav_info() {
    let wav_bytes = create_mock_wav_with_info(
        22050,
        "TTZip Symphony",
        "Witt Kung",
        "Microkernel Audio",
        "2026",
    );

    let meta = AudioMetadataExtractor::extract_from_bytes(&wav_bytes).expect("Failed to extract metadata");
    assert_eq!(meta.title.as_deref(), Some("TTZip Symphony"));
    assert_eq!(meta.artist.as_deref(), Some("Witt Kung"));
    assert_eq!(meta.album.as_deref(), Some("Microkernel Audio"));
    assert_eq!(meta.year, Some(2026));
    assert_eq!(meta.sample_rate, Some(44100));
}

#[test]
fn test_audio_waveform_sampler_peaks_and_rms() {
    let wav_data = create_mock_wav_pcm16(44100, 1, 44100, true);
    let waveform = AudioWaveformSampler::sample_waveform_from_bytes(&wav_data, 32).expect("Waveform sampling failed");

    assert_eq!(waveform.points(), 32);
    assert_eq!(waveform.peaks().len(), 32);
    assert_eq!(waveform.rms().len(), 32);

    for (idx, (&p, &r)) in waveform.peaks().iter().zip(waveform.rms().iter()).enumerate() {
        assert!((0.0..=1.0).contains(&p), "Peak out of range at {}: {}", idx, p);
        assert!((0.0..=1.0).contains(&r), "RMS out of range at {}: {}", idx, r);
        assert!(r <= p + 0.0001, "RMS ({}) must be <= Peak ({}) at {}", r, p, idx);
    }

    // Sine envelope test: peak in middle > peak at start
    assert!(waveform.peaks()[16] > waveform.peaks()[1]);
}

#[test]
fn test_audio_waveform_resample() {
    let wav_data = create_mock_wav_pcm16(22050, 1, 44100, true);
    let waveform = AudioWaveformSampler::sample_waveform_from_bytes(&wav_data, 64).unwrap();
    assert_eq!(waveform.points(), 64);

    let downsampled = waveform.resample(16);
    assert_eq!(downsampled.points(), 16);
    assert_eq!(downsampled.peaks().len(), 16);

    let upsampled = waveform.resample(128);
    assert_eq!(upsampled.points(), 128);
    assert_eq!(upsampled.peaks().len(), 128);
}

#[test]
fn test_legacy_convenience_functions() {
    let wav_data = create_mock_wav_pcm16(22050, 1, 44100, false);
    let wf = extract_waveform_from_bytes(&wav_data, 24);
    assert_eq!(wf.len(), 24);
    for &val in &wf {
        assert!((0.0..=1.0).contains(&val));
    }

    let corrupt = b"not an audio format at all";
    let fallback = extract_waveform_from_bytes(corrupt, 24);
    assert_eq!(fallback.len(), 24);
}
