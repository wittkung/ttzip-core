// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe Rust streaming audio decoder and seek state machine based on Symphonia.
//!
//! Provides zero-unsafe audio stream probing, packet iteration, format inspection,
//! and accurate time/frame seeking across MP3, FLAC, WAV, AAC, ALAC, OGG Vorbis, and Opus.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use serde::{Deserialize, Serialize};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

use super::pcm::AudioPcmConverter;
use super::AudioError;

/// Stream format and codec parameters for an opened audio track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioStreamInfo {
    /// Container format descriptor (e.g. "wav", "mp3", "flac", "ogg", "isomp4").
    pub format_name: String,
    /// Codec identifier string (e.g. "pcm_s16", "mp3", "flac", "vorbis", "aac", "alac").
    pub codec_name: String,
    /// Sample rate in Hz (e.g. 44100, 48000, 96000).
    pub sample_rate: u32,
    /// Number of audio channels (e.g. 1 for mono, 2 for stereo, 6 for 5.1).
    pub channels: u32,
    /// Optional channel layout bitmask.
    pub channel_mask: Option<u32>,
    /// Estimated or exact total duration in seconds.
    pub duration_seconds: Option<f64>,
    /// Total audio frame/sample count if known.
    pub total_samples: Option<u64>,
    /// Bitrate in kilobits per second (kbps).
    pub bitrate_kbps: Option<u32>,
    /// Bits per audio sample (e.g. 16, 24, 32).
    pub bits_per_sample: Option<u32>,
}

/// A decoded audio packet containing normalized interleaved `f32` samples and timing metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedAudioPacket {
    /// Interleaved audio sample buffer normalized to `[-1.0, 1.0]`.
    pub samples_interleaved: Vec<f32>,
    /// Number of audio sample frames in this packet.
    pub frames: usize,
    /// Number of audio channels.
    pub channels: usize,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Packet presentation timestamp in seconds.
    pub timestamp_seconds: f64,
    /// Packet duration in seconds.
    pub duration_seconds: f64,
}

/// High-level safe Rust audio stream decoder.
pub struct TTZipAudioDecoder {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    stream_info: AudioStreamInfo,
    time_base: Option<TimeBase>,
    current_timestamp: f64,
    current_frame: u64,
    exhausted: bool,
}

impl TTZipAudioDecoder {
    /// Opens and probes an audio stream from an in-memory byte slice.
    pub fn open_from_bytes(data: &[u8]) -> Result<Self, AudioError> {
        Self::open_from_bytes_with_hint(data, None)
    }

    /// Opens and probes an audio stream from an in-memory byte slice with a format hint.
    pub fn open_from_bytes_with_hint(
        data: &[u8],
        hint_str: Option<&str>,
    ) -> Result<Self, AudioError> {
        if data.is_empty() {
            return Err(AudioError::InvalidParameter("Audio byte slice is empty".to_string()));
        }

        let cursor = Cursor::new(data.to_vec());
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        let mut hint = Hint::new();
        if let Some(h) = hint_str {
            if h.contains('/') {
                hint.with_extension(h.split('/').next_back().unwrap_or(h));
            } else {
                hint.with_extension(h);
            }
        }

        Self::init_from_media_source_stream(mss, &hint)
    }

    /// Opens and probes an audio stream from a file on disk.
    pub fn open_from_file<P: AsRef<Path>>(path: P) -> Result<Self, AudioError> {
        let p = path.as_ref();
        let file = File::open(p).map_err(AudioError::Io)?;

        let mut hint = Hint::new();
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        Self::init_from_media_source_stream(mss, &hint)
    }

    /// Initializes format reader and default audio codec from a media source stream.
    fn init_from_media_source_stream(
        mss: MediaSourceStream,
        hint: &Hint,
    ) -> Result<Self, AudioError> {
        let fmt_opts = FormatOptions::default();
        let meta_opts = MetadataOptions::default();

        let probed_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            symphonia::default::get_probe().format(hint, mss, &fmt_opts, &meta_opts)
        }));

        let probed = match probed_res {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return Err(AudioError::UnsupportedFormat(e.to_string())),
            Err(_) => {
                return Err(AudioError::Format(
                    "Symphonia probe panicked on corrupted audio stream".to_string(),
                ))
            }
        };

        let format_reader = probed.format;
        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::UnsupportedFormat("No playable audio track found".to_string()))?;

        let track_id = track.id;
        let codec_params = &track.codec_params;
        let dec_opts = DecoderOptions::default();

        let decoder_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            symphonia::default::get_codecs().make(codec_params, &dec_opts)
        }));

        let decoder = match decoder_res {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(AudioError::UnsupportedCodec(e.to_string())),
            Err(_) => {
                return Err(AudioError::Codec(
                    "Symphonia decoder instantiation panicked on corrupted stream".to_string(),
                ))
            }
        };

        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params
            .channels
            .map(|c| c.count() as u32)
            .unwrap_or(2);
        let channel_mask = codec_params.channels.map(|c| c.bits());
        let bits_per_sample = codec_params.bits_per_sample;
        let total_samples = codec_params.n_frames;

        let time_base = codec_params.time_base;
        let duration_seconds = if let Some(n_frames) = total_samples {
            if let Some(tb) = time_base {
                let time = tb.calc_time(n_frames);
                Some(time.seconds as f64 + time.frac)
            } else if sample_rate > 0 {
                Some(n_frames as f64 / sample_rate as f64)
            } else {
                None
            }
        } else {
            None
        };

        let format_name = "audio".to_string();
        let codec_name = codec_type_to_string(codec_params.codec);

        let stream_info = AudioStreamInfo {
            format_name,
            codec_name,
            sample_rate,
            channels,
            channel_mask,
            duration_seconds,
            total_samples,
            bitrate_kbps: None,
            bits_per_sample,
        };

        Ok(Self {
            format_reader,
            decoder,
            track_id,
            stream_info,
            time_base,
            current_timestamp: 0.0,
            current_frame: 0,
            exhausted: false,
        })
    }

    /// Returns audio stream and codec metadata information.
    pub fn stream_info(&self) -> &AudioStreamInfo {
        &self.stream_info
    }

    /// Returns the current playback / decoding timestamp in seconds.
    pub fn current_timestamp(&self) -> f64 {
        self.current_timestamp
    }

    /// Returns the current decoded frame index.
    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// Decodes the next available audio packet from the media stream.
    pub fn decode_next_packet(&mut self) -> Result<Option<DecodedAudioPacket>, AudioError> {
        if self.exhausted {
            return Ok(None);
        }

        loop {
            let next_pkt_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.format_reader.next_packet()
            }));

            let packet = match next_pkt_res {
                Ok(Ok(pkt)) => pkt,
                Ok(Err(SymphoniaError::IoError(e)))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    self.exhausted = true;
                    return Ok(None);
                }
                Ok(Err(SymphoniaError::ResetRequired)) => {
                    self.decoder.reset();
                    continue;
                }
                Ok(Err(e)) => {
                    self.exhausted = true;
                    return Err(AudioError::Format(e.to_string()));
                }
                Err(_) => {
                    self.exhausted = true;
                    return Err(AudioError::Format(
                        "Symphonia format reader panicked on next packet".to_string(),
                    ));
                }
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            let dec_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                match self.decoder.decode(&packet) {
                    Ok(decoded_buf) => {
                        let samples_interleaved =
                            AudioPcmConverter::convert_buffer_ref_to_interleaved_f32(&decoded_buf);
                        let frames = match &decoded_buf {
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
                        Ok((samples_interleaved, frames))
                    }
                    Err(e) => Err(e),
                }
            }));

            match dec_res {
                Ok(Ok((samples_interleaved, frames))) => {
                    let channels = self.stream_info.channels as usize;
                    let sample_rate = self.stream_info.sample_rate;
                    let packet_duration = if sample_rate > 0 {
                        frames as f64 / sample_rate as f64
                    } else {
                        0.0
                    };

                    let ts_seconds = if let Some(tb) = self.time_base {
                        let time = tb.calc_time(packet.ts());
                        time.seconds as f64 + time.frac
                    } else {
                        self.current_timestamp
                    };

                    self.current_timestamp = ts_seconds + packet_duration;
                    self.current_frame += frames as u64;

                    return Ok(Some(DecodedAudioPacket {
                        samples_interleaved,
                        frames,
                        channels,
                        sample_rate,
                        timestamp_seconds: ts_seconds,
                        duration_seconds: packet_duration,
                    }));
                }
                Ok(Err(SymphoniaError::IoError(e)))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    self.exhausted = true;
                    return Ok(None);
                }
                Ok(Err(SymphoniaError::DecodeError(msg))) => {
                    log::warn!("Audio decode packet warning: {}", msg);
                    continue;
                }
                Ok(Err(SymphoniaError::ResetRequired)) => {
                    self.decoder.reset();
                    continue;
                }
                Ok(Err(e)) => {
                    return Err(AudioError::Codec(e.to_string()));
                }
                Err(_) => {
                    return Err(AudioError::Codec(
                        "Symphonia decode panicked on packet".to_string(),
                    ));
                }
            }
        }
    }

    /// Seeks the audio stream to the target timestamp in seconds.
    pub fn seek(&mut self, time_seconds: f64) -> Result<f64, AudioError> {
        if time_seconds < 0.0 {
            return Err(AudioError::InvalidParameter(
                "Seek time cannot be negative".to_string(),
            ));
        }

        let seek_to = if let Some(tb) = self.time_base {
            let time = Time::from(time_seconds);
            let ts = tb.calc_timestamp(time);
            SeekTo::TimeStamp {
                ts,
                track_id: self.track_id,
            }
        } else {
            let sample_rate = self.stream_info.sample_rate.max(1);
            let frame = (time_seconds * sample_rate as f64).round() as u64;
            SeekTo::TimeStamp {
                ts: frame,
                track_id: self.track_id,
            }
        };

        let seek_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.format_reader.seek(SeekMode::Accurate, seek_to)
        }));

        match seek_res {
            Ok(Ok(actual_seek)) => {
                self.decoder.reset();
                self.exhausted = false;
                let actual_time = if let Some(tb) = self.time_base {
                    let time = tb.calc_time(actual_seek.actual_ts);
                    time.seconds as f64 + time.frac
                } else {
                    actual_seek.actual_ts as f64 / self.stream_info.sample_rate.max(1) as f64
                };
                self.current_timestamp = actual_time;
                self.current_frame = actual_seek.required_ts;
                Ok(actual_time)
            }
            Ok(Err(SymphoniaError::SeekError(e))) => {
                Err(AudioError::SeekError(format!("{:?}", e)))
            }
            Ok(Err(e)) => Err(AudioError::SeekError(e.to_string())),
            Err(_) => Err(AudioError::SeekError(
                "Symphonia seek panicked on target timestamp".to_string(),
            )),
        }
    }

    /// Seeks the audio stream to the target sample frame index.
    pub fn seek_to_frame(&mut self, frame: u64) -> Result<u64, AudioError> {
        let seek_to = SeekTo::TimeStamp {
            ts: frame,
            track_id: self.track_id,
        };
        match self.format_reader.seek(SeekMode::Accurate, seek_to) {
            Ok(actual_seek) => {
                self.decoder.reset();
                self.exhausted = false;
                self.current_frame = actual_seek.required_ts;
                if let Some(tb) = self.time_base {
                    let time = tb.calc_time(actual_seek.actual_ts);
                    self.current_timestamp = time.seconds as f64 + time.frac;
                } else {
                    self.current_timestamp =
                        actual_seek.actual_ts as f64 / self.stream_info.sample_rate.max(1) as f64;
                }
                Ok(actual_seek.actual_ts)
            }
            Err(SymphoniaError::SeekError(e)) => Err(AudioError::SeekError(format!("{:?}", e))),
            Err(e) => Err(AudioError::SeekError(e.to_string())),
        }
    }

    /// Resets the decoder back to timestamp 0.0.
    pub fn reset(&mut self) -> Result<(), AudioError> {
        self.seek(0.0).map(|_| ())
    }
}

/// Helper function to convert Symphonia `CodecType` into a standard string representation.
fn codec_type_to_string(codec: symphonia::core::codecs::CodecType) -> String {
    use symphonia::core::codecs::*;
    if codec == CODEC_TYPE_MP3 {
        "mp3".to_string()
    } else if codec == CODEC_TYPE_AAC {
        "aac".to_string()
    } else if codec == CODEC_TYPE_VORBIS {
        "vorbis".to_string()
    } else if codec == CODEC_TYPE_OPUS {
        "opus".to_string()
    } else if codec == CODEC_TYPE_FLAC {
        "flac".to_string()
    } else if codec == CODEC_TYPE_ALAC {
        "alac".to_string()
    } else if codec == CODEC_TYPE_PCM_S16LE || codec == CODEC_TYPE_PCM_S16BE {
        "pcm_s16".to_string()
    } else if codec == CODEC_TYPE_PCM_S24LE || codec == CODEC_TYPE_PCM_S24BE {
        "pcm_s24".to_string()
    } else if codec == CODEC_TYPE_PCM_S32LE || codec == CODEC_TYPE_PCM_S32BE {
        "pcm_s32".to_string()
    } else if codec == CODEC_TYPE_PCM_F32LE || codec == CODEC_TYPE_PCM_F32BE {
        "pcm_f32".to_string()
    } else if codec == CODEC_TYPE_PCM_U8 {
        "pcm_u8".to_string()
    } else {
        format!("{:?}", codec)
    }
}
