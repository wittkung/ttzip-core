// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PCM audio sample conversion, channel layout transformation, and multi-channel remixing.
//!
//! Provides zero-allocation hot loops and robust sample normalization across
//! interleaved/planar representations, bit-depth transformations (8, 16, 24, 32-bit, f32, f64),
//! and surround downmixing (e.g. 5.1 / 7.1 to stereo / mono).

use symphonia::core::audio::{AudioBufferRef, Signal};

/// High-performance converter for PCM sample formats, layouts, and channel downmixing/upmixing.
pub struct AudioPcmConverter;

impl AudioPcmConverter {
    /// Converts any Symphonia [`AudioBufferRef`] into normalized interleaved `f32` samples in range `[-1.0, 1.0]`.
    pub fn convert_buffer_ref_to_interleaved_f32(buf_ref: &AudioBufferRef) -> Vec<f32> {
        match buf_ref {
            AudioBufferRef::F32(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push(buf.chan(c)[f]);
                    }
                }
                out
            }
            AudioBufferRef::F64(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push(buf.chan(c)[f] as f32);
                    }
                }
                out
            }
            AudioBufferRef::S16(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push(buf.chan(c)[f] as f32 / 32768.0);
                    }
                }
                out
            }
            AudioBufferRef::S24(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push(buf.chan(c)[f].0 as f32 / 8_388_608.0);
                    }
                }
                out
            }
            AudioBufferRef::S32(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push((buf.chan(c)[f] as f64 / 2_147_483_648.0) as f32);
                    }
                }
                out
            }
            AudioBufferRef::S8(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push(buf.chan(c)[f] as f32 / 128.0);
                    }
                }
                out
            }
            AudioBufferRef::U8(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push((buf.chan(c)[f] as f32 - 128.0) / 128.0);
                    }
                }
                out
            }
            AudioBufferRef::U16(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push((buf.chan(c)[f] as f32 - 32768.0) / 32768.0);
                    }
                }
                out
            }
            AudioBufferRef::U24(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push((buf.chan(c)[f].0 as f32 - 8_388_608.0) / 8_388_608.0);
                    }
                }
                out
            }
            AudioBufferRef::U32(buf) => {
                let frames = buf.frames();
                let channels = buf.spec().channels.count();
                let mut out = Vec::with_capacity(frames * channels);
                for f in 0..frames {
                    for c in 0..channels {
                        out.push(((buf.chan(c)[f] as f64 - 2_147_483_648.0) / 2_147_483_648.0) as f32);
                    }
                }
                out
            }
        }
    }

    /// Converts any Symphonia [`AudioBufferRef`] into normalized planar `Vec<Vec<f32>>` (channel-separated).
    pub fn convert_buffer_ref_to_planar_f32(buf_ref: &AudioBufferRef) -> Vec<Vec<f32>> {
        let channels = match buf_ref {
            AudioBufferRef::F32(b) => b.spec().channels.count(),
            AudioBufferRef::F64(b) => b.spec().channels.count(),
            AudioBufferRef::S16(b) => b.spec().channels.count(),
            AudioBufferRef::S24(b) => b.spec().channels.count(),
            AudioBufferRef::S32(b) => b.spec().channels.count(),
            AudioBufferRef::S8(b) => b.spec().channels.count(),
            AudioBufferRef::U8(b) => b.spec().channels.count(),
            AudioBufferRef::U16(b) => b.spec().channels.count(),
            AudioBufferRef::U24(b) => b.spec().channels.count(),
            AudioBufferRef::U32(b) => b.spec().channels.count(),
        };

        let frames = match buf_ref {
            AudioBufferRef::F32(b) => b.frames(),
            AudioBufferRef::F64(b) => b.frames(),
            AudioBufferRef::S16(b) => b.frames(),
            AudioBufferRef::S24(b) => b.frames(),
            AudioBufferRef::S32(b) => b.frames(),
            AudioBufferRef::S8(b) => b.frames(),
            AudioBufferRef::U8(b) => b.frames(),
            AudioBufferRef::U16(b) => b.frames(),
            AudioBufferRef::U24(b) => b.frames(),
            AudioBufferRef::U32(b) => b.frames(),
        };

        let mut planar = vec![Vec::with_capacity(frames); channels];
        for (c, plane) in planar.iter_mut().enumerate().take(channels) {
            match buf_ref {
                AudioBufferRef::F32(b) => plane.extend_from_slice(b.chan(c)),
                AudioBufferRef::F64(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push(s as f32);
                    }
                }
                AudioBufferRef::S16(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push(s as f32 / 32768.0);
                    }
                }
                AudioBufferRef::S24(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push(s.0 as f32 / 8_388_608.0);
                    }
                }
                AudioBufferRef::S32(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push((s as f64 / 2_147_483_648.0) as f32);
                    }
                }
                AudioBufferRef::S8(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push(s as f32 / 128.0);
                    }
                }
                AudioBufferRef::U8(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push((s as f32 - 128.0) / 128.0);
                    }
                }
                AudioBufferRef::U16(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push((s as f32 - 32768.0) / 32768.0);
                    }
                }
                AudioBufferRef::U24(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push((s.0 as f32 - 8_388_608.0) / 8_388_608.0);
                    }
                }
                AudioBufferRef::U32(b) => {
                    let chan = b.chan(c);
                    for &s in chan {
                        plane.push(((s as f64 - 2_147_483_648.0) / 2_147_483_648.0) as f32);
                    }
                }
            }
        }
        planar
    }

    /// Converts interleaved `f32` buffer to planar `Vec<Vec<f32>>`.
    pub fn interleaved_to_planar_f32(interleaved: &[f32], channels: usize) -> Vec<Vec<f32>> {
        if channels == 0 || interleaved.is_empty() {
            return Vec::new();
        }
        let frames = interleaved.len() / channels;
        let mut planar = vec![Vec::with_capacity(frames); channels];
        for frame in 0..frames {
            let offset = frame * channels;
            for ch in 0..channels {
                planar[ch].push(interleaved[offset + ch]);
            }
        }
        planar
    }

    /// Converts planar `&[Vec<f32>]` to interleaved `Vec<f32>`.
    pub fn planar_to_interleaved_f32(planar: &[Vec<f32>]) -> Vec<f32> {
        if planar.is_empty() || planar[0].is_empty() {
            return Vec::new();
        }
        let channels = planar.len();
        let frames = planar[0].len();
        let mut interleaved = Vec::with_capacity(frames * channels);
        for frame in 0..frames {
            for ch in 0..channels {
                let sample = planar[ch].get(frame).copied().unwrap_or(0.0);
                interleaved.push(sample);
            }
        }
        interleaved
    }

    /// Converts normalized `f32` samples in `[-1.0, 1.0]` to signed 16-bit integers (`i16`).
    pub fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                if clamped >= 0.0 {
                    (clamped * 32767.0).round() as i16
                } else {
                    (clamped * 32768.0).round() as i16
                }
            })
            .collect()
    }

    /// Converts signed 16-bit integers (`i16`) to normalized `f32` in `[-1.0, 1.0]`.
    pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
        samples.iter().map(|&s| s as f32 / 32768.0).collect()
    }

    /// Converts normalized `f32` samples in `[-1.0, 1.0]` to signed 32-bit integers (`i32`).
    pub fn f32_to_i32(samples: &[f32]) -> Vec<i32> {
        samples
            .iter()
            .map(|&s| {
                let clamped = (s as f64).clamp(-1.0, 1.0);
                if clamped >= 0.0 {
                    (clamped * 2_147_483_647.0).round() as i32
                } else {
                    (clamped * 2_147_483_648.0).round() as i32
                }
            })
            .collect()
    }

    /// Converts signed 32-bit integers (`i32`) to normalized `f32` in `[-1.0, 1.0]`.
    pub fn i32_to_f32(samples: &[i32]) -> Vec<f32> {
        samples
            .iter()
            .map(|&s| (s as f64 / 2_147_483_648.0) as f32)
            .collect()
    }

    /// Converts normalized `f32` samples in `[-1.0, 1.0]` to 3-byte packed 24-bit little-endian PCM bytes.
    pub fn f32_to_i24_packed(samples: &[f32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(samples.len() * 3);
        for &s in samples {
            let clamped = (s as f64).clamp(-1.0, 1.0);
            let val = if clamped >= 0.0 {
                (clamped * 8_388_607.0).round() as i32
            } else {
                (clamped * 8_388_608.0).round() as i32
            };
            let bytes = val.to_le_bytes();
            out.push(bytes[0]);
            out.push(bytes[1]);
            out.push(bytes[2]);
        }
        out
    }

    /// Converts 3-byte packed 24-bit little-endian PCM bytes to normalized `f32` in `[-1.0, 1.0]`.
    pub fn i24_packed_to_f32(bytes: &[u8]) -> Vec<f32> {
        let sample_count = bytes.len() / 3;
        let mut out = Vec::with_capacity(sample_count);
        for chunk in bytes.chunks_exact(3) {
            let b0 = chunk[0] as u32;
            let b1 = chunk[1] as u32;
            let b2 = chunk[2] as u32;
            let mut raw = (b0 | (b1 << 8) | (b2 << 16)) as i32;
            if raw & 0x0080_0000 != 0 {
                raw |= !0x00FF_FFFF;
            }
            out.push(raw as f32 / 8_388_608.0);
        }
        out
    }

    /// Downmixes an interleaved multi-channel `f32` audio stream to single-channel (Mono).
    pub fn downmix_to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
        if channels == 0 || interleaved.is_empty() {
            return Vec::new();
        }
        if channels == 1 {
            return interleaved.to_vec();
        }

        let frames = interleaved.len() / channels;
        let mut mono = Vec::with_capacity(frames);

        match channels {
            2 => {
                // Standard stereo to mono: 0.5 * Left + 0.5 * Right
                for chunk in interleaved.chunks_exact(2) {
                    mono.push((chunk[0] + chunk[1]) * 0.5);
                }
            }
            6 => {
                // 5.1 Surround (L, R, C, LFE, Ls, Rs) ITU-R BS.775 downmix
                // Mono = 0.3205 * (L + R) + 0.2265 * (C + Ls + Rs)
                for chunk in interleaved.chunks_exact(6) {
                    let l = chunk[0];
                    let r = chunk[1];
                    let c = chunk[2];
                    let ls = chunk[4];
                    let rs = chunk[5];
                    let mixed = 0.3205 * (l + r) + 0.2265 * (c + ls + rs);
                    mono.push(mixed.clamp(-1.0, 1.0));
                }
            }
            _ => {
                // Generic N-channel downmix: simple arithmetic average
                let inv_ch = 1.0 / channels as f32;
                for chunk in interleaved.chunks_exact(channels) {
                    let sum: f32 = chunk.iter().sum();
                    mono.push(sum * inv_ch);
                }
            }
        }

        mono
    }

    /// Downmixes / upmixes an interleaved multi-channel `f32` audio stream to 2-channel (Stereo).
    pub fn downmix_to_stereo(interleaved: &[f32], channels: usize) -> Vec<f32> {
        if channels == 0 || interleaved.is_empty() {
            return Vec::new();
        }
        if channels == 2 {
            return interleaved.to_vec();
        }

        let frames = interleaved.len() / channels;
        let mut stereo = Vec::with_capacity(frames * 2);

        match channels {
            1 => {
                // Mono upmix: duplicate mono channel to Left and Right
                for &sample in interleaved {
                    stereo.push(sample);
                    stereo.push(sample);
                }
            }
            6 => {
                // 5.1 Surround (L, R, C, LFE, Ls, Rs) ITU-R BS.775 stereo downmix
                // Out_L = (L + 0.7071 * C + 0.7071 * Ls) / (1.0 + 2.0 * 0.7071)
                // Out_R = (R + 0.7071 * C + 0.7071 * Rs) / (1.0 + 2.0 * 0.7071)
                let norm_factor = 1.0 / (1.0 + std::f32::consts::FRAC_1_SQRT_2 * 2.0);
                let sqrt2_inv = std::f32::consts::FRAC_1_SQRT_2;
                for chunk in interleaved.chunks_exact(6) {
                    let l = chunk[0];
                    let r = chunk[1];
                    let c = chunk[2];
                    let ls = chunk[4];
                    let rs = chunk[5];

                    let out_l = (l + sqrt2_inv * c + sqrt2_inv * ls) * norm_factor;
                    let out_r = (r + sqrt2_inv * c + sqrt2_inv * rs) * norm_factor;

                    stereo.push(out_l.clamp(-1.0, 1.0));
                    stereo.push(out_r.clamp(-1.0, 1.0));
                }
            }
            _ => {
                // General arbitrary N-channel downmix to Stereo
                for chunk in interleaved.chunks_exact(channels) {
                    let mut l = 0.0f32;
                    let mut r = 0.0f32;
                    let mut l_count = 0usize;
                    let mut r_count = 0usize;

                    for (idx, &s) in chunk.iter().enumerate() {
                        if idx.is_multiple_of(2) {
                            l += s;
                            l_count += 1;
                        } else {
                            r += s;
                            r_count += 1;
                        }
                    }

                    let avg_l = if l_count > 0 { l / l_count as f32 } else { 0.0 };
                    let avg_r = if r_count > 0 { r / r_count as f32 } else { 0.0 };
                    stereo.push(avg_l);
                    stereo.push(avg_r);
                }
            }
        }

        stereo
    }

    /// Remixes interleaved audio stream from `in_channels` to `out_channels`.
    pub fn remix_channels(interleaved: &[f32], in_channels: usize, out_channels: usize) -> Vec<f32> {
        if in_channels == out_channels {
            return interleaved.to_vec();
        }
        match out_channels {
            1 => Self::downmix_to_mono(interleaved, in_channels),
            2 => Self::downmix_to_stereo(interleaved, in_channels),
            _ => {
                // If out_channels > 2, downmix first to stereo and replicate or zero-pad
                let stereo = Self::downmix_to_stereo(interleaved, in_channels);
                let frames = stereo.len() / 2;
                let mut out = Vec::with_capacity(frames * out_channels);
                for chunk in stereo.chunks_exact(2) {
                    out.push(chunk[0]);
                    out.push(chunk[1]);
                    out.resize(out.len() + (out_channels - 2), 0.0);
                }
                out
            }
        }
    }
}
