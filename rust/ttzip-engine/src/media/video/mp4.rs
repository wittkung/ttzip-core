// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust ISO Base Media File Format (MP4 / QuickTime MOV / M4V) demuxer.
//!
//! Provides zero-unsafe, bounds-checked container parsing, track descriptor extraction,
//! timescale-accurate timeline normalization, and embedded cover art extraction.

use super::types::{
    AudioCodec, AudioTrackInfo, ChapterInfo, SubtitleTrackInfo, VideoCodec, VideoError, VideoFormat,
    VideoMetadata, VideoResult, VideoTrackInfo,
};

/// Pure Safe Rust ISO Base Media (MP4 / QuickTime MOV) container demuxer.
pub struct TTZipMp4Demuxer<'a> {
    data: &'a [u8],
}

#[derive(Debug, Clone)]
struct Atom<'a> {
    fourcc: [u8; 4],
    payload: &'a [u8],
}

impl<'a> TTZipMp4Demuxer<'a> {
    /// Creates a new `TTZipMp4Demuxer` over an in-memory byte slice.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Sniffs the container format from the initial atoms (e.g. `ftyp` major brand).
    #[must_use]
    pub fn probe_format(data: &[u8]) -> VideoFormat {
        if data.len() < 8 {
            return VideoFormat::Unknown;
        }

        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(data, offset) {
            if &atom.fourcc == b"ftyp" {
                if atom.payload.len() >= 4 {
                    let brand = &atom.payload[0..4];
                    return match brand {
                        b"qt  " | b"moov" => VideoFormat::Mov,
                        b"isom" | b"iso2" | b"mp41" | b"mp42" | b"M4V " | b"M4A " | b"dash"
                        | b"MSNV" | b"avc1" => VideoFormat::Mp4,
                        _ => VideoFormat::Mp4,
                    };
                }
                return VideoFormat::Mp4;
            } else if &atom.fourcc == b"moov" || &atom.fourcc == b"wide" || &atom.fourcc == b"mdat" {
                return VideoFormat::Mp4;
            }
            offset = next_offset;
            if offset >= data.len() || offset >= 4096 {
                break;
            }
        }

        VideoFormat::Unknown
    }

    /// Demuxes the MP4 container and returns normalized metadata.
    pub fn demux(&self) -> VideoResult<VideoMetadata> {
        let format = match Self::probe_format(self.data) {
            VideoFormat::Mov => VideoFormat::Mov,
            _ => VideoFormat::Mp4,
        };

        let moov_payload = self.find_moov_payload()?;
        let (duration_ms, chapters_from_udta) = self.parse_mvhd_and_udta(moov_payload)?;

        let mut video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();
        let mut subtitle_tracks = Vec::new();

        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(moov_payload, offset) {
            if &atom.fourcc == b"trak" {
                self.parse_trak(
                    atom.payload,
                    &mut video_tracks,
                    &mut audio_tracks,
                    &mut subtitle_tracks,
                )?;
            }
            offset = next_offset;
        }

        let has_cover = self.extract_cover().is_some();

        Ok(VideoMetadata {
            format,
            duration_ms,
            video_tracks,
            audio_tracks,
            subtitle_tracks,
            chapters: chapters_from_udta,
            has_cover,
        })
    }

    /// Extracts embedded cover artwork payload bytes from `moov.udta.meta.ilst.covr`.
    #[must_use]
    pub fn extract_cover(&self) -> Option<Vec<u8>> {
        let moov_payload = self.find_moov_payload().ok()?;
        let udta = Self::find_child_atom(moov_payload, b"udta")?;
        let meta = Self::find_child_atom(udta, b"meta")?;

        let ilst_payload = if meta.len() >= 4 && meta[0] == 0 {
            // FullBox with 4 bytes (version + flags)
            &meta[4..]
        } else {
            meta
        };

        let ilst = Self::find_child_atom(ilst_payload, b"ilst")?;
        let covr = Self::find_child_atom(ilst, b"covr")?;
        let data = Self::find_child_atom(covr, b"data")?;

        if data.len() > 8 {
            // data box starts with 4 bytes type/flags (0x0000000D for JPEG, 0x0000000E for PNG) + 4 bytes locale
            Some(data[8..].to_vec())
        } else {
            None
        }
    }

    fn find_moov_payload(&self) -> VideoResult<&'a [u8]> {
        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(self.data, offset) {
            if &atom.fourcc == b"moov" {
                return Ok(atom.payload);
            }
            offset = next_offset;
        }
        Err(VideoError::InvalidData("Missing 'moov' atom in MP4/MOV container".to_string()))
    }

    fn parse_mvhd_and_udta(&self, moov_payload: &[u8]) -> VideoResult<(u64, Vec<ChapterInfo>)> {
        let mut duration_ms = 0;
        let mut chapters = Vec::new();

        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(moov_payload, offset) {
            if &atom.fourcc == b"mvhd" {
                duration_ms = Self::parse_mvhd(atom.payload)?;
            } else if &atom.fourcc == b"udta" {
                chapters = Self::parse_udta_chapters(atom.payload);
            }
            offset = next_offset;
        }

        Ok((duration_ms, chapters))
    }

    fn parse_mvhd(payload: &[u8]) -> VideoResult<u64> {
        if payload.len() < 20 {
            return Ok(0);
        }
        let version = payload[0];
        if version == 1 {
            if payload.len() < 32 {
                return Ok(0);
            }
            let timescale = u32::from_be_bytes(payload[20..24].try_into().unwrap_or([0; 4]));
            let duration = u64::from_be_bytes(payload[24..32].try_into().unwrap_or([0; 8]));
            if timescale > 0 {
                Ok((duration.saturating_mul(1000)) / (timescale as u64))
            } else {
                Ok(0)
            }
        } else {
            let timescale = u32::from_be_bytes(payload[12..16].try_into().unwrap_or([0; 4]));
            let duration = u32::from_be_bytes(payload[16..20].try_into().unwrap_or([0; 4]));
            if timescale > 0 {
                Ok(((duration as u64).saturating_mul(1000)) / (timescale as u64))
            } else {
                Ok(0)
            }
        }
    }

    fn parse_trak(
        &self,
        trak_payload: &[u8],
        video_tracks: &mut Vec<VideoTrackInfo>,
        audio_tracks: &mut Vec<AudioTrackInfo>,
        subtitle_tracks: &mut Vec<SubtitleTrackInfo>,
    ) -> VideoResult<()> {
        let mut track_id = 0u32;
        let mut tkhd_width = 0u32;
        let mut tkhd_height = 0u32;
        let mut handler_type = [0u8; 4];
        let mut media_timescale = 0u32;
        let mut media_duration = 0u64;
        let mut language = None;

        let mut sample_entries = Vec::new();
        let mut total_samples = 0u64;
        let mut total_sample_duration = 0u64;
        let mut total_track_bytes = 0u64;

        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(trak_payload, offset) {
            if &atom.fourcc == b"tkhd" {
                if let Some((id, w, h)) = Self::parse_tkhd(atom.payload) {
                    track_id = id;
                    tkhd_width = w;
                    tkhd_height = h;
                }
            } else if &atom.fourcc == b"mdia" {
                Self::parse_mdia(
                    atom.payload,
                    &mut handler_type,
                    &mut media_timescale,
                    &mut media_duration,
                    &mut language,
                    &mut sample_entries,
                    &mut total_samples,
                    &mut total_sample_duration,
                    &mut total_track_bytes,
                );
            }
            offset = next_offset;
        }

        match &handler_type {
            b"vide" => {
                let (codec, entry_w, entry_h) = if let Some(first) = sample_entries.first() {
                    let c = VideoCodec::from_fourcc(&first.fourcc);
                    let (w, h) = Self::parse_visual_sample_entry(&first.payload);
                    (c, w, h)
                } else {
                    (VideoCodec::Unknown("unknown".to_string()), 0, 0)
                };

                let width = if tkhd_width > 0 { tkhd_width } else { entry_w };
                let height = if tkhd_height > 0 { tkhd_height } else { entry_h };

                let fps = if media_timescale > 0 && total_sample_duration > 0 {
                    (total_samples as f64 * media_timescale as f64)
                        / (total_sample_duration as f64)
                } else if media_timescale > 0 && media_duration > 0 && total_samples > 0 {
                    (total_samples as f64 * media_timescale as f64) / (media_duration as f64)
                } else {
                    0.0
                };

                let bitrate_kbps = if media_timescale > 0 && media_duration > 0 && total_track_bytes > 0 {
                    let duration_sec = media_duration as f64 / media_timescale as f64;
                    if duration_sec > 0.0 {
                        Some(((total_track_bytes as f64 * 8.0) / (duration_sec * 1000.0)) as u32)
                    } else {
                        None
                    }
                } else {
                    None
                };

                video_tracks.push(VideoTrackInfo::new(
                    track_id,
                    codec,
                    width,
                    height,
                    fps,
                    bitrate_kbps,
                ));
            }
            b"soun" => {
                let (codec, channels, sample_rate) = if let Some(first) = sample_entries.first() {
                    let c = AudioCodec::from_mp4_fourcc(&first.fourcc);
                    let (ch, sr) = Self::parse_audio_sample_entry(&first.payload);
                    (c, ch, sr)
                } else {
                    (AudioCodec::Unknown("unknown".to_string()), 2, 44100)
                };

                audio_tracks.push(AudioTrackInfo::new(
                    track_id,
                    codec,
                    channels,
                    if sample_rate > 0 { sample_rate } else { media_timescale },
                    language,
                ));
            }
            b"subt" | b"sbtl" | b"text" | b"clcp" => {
                let format_str = if let Some(first) = sample_entries.first() {
                    String::from_utf8_lossy(&first.fourcc).trim().to_string()
                } else {
                    "text".to_string()
                };

                subtitle_tracks.push(SubtitleTrackInfo::new(
                    track_id,
                    format_str,
                    language,
                    None,
                ));
            }
            _ => {}
        }

        Ok(())
    }

    fn parse_tkhd(payload: &[u8]) -> Option<(u32, u32, u32)> {
        if payload.len() < 24 {
            return None;
        }
        let version = payload[0];
        let (track_id, width_offset) = if version == 1 {
            if payload.len() < 96 {
                return None;
            }
            let id = u32::from_be_bytes(payload[20..24].try_into().ok()?);
            (id, 88)
        } else {
            if payload.len() < 84 {
                return None;
            }
            let id = u32::from_be_bytes(payload[12..16].try_into().ok()?);
            (id, 76)
        };

        if payload.len() >= width_offset + 8 {
            let width_fp = u32::from_be_bytes(
                payload[width_offset..width_offset + 4].try_into().ok()?,
            );
            let height_fp = u32::from_be_bytes(
                payload[width_offset + 4..width_offset + 8].try_into().ok()?,
            );
            Some((track_id, width_fp >> 16, height_fp >> 16))
        } else {
            Some((track_id, 0, 0))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_mdia(
        mdia_payload: &[u8],
        handler_type: &mut [u8; 4],
        media_timescale: &mut u32,
        media_duration: &mut u64,
        language: &mut Option<String>,
        sample_entries: &mut Vec<SampleEntry>,
        total_samples: &mut u64,
        total_sample_duration: &mut u64,
        total_track_bytes: &mut u64,
    ) {
        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(mdia_payload, offset) {
            if &atom.fourcc == b"mdhd" {
                if let Some((ts, dur, lang)) = Self::parse_mdhd(atom.payload) {
                    *media_timescale = ts;
                    *media_duration = dur;
                    if let Some(l) = lang {
                        *language = Some(l);
                    }
                }
            } else if &atom.fourcc == b"hdlr" {
                if atom.payload.len() >= 12 {
                    handler_type.copy_from_slice(&atom.payload[8..12]);
                }
            } else if &atom.fourcc == b"minf" {
                Self::parse_minf(
                    atom.payload,
                    sample_entries,
                    total_samples,
                    total_sample_duration,
                    total_track_bytes,
                );
            }
            offset = next_offset;
        }
    }

    fn parse_mdhd(payload: &[u8]) -> Option<(u32, u64, Option<String>)> {
        if payload.len() < 24 {
            return None;
        }
        let version = payload[0];
        let (timescale, duration, lang_code) = if version == 1 {
            if payload.len() < 36 {
                return None;
            }
            let ts = u32::from_be_bytes(payload[20..24].try_into().ok()?);
            let dur = u64::from_be_bytes(payload[24..32].try_into().ok()?);
            let lang = u16::from_be_bytes(payload[32..34].try_into().ok()?);
            (ts, dur, lang)
        } else {
            let ts = u32::from_be_bytes(payload[12..16].try_into().ok()?);
            let dur = u32::from_be_bytes(payload[16..20].try_into().ok()?);
            let lang = u16::from_be_bytes(payload[20..22].try_into().ok()?);
            (ts, dur as u64, lang)
        };

        let lang_str = Self::decode_iso639_2_language(lang_code);
        Some((timescale, duration, lang_str))
    }

    fn decode_iso639_2_language(code: u16) -> Option<String> {
        let c1 = ((code >> 10) & 0x1F) as u8 + 0x60;
        let c2 = ((code >> 5) & 0x1F) as u8 + 0x60;
        let c3 = (code & 0x1F) as u8 + 0x60;

        if c1.is_ascii_lowercase() && c2.is_ascii_lowercase() && c3.is_ascii_lowercase() {
            Some(format!("{}{}{}", c1 as char, c2 as char, c3 as char))
        } else {
            None
        }
    }

    fn parse_minf(
        minf_payload: &[u8],
        sample_entries: &mut Vec<SampleEntry>,
        total_samples: &mut u64,
        total_sample_duration: &mut u64,
        total_track_bytes: &mut u64,
    ) {
        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(minf_payload, offset) {
            if &atom.fourcc == b"stbl" {
                Self::parse_stbl(
                    atom.payload,
                    sample_entries,
                    total_samples,
                    total_sample_duration,
                    total_track_bytes,
                );
            }
            offset = next_offset;
        }
    }

    fn parse_stbl(
        stbl_payload: &[u8],
        sample_entries: &mut Vec<SampleEntry>,
        total_samples: &mut u64,
        total_sample_duration: &mut u64,
        total_track_bytes: &mut u64,
    ) {
        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(stbl_payload, offset) {
            if &atom.fourcc == b"stsd" {
                Self::parse_stsd(atom.payload, sample_entries);
            } else if &atom.fourcc == b"stts" {
                let (samples, duration) = Self::parse_stts(atom.payload);
                *total_samples = samples;
                *total_sample_duration = duration;
            } else if &atom.fourcc == b"stsz" {
                *total_track_bytes = Self::parse_stsz(atom.payload);
            }
            offset = next_offset;
        }
    }

    fn parse_stsd(payload: &[u8], sample_entries: &mut Vec<SampleEntry>) {
        if payload.len() < 8 {
            return;
        }
        let entry_count = u32::from_be_bytes(payload[4..8].try_into().unwrap_or([0; 4]));
        let mut offset = 8;

        for _ in 0..entry_count {
            if offset + 8 > payload.len() {
                break;
            }
            let entry_len =
                u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap_or([0; 4])) as usize;
            if entry_len < 8 || offset + entry_len > payload.len() {
                break;
            }
            let fourcc: [u8; 4] = payload[offset + 4..offset + 8]
                .try_into()
                .unwrap_or([0; 4]);
            let entry_payload = &payload[offset + 8..offset + entry_len];

            sample_entries.push(SampleEntry {
                fourcc,
                payload: entry_payload.to_vec(),
            });

            offset += entry_len;
        }
    }

    fn parse_stts(payload: &[u8]) -> (u64, u64) {
        if payload.len() < 8 {
            return (0, 0);
        }
        let entry_count = u32::from_be_bytes(payload[4..8].try_into().unwrap_or([0; 4])) as usize;
        let mut total_samples = 0u64;
        let mut total_duration = 0u64;
        let mut offset = 8;

        for _ in 0..entry_count {
            if offset + 8 > payload.len() {
                break;
            }
            let count = u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap_or([0; 4]))
                as u64;
            let delta = u32::from_be_bytes(
                payload[offset + 4..offset + 8].try_into().unwrap_or([0; 4]),
            ) as u64;

            total_samples = total_samples.saturating_add(count);
            total_duration = total_duration.saturating_add(count.saturating_mul(delta));
            offset += 8;
        }

        (total_samples, total_duration)
    }

    fn parse_stsz(payload: &[u8]) -> u64 {
        if payload.len() < 12 {
            return 0;
        }
        let sample_size =
            u32::from_be_bytes(payload[4..8].try_into().unwrap_or([0; 4])) as u64;
        let sample_count =
            u32::from_be_bytes(payload[8..12].try_into().unwrap_or([0; 4])) as u64;

        if sample_size > 0 {
            sample_size.saturating_mul(sample_count)
        } else {
            let mut total = 0u64;
            let mut offset = 12;
            for _ in 0..sample_count {
                if offset + 4 > payload.len() {
                    break;
                }
                let sz = u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap_or([0; 4]))
                    as u64;
                total = total.saturating_add(sz);
                offset += 4;
            }
            total
        }
    }

    fn parse_visual_sample_entry(payload: &[u8]) -> (u32, u32) {
        if payload.len() >= 28 {
            // VisualSampleEntry has 6 reserved + 2 data ref index + 16 pre-defined + width (u16) + height (u16)
            let width = u16::from_be_bytes(payload[24..26].try_into().unwrap_or([0; 2])) as u32;
            let height = u16::from_be_bytes(payload[26..28].try_into().unwrap_or([0; 2])) as u32;
            (width, height)
        } else {
            (0, 0)
        }
    }

    fn parse_audio_sample_entry(payload: &[u8]) -> (u32, u32) {
        if payload.len() >= 24 {
            // AudioSampleEntry has 6 reserved + 2 data ref index + 8 reserved + channel_count (u16) + sample_size (u16) + pre-defined (u16) + reserved (u16) + sample_rate (u16.u16)
            let channels = u16::from_be_bytes(payload[16..18].try_into().unwrap_or([0; 2])) as u32;
            let sample_rate =
                u16::from_be_bytes(payload[22..24].try_into().unwrap_or([0; 2])) as u32;
            (channels, sample_rate)
        } else {
            (2, 44100)
        }
    }

    fn parse_udta_chapters(udta_payload: &[u8]) -> Vec<ChapterInfo> {
        let mut chapters = Vec::new();
        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(udta_payload, offset) {
            if &atom.fourcc == b"chpl" && atom.payload.len() >= 5 {
                // Chapter list atom
                let chapter_count = atom.payload[4] as usize;
                let mut ch_offset = 5;
                for _ in 0..chapter_count {
                    if ch_offset + 9 > atom.payload.len() {
                        break;
                    }
                    let start_ts = u64::from_be_bytes(
                        atom.payload[ch_offset..ch_offset + 8]
                            .try_into()
                            .unwrap_or([0; 8]),
                    );
                    let title_len = atom.payload[ch_offset + 8] as usize;
                    ch_offset += 9;
                    if ch_offset + title_len > atom.payload.len() {
                        break;
                    }
                    let title = String::from_utf8_lossy(
                        &atom.payload[ch_offset..ch_offset + title_len],
                    )
                    .to_string();
                    ch_offset += title_len;

                    // Standard timescale for chpl is 10,000,000 (100ns units)
                    let start_ms = start_ts / 10000;
                    chapters.push(ChapterInfo::new(start_ms, 0, title));
                }
            }
            offset = next_offset;
        }

        // Fill in end_ms from next chapter start
        let len = chapters.len();
        for i in 0..len {
            if i + 1 < len {
                let next_start = chapters[i + 1].start_ms;
                chapters[i].end_ms = next_start;
            }
        }

        chapters
    }

    fn next_atom(data: &[u8], offset: usize) -> Option<(Atom<'_>, usize)> {
        if offset + 8 > data.len() {
            return None;
        }

        let len_u32 = u32::from_be_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
        let fourcc: [u8; 4] = data[offset + 4..offset + 8].try_into().ok()?;

        let (header_len, total_len) = if len_u32 == 1 {
            // Extended 64-bit size
            if offset + 16 > data.len() {
                return None;
            }
            let len_u64 =
                u64::from_be_bytes(data[offset + 8..offset + 16].try_into().ok()?) as usize;
            if len_u64 < 16 {
                return None;
            }
            (16, len_u64)
        } else if len_u32 == 0 {
            // Extends to end of data
            (8, data.len().saturating_sub(offset))
        } else {
            if len_u32 < 8 {
                return None;
            }
            (8, len_u32)
        };

        let atom_end = offset.checked_add(total_len)?;
        let clamped_end = atom_end.min(data.len());
        let payload_start = offset.checked_add(header_len)?;

        if payload_start > clamped_end {
            return None;
        }

        let payload = &data[payload_start..clamped_end];
        Some((Atom { fourcc, payload }, clamped_end))
    }

    fn find_child_atom<'b>(parent_payload: &'b [u8], target_fourcc: &[u8; 4]) -> Option<&'b [u8]> {
        let mut offset = 0;
        while let Some((atom, next_offset)) = Self::next_atom(parent_payload, offset) {
            if &atom.fourcc == target_fourcc {
                return Some(atom.payload);
            }
            offset = next_offset;
        }
        None
    }
}

#[derive(Debug, Clone)]
struct SampleEntry {
    fourcc: [u8; 4],
    payload: Vec<u8>,
}
