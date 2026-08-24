// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;

fn create_mock_wav_pcm16(sample_count: usize, duration_envelope: bool) -> Vec<u8> {
    let num_channels = 1u16;
    let sample_rate = 44100u32;
    let bits_per_sample = 16u16;
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = num_channels * (bits_per_sample / 8);
    let data_size = (sample_count * num_channels as usize * 2) as u32;

    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + data_size).to_le_bytes());
    data.extend_from_slice(b"WAVE");

    // fmt subchunk
    data.extend_from_slice(b"fmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&num_channels.to_le_bytes());
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
            (std::f32::consts::PI * t).sin()
        } else {
            1.0
        };
        let sample = ((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * env * 30000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

#[test]
fn test_wav_pcm16_waveform_extraction() {
    let wav_data = create_mock_wav_pcm16(44100, true);
    let waveform = extract_waveform_from_bytes(&wav_data, 32);

    assert_eq!(waveform.len(), 32);
    for (idx, &val) in waveform.iter().enumerate() {
        assert!(
            val >= 0.0 && val <= 1.0,
            "Bucket {} out of range: {}",
            idx,
            val
        );
    }

    // Envelope in the middle should be higher than the edge
    let mid = waveform[16];
    let edge = waveform[1];
    assert!(
        mid > edge,
        "Center peak ({}) should be higher than start peak ({})",
        mid,
        edge
    );
}

#[test]
fn test_default_waveform_fallback() {
    let corrupt = b"not an audio stream at all";
    let waveform = extract_waveform_from_bytes(corrupt, 24);
    assert!(waveform.is_empty());

    let def = default_waveform(24);
    assert_eq!(def.len(), 24);
    for &val in def.iter() {
        assert!(val >= 0.04 && val <= 1.0);
    }
}
