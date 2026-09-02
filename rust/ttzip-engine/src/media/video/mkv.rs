// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Matroska (MKV) & WebM EBML container demuxer.
//!
//! Provides variable-size integer (VINT) stream parsing, track descriptor extraction,
//! millisecond-accurate timeline normalization, chapter decoding, and attachment cover extraction.

use super::types::{
    AudioCodec, AudioTrackInfo, ChapterInfo, SubtitleTrackInfo, VideoCodec, VideoError, VideoFormat,
    VideoMetadata, VideoResult, VideoTrackInfo,
};

/// Pure Safe Rust Matroska / WebM EBML container demuxer.
pub struct TTZipMkvDemuxer<'a> {
    data: &'a [u8],
}

// EBML Top-level and Segment Element IDs
const EBML_HEADER_ID: u32 = 0x1A45_DFA3;
const SEGMENT_ID: u32 = 0x1853_8067;
const SEGMENT_INFO_ID: u32 = 0x1549_A966;
const TRACKS_ID: u32 = 0x1654_AE6B;
const CHAPTERS_ID: u32 = 0x1043_A770;
const ATTACHMENTS_ID: u32 = 0x1941_A469;

// EBML Header Children
const DOC_TYPE_ID: u32 = 0x4282;

// Info Children
const TIMECODE_SCALE_ID: u32 = 0x2A_D7B1;
const DURATION_ID: u32 = 0x4489;

// Tracks Children
const TRACK_ENTRY_ID: u32 = 0xAE;
const TRACK_NUMBER_ID: u32 = 0xD7;
const TRACK_TYPE_ID: u32 = 0x83;
const CODEC_ID: u32 = 0x86;
const LANGUAGE_ID: u32 = 0x22_B59C;
const LANGUAGE_BCP47_ID: u32 = 0x22_B59D;
const TRACK_NAME_ID: u32 = 0x536E;
const DEFAULT_DURATION_ID: u32 = 0x23_E383;

// Video Element Children
const VIDEO_SETTINGS_ID: u32 = 0xE0;
const PIXEL_WIDTH_ID: u32 = 0xB0;
const PIXEL_HEIGHT_ID: u32 = 0xBA;

// Audio Element Children
const AUDIO_SETTINGS_ID: u32 = 0xE1;
const SAMPLING_FREQ_ID: u32 = 0xB5;
const CHANNELS_ID: u32 = 0x9F;

// Chapters Children
const EDITION_ENTRY_ID: u32 = 0x45B9;
const CHAPTER_ATOM_ID: u32 = 0xB6;
const CHAPTER_TIME_START_ID: u32 = 0x91;
const CHAPTER_TIME_END_ID: u32 = 0x92;
const CHAPTER_DISPLAY_ID: u32 = 0x80;
const CHAP_STRING_ID: u32 = 0x85;

// Attachments Children
const ATTACHED_FILE_ID: u32 = 0x61A7;
const FILE_NAME_ID: u32 = 0x466E;
const FILE_MIME_TYPE_ID: u32 = 0x4660;
const FILE_DATA_ID: u32 = 0x465C;

impl<'a> TTZipMkvDemuxer<'a> {
    /// Creates a new `TTZipMkvDemuxer` over an in-memory byte slice.
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Sniffs whether the buffer begins with an EBML Header and extracts `DocType`.
    #[must_use]
    pub fn probe_format(data: &[u8]) -> VideoFormat {
        if data.len() < 4 {
            return VideoFormat::Unknown;
        }

        let mut offset = 0;
        if let Ok(Some((id, size))) = Self::read_element_header(data, &mut offset) {
            if id == EBML_HEADER_ID {
                let end = offset.saturating_add(size as usize).min(data.len());
                let header_slice = &data[offset..end];
                let mut h_offset = 0;
                while let Ok(Some((child_id, child_size))) =
                    Self::read_element_header(header_slice, &mut h_offset)
                {
                    let child_end =
                        h_offset.saturating_add(child_size as usize).min(header_slice.len());
                    if child_id == DOC_TYPE_ID {
                        let doc_type =
                            String::from_utf8_lossy(&header_slice[h_offset..child_end]);
                        if doc_type.contains("webm") {
                            return VideoFormat::Webm;
                        } else if doc_type.contains("matroska") {
                            return VideoFormat::Mkv;
                        }
                    }
                    h_offset = child_end;
                }
                return VideoFormat::Mkv;
            }
        }

        VideoFormat::Unknown
    }

    /// Demuxes the Matroska / WebM container and returns normalized metadata.
    pub fn demux(&self) -> VideoResult<VideoMetadata> {
        let mut offset = 0;
        let mut format = VideoFormat::Mkv;

        // 1. Parse EBML Header
        let (id, size) = Self::read_element_header(self.data, &mut offset)?
            .ok_or_else(|| VideoError::InvalidData("Empty or truncated EBML buffer".to_string()))?;

        if id != EBML_HEADER_ID {
            return Err(VideoError::InvalidData(format!(
                "Invalid EBML magic ID: 0x{id:08X}"
            )));
        }

        let header_end = offset.saturating_add(size as usize).min(self.data.len());
        let header_slice = &self.data[offset..header_end];
        let mut h_offset = 0;
        while let Some((child_id, child_size)) =
            Self::read_element_header(header_slice, &mut h_offset)?
        {
            let child_end =
                h_offset.saturating_add(child_size as usize).min(header_slice.len());
            if child_id == DOC_TYPE_ID {
                let doc_type = String::from_utf8_lossy(&header_slice[h_offset..child_end]);
                if doc_type.contains("webm") {
                    format = VideoFormat::Webm;
                } else {
                    format = VideoFormat::Mkv;
                }
            }
            h_offset = child_end;
        }

        offset = header_end;

        // 2. Locate Segment Element
        let mut duration_ms = 0u64;
        let mut video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();
        let mut subtitle_tracks = Vec::new();
        let mut chapters = Vec::new();
        let mut has_cover = false;

        while offset < self.data.len() {
            let (elem_id, elem_size) = match Self::read_element_header(self.data, &mut offset)? {
                Some(header) => header,
                None => break,
            };

            if elem_id == SEGMENT_ID {
                let segment_end = if elem_size == u64::MAX {
                    self.data.len()
                } else {
                    offset.saturating_add(elem_size as usize).min(self.data.len())
                };

                let mut seg_offset = offset;
                let mut timecode_scale = 1_000_000u64; // Default 1ms in nanoseconds

                while seg_offset < segment_end {
                    let (sub_id, sub_size) =
                        match Self::read_element_header(self.data, &mut seg_offset)? {
                            Some(sub) => sub,
                            None => break,
                        };

                    let sub_end = if sub_size == u64::MAX {
                        segment_end
                    } else {
                        seg_offset.saturating_add(sub_size as usize).min(segment_end)
                    };
                    let sub_slice = &self.data[seg_offset..sub_end];

                    match sub_id {
                        SEGMENT_INFO_ID => {
                            let (dur, scale) = Self::parse_segment_info(sub_slice)?;
                            timecode_scale = scale;
                            duration_ms = dur;
                        }
                        TRACKS_ID => {
                            Self::parse_tracks(
                                sub_slice,
                                timecode_scale,
                                &mut video_tracks,
                                &mut audio_tracks,
                                &mut subtitle_tracks,
                            )?;
                        }
                        CHAPTERS_ID => {
                            chapters = Self::parse_chapters(sub_slice)?;
                        }
                        ATTACHMENTS_ID => {
                            if Self::extract_cover_from_attachments(sub_slice).is_some() {
                                has_cover = true;
                            }
                        }
                        _ => {}
                    }

                    seg_offset = sub_end;
                }
                break;
            } else {
                offset = offset.saturating_add(elem_size as usize).min(self.data.len());
            }
        }

        Ok(VideoMetadata {
            format,
            duration_ms,
            video_tracks,
            audio_tracks,
            subtitle_tracks,
            chapters,
            has_cover,
        })
    }

    /// Extracts embedded cover artwork payload bytes from Matroska Attachments.
    #[must_use]
    pub fn extract_cover(&self) -> Option<Vec<u8>> {
        let mut offset = 0;
        let (id, size) = Self::read_element_header(self.data, &mut offset).ok()??;
        if id != EBML_HEADER_ID {
            return None;
        }
        offset = offset.saturating_add(size as usize).min(self.data.len());

        while offset < self.data.len() {
            let (elem_id, elem_size) =
                Self::read_element_header(self.data, &mut offset).ok()??;
            if elem_id == SEGMENT_ID {
                let segment_end = if elem_size == u64::MAX {
                    self.data.len()
                } else {
                    offset.saturating_add(elem_size as usize).min(self.data.len())
                };

                let mut seg_offset = offset;
                while seg_offset < segment_end {
                    let (sub_id, sub_size) =
                        Self::read_element_header(self.data, &mut seg_offset).ok()??;
                    let sub_end = if sub_size == u64::MAX {
                        segment_end
                    } else {
                        seg_offset.saturating_add(sub_size as usize).min(segment_end)
                    };

                    if sub_id == ATTACHMENTS_ID {
                        let sub_slice = &self.data[seg_offset..sub_end];
                        return Self::extract_cover_from_attachments(sub_slice);
                    }
                    seg_offset = sub_end;
                }
                break;
            } else {
                offset = offset.saturating_add(elem_size as usize).min(self.data.len());
            }
        }
        None
    }

    fn parse_segment_info(info_slice: &[u8]) -> VideoResult<(u64, u64)> {
        let mut offset = 0;
        let mut timecode_scale = 1_000_000u64;
        let mut raw_duration = 0.0f64;

        while offset < info_slice.len() {
            let (id, size) = match Self::read_element_header(info_slice, &mut offset)? {
                Some(header) => header,
                None => break,
            };

            let end = offset.saturating_add(size as usize).min(info_slice.len());
            let payload = &info_slice[offset..end];

            if id == TIMECODE_SCALE_ID {
                timecode_scale = Self::parse_uint(payload).unwrap_or(1_000_000);
            } else if id == DURATION_ID {
                raw_duration = Self::parse_float(payload).unwrap_or(0.0);
            }

            offset = end;
        }

        let duration_ms = if raw_duration > 0.0 {
            ((raw_duration * (timecode_scale as f64)) / 1_000_000.0) as u64
        } else {
            0
        };

        Ok((duration_ms, timecode_scale))
    }

    fn parse_tracks(
        tracks_slice: &[u8],
        _timecode_scale: u64,
        video_tracks: &mut Vec<VideoTrackInfo>,
        audio_tracks: &mut Vec<AudioTrackInfo>,
        subtitle_tracks: &mut Vec<SubtitleTrackInfo>,
    ) -> VideoResult<()> {
        let mut offset = 0;
        while offset < tracks_slice.len() {
            let (id, size) = match Self::read_element_header(tracks_slice, &mut offset)? {
                Some(header) => header,
                None => break,
            };

            let end = offset.saturating_add(size as usize).min(tracks_slice.len());
            let payload = &tracks_slice[offset..end];

            if id == TRACK_ENTRY_ID {
                Self::parse_track_entry(
                    payload,
                    video_tracks,
                    audio_tracks,
                    subtitle_tracks,
                )?;
            }

            offset = end;
        }
        Ok(())
    }

    fn parse_track_entry(
        entry_slice: &[u8],
        video_tracks: &mut Vec<VideoTrackInfo>,
        audio_tracks: &mut Vec<AudioTrackInfo>,
        subtitle_tracks: &mut Vec<SubtitleTrackInfo>,
    ) -> VideoResult<()> {
        let mut offset = 0;
        let mut track_id = 0u32;
        let mut track_type = 0u64;
        let mut codec_id = String::new();
        let mut language = None;
        let mut track_name = None;
        let mut default_duration_ns = 0u64;

        let mut pixel_width = 0u32;
        let mut pixel_height = 0u32;

        let mut channels = 2u32;
        let mut sample_rate = 44100u32;

        while offset < entry_slice.len() {
            let (id, size) = match Self::read_element_header(entry_slice, &mut offset)? {
                Some(header) => header,
                None => break,
            };

            let end = offset.saturating_add(size as usize).min(entry_slice.len());
            let payload = &entry_slice[offset..end];

            match id {
                TRACK_NUMBER_ID => {
                    track_id = Self::parse_uint(payload).unwrap_or(0) as u32;
                }
                TRACK_TYPE_ID => {
                    track_type = Self::parse_uint(payload).unwrap_or(0);
                }
                CODEC_ID => {
                    codec_id = String::from_utf8_lossy(payload).trim().to_string();
                }
                LANGUAGE_ID | LANGUAGE_BCP47_ID => {
                    language = Some(String::from_utf8_lossy(payload).trim().to_string());
                }
                TRACK_NAME_ID => {
                    track_name = Some(String::from_utf8_lossy(payload).trim().to_string());
                }
                DEFAULT_DURATION_ID => {
                    default_duration_ns = Self::parse_uint(payload).unwrap_or(0);
                }
                VIDEO_SETTINGS_ID => {
                    let mut v_offset = 0;
                    while v_offset < payload.len() {
                        if let Some((v_id, v_sz)) =
                            Self::read_element_header(payload, &mut v_offset)?
                        {
                            let v_end =
                                v_offset.saturating_add(v_sz as usize).min(payload.len());
                            let v_pay = &payload[v_offset..v_end];
                            if v_id == PIXEL_WIDTH_ID {
                                pixel_width = Self::parse_uint(v_pay).unwrap_or(0) as u32;
                            } else if v_id == PIXEL_HEIGHT_ID {
                                pixel_height = Self::parse_uint(v_pay).unwrap_or(0) as u32;
                            }
                            v_offset = v_end;
                        } else {
                            break;
                        }
                    }
                }
                AUDIO_SETTINGS_ID => {
                    let mut a_offset = 0;
                    while a_offset < payload.len() {
                        if let Some((a_id, a_sz)) =
                            Self::read_element_header(payload, &mut a_offset)?
                        {
                            let a_end =
                                a_offset.saturating_add(a_sz as usize).min(payload.len());
                            let a_pay = &payload[a_offset..a_end];
                            if a_id == CHANNELS_ID {
                                channels = Self::parse_uint(a_pay).unwrap_or(2) as u32;
                            } else if a_id == SAMPLING_FREQ_ID {
                                sample_rate =
                                    Self::parse_float(a_pay).unwrap_or(44100.0) as u32;
                            }
                            a_offset = a_end;
                        } else {
                            break;
                        }
                    }
                }
                _ => {}
            }

            offset = end;
        }

        match track_type {
            1 => {
                // Video
                let codec = VideoCodec::from_mkv_codec_id(&codec_id);
                let fps = if default_duration_ns > 0 {
                    1_000_000_000.0 / (default_duration_ns as f64)
                } else {
                    0.0
                };

                video_tracks.push(VideoTrackInfo::new(
                    track_id,
                    codec,
                    pixel_width,
                    pixel_height,
                    fps,
                    None,
                ));
            }
            2 => {
                // Audio
                let codec = AudioCodec::from_mkv_codec_id(&codec_id);
                audio_tracks.push(AudioTrackInfo::new(
                    track_id,
                    codec,
                    channels,
                    sample_rate,
                    language,
                ));
            }
            17 => {
                // Subtitle (TrackType = 17 / 0x11)
                let format_str = if codec_id.is_empty() {
                    "subtitle".to_string()
                } else {
                    codec_id
                };
                subtitle_tracks.push(SubtitleTrackInfo::new(
                    track_id,
                    format_str,
                    language,
                    track_name,
                ));
            }
            _ => {}
        }

        Ok(())
    }

    fn parse_chapters(chapters_slice: &[u8]) -> VideoResult<Vec<ChapterInfo>> {
        let mut chapters = Vec::new();
        let mut offset = 0;

        while offset < chapters_slice.len() {
            let (id, size) = match Self::read_element_header(chapters_slice, &mut offset)? {
                Some(header) => header,
                None => break,
            };

            let end = offset.saturating_add(size as usize).min(chapters_slice.len());
            let payload = &chapters_slice[offset..end];

            if id == EDITION_ENTRY_ID {
                let mut e_offset = 0;
                while e_offset < payload.len() {
                    let (atom_id, atom_size) =
                        match Self::read_element_header(payload, &mut e_offset)? {
                            Some(h) => h,
                            None => break,
                        };
                    let atom_end =
                        e_offset.saturating_add(atom_size as usize).min(payload.len());
                    let atom_payload = &payload[e_offset..atom_end];

                    if atom_id == CHAPTER_ATOM_ID {
                        if let Some(ch) = Self::parse_chapter_atom(atom_payload)? {
                            chapters.push(ch);
                        }
                    }
                    e_offset = atom_end;
                }
            }

            offset = end;
        }

        Ok(chapters)
    }

    fn parse_chapter_atom(atom_payload: &[u8]) -> VideoResult<Option<ChapterInfo>> {
        let mut offset = 0;
        let mut start_ms = 0u64;
        let mut end_ms = 0u64;
        let mut title = String::new();

        while offset < atom_payload.len() {
            let (id, size) = match Self::read_element_header(atom_payload, &mut offset)? {
                Some(header) => header,
                None => break,
            };

            let end = offset.saturating_add(size as usize).min(atom_payload.len());
            let payload = &atom_payload[offset..end];

            match id {
                CHAPTER_TIME_START_ID => {
                    let ns = Self::parse_uint(payload).unwrap_or(0);
                    start_ms = ns / 1_000_000;
                }
                CHAPTER_TIME_END_ID => {
                    let ns = Self::parse_uint(payload).unwrap_or(0);
                    end_ms = ns / 1_000_000;
                }
                CHAPTER_DISPLAY_ID => {
                    let mut d_offset = 0;
                    while d_offset < payload.len() {
                        if let Some((d_id, d_size)) =
                            Self::read_element_header(payload, &mut d_offset)?
                        {
                            let d_end =
                                d_offset.saturating_add(d_size as usize).min(payload.len());
                            let d_payload = &payload[d_offset..d_end];
                            if d_id == CHAP_STRING_ID {
                                title =
                                    String::from_utf8_lossy(d_payload).trim().to_string();
                            }
                            d_offset = d_end;
                        } else {
                            break;
                        }
                    }
                }
                _ => {}
            }

            offset = end;
        }

        if !title.is_empty() || start_ms > 0 || end_ms > 0 {
            Ok(Some(ChapterInfo::new(start_ms, end_ms, title)))
        } else {
            Ok(None)
        }
    }

    fn extract_cover_from_attachments(attachments_slice: &[u8]) -> Option<Vec<u8>> {
        let mut offset = 0;
        while offset < attachments_slice.len() {
            let (id, size) = Self::read_element_header(attachments_slice, &mut offset).ok()??;
            let end = offset.saturating_add(size as usize).min(attachments_slice.len());
            let payload = &attachments_slice[offset..end];

            if id == ATTACHED_FILE_ID {
                let mut file_offset = 0;
                let mut file_name = String::new();
                let mut mime_type = String::new();
                let mut file_data = Vec::new();

                while file_offset < payload.len() {
                    let (f_id, f_sz) =
                        Self::read_element_header(payload, &mut file_offset).ok()??;
                    let f_end = file_offset.saturating_add(f_sz as usize).min(payload.len());
                    let f_pay = &payload[file_offset..f_end];

                    match f_id {
                        FILE_NAME_ID => {
                            file_name = String::from_utf8_lossy(f_pay).to_lowercase();
                        }
                        FILE_MIME_TYPE_ID => {
                            mime_type = String::from_utf8_lossy(f_pay).to_lowercase();
                        }
                        FILE_DATA_ID => {
                            file_data = f_pay.to_vec();
                        }
                        _ => {}
                    }
                    file_offset = f_end;
                }

                if (!file_data.is_empty())
                    && (file_name.contains("cover") || mime_type.starts_with("image/"))
                {
                    return Some(file_data);
                }
            }
            offset = end;
        }
        None
    }

    fn read_element_header(data: &[u8], offset: &mut usize) -> VideoResult<Option<(u32, u64)>> {
        if *offset >= data.len() {
            return Ok(None);
        }

        // Read element ID (VINT with length marker preserved)
        let first_byte = data[*offset];
        let id_len = if first_byte & 0x80 != 0 {
            1
        } else if first_byte & 0x40 != 0 {
            2
        } else if first_byte & 0x20 != 0 {
            3
        } else if first_byte & 0x10 != 0 {
            4
        } else {
            return Err(VideoError::InvalidData(format!(
                "Invalid EBML ID marker byte: 0x{first_byte:02X} at offset {}",
                *offset
            )));
        };

        if *offset + id_len > data.len() {
            return Ok(None);
        }

        let mut id = 0u32;
        for i in 0..id_len {
            id = (id << 8) | (data[*offset + i] as u32);
        }
        *offset += id_len;

        // Read element Size (VINT with length marker stripped)
        if *offset >= data.len() {
            return Ok(None);
        }

        let size_first = data[*offset];
        let (size_len, mask) = if size_first & 0x80 != 0 {
            (1, 0x7F)
        } else if size_first & 0x40 != 0 {
            (2, 0x3F)
        } else if size_first & 0x20 != 0 {
            (3, 0x1F)
        } else if size_first & 0x10 != 0 {
            (4, 0x0F)
        } else if size_first & 0x08 != 0 {
            (5, 0x07)
        } else if size_first & 0x04 != 0 {
            (6, 0x03)
        } else if size_first & 0x02 != 0 {
            (7, 0x01)
        } else if size_first & 0x01 != 0 {
            (8, 0x00)
        } else {
            return Err(VideoError::InvalidData(format!(
                "Invalid EBML size marker byte: 0x{size_first:02X} at offset {}",
                *offset
            )));
        };

        if *offset + size_len > data.len() {
            return Ok(None);
        }

        let mut raw_size = (size_first & mask) as u64;
        for i in 1..size_len {
            raw_size = (raw_size << 8) | (data[*offset + i] as u64);
        }

        let is_unknown_size = match size_len {
            1 => raw_size == 0x7F,
            2 => raw_size == 0x3FFF,
            3 => raw_size == 0x1F_FFFF,
            4 => raw_size == 0x0FFF_FFFF,
            5 => raw_size == 0x07_FFFF_FFFF,
            6 => raw_size == 0x03FF_FFFF_FFFF,
            7 => raw_size == 0x01_FFFF_FFFF_FFFF,
            8 => raw_size == 0x00FF_FFFF_FFFF_FFFF,
            _ => false,
        };

        let size = if is_unknown_size {
            u64::MAX
        } else {
            raw_size
        };

        *offset += size_len;
        Ok(Some((id, size)))
    }

    fn parse_uint(payload: &[u8]) -> Option<u64> {
        if payload.is_empty() || payload.len() > 8 {
            return None;
        }
        let mut val = 0u64;
        for &b in payload {
            val = (val << 8) | (b as u64);
        }
        Some(val)
    }

    fn parse_float(payload: &[u8]) -> Option<f64> {
        match payload.len() {
            4 => {
                let arr: [u8; 4] = payload.try_into().ok()?;
                Some(f32::from_be_bytes(arr) as f64)
            }
            8 => {
                let arr: [u8; 8] = payload.try_into().ok()?;
                Some(f64::from_be_bytes(arr))
            }
            _ => None,
        }
    }
}
