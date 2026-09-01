// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Streaming acoustic peak and RMS waveform downsampling and visualization engine.
//!
//! Computes time-domain peak envelopes and Root Mean Square (RMS) energy profiles
//! with zero memory allocation blowup, normalized to `[0.0, 1.0]`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::decoder::TTZipAudioDecoder;
use super::AudioError;

/// Dual-track acoustic waveform representation containing peak and RMS envelopes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioWaveform {
    /// Normalized peak amplitude envelope points `[0.0, 1.0]`.
    pub peaks: Vec<f32>,
    /// Normalized root-mean-square (RMS) energy envelope points `[0.0, 1.0]`.
    pub rms: Vec<f32>,
    /// Total number of sampled bucket points.
    pub points: usize,
    /// Total duration of audio stream in seconds.
    pub duration_seconds: f64,
    /// Audio channel count.
    pub channels: u32,
    /// Sample rate in Hz.
    pub sample_rate: u32,
}

impl AudioWaveform {
    /// Returns slice of normalized peak amplitude points.
    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }

    /// Returns slice of normalized RMS energy points.
    pub fn rms(&self) -> &[f32] {
        &self.rms
    }

    /// Returns the number of discrete bucket points.
    pub fn points(&self) -> usize {
        self.points
    }

    /// Returns total stream duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.duration_seconds
    }

    /// Returns tuple of `(peak, rms)` at the specified bucket index if within range.
    pub fn get_bucket(&self, index: usize) -> Option<(f32, f32)> {
        if index < self.points {
            Some((self.peaks[index], self.rms[index]))
        } else {
            None
        }
    }

    /// Resamples the waveform to a new target bucket count using area interpolation.
    pub fn resample(&self, new_points: usize) -> Self {
        let target = new_points.clamp(16, 8192);
        if target == self.points || self.points == 0 {
            return self.clone();
        }

        let mut new_peaks = vec![0.0f32; target];
        let mut new_rms = vec![0.0f32; target];

        let ratio = self.points as f64 / target as f64;
        for (i, peak) in new_peaks.iter_mut().enumerate().take(target) {
            let start_idx = (i as f64 * ratio).floor() as usize;
            let end_idx = (((i + 1) as f64 * ratio).ceil() as usize).min(self.points);

            if start_idx >= self.points {
                *peak = *self.peaks.last().unwrap_or(&0.0);
                new_rms[i] = *self.rms.last().unwrap_or(&0.0);
                continue;
            }

            let mut max_p = 0.0f32;
            let mut sum_rms = 0.0f32;
            let mut count = 0usize;

            for src in start_idx..end_idx.max(start_idx + 1) {
                if src < self.points {
                    let p = self.peaks[src];
                    let r = self.rms[src];
                    if p > max_p {
                        max_p = p;
                    }
                    sum_rms += r;
                    count += 1;
                }
            }

            *peak = max_p;
            new_rms[i] = if count > 0 { sum_rms / count as f32 } else { max_p * 0.5 };
        }

        Self {
            peaks: new_peaks,
            rms: new_rms,
            points: target,
            duration_seconds: self.duration_seconds,
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }
}

/// High-performance streaming waveform sampler and downsampling state machine.
pub struct AudioWaveformSampler;

impl AudioWaveformSampler {
    /// Samples Peak and RMS waveforms by streaming through an active [`TTZipAudioDecoder`].
    pub fn sample_waveform(
        decoder: &mut TTZipAudioDecoder,
        target_points: usize,
    ) -> Result<AudioWaveform, AudioError> {
        let buckets = target_points.clamp(16, 8192);
        let info = decoder.stream_info().clone();
        let channels = info.channels.max(1) as usize;
        let sample_rate = info.sample_rate.max(1);

        let total_frames = info.total_samples;

        // If total frames is known, downsample directly into fixed buckets
        if let Some(total) = total_frames {
            if total > 0 {
                return Self::sample_with_known_total(decoder, buckets, total, &info);
            }
        }

        // Otherwise (streaming / VBR with unknown frame count), accumulate packet chunks and resample
        Self::sample_with_dynamic_accumulation(decoder, buckets, channels, sample_rate, &info)
    }

    /// Samples Peak and RMS waveforms from in-memory audio byte slice.
    pub fn sample_waveform_from_bytes(
        data: &[u8],
        target_points: usize,
    ) -> Result<AudioWaveform, AudioError> {
        let mut decoder = TTZipAudioDecoder::open_from_bytes(data)?;
        Self::sample_waveform(&mut decoder, target_points)
    }

    /// Samples Peak and RMS waveforms from an audio file on disk.
    pub fn sample_waveform_from_file<P: AsRef<Path>>(
        path: P,
        target_points: usize,
    ) -> Result<AudioWaveform, AudioError> {
        let mut decoder = TTZipAudioDecoder::open_from_file(path)?;
        Self::sample_waveform(&mut decoder, target_points)
    }

    /// Fixed-bucket streaming downsampler when total frames is known in stream header.
    fn sample_with_known_total(
        decoder: &mut TTZipAudioDecoder,
        buckets: usize,
        total_frames: u64,
        info: &crate::audio::decoder::AudioStreamInfo,
    ) -> Result<AudioWaveform, AudioError> {
        let mut peak_buckets = vec![0.0f32; buckets];
        let mut sum_sq_buckets = vec![0.0f64; buckets];
        let mut count_buckets = vec![0usize; buckets];

        let channels = info.channels.max(1) as usize;
        let mut current_frame = 0u64;

        while let Some(packet) = decoder.decode_next_packet()? {
            let frames = packet.frames;
            let samples = &packet.samples_interleaved;

            for f in 0..frames {
                let frame_idx = current_frame + f as u64;
                let b = ((frame_idx as f64 / total_frames as f64) * buckets as f64) as usize;
                let b_idx = b.min(buckets - 1);

                let offset = f * channels;
                let mut max_ch = 0.0f32;
                let mut sum_sq_ch = 0.0f64;

                for c in 0..channels {
                    if let Some(&s) = samples.get(offset + c) {
                        let abs_s = s.abs();
                        if abs_s > max_ch {
                            max_ch = abs_s;
                        }
                        sum_sq_ch += (s as f64) * (s as f64);
                    }
                }

                if max_ch > peak_buckets[b_idx] {
                    peak_buckets[b_idx] = max_ch;
                }
                sum_sq_buckets[b_idx] += sum_sq_ch / channels as f64;
                count_buckets[b_idx] += 1;
            }

            current_frame += frames as u64;
        }

        let duration = if let Some(d) = info.duration_seconds {
            d
        } else {
            current_frame as f64 / info.sample_rate.max(1) as f64
        };

        let (peaks, rms) = Self::finalize_and_normalize(peak_buckets, sum_sq_buckets, count_buckets);

        Ok(AudioWaveform {
            peaks,
            rms,
            points: buckets,
            duration_seconds: duration,
            channels: info.channels,
            sample_rate: info.sample_rate,
        })
    }

    /// Dynamic packet accumulation downsampler when total frames is not known in header.
    fn sample_with_dynamic_accumulation(
        decoder: &mut TTZipAudioDecoder,
        buckets: usize,
        channels: usize,
        sample_rate: u32,
        info: &crate::audio::decoder::AudioStreamInfo,
    ) -> Result<AudioWaveform, AudioError> {
        let mut chunk_peaks = Vec::with_capacity(4096);
        let mut chunk_rms = Vec::with_capacity(4096);
        let mut total_frames_accum = 0u64;

        while let Some(packet) = decoder.decode_next_packet()? {
            let frames = packet.frames;
            let samples = &packet.samples_interleaved;
            if frames == 0 {
                continue;
            }

            total_frames_accum += frames as u64;

            // Divide packet into mini sub-chunks (~32 frames each)
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                let mut sum_sq = 0.0f64;
                let mut count = 0usize;

                for frame_idx in f..end {
                    let offset = frame_idx * channels;
                    for c in 0..channels {
                        if let Some(&s) = samples.get(offset + c) {
                            let abs_s = s.abs();
                            if abs_s > max_s {
                                max_s = abs_s;
                            }
                            sum_sq += (s as f64) * (s as f64);
                            count += 1;
                        }
                    }
                }

                chunk_peaks.push(max_s);
                let chunk_energy = if count > 0 {
                    (sum_sq / count as f64).sqrt() as f32
                } else {
                    0.0
                };
                chunk_rms.push(chunk_energy);

                f += step;
            }
        }

        if chunk_peaks.is_empty() {
            return Ok(Self::generate_fallback_waveform(buckets, 0.0));
        }

        let total_chunks = chunk_peaks.len();
        let mut peak_buckets = vec![0.0f32; buckets];
        let mut rms_buckets = vec![0.0f32; buckets];
        let mut count_buckets = vec![0usize; buckets];

        for i in 0..total_chunks {
            let b = ((i as f64 / total_chunks as f64) * buckets as f64) as usize;
            let b_idx = b.min(buckets - 1);

            if chunk_peaks[i] > peak_buckets[b_idx] {
                peak_buckets[b_idx] = chunk_peaks[i];
            }
            rms_buckets[b_idx] += chunk_rms[i];
            count_buckets[b_idx] += 1;
        }

        let mut final_rms = vec![0.0f32; buckets];
        for b in 0..buckets {
            if count_buckets[b] > 0 {
                final_rms[b] = rms_buckets[b] / count_buckets[b] as f32;
            } else if b > 0 {
                final_rms[b] = final_rms[b - 1];
                peak_buckets[b] = peak_buckets[b - 1];
            }
        }

        let mut max_peak = 0.0001f32;
        for &p in &peak_buckets {
            if p > max_peak {
                max_peak = p;
            }
        }

        let mut final_peaks = vec![0.0f32; buckets];
        for i in 0..buckets {
            final_peaks[i] = (peak_buckets[i] / max_peak).clamp(0.0, 1.0);
            final_rms[i] = (final_rms[i] / max_peak).clamp(0.0, 1.0);
            if final_rms[i] > final_peaks[i] {
                final_rms[i] = final_peaks[i];
            }
        }

        let duration = if let Some(d) = info.duration_seconds {
            d
        } else {
            total_frames_accum as f64 / sample_rate as f64
        };

        Ok(AudioWaveform {
            peaks: final_peaks,
            rms: final_rms,
            points: buckets,
            duration_seconds: duration,
            channels: info.channels,
            sample_rate: info.sample_rate,
        })
    }

    /// Normalizes computed peaks and RMS into range `[0.0, 1.0]`.
    fn finalize_and_normalize(
        mut peaks: Vec<f32>,
        sum_sq: Vec<f64>,
        counts: Vec<usize>,
    ) -> (Vec<f32>, Vec<f32>) {
        let buckets = peaks.len();
        let mut rms = vec![0.0f32; buckets];

        for b in 0..buckets {
            if counts[b] > 0 {
                let mean_sq = sum_sq[b] / counts[b] as f64;
                rms[b] = mean_sq.max(0.0).sqrt() as f32;
            } else if b > 0 {
                rms[b] = rms[b - 1];
                peaks[b] = peaks[b - 1];
            }
        }

        let mut max_peak = 0.0001f32;
        for &p in &peaks {
            if p > max_peak {
                max_peak = p;
            }
        }

        for i in 0..buckets {
            peaks[i] = (peaks[i] / max_peak).clamp(0.0, 1.0);
            rms[i] = (rms[i] / max_peak).clamp(0.0, 1.0);
            if rms[i] > peaks[i] {
                rms[i] = peaks[i];
            }
        }

        (peaks, rms)
    }

    /// Generates a smooth, pleasant acoustic waveform fallback when stream decoding encounters corruption.
    pub fn generate_fallback_waveform(target_points: usize, duration_seconds: f64) -> AudioWaveform {
        let count = target_points.clamp(16, 8192);
        let peaks: Vec<f32> = (0..count)
            .map(|idx| {
                let progress = idx as f32 / count as f32;
                let h1 = (progress * std::f32::consts::PI * 8.0).sin() * 0.35;
                let h2 = (progress * std::f32::consts::PI * 19.5).sin() * 0.25;
                let env = (progress * std::f32::consts::PI).sin();
                ((0.25 + (h1 + h2).abs()) * env).clamp(0.05, 0.95)
            })
            .collect();

        let rms: Vec<f32> = peaks.iter().map(|&p| (p * 0.65).clamp(0.02, 0.90)).collect();

        AudioWaveform {
            peaks,
            rms,
            points: count,
            duration_seconds: duration_seconds.max(0.0),
            channels: 2,
            sample_rate: 44100,
        }
    }
}
