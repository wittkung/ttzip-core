// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Pure Safe Rust audio waveform extraction and acoustic envelope downsampler.

use std::io::Cursor;
use std::path::Path;
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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
    if data.len() < 12 {
        return default_waveform(count);
    }

    let source = Box::new(Cursor::new(data.to_vec()));
    let mss = MediaSourceStream::new(source, Default::default());
    let hint = Hint::new();
    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = match symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts) {
        Ok(p) => p,
        Err(_) => return default_waveform(count),
    };

    let mut format = probed.format;
    let track = match format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL) {
        Some(t) => t,
        None => return default_waveform(count),
    };

    let track_id = track.id;
    let dec_opts = DecoderOptions::default();
    let mut decoder = match symphonia::default::get_codecs().make(&track.codec_params, &dec_opts) {
        Ok(d) => d,
        Err(_) => return default_waveform(count),
    };

    let mut samples: Vec<f32> = Vec::new();
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        if let Ok(decoded) = decoder.decode(&packet) {
            match decoded {
                AudioBufferRef::F32(buf) => {
                    for plane in buf.planes().planes() {
                        samples.extend_from_slice(plane);
                    }
                }
                AudioBufferRef::U8(buf) => {
                    for plane in buf.planes().planes() {
                        for &v in *plane {
                            samples.push((v as f32 - 128.0) / 128.0);
                        }
                    }
                }
                AudioBufferRef::U16(buf) => {
                    for plane in buf.planes().planes() {
                        for &v in *plane {
                            samples.push((v as f32 - 32768.0) / 32768.0);
                        }
                    }
                }
                AudioBufferRef::S16(buf) => {
                    for plane in buf.planes().planes() {
                        for &v in *plane {
                            samples.push(v as f32 / 32768.0);
                        }
                    }
                }
                AudioBufferRef::S24(buf) => {
                    for plane in buf.planes().planes() {
                        for &v in *plane {
                            samples.push(v.0 as f32 / 8388608.0);
                        }
                    }
                }
                AudioBufferRef::S32(buf) => {
                    for plane in buf.planes().planes() {
                        for &v in *plane {
                            samples.push(v as f32 / 2147483648.0);
                        }
                    }
                }
                AudioBufferRef::F64(buf) => {
                    for plane in buf.planes().planes() {
                        for &v in *plane {
                            samples.push(v as f32);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if samples.is_empty() {
        return default_waveform(count);
    }

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

    for i in 0..count {
        peak_buckets[i] = (peak_buckets[i] / max_peak).clamp(0.0, 1.0);
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
