// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio Probing, Metadata Extraction, Waveform Generation, and PCM Decoding Engine.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey, Value};
use symphonia::core::probe::Hint;

use super::types::{
    UniFFIAudioCoverArt, UniFFIAudioError, UniFFIAudioMetadata, UniFFIAudioPacket,
    UniFFIAudioStreamInfo, UniFFIAudioWaveform,
};

/// Reads file contents safely into memory.
pub fn read_file_bytes(path_str: &str) -> Result<Vec<u8>, UniFFIAudioError> {
    let p = Path::new(path_str);
    if !p.exists() {
        return Err(UniFFIAudioError::IoError {
            message: format!("File not found: {path_str}"),
        });
    }
    let mut file = File::open(p).map_err(UniFFIAudioError::io_err)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(UniFFIAudioError::io_err)?;
    Ok(buffer)
}

/// Configures Symphonia hint from optional filename.
fn build_hint(file_name: Option<&str>) -> Hint {
    let mut hint = Hint::new();
    if let Some(name) = file_name {
        if let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
    }
    hint
}

/// Probes primary audio track stream info.
pub fn probe_stream_info_from_bytes(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIAudioStreamInfo, UniFFIAudioError> {
    if data.len() < 12 {
        return Err(UniFFIAudioError::InvalidParameter {
            parameter: "Audio buffer too small (< 12 bytes)".to_string(),
        });
    }

    // Fast-path for WAV header parsing
    if data.starts_with(b"RIFF") && data.len() >= 44 && &data[8..12] == b"WAVE" {
        if let Some(info) = parse_wav_stream_info(data) {
            return Ok(info);
        }
    }

    // Fast-path for AIFF header parsing
    if data.starts_with(b"FORM") && data.len() >= 44 && (&data[8..12] == b"AIFF" || &data[8..12] == b"AIFC") {
        if let Some(info) = parse_aiff_stream_info(data) {
            return Ok(info);
        }
    }

    let source = Box::new(Cursor::new(data.to_vec()));
    let mss = MediaSourceStream::new(source, Default::default());
    let hint = build_hint(file_name);
    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| UniFFIAudioError::UnsupportedFormat {
            format: e.to_string(),
        })?;

    let format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| UniFFIAudioError::UnsupportedFormat {
            format: "No valid audio track detected in stream".to_string(),
        })?;

    let params = &track.codec_params;
    let sample_rate = params.sample_rate.unwrap_or(44100);
    let channels = params.channels.map(|c| c.count() as u32).unwrap_or(2);
    let channel_layout = match channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        n => format!("{n} channels"),
    };
    let bits_per_sample = params.bits_per_sample.or(params.bits_per_coded_sample);
    let bit_rate = bits_per_sample.map(|bps| (sample_rate * channels * bps) as u64);

    let duration_seconds = if let Some(n_frames) = params.n_frames {
        if sample_rate > 0 {
            n_frames as f64 / sample_rate as f64
        } else {
            0.0
        }
    } else {
        0.0
    };

    let codec_name = format!("{:?}", params.codec).to_lowercase();
    let codec_long_name = format!("{:?}", params.codec);

    Ok(UniFFIAudioStreamInfo {
        codec_name,
        codec_long_name,
        sample_rate,
        channels,
        channel_layout,
        bits_per_sample,
        bit_rate,
        duration_seconds,
        total_frames: params.n_frames,
    })
}

/// Extracts metadata tags, technical parameters, and artwork from audio bytes.
pub fn extract_metadata_from_bytes(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIAudioMetadata, UniFFIAudioError> {
    if data.is_empty() {
        return Err(UniFFIAudioError::InvalidParameter {
            parameter: "Audio buffer cannot be empty".to_string(),
        });
    }

    let stream_info = probe_stream_info_from_bytes(data, file_name).unwrap_or_else(|_| {
        UniFFIAudioStreamInfo {
            codec_name: "unknown".to_string(),
            codec_long_name: "Unknown Codec".to_string(),
            sample_rate: 44100,
            channels: 2,
            channel_layout: "stereo".to_string(),
            bits_per_sample: Some(16),
            bit_rate: None,
            duration_seconds: 0.0,
            total_frames: None,
        }
    });

    let container_format = file_name
        .and_then(|n| Path::new(n).extension())
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| {
            if data.starts_with(b"RIFF") {
                "wav".to_string()
            } else if data.starts_with(b"FORM") {
                "aiff".to_string()
            } else if data.starts_with(b"fLaC") {
                "flac".to_string()
            } else if data.starts_with(b"OggS") {
                "ogg".to_string()
            } else if data.starts_with(b"ID3") || data.starts_with(&[0xFF, 0xFB]) {
                "mp3".to_string()
            } else {
                "audio".to_string()
            }
        });

    let mut metadata = UniFFIAudioMetadata {
        title: None,
        artist: None,
        album: None,
        album_artist: None,
        track_number: None,
        track_total: None,
        disc_number: None,
        disc_total: None,
        year: None,
        genre: None,
        composer: None,
        lyrics: None,
        copyright: None,
        cover_art: None,
        stream_info,
        file_size_bytes: data.len() as u64,
        container_format,
        extra_tags: HashMap::new(),
    };

    let source = Box::new(Cursor::new(data.to_vec()));
    let mss = MediaSourceStream::new(source, Default::default());
    let hint = build_hint(file_name);
    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    if let Ok(mut probed) = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts) {
        if let Some(rev) = probed.format.metadata().current() {
            populate_metadata_from_revision(rev, &mut metadata);
        }
        if let Some(meta_log) = probed.metadata.get() {
            if let Some(rev) = meta_log.current() {
                populate_metadata_from_revision(rev, &mut metadata);
            }
        }
    }

    Ok(metadata)
}

/// Populates metadata fields and visual cover art from a Symphonia MetadataRevision.
fn populate_metadata_from_revision(
    rev: &symphonia::core::meta::MetadataRevision,
    metadata: &mut UniFFIAudioMetadata,
) {
    for tag in rev.tags() {
        let val_str = match &tag.value {
            Value::String(s) => s.clone(),
            Value::UnsignedInt(u) => u.to_string(),
            Value::SignedInt(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Binary(bytes) => format!("<binary {} bytes>", bytes.len()),
            Value::Flag => "true".to_string(),
        };

        if let Some(std_key) = tag.std_key {
            match std_key {
                StandardTagKey::TrackTitle => metadata.title = Some(val_str.clone()),
                StandardTagKey::Artist => metadata.artist = Some(val_str.clone()),
                StandardTagKey::Album => metadata.album = Some(val_str.clone()),
                StandardTagKey::AlbumArtist => metadata.album_artist = Some(val_str.clone()),
                StandardTagKey::TrackNumber => {
                    metadata.track_number = parse_int_or_fraction(&val_str);
                }
                StandardTagKey::TrackTotal => {
                    metadata.track_total = parse_int_or_fraction(&val_str);
                }
                StandardTagKey::DiscNumber => {
                    metadata.disc_number = parse_int_or_fraction(&val_str);
                }
                StandardTagKey::DiscTotal => {
                    metadata.disc_total = parse_int_or_fraction(&val_str);
                }
                StandardTagKey::Date | StandardTagKey::ReleaseDate => {
                    metadata.year = Some(val_str.clone());
                }
                StandardTagKey::Genre => metadata.genre = Some(val_str.clone()),
                StandardTagKey::Composer => metadata.composer = Some(val_str.clone()),
                StandardTagKey::Lyrics => metadata.lyrics = Some(val_str.clone()),
                StandardTagKey::Copyright => metadata.copyright = Some(val_str.clone()),
                _ => {}
            }
        }

        metadata.extra_tags.insert(tag.key.clone(), val_str);
    }

    if metadata.cover_art.is_none() {
        if let Some(visual) = rev.visuals().first() {
            let (width, height) = visual
                .dimensions
                .map(|d| (d.width, d.height))
                .unzip();
            let desc = visual.usage.map(|u| format!("{u:?}"));
            metadata.cover_art = Some(UniFFIAudioCoverArt {
                mime_type: visual.media_type.clone(),
                width,
                height,
                data: visual.data.to_vec(),
                description: desc,
            });
        }
    }
}

/// Helper parsing integer numbers or fraction strings like "3/12".
fn parse_int_or_fraction(s: &str) -> Option<u32> {
    if let Some((num, _)) = s.split_once('/') {
        num.trim().parse::<u32>().ok()
    } else {
        s.trim().parse::<u32>().ok()
    }
}

/// Computes normalized waveform envelope amplitudes.
pub fn generate_waveform_from_bytes(
    data: &[u8],
    bucket_count: u32,
    file_name: Option<&str>,
) -> Result<UniFFIAudioWaveform, UniFFIAudioError> {
    let count = bucket_count.clamp(16, 2048) as usize;
    let amplitudes = crate::audio::extract_waveform_from_bytes(data, count);
    let rms_amplitudes: Vec<f32> = amplitudes
        .iter()
        .map(|&a| (a * std::f32::consts::FRAC_1_SQRT_2).clamp(0.0, 1.0))
        .collect();

    let stream_info = probe_stream_info_from_bytes(data, file_name).unwrap_or_else(|_| {
        UniFFIAudioStreamInfo {
            codec_name: "unknown".to_string(),
            codec_long_name: "Unknown Codec".to_string(),
            sample_rate: 44100,
            channels: 2,
            channel_layout: "stereo".to_string(),
            bits_per_sample: Some(16),
            bit_rate: None,
            duration_seconds: 0.0,
            total_frames: None,
        }
    });

    let effective_count = amplitudes.len() as u32;

    Ok(UniFFIAudioWaveform {
        amplitudes,
        bucket_count: effective_count,
        duration_seconds: stream_info.duration_seconds,
        sample_rate: stream_info.sample_rate,
        channels: stream_info.channels,
        rms_amplitudes,
    })
}

/// Decodes audio stream packets into chunked float PCM samples.
pub fn decode_stream_packets_from_bytes(
    data: &[u8],
    max_packets: Option<u32>,
    file_name: Option<&str>,
) -> Result<Vec<UniFFIAudioPacket>, UniFFIAudioError> {
    if data.is_empty() {
        return Err(UniFFIAudioError::InvalidParameter {
            parameter: "Audio data cannot be empty".to_string(),
        });
    }

    let source = Box::new(Cursor::new(data.to_vec()));
    let mss = MediaSourceStream::new(source, Default::default());
    let hint = build_hint(file_name);
    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| UniFFIAudioError::UnsupportedFormat {
            format: e.to_string(),
        })?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| UniFFIAudioError::UnsupportedFormat {
            format: "No valid audio track found in stream".to_string(),
        })?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u32).unwrap_or(2);
    let dec_opts = DecoderOptions::default();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| UniFFIAudioError::decode_err(e.to_string()))?;

    let limit = max_packets.unwrap_or(u32::MAX) as usize;
    let mut packets_out = Vec::new();
    let mut current_pts_frames: u64 = 0;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let frames = decoded.frames();
                let mut pcm_samples = Vec::with_capacity(frames * channels as usize);
                convert_buffer_to_interleaved_f32(&decoded, channels as usize, &mut pcm_samples);

                let dur_ms = if sample_rate > 0 {
                    (frames as u64 * 1000) / sample_rate as u64
                } else {
                    0
                };
                let pts_ms = if sample_rate > 0 {
                    (current_pts_frames * 1000) / sample_rate as u64
                } else {
                    0
                };
                current_pts_frames += frames as u64;

                packets_out.push(UniFFIAudioPacket {
                    pts_ms,
                    duration_ms: dur_ms,
                    channels,
                    sample_rate,
                    pcm_f32_samples: pcm_samples,
                    frame_count: frames as u32,
                    is_eof: false,
                });

                if packets_out.len() >= limit {
                    break;
                }
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

    if let Some(last) = packets_out.last_mut() {
        last.is_eof = true;
    }

    Ok(packets_out)
}

/// Converts any Symphonia AudioBufferRef into normalized interleaved `[-1.0, 1.0]` f32 samples.
fn convert_buffer_to_interleaved_f32(
    buf_ref: &AudioBufferRef,
    channels: usize,
    out: &mut Vec<f32>,
) {
    match buf_ref {
        AudioBufferRef::F32(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push(buf.chan(c)[f]);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::F64(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push(buf.chan(c)[f] as f32);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::S16(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push(buf.chan(c)[f] as f32 / 32768.0);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::S32(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push((buf.chan(c)[f] as f64 / 2_147_483_648.0) as f32);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::U8(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push((buf.chan(c)[f] as f32 - 128.0) / 128.0);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::S8(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push(buf.chan(c)[f] as f32 / 128.0);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::U16(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push((buf.chan(c)[f] as f32 - 32768.0) / 32768.0);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::U24(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push((buf.chan(c)[f].0 as f32 - 8_388_608.0) / 8_388_608.0);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::S24(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push(buf.chan(c)[f].0 as f32 / 8_388_608.0);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
        AudioBufferRef::U32(buf) => {
            let frames = buf.frames();
            let actual_ch = buf.spec().channels.count().min(channels);
            for f in 0..frames {
                for c in 0..actual_ch {
                    out.push(((buf.chan(c)[f] as f64 - 2_147_483_648.0) / 2_147_483_648.0) as f32);
                }
                for _ in actual_ch..channels {
                    out.push(0.0);
                }
            }
        }
    }
}

/// Fallback parser for WAV headers.
fn parse_wav_stream_info(data: &[u8]) -> Option<UniFFIAudioStreamInfo> {
    let mut offset = 12;
    let mut sample_rate = 44100u32;
    let mut channels = 2u16;
    let mut bits_per_sample = 16u16;
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

        if chunk_id == b"fmt " && offset + 16 <= data.len() {
            channels = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
            sample_rate = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            bits_per_sample = u16::from_le_bytes([data[offset + 14], data[offset + 15]]);
        } else if chunk_id == b"data" {
            data_len = chunk_size.min(data.len().saturating_sub(offset));
            break;
        }

        offset += chunk_size;
        if !chunk_size.is_multiple_of(2) {
            offset += 1;
        }
    }

    let bytes_per_sample = (bits_per_sample / 8).max(1) as usize;
    let frame_size = bytes_per_sample * channels.max(1) as usize;
    let total_frames = data_len.checked_div(frame_size).unwrap_or(0) as u64;
    let duration_seconds = if sample_rate > 0 {
        total_frames as f64 / sample_rate as f64
    } else {
        0.0
    };

    Some(UniFFIAudioStreamInfo {
        codec_name: "pcm_s16le".to_string(),
        codec_long_name: "PCM 16-bit little-endian".to_string(),
        sample_rate,
        channels: channels as u32,
        channel_layout: if channels == 1 { "mono".to_string() } else { "stereo".to_string() },
        bits_per_sample: Some(bits_per_sample as u32),
        bit_rate: Some((sample_rate * channels as u32 * bits_per_sample as u32) as u64),
        duration_seconds,
        total_frames: Some(total_frames),
    })
}

/// Fallback parser for AIFF headers.
fn parse_aiff_stream_info(data: &[u8]) -> Option<UniFFIAudioStreamInfo> {
    let mut offset = 12;
    let mut channels = 2u16;
    let sample_rate = 44100u32;
    let mut bits_per_sample = 16u16;
    let mut num_sample_frames = 0u32;

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
            channels = u16::from_be_bytes([data[offset], data[offset + 1]]);
            num_sample_frames = u32::from_be_bytes([
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
            ]);
            bits_per_sample = u16::from_be_bytes([data[offset + 6], data[offset + 7]]);
        }

        offset += chunk_size;
        if !chunk_size.is_multiple_of(2) {
            offset += 1;
        }
    }

    let duration_seconds = if sample_rate > 0 {
        num_sample_frames as f64 / sample_rate as f64
    } else {
        0.0
    };

    Some(UniFFIAudioStreamInfo {
        codec_name: "pcm_s16be".to_string(),
        codec_long_name: "PCM 16-bit big-endian".to_string(),
        sample_rate,
        channels: channels as u32,
        channel_layout: if channels == 1 { "mono".to_string() } else { "stereo".to_string() },
        bits_per_sample: Some(bits_per_sample as u32),
        bit_rate: Some((sample_rate * channels as u32 * bits_per_sample as u32) as u64),
        duration_seconds,
        total_frames: Some(num_sample_frames as u64),
    })
}
