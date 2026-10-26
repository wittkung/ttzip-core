// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Safe Rust audio waveform extraction and acoustic envelope downsampler.

use std::path::Path;

/// Dual-track acoustic waveform representation containing peak and RMS envelopes.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioWaveform {
    pub peaks: Vec<f32>,
    pub rms: Vec<f32>,
    pub points: usize,
    pub duration_seconds: f64,
    pub channels: u32,
    pub sample_rate: u32,
}

/// Generates a pleasant organic fallback waveform when audio decoding fails or payload is empty.
pub fn default_waveform(count: usize) -> Vec<f32> {
    let count = count.clamp(16, 2048);
    (0..count)
        .map(|idx| {
            let progress = idx as f32 / count as f32;
            let h1 = (progress * std::f32::consts::PI * 8.0).sin() * 0.35;
            let h2 = (progress * std::f32::consts::PI * 19.5).sin() * 0.25;
            let env = (progress * std::f32::consts::PI).sin();
            ((0.25 + (h1 + h2).abs()) * env).clamp(0.05, 0.95)
        })
        .collect()
}

/// Extracts normalized peak amplitudes `[0.0 .. 1.0]` across `bucket_count` buckets from memory data.
pub fn extract_waveform_from_bytes(data: &[u8], bucket_count: usize) -> Vec<f32> {
    let count = bucket_count.clamp(16, 2048);
    if data.len() < 44 {
        return default_waveform(count);
    }

    if let Some(samples) = decode_wav_samples(data) {
        if !samples.is_empty() {
            return downsample_peaks(&samples, count);
        }
    }

    default_waveform(count)
}

fn decode_wav_samples(data: &[u8]) -> Option<Vec<f32>> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }

    let mut cursor = 12;
    let mut bits_per_sample = 16u16;
    let mut audio_format = 1u16;
    let mut data_chunk: Option<&[u8]> = None;

    while cursor + 8 <= data.len() {
        let chunk_id = &data[cursor..cursor + 4];
        let chunk_size = u32::from_le_bytes(data[cursor + 4..cursor + 8].try_into().ok()?) as usize;
        cursor += 8;

        if chunk_id == b"fmt " {
            if cursor + 16 <= data.len() && chunk_size >= 16 {
                audio_format = u16::from_le_bytes(data[cursor..cursor + 2].try_into().ok()?);
                bits_per_sample = u16::from_le_bytes(data[cursor + 14..cursor + 16].try_into().ok()?);
            }
        } else if chunk_id == b"data" {
            let end = (cursor + chunk_size).min(data.len());
            data_chunk = Some(&data[cursor..end]);
            break;
        }

        cursor = cursor.checked_add(chunk_size)?;
    }

    let raw = data_chunk?;
    let mut samples = Vec::with_capacity(raw.len() / (bits_per_sample.max(8) as usize / 8));

    match (audio_format, bits_per_sample) {
        (1, 8) => {
            for &b in raw {
                samples.push((b as f32 - 128.0) / 128.0);
            }
        }
        (1, 16) => {
            for chunk in raw.chunks_exact(2) {
                let val = i16::from_le_bytes([chunk[0], chunk[1]]);
                samples.push(val as f32 / 32768.0);
            }
        }
        (1, 24) => {
            for chunk in raw.chunks_exact(3) {
                let val = (chunk[0] as i32) | ((chunk[1] as i32) << 8) | (((chunk[2] as i8) as i32) << 16);
                samples.push(val as f32 / 8388608.0);
            }
        }
        (1, 32) => {
            for chunk in raw.chunks_exact(4) {
                let val = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                samples.push(val as f32 / 2147483648.0);
            }
        }
        (3, 32) => {
            for chunk in raw.chunks_exact(4) {
                let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                samples.push(val);
            }
        }
        _ => return None,
    }

    Some(samples)
}

fn downsample_peaks(samples: &[f32], count: usize) -> Vec<f32> {
    let mut peak_buckets = vec![0.0f32; count];
    let total_samples = samples.len();
    for (i, &s) in samples.iter().enumerate() {
        let b = ((i as f64 / total_samples as f64) * count as f64) as usize;
        let b_idx = b.min(count - 1);
        let abs_s = s.abs();
        if abs_s > peak_buckets[b_idx] {
            peak_buckets[b_idx] = abs_s;
        }
    }

    let mut max_peak = 0.0001f32;
    for &p in &peak_buckets {
        if p > max_peak {
            max_peak = p;
        }
    }

    for p in &mut peak_buckets {
        *p = (*p / max_peak).clamp(0.0, 1.0);
    }

    peak_buckets
}

/// Extracts normalized peak amplitudes `[0.0 .. 1.0]` from an audio file on disk.
pub fn extract_waveform_from_file<P: AsRef<Path>>(
    path: P,
    bucket_count: usize,
) -> std::io::Result<Vec<f32>> {
    let bytes = std::fs::read(path)?;
    Ok(extract_waveform_from_bytes(&bytes, bucket_count))
}
