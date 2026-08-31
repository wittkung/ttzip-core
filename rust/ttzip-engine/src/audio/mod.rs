// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Cross-platform high-performance acoustic waveform analysis and extraction engine.
//! Decodes all major audio codecs (MP3, AAC, M4A, FLAC, WAV, AIFF, OGG, ALAC, CAF)
//! in 100% pure Rust via Symphonia and outputs true time-domain physical oscillogram peaks.

use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[cfg(test)]
mod tests;

/// Extracts normalized peak amplitudes `[0.0 .. 1.0]` across `bucket_count` buckets from audio file data in memory.
pub fn extract_waveform_from_bytes(data: &[u8], bucket_count: usize) -> Vec<f32> {
    let buckets = bucket_count.clamp(16, 2048);
    if data.len() < 12 {
        return default_waveform(buckets);
    }

    // 1. Fast path: Direct WAV / RIFF PCM parser
    if data.starts_with(b"RIFF") && data.len() >= 44 && &data[8..12] == b"WAVE" {
        if let Some(waveform) = parse_wav_waveform(data, buckets) {
            return waveform;
        }
    }

    // 2. Fast path: Direct AIFF / AIFC parser
    if data.starts_with(b"FORM") && data.len() >= 44 && (&data[8..12] == b"AIFF" || &data[8..12] == b"AIFC") {
        if let Some(waveform) = parse_aiff_waveform(data, buckets) {
            return waveform;
        }
    }

    // 3. Full Symphonia multi-codec stream decoder (MP3, AAC, M4A, FLAC, OGG, ALAC, CAF)
    let cursor = Cursor::new(data.to_vec());
    let hint = Hint::new();
    if let Some(waveform) = decode_symphonia_waveform(cursor, &hint, buckets) {
        return waveform;
    }

    Vec::new()
}

/// Extracts normalized peak amplitudes `[0.0 .. 1.0]` from an audio file on disk.
pub fn extract_waveform_from_file<P: AsRef<Path>>(path: P, bucket_count: usize) -> io::Result<Vec<f32>> {
    let buckets = bucket_count.clamp(16, 2048);
    let p = path.as_ref();
    let file = File::open(p)?;

    let mut hint = Hint::new();
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    if let Some(waveform) = decode_symphonia_waveform(file, &hint, buckets) {
        return Ok(waveform);
    }

    // Fallback: read bytes if stream probe failed
    let mut f2 = File::open(p)?;
    let mut buffer = Vec::new();
    f2.read_to_end(&mut buffer)?;
    Ok(extract_waveform_from_bytes(&buffer, buckets))
}

/// Symphonia stream decoder core for extracting acoustic peak envelopes across all formats.
fn decode_symphonia_waveform<R: MediaSource + 'static>(
    source: R,
    hint: &Hint,
    bucket_count: usize,
) -> Option<Vec<f32>> {
    let mss = MediaSourceStream::new(Box::new(source), Default::default());
    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(hint, mss, &fmt_opts, &meta_opts)
        .ok()?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)?;

    let track_id = track.id;
    let dec_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .ok()?;

    let mut all_frame_peaks = Vec::with_capacity(32768);

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                extract_buffer_peaks(&decoded, &mut all_frame_peaks);
            }
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::DecodeError(_)) => {
                continue;
            }
            Err(_) => {
                break;
            }
        }
    }

    if all_frame_peaks.is_empty() {
        return None;
    }

    let total = all_frame_peaks.len();
    let mut buckets = vec![0.0f32; bucket_count];

    for (i, &peak) in all_frame_peaks.iter().enumerate() {
        let b = ((i as f64 / total as f64) * bucket_count as f64) as usize;
        let b_idx = b.min(bucket_count - 1);
        if peak > buckets[b_idx] {
            buckets[b_idx] = peak;
        }
    }

    normalize_waveform(&mut buckets);
    Some(buckets)
}

fn extract_buffer_peaks(buf_ref: &AudioBufferRef, out_peaks: &mut Vec<f32>) {
    match buf_ref {
        AudioBufferRef::F32(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = buf.chan(c)[i].abs();
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::U8(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = ((buf.chan(c)[i] as f32 - 128.0) / 128.0).abs();
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::U16(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = ((buf.chan(c)[i] as f32 - 32768.0) / 32768.0).abs();
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::U24(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = ((buf.chan(c)[i].0 as f32 - 8_388_608.0) / 8_388_608.0).abs();
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::U32(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = ((buf.chan(c)[i] as f64 - 2_147_483_648.0) / 2_147_483_648.0).abs() as f32;
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::S8(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = (buf.chan(c)[i] as f32 / 128.0).abs();
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::S16(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = (buf.chan(c)[i] as f32 / 32768.0).abs();
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::S24(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = (buf.chan(c)[i].0 as f32 / 8_388_608.0).abs();
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::S32(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = (buf.chan(c)[i] as f64 / 2_147_483_648.0).abs() as f32;
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
        AudioBufferRef::F64(buf) => {
            let frames = buf.frames();
            let channels = buf.spec().channels.count();
            let step = (frames / 16).max(1);
            let mut f = 0;
            while f < frames {
                let end = (f + step).min(frames);
                let mut max_s = 0.0f32;
                for i in f..end {
                    for c in 0..channels {
                        let s = buf.chan(c)[i].abs() as f32;
                        if s > max_s { max_s = s; }
                    }
                }
                out_peaks.push(max_s);
                f += step;
            }
        }
    }
}

/// Parses uncompressed WAV RIFF PCM data.
fn parse_wav_waveform(data: &[u8], bucket_count: usize) -> Option<Vec<f32>> {
    let mut offset = 12;
    let mut audio_format = 1u16;
    let mut channels = 1u16;
    let mut bits_per_sample = 16u16;
    let mut data_start = 0usize;
    let mut data_len = 0usize;

    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        offset += 8;

        if chunk_id == b"fmt " && offset + 14 <= data.len() {
            audio_format = u16::from_le_bytes([data[offset], data[offset + 1]]);
            channels = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
            bits_per_sample = u16::from_le_bytes([data[offset + 14], data[offset + 15]]);
        } else if chunk_id == b"data" {
            data_start = offset;
            data_len = chunk_size.min(data.len().saturating_sub(offset));
            break;
        }

        offset += chunk_size;
        if !chunk_size.is_multiple_of(2) {
            offset += 1;
        }
    }

    if data_start == 0 || data_len == 0 {
        return None;
    }

    let pcm_bytes = &data[data_start..data_start + data_len];
    let bytes_per_sample = (bits_per_sample / 8).max(1) as usize;
    let frame_size = bytes_per_sample * channels.max(1) as usize;
    if frame_size == 0 || pcm_bytes.len() < frame_size {
        return None;
    }

    let total_frames = pcm_bytes.len() / frame_size;
    let mut buckets = vec![0.0f32; bucket_count];

    for (b, bucket) in buckets.iter_mut().enumerate().take(bucket_count) {
        let start_frame = (b * total_frames) / bucket_count;
        let end_frame = ((b + 1) * total_frames) / bucket_count;

        if start_frame >= total_frames {
            break;
        }

        let mut max_val = 0.0f32;
        let step = ((end_frame - start_frame) / 256).max(1);
        let mut frame_idx = start_frame;

        while frame_idx < end_frame {
            let byte_pos = frame_idx * frame_size;
            for ch in 0..channels as usize {
                let sample_offset = byte_pos + ch * bytes_per_sample;
                let sample_val = match (audio_format, bits_per_sample) {
                    (1, 16) if sample_offset + 2 <= pcm_bytes.len() => {
                        let raw = i16::from_le_bytes([
                            pcm_bytes[sample_offset],
                            pcm_bytes[sample_offset + 1],
                        ]);
                        (raw as f32).abs() / 32768.0
                    }
                    (1, 8) if sample_offset < pcm_bytes.len() => {
                        let raw = pcm_bytes[sample_offset] as f32;
                        ((raw - 128.0) / 128.0).abs()
                    }
                    (1, 24) if sample_offset + 3 <= pcm_bytes.len() => {
                        let b0 = pcm_bytes[sample_offset] as u32;
                        let b1 = pcm_bytes[sample_offset + 1] as u32;
                        let b2 = pcm_bytes[sample_offset + 2] as u32;
                        let mut raw = (b0 | (b1 << 8) | (b2 << 16)) as i32;
                        if raw & 0x0080_0000 != 0 {
                            raw |= !0x00FF_FFFF;
                        }
                        (raw as f32).abs() / 8_388_608.0
                    }
                    (3, 32) if sample_offset + 4 <= pcm_bytes.len() => {
                        let raw = f32::from_le_bytes([
                            pcm_bytes[sample_offset],
                            pcm_bytes[sample_offset + 1],
                            pcm_bytes[sample_offset + 2],
                            pcm_bytes[sample_offset + 3],
                        ]);
                        raw.abs()
                    }
                    _ => 0.0,
                };

                if sample_val > max_val {
                    max_val = sample_val;
                }
            }
            frame_idx += step;
        }

        *bucket = max_val;
    }

    normalize_waveform(&mut buckets);
    Some(buckets)
}

/// Parses uncompressed AIFF/AIFC big-endian PCM data.
fn parse_aiff_waveform(data: &[u8], bucket_count: usize) -> Option<Vec<f32>> {
    let mut offset = 12;
    let mut channels = 1u16;
    let mut bits_per_sample = 16u16;
    let mut data_start = 0usize;
    let mut data_len = 0usize;

    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = u32::from_be_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        offset += 8;

        if chunk_id == b"COMM" && offset + 8 <= data.len() {
            channels = u16::from_be_bytes([data[offset], data[offset + 1]]);
            bits_per_sample = u16::from_be_bytes([data[offset + 6], data[offset + 7]]);
        } else if chunk_id == b"SSND" && offset + 8 <= data.len() {
            let ssnd_offset = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            data_start = offset + 8 + ssnd_offset;
            data_len = chunk_size.saturating_sub(8 + ssnd_offset).min(data.len().saturating_sub(data_start));
            break;
        }

        offset += chunk_size;
        if !chunk_size.is_multiple_of(2) {
            offset += 1;
        }
    }

    if data_start == 0 || data_len == 0 {
        return None;
    }

    let pcm_bytes = &data[data_start..data_start + data_len];
    let bytes_per_sample = (bits_per_sample / 8).max(1) as usize;
    let frame_size = bytes_per_sample * channels.max(1) as usize;
    if frame_size == 0 || pcm_bytes.len() < frame_size {
        return None;
    }

    let total_frames = pcm_bytes.len() / frame_size;
    let mut buckets = vec![0.0f32; bucket_count];

    for (b, bucket) in buckets.iter_mut().enumerate().take(bucket_count) {
        let start_frame = (b * total_frames) / bucket_count;
        let end_frame = ((b + 1) * total_frames) / bucket_count;

        if start_frame >= total_frames {
            break;
        }

        let mut max_val = 0.0f32;
        let step = ((end_frame - start_frame) / 256).max(1);
        let mut frame_idx = start_frame;

        while frame_idx < end_frame {
            let byte_pos = frame_idx * frame_size;
            for ch in 0..channels as usize {
                let sample_offset = byte_pos + ch * bytes_per_sample;
                if sample_offset + 2 <= pcm_bytes.len() {
                    let raw = i16::from_be_bytes([pcm_bytes[sample_offset], pcm_bytes[sample_offset + 1]]);
                    let val = (raw as f32).abs() / 32768.0;
                    if val > max_val {
                        max_val = val;
                    }
                }
            }
            frame_idx += step;
        }
        *bucket = max_val;
    }

    normalize_waveform(&mut buckets);
    Some(buckets)
}

/// Normalizes waveform array into range `[0.0 .. 1.0]`.
fn normalize_waveform(buckets: &mut [f32]) {
    let mut max_peak = 0.0001f32;
    for &val in buckets.iter() {
        if val > max_peak {
            max_peak = val;
        }
    }

    for val in buckets.iter_mut() {
        let normalized = *val / max_peak;
        *val = normalized.clamp(0.0, 1.0);
    }
}

/// Generates a pleasant organic fallback waveform when audio decoding fails.
pub fn default_waveform(count: usize) -> Vec<f32> {
    (0..count)
        .map(|idx| {
            let progress = idx as f32 / count as f32;
            let harmonic1 = (progress * std::f32::consts::PI * 8.0).sin() * 0.35;
            let harmonic2 = (progress * std::f32::consts::PI * 19.5).sin() * 0.25;
            let envelope = (progress * std::f32::consts::PI).sin();
            ((0.25 + (harmonic1 + harmonic2).abs()) * envelope).clamp(0.04, 0.95)
        })
        .collect()
}
