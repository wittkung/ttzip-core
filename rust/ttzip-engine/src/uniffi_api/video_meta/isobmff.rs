// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! ISO-BMFF (MP4 / MOV / M4V) container metadata, track topology, and cover parser.

use std::collections::HashMap;
use super::types::{
    UniFFIAudioCodec, UniFFIAudioTrackInfo, UniFFIChapterInfo, UniFFISubtitleTrackInfo,
    UniFFIVideoCodec, UniFFIVideoError, UniFFIVideoFormat, UniFFIVideoMetadata,
    UniFFIVideoTrackInfo,
};

/// Maximum recursion depth for nested ISO-BMFF container structures.
const MAX_CONTAINER_DEPTH: usize = 8;

pub(crate) fn is_isobmff(data: &[u8]) -> bool {
    if data.len() < 8 {
        return false;
    }
    &data[4..8] == b"ftyp" || &data[4..8] == b"moov" || &data[4..8] == b"wide"
}

#[derive(Default)]
pub(crate) struct IsoBmffAccumulator {
    pub(crate) duration_seconds: f64,
    pub(crate) timescale: u32,
    pub(crate) video_tracks: Vec<UniFFIVideoTrackInfo>,
    pub(crate) audio_tracks: Vec<UniFFIAudioTrackInfo>,
    pub(crate) subtitle_tracks: Vec<UniFFISubtitleTrackInfo>,
    pub(crate) chapters: Vec<UniFFIChapterInfo>,
    pub(crate) title: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) creation_date: Option<String>,
    pub(crate) encoder: Option<String>,
    pub(crate) cover_data: Option<Vec<u8>>,
    pub(crate) cover_mime: Option<String>,
    pub(crate) extra_tags: HashMap<String, String>,
}

pub(crate) fn parse_isobmff_metadata(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIVideoMetadata, UniFFIVideoError> {
    let mut acc = IsoBmffAccumulator::default();
    let format = infer_isobmff_format(data, file_name);

    scan_isobmff_tree(data, 0, data.len(), &mut acc, 0);

    let has_cover = acc.cover_data.is_some();
    let file_size = data.len() as u64;
    let bitrate = if acc.duration_seconds > 0.0 {
        ((file_size as f64 * 8.0) / acc.duration_seconds / 1000.0) as u32
    } else {
        0
    };

    Ok(UniFFIVideoMetadata {
        format,
        container_name: format.display_name().to_string(),
        duration_seconds: acc.duration_seconds,
        file_size_bytes: file_size,
        bitrate_kbps: bitrate,
        video_tracks: acc.video_tracks,
        audio_tracks: acc.audio_tracks,
        subtitle_tracks: acc.subtitle_tracks,
        chapters: acc.chapters,
        title: acc.title,
        artist_or_director: acc.artist,
        creation_date: acc.creation_date,
        encoder: acc.encoder,
        has_cover,
        cover_mime_type: acc.cover_mime,
        extra_tags: acc.extra_tags,
    })
}

pub(crate) fn extract_isobmff_cover(data: &[u8]) -> Option<Vec<u8>> {
    let mut acc = IsoBmffAccumulator::default();
    scan_isobmff_tree(data, 0, data.len(), &mut acc, 0);
    acc.cover_data
}

fn infer_isobmff_format(data: &[u8], file_name: Option<&str>) -> UniFFIVideoFormat {
    if let Some(name) = file_name {
        let fmt = UniFFIVideoFormat::from_extension(name);
        if fmt != UniFFIVideoFormat::Unknown {
            return fmt;
        }
    }
    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        let brand = &data[8..12];
        match brand {
            b"qt  " | b"moov" => return UniFFIVideoFormat::Mov,
            b"M4V " | b"M4VH" | b"M4VP" => return UniFFIVideoFormat::M4v,
            _ => return UniFFIVideoFormat::Mp4,
        }
    }
    UniFFIVideoFormat::Mp4
}

fn scan_isobmff_tree(
    data: &[u8],
    mut offset: usize,
    end: usize,
    acc: &mut IsoBmffAccumulator,
    depth: usize,
) {
    if depth > MAX_CONTAINER_DEPTH {
        return;
    }

    while offset + 8 <= end && offset + 8 <= data.len() {
        let box_size = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        let box_type = &data[offset + 4..offset + 8];

        let actual_size = if box_size == 1 && offset + 16 <= data.len() {
            u64::from_be_bytes([
                data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11],
                data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15],
            ]) as usize
        } else if box_size == 0 {
            end.saturating_sub(offset)
        } else {
            box_size
        };

        if actual_size < 8 || offset + actual_size > end.min(data.len()) {
            break;
        }

        let header_len = if box_size == 1 { 16 } else { 8 };
        let payload_offset = offset + header_len;
        let payload_end = offset + actual_size;

        match box_type {
            b"moov" | b"mdia" | b"minf" | b"stbl" | b"udta" => {
                scan_isobmff_tree(data, payload_offset, payload_end, acc, depth + 1);
            }
            b"meta" => {
                let meta_start = if payload_offset + 4 <= payload_end {
                    payload_offset + 4
                } else {
                    payload_offset
                };
                scan_isobmff_tree(data, meta_start, payload_end, acc, depth + 1);
            }
            b"ilst" => {
                parse_ilst_tags(data, payload_offset, payload_end, acc);
            }
            b"mvhd" if payload_offset + 20 <= payload_end => {
                let version = data[payload_offset];
                if version == 0 && payload_offset + 20 <= data.len() {
                    let timescale = u32::from_be_bytes([
                        data[payload_offset + 12], data[payload_offset + 13],
                        data[payload_offset + 14], data[payload_offset + 15],
                    ]);
                    let duration = u32::from_be_bytes([
                        data[payload_offset + 16], data[payload_offset + 17],
                        data[payload_offset + 18], data[payload_offset + 19],
                    ]);
                    if timescale > 0 {
                        acc.timescale = timescale;
                        acc.duration_seconds = duration as f64 / timescale as f64;
                    }
                } else if version == 1 && payload_offset + 28 <= data.len() {
                    let timescale = u32::from_be_bytes([
                        data[payload_offset + 20], data[payload_offset + 21],
                        data[payload_offset + 22], data[payload_offset + 23],
                    ]);
                    let duration = u64::from_be_bytes([
                        data[payload_offset + 24], data[payload_offset + 25],
                        data[payload_offset + 26], data[payload_offset + 27],
                        data[payload_offset + 28], data[payload_offset + 29],
                        data[payload_offset + 30], data[payload_offset + 31],
                    ]);
                    if timescale > 0 {
                        acc.timescale = timescale;
                        acc.duration_seconds = duration as f64 / timescale as f64;
                    }
                }
            }
            b"trak" => {
                parse_trak_box(data, payload_offset, payload_end, acc, depth + 1);
            }
            b"chpl" if payload_offset + 8 <= payload_end => {
                parse_chpl_box(data, payload_offset, payload_end, acc);
            }
            _ => {}
        }

        offset += actual_size;
    }
}

fn parse_trak_box(
    data: &[u8],
    offset: usize,
    end: usize,
    acc: &mut IsoBmffAccumulator,
    depth: usize,
) {
    let mut track_id = (acc.video_tracks.len() + acc.audio_tracks.len() + acc.subtitle_tracks.len() + 1) as u32;
    let mut handler_type = [0u8; 4];
    let mut width = 0u32;
    let mut height = 0u32;
    let mut rotation = 0u32;
    let mut sample_rate = 0u32;
    let mut channels = 0u32;
    let mut codec_fourcc = [0u8; 4];
    let mut track_duration = acc.duration_seconds;

    let mut cur = offset;
    while cur + 8 <= end {
        let bsize = u32::from_be_bytes([data[cur], data[cur + 1], data[cur + 2], data[cur + 3]]) as usize;
        let btype = &data[cur + 4..cur + 8];
        let actual = if bsize == 1 && cur + 16 <= end {
            u64::from_be_bytes([
                data[cur + 8], data[cur + 9], data[cur + 10], data[cur + 11],
                data[cur + 12], data[cur + 13], data[cur + 14], data[cur + 15],
            ]) as usize
        } else if bsize == 0 {
            end - cur
        } else {
            bsize
        };

        if actual < 8 || cur + actual > end {
            break;
        }

        let p_off = if bsize == 1 { cur + 16 } else { cur + 8 };
        let p_end = cur + actual;

        match btype {
            b"tkhd" if p_off + 84 <= p_end => {
                let ver = data[p_off];
                let tid_off = if ver == 0 { p_off + 12 } else { p_off + 16 };
                if tid_off + 4 <= data.len() {
                    let tid = u32::from_be_bytes([data[tid_off], data[tid_off + 1], data[tid_off + 2], data[tid_off + 3]]);
                    if tid > 0 {
                        track_id = tid;
                    }
                }
                let m_off = if ver == 0 { p_off + 40 } else { p_off + 52 };
                let dim_off = m_off + 36;
                if dim_off + 8 <= data.len() {
                    let w = u32::from_be_bytes([data[dim_off], data[dim_off + 1], data[dim_off + 2], data[dim_off + 3]]) >> 16;
                    let h = u32::from_be_bytes([data[dim_off + 4], data[dim_off + 5], data[dim_off + 6], data[dim_off + 7]]) >> 16;
                    if w > 0 && h > 0 {
                        width = w;
                        height = h;
                    }
                }
                if m_off + 16 <= data.len() {
                    let (m0, m1, m3) = (
                        i32::from_be_bytes([data[m_off], data[m_off + 1], data[m_off + 2], data[m_off + 3]]),
                        i32::from_be_bytes([data[m_off + 4], data[m_off + 5], data[m_off + 6], data[m_off + 7]]),
                        i32::from_be_bytes([data[m_off + 12], data[m_off + 13], data[m_off + 14], data[m_off + 15]]),
                    );
                    if m1 == 0x0001_0000 && m3 == -0x0001_0000 {
                        rotation = 90;
                    } else if m0 == -0x0001_0000 && m1 == 0 && m3 == 0 {
                        rotation = 180;
                    } else if m1 == -0x0001_0000 && m3 == 0x0001_0000 {
                        rotation = 270;
                    }
                }
            }
            b"mdia" => {
                scan_mdia_box(data, p_off, p_end, &mut handler_type, &mut codec_fourcc, &mut sample_rate, &mut channels, &mut width, &mut height, &mut track_duration, acc.timescale);
            }
            _ => {}
        }
        cur += actual;
    }

    if &handler_type == b"vide" || (width > 0 && height > 0) {
        let (codec, codec_name) = map_isobmff_video_codec(&codec_fourcc);
        let ar = if width > 0 && height > 0 {
            super::parser::compute_aspect_ratio(width, height)
        } else {
            "16:9".to_string()
        };
        acc.video_tracks.push(UniFFIVideoTrackInfo {
            track_id,
            codec,
            codec_name,
            width: if width == 0 { 1920 } else { width },
            height: if height == 0 { 1080 } else { height },
            frame_rate: 30.0,
            bitrate_kbps: 0,
            duration_seconds: if track_duration > 0.0 { track_duration } else { acc.duration_seconds },
            aspect_ratio: ar,
            color_space: Some("BT.709".to_string()),
            hdr_format: None,
            rotation_degrees: rotation,
        });
    } else if &handler_type == b"soun" {
        let (codec, codec_name) = map_isobmff_audio_codec(&codec_fourcc);
        let layout = if channels == 1 { "Mono" } else if channels == 6 { "5.1 Surround" } else { "Stereo" };
        acc.audio_tracks.push(UniFFIAudioTrackInfo {
            track_id,
            codec,
            codec_name,
            sample_rate: if sample_rate == 0 { 44100 } else { sample_rate },
            channels: if channels == 0 { 2 } else { channels },
            channel_layout: layout.to_string(),
            bit_depth: Some(16),
            bitrate_kbps: 0,
            language: Some("und".to_string()),
            title: None,
            is_default: acc.audio_tracks.is_empty(),
        });
    } else if &handler_type == b"sbtl" || &handler_type == b"subt" || &handler_type == b"text" || &handler_type == b"clcp" {
        let fmt_str = String::from_utf8_lossy(&codec_fourcc).trim().to_string();
        acc.subtitle_tracks.push(UniFFISubtitleTrackInfo {
            track_id,
            format: if fmt_str.is_empty() { "Timed Text (tx3g)".to_string() } else { fmt_str },
            language: Some("und".to_string()),
            title: None,
            is_forced: false,
            is_default: acc.subtitle_tracks.is_empty(),
            is_sdh: false,
        });
    }

    let _ = depth;
}

fn scan_mdia_box(
    data: &[u8],
    offset: usize,
    end: usize,
    handler: &mut [u8; 4],
    fourcc: &mut [u8; 4],
    sr: &mut u32,
    ch: &mut u32,
    w: &mut u32,
    h: &mut u32,
    dur: &mut f64,
    media_timescale: u32,
) {
    let mut cur = offset;
    while cur + 8 <= end {
        let bsize = u32::from_be_bytes([data[cur], data[cur + 1], data[cur + 2], data[cur + 3]]) as usize;
        let btype = &data[cur + 4..cur + 8];
        let actual = if bsize == 1 && cur + 16 <= end {
            u64::from_be_bytes([
                data[cur + 8], data[cur + 9], data[cur + 10], data[cur + 11],
                data[cur + 12], data[cur + 13], data[cur + 14], data[cur + 15],
            ]) as usize
        } else if bsize == 0 {
            end - cur
        } else {
            bsize
        };

        if actual < 8 || cur + actual > end {
            break;
        }

        let p_off = if bsize == 1 { cur + 16 } else { cur + 8 };
        let p_end = cur + actual;

        match btype {
            b"hdlr" if p_off + 12 <= p_end => {
                handler.copy_from_slice(&data[p_off + 8..p_off + 12]);
            }
            b"mdhd" if p_off + 20 <= p_end => {
                let ver = data[p_off];
                if ver == 0 && p_off + 20 <= data.len() {
                    let ts = u32::from_be_bytes([data[p_off + 12], data[p_off + 13], data[p_off + 14], data[p_off + 15]]);
                    let d = u32::from_be_bytes([data[p_off + 16], data[p_off + 17], data[p_off + 18], data[p_off + 19]]);
                    if ts > 0 {
                        *dur = d as f64 / ts as f64;
                    }
                }
            }
            b"minf" | b"stbl" => {
                scan_mdia_box(data, p_off, p_end, handler, fourcc, sr, ch, w, h, dur, media_timescale);
            }
            b"stsd" if p_off + 8 <= p_end => {
                let cnt = u32::from_be_bytes([data[p_off + 4], data[p_off + 5], data[p_off + 6], data[p_off + 7]]);
                let mut e_off = p_off + 8;
                for _ in 0..cnt.min(4) {
                    if e_off + 8 > p_end {
                        break;
                    }
                    let esz = u32::from_be_bytes([data[e_off], data[e_off + 1], data[e_off + 2], data[e_off + 3]]) as usize;
                    fourcc.copy_from_slice(&data[e_off + 4..e_off + 8]);

                    if esz >= 36 && e_off + 36 <= p_end {
                        let sample_w = u16::from_be_bytes([data[e_off + 32], data[e_off + 33]]) as u32;
                        let sample_h = u16::from_be_bytes([data[e_off + 34], data[e_off + 35]]) as u32;
                        if sample_w > 0 && sample_h > 0 && *w == 0 {
                            *w = sample_w;
                            *h = sample_h;
                        }
                    }

                    if esz >= 28 && e_off + 28 <= p_end {
                        let audio_ch = u16::from_be_bytes([data[e_off + 16], data[e_off + 17]]) as u32;
                        let audio_sr = u32::from_be_bytes([data[e_off + 24], data[e_off + 25], data[e_off + 26], data[e_off + 27]]) >> 16;
                        if audio_ch > 0 && *ch == 0 {
                            *ch = audio_ch;
                        }
                        if audio_sr > 0 && *sr == 0 {
                            *sr = audio_sr;
                        }
                    }

                    if esz < 8 {
                        break;
                    }
                    e_off += esz;
                }
            }
            _ => {}
        }
        cur += actual;
    }
}

fn map_isobmff_video_codec(fourcc: &[u8; 4]) -> (UniFFIVideoCodec, String) {
    match fourcc {
        b"avc1" | b"avc3" => (UniFFIVideoCodec::H264, "H.264 / AVC".to_string()),
        b"hvc1" | b"hev1" => (UniFFIVideoCodec::Hevc, "H.265 / HEVC".to_string()),
        b"av01" => (UniFFIVideoCodec::Av1, "AV1".to_string()),
        b"vp09" | b"vp08" => (UniFFIVideoCodec::Vp9, "VP9".to_string()),
        b"apcn" | b"apch" | b"ap4h" | b"ap4x" | b"apco" | b"apcs" => (UniFFIVideoCodec::ProRes, "Apple ProRes".to_string()),
        b"mp4v" => (UniFFIVideoCodec::Mpeg4, "MPEG-4 Part 2".to_string()),
        _ => {
            let name = String::from_utf8_lossy(fourcc).to_string();
            if name.trim().is_empty() {
                (UniFFIVideoCodec::H264, "H.264 / AVC".to_string())
            } else {
                (UniFFIVideoCodec::Unknown, name)
            }
        }
    }
}

fn map_isobmff_audio_codec(fourcc: &[u8; 4]) -> (UniFFIAudioCodec, String) {
    match fourcc {
        b"mp4a" => (UniFFIAudioCodec::Aac, "AAC".to_string()),
        b"alac" => (UniFFIAudioCodec::Alac, "Apple Lossless (ALAC)".to_string()),
        b"ac-3" => (UniFFIAudioCodec::Ac3, "Dolby Digital (AC-3)".to_string()),
        b"ec-3" => (UniFFIAudioCodec::Eac3, "Dolby Digital Plus (E-AC-3)".to_string()),
        b"Opus" | b"opus" => (UniFFIAudioCodec::Opus, "Opus".to_string()),
        b"fLaC" | b"flac" => (UniFFIAudioCodec::Flac, "FLAC".to_string()),
        b".mp3" | b"mp3 " => (UniFFIAudioCodec::Mp3, "MP3".to_string()),
        b"lpcm" | b"raw " => (UniFFIAudioCodec::Pcm, "Linear PCM".to_string()),
        _ => {
            let name = String::from_utf8_lossy(fourcc).to_string();
            if name.trim().is_empty() {
                (UniFFIAudioCodec::Aac, "AAC".to_string())
            } else {
                (UniFFIAudioCodec::Unknown, name)
            }
        }
    }
}

fn parse_ilst_tags(
    data: &[u8],
    offset: usize,
    end: usize,
    acc: &mut IsoBmffAccumulator,
) {
    let mut cur = offset;
    while cur + 8 <= end {
        let bsize = u32::from_be_bytes([data[cur], data[cur + 1], data[cur + 2], data[cur + 3]]) as usize;
        let btype = &data[cur + 4..cur + 8];
        if bsize < 8 || cur + bsize > end {
            break;
        }

        let p_off = cur + 8;
        let p_end = cur + bsize;

        if btype == b"covr" {
            if let Some((img_bytes, mime)) = extract_ilst_data_box(data, p_off, p_end) {
                acc.cover_mime = Some(mime);
                acc.cover_data = Some(img_bytes);
            }
        } else if let Some((str_val, _)) = extract_ilst_text_box(data, p_off, p_end) {
            match btype {
                b"\xa9nam" => acc.title = Some(str_val.clone()),
                b"\xa9ART" | b"\xa9art" | b"\xa9dir" => acc.artist = Some(str_val.clone()),
                b"\xa9day" => acc.creation_date = Some(str_val.clone()),
                b"\xa9too" | b"\xa9enc" => acc.encoder = Some(str_val.clone()),
                _ => {
                    let key = String::from_utf8_lossy(btype).to_string();
                    acc.extra_tags.insert(key, str_val);
                }
            }
        }

        cur += bsize;
    }
}

fn extract_ilst_data_box(data: &[u8], offset: usize, end: usize) -> Option<(Vec<u8>, String)> {
    let mut cur = offset;
    while cur + 8 <= end {
        let bsize = u32::from_be_bytes([data[cur], data[cur + 1], data[cur + 2], data[cur + 3]]) as usize;
        let btype = &data[cur + 4..cur + 8];
        if bsize < 8 || cur + bsize > end {
            break;
        }
        if btype == b"data" && cur + 16 <= end {
            let flag = u32::from_be_bytes([data[cur + 8], data[cur + 9], data[cur + 10], data[cur + 11]]) & 0xFF;
            let mime = if flag == 13 {
                "image/jpeg"
            } else if flag == 14 {
                "image/png"
            } else {
                "image/jpeg"
            };
            let payload = data[cur + 16..cur + bsize].to_vec();
            return Some((payload, mime.to_string()));
        }
        cur += bsize;
    }
    None
}

fn extract_ilst_text_box(data: &[u8], offset: usize, end: usize) -> Option<(String, u32)> {
    let mut cur = offset;
    while cur + 8 <= end {
        let bsize = u32::from_be_bytes([data[cur], data[cur + 1], data[cur + 2], data[cur + 3]]) as usize;
        let btype = &data[cur + 4..cur + 8];
        if bsize < 8 || cur + bsize > end {
            break;
        }
        if btype == b"data" && cur + 16 <= end {
            let flag = u32::from_be_bytes([data[cur + 8], data[cur + 9], data[cur + 10], data[cur + 11]]) & 0xFF;
            let text = String::from_utf8_lossy(&data[cur + 16..cur + bsize]).trim().to_string();
            return Some((text, flag));
        }
        cur += bsize;
    }
    None
}

fn parse_chpl_box(data: &[u8], offset: usize, end: usize, acc: &mut IsoBmffAccumulator) {
    if offset + 9 > end {
        return;
    }
    let count = data[offset + 8] as usize;
    let mut cur = offset + 9;
    for i in 0..count {
        if cur + 9 > end {
            break;
        }
        let start_time_ms = u64::from_be_bytes([
            data[cur], data[cur + 1], data[cur + 2], data[cur + 3],
            data[cur + 4], data[cur + 5], data[cur + 6], data[cur + 7],
        ]) / 10000;
        let title_len = data[cur + 8] as usize;
        cur += 9;
        if cur + title_len > end {
            break;
        }
        let title = String::from_utf8_lossy(&data[cur..cur + title_len]).to_string();
        cur += title_len;
        acc.chapters.push(UniFFIChapterInfo {
            chapter_id: (i + 1) as u32,
            title,
            start_time_seconds: start_time_ms as f64 / 1000.0,
            end_time_seconds: acc.duration_seconds,
        });
    }
}
