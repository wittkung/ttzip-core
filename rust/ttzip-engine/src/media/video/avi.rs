// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Resource Interchange File Format (RIFF) AVI container demuxer.
//!
//! Provides zero-unsafe, bounds-checked RIFF chunk iteration, stream header decoding,
//! `BITMAPINFOHEADER` and `WAVEFORMATEX` structural extraction, and normalized metadata generation.

use super::types::{
    AudioCodec, AudioTrackInfo, VideoCodec, VideoError, VideoFormat, VideoMetadata, VideoResult,
    VideoTrackInfo,
};

/// Pure Safe Rust RIFF AVI container demuxer.
pub struct TTZipAviDemuxer<'a> {
    data: &'a [u8],
}

impl<'a> TTZipAviDemuxer<'a> {
    /// Creates a new `TTZipAviDemuxer` over an in-memory byte slice.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Sniffs whether the buffer begins with a valid RIFF AVI header.
    #[must_use]
    pub fn probe_format(data: &[u8]) -> VideoFormat {
        if data.len() < 12 {
            return VideoFormat::Unknown;
        }

        if &data[0..4] == b"RIFF" || &data[0..4] == b"ON2 " {
            let form_type = &data[8..12];
            if form_type == b"AVI " || form_type == b"AVIX" {
                return VideoFormat::Avi;
            }
        }

        VideoFormat::Unknown
    }

    /// Demuxes the AVI container and returns normalized metadata.
    pub fn demux(&self) -> VideoResult<VideoMetadata> {
        if self.data.len() < 12 {
            return Err(VideoError::UnexpectedEof(
                "Buffer too small for RIFF AVI header".to_string(),
            ));
        }

        if &self.data[0..4] != b"RIFF" && &self.data[0..4] != b"ON2 " {
            return Err(VideoError::InvalidData(
                "Missing 'RIFF' magic signature".to_string(),
            ));
        }

        let form_type = &self.data[8..12];
        if form_type != b"AVI " && form_type != b"AVIX" {
            return Err(VideoError::InvalidData(format!(
                "Invalid RIFF form type: {:?}",
                String::from_utf8_lossy(form_type)
            )));
        }

        let riff_payload_len =
            u32::from_le_bytes(self.data[4..8].try_into().unwrap_or([0; 4])) as usize;
        let file_end = riff_payload_len.saturating_add(8).min(self.data.len());
        let riff_slice = &self.data[12..file_end];

        let mut duration_ms = 0u64;
        let mut global_width = 0u32;
        let mut global_height = 0u32;
        let mut global_fps = 0.0f64;

        let mut video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();
        let mut track_counter = 0u32;

        let mut offset = 0;
        while offset < riff_slice.len() {
            let (chunk_id, chunk_size, chunk_payload, next_offset) =
                match Self::read_chunk(riff_slice, offset) {
                    Some(c) => c,
                    None => break,
                };

            if &chunk_id == b"LIST" && chunk_payload.len() >= 4 {
                let list_type = &chunk_payload[0..4];
                let list_payload = &chunk_payload[4..];

                if list_type == b"hdrl" {
                    Self::parse_hdrl(
                        list_payload,
                        &mut duration_ms,
                        &mut global_width,
                        &mut global_height,
                        &mut global_fps,
                        &mut video_tracks,
                        &mut audio_tracks,
                        &mut track_counter,
                    )?;
                }
            }

            offset = next_offset;
            let _ = chunk_size;
        }

        // Apply fallback dimensions/fps to video tracks if missing
        for v in &mut video_tracks {
            if v.width == 0 {
                v.width = global_width;
            }
            if v.height == 0 {
                v.height = global_height;
            }
            if v.fps <= 0.0 {
                v.fps = global_fps;
            }
        }

        Ok(VideoMetadata {
            format: VideoFormat::Avi,
            duration_ms,
            video_tracks,
            audio_tracks,
            subtitle_tracks: Vec::new(),
            chapters: Vec::new(),
            has_cover: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_hdrl(
        hdrl_payload: &[u8],
        duration_ms: &mut u64,
        global_width: &mut u32,
        global_height: &mut u32,
        global_fps: &mut f64,
        video_tracks: &mut Vec<VideoTrackInfo>,
        audio_tracks: &mut Vec<AudioTrackInfo>,
        track_counter: &mut u32,
    ) -> VideoResult<()> {
        let mut offset = 0;
        while offset < hdrl_payload.len() {
            let (chunk_id, _, chunk_payload, next_offset) =
                match Self::read_chunk(hdrl_payload, offset) {
                    Some(c) => c,
                    None => break,
                };

            if &chunk_id == b"avih" {
                if let Some((dur, w, h, fps)) = Self::parse_avih(chunk_payload) {
                    *duration_ms = dur;
                    *global_width = w;
                    *global_height = h;
                    *global_fps = fps;
                }
            } else if &chunk_id == b"LIST" && chunk_payload.len() >= 4 {
                let list_type = &chunk_payload[0..4];
                let list_payload = &chunk_payload[4..];

                if list_type == b"strl" {
                    *track_counter = track_counter.saturating_add(1);
                    Self::parse_strl(
                        list_payload,
                        *track_counter,
                        video_tracks,
                        audio_tracks,
                    )?;
                }
            }

            offset = next_offset;
        }

        Ok(())
    }

    fn parse_avih(payload: &[u8]) -> Option<(u64, u32, u32, f64)> {
        if payload.len() < 40 {
            return None;
        }

        let micro_sec_per_frame =
            u32::from_le_bytes(payload[0..4].try_into().unwrap_or([0; 4])) as u64;
        let total_frames =
            u32::from_le_bytes(payload[16..20].try_into().unwrap_or([0; 4])) as u64;
        let width = u32::from_le_bytes(payload[32..36].try_into().unwrap_or([0; 4]));
        let height = u32::from_le_bytes(payload[36..40].try_into().unwrap_or([0; 4]));

        let duration_ms = if micro_sec_per_frame > 0 && total_frames > 0 {
            (total_frames.saturating_mul(micro_sec_per_frame)) / 1000
        } else {
            0
        };

        let fps = if micro_sec_per_frame > 0 {
            1_000_000.0 / (micro_sec_per_frame as f64)
        } else {
            0.0
        };

        Some((duration_ms, width, height, fps))
    }

    fn parse_strl(
        strl_payload: &[u8],
        track_id: u32,
        video_tracks: &mut Vec<VideoTrackInfo>,
        audio_tracks: &mut Vec<AudioTrackInfo>,
    ) -> VideoResult<()> {
        let mut stream_type = [0u8; 4];
        let mut stream_handler = [0u8; 4];
        let mut scale = 1u32;
        let mut rate = 0u32;
        let mut strh_found = false;

        let mut strf_payload = None;

        let mut offset = 0;
        while offset < strl_payload.len() {
            let (chunk_id, _, chunk_payload, next_offset) =
                match Self::read_chunk(strl_payload, offset) {
                    Some(c) => c,
                    None => break,
                };

            if &chunk_id == b"strh" && chunk_payload.len() >= 48 {
                stream_type.copy_from_slice(&chunk_payload[0..4]);
                stream_handler.copy_from_slice(&chunk_payload[4..8]);
                scale = u32::from_le_bytes(chunk_payload[20..24].try_into().unwrap_or(1u32.to_le_bytes()));
                rate = u32::from_le_bytes(chunk_payload[24..28].try_into().unwrap_or([0; 4]));

                strh_found = true;
            } else if &chunk_id == b"strf" {
                strf_payload = Some(chunk_payload);
            }

            offset = next_offset;
        }

        if !strh_found {
            return Ok(());
        }

        let stream_fps = if scale > 0 && rate > 0 {
            rate as f64 / scale as f64
        } else {
            0.0
        };

        match &stream_type {
            b"vids" => {
                let mut codec = VideoCodec::from_fourcc(&stream_handler);
                let mut width = 0u32;
                let mut height = 0u32;

                if let Some(strf) = strf_payload {
                    if strf.len() >= 40 {
                        let bi_w = i32::from_le_bytes(strf[4..8].try_into().unwrap_or([0; 4]));
                        let bi_h = i32::from_le_bytes(strf[8..12].try_into().unwrap_or([0; 4]));
                        let bi_comp: [u8; 4] = strf[16..20].try_into().unwrap_or([0; 4]);

                        width = bi_w.unsigned_abs();
                        height = bi_h.unsigned_abs();

                        if &bi_comp != b"\0\0\0\0" {
                            let strf_codec = VideoCodec::from_fourcc(&bi_comp);
                            if !matches!(strf_codec, VideoCodec::Unknown(_))
                                || matches!(codec, VideoCodec::Unknown(_))
                            {
                                codec = strf_codec;
                            }
                        }
                    }
                }

                video_tracks.push(VideoTrackInfo::new(
                    track_id, codec, width, height, stream_fps, None,
                ));
            }
            b"auds" => {
                let mut codec = AudioCodec::Unknown("unknown".to_string());
                let mut channels = 2u32;
                let mut sample_rate = 44100u32;

                if let Some(strf) = strf_payload {
                    if strf.len() >= 16 {
                        let w_format_tag =
                            u16::from_le_bytes(strf[0..2].try_into().unwrap_or(0u16.to_le_bytes()));
                        let n_channels =
                            u16::from_le_bytes(strf[2..4].try_into().unwrap_or(2u16.to_le_bytes())) as u32;
                        let n_samples_per_sec =
                            u32::from_le_bytes(strf[4..8].try_into().unwrap_or(44100u32.to_le_bytes()));

                        codec = AudioCodec::from_avi_tag(w_format_tag);
                        channels = n_channels;
                        sample_rate = n_samples_per_sec;
                    }
                }

                audio_tracks.push(AudioTrackInfo::new(
                    track_id,
                    codec,
                    channels,
                    sample_rate,
                    None,
                ));
            }
            _ => {}
        }

        Ok(())
    }

    fn read_chunk(data: &[u8], offset: usize) -> Option<([u8; 4], usize, &[u8], usize)> {
        if offset + 8 > data.len() {
            return None;
        }

        let chunk_id: [u8; 4] = data[offset..offset + 4].try_into().ok()?;
        let chunk_size =
            u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?) as usize;

        let payload_start = offset + 8;
        let payload_end = payload_start.saturating_add(chunk_size).min(data.len());
        let payload = &data[payload_start..payload_end];

        // RIFF chunks are padded to 2-byte boundary
        let padded_size = (chunk_size.saturating_add(1)) & !1;
        let next_offset = offset.saturating_add(8).saturating_add(padded_size).min(data.len());

        Some((chunk_id, chunk_size, payload, next_offset))
    }
}
