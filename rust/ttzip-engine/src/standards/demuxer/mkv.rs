// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Matroska (MKV) / WebM container demuxer and EBML parser.

use crate::types::status::TTZipStatus;
use super::types::{MediaAttachment, MediaChapter, MediaDemuxSummary, MediaTrackInfo, MediaTrackType};

pub(crate) const EBML_HEADER_ID: u32 = 0x1A45_DFA3;
const SEGMENT_ID: u32 = 0x1853_8067;
const INFO_ID: u32 = 0x1549_A966;
const TRACKS_ID: u32 = 0x1654_AE6B;
const CHAPTERS_ID: u32 = 0x1043_A770;
const ATTACHMENTS_ID: u32 = 0x1941_A469;

/// Parses an MKV/WebM byte stream into a structured `MediaDemuxSummary`.
pub fn parse_mkv_demux(data: &[u8]) -> Result<MediaDemuxSummary, TTZipStatus> {
    if data.len() < 4 {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    let mut summary = MediaDemuxSummary::new("Matroska");
    let mut timecode_scale: u64 = 1_000_000;
    let mut found_ebml = false;

    for_each_ebml_child(data, |id, payload| match id {
        EBML_HEADER_ID => {
            found_ebml = true;
            for_each_ebml_child(payload, |hid, hp| {
                if hid == 0x4282 {
                    let doc = read_str(hp);
                    summary.container_format = if doc.eq_ignore_ascii_case("webm") {
                        "WebM".into()
                    } else {
                        doc
                    };
                }
            });
        }
        SEGMENT_ID => parse_segment(payload, &mut summary, &mut timecode_scale),
        _ => {}
    });

    if !found_ebml {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(summary)
}

fn parse_segment(data: &[u8], summary: &mut MediaDemuxSummary, scale: &mut u64) {
    for_each_ebml_child(data, |id, payload| match id {
        INFO_ID => {
            let mut raw_dur: Option<f64> = None;
            for_each_ebml_child(payload, |iid, ip| match iid {
                0x2AD7_B1 => *scale = read_uint(ip).max(1),
                0x4489 => raw_dur = read_float(ip),
                0x7BA9 => summary.title = Some(read_str(ip)),
                _ => {}
            });
            if let Some(dur) = raw_dur {
                summary.duration_ms = Some(((dur * (*scale as f64)) / 1_000_000.0) as u64);
            }
        }
        TRACKS_ID => for_each_ebml_child(payload, |tid, tp| {
            if tid == 0xAE {
                if let Some(t) = parse_track_entry(tp) {
                    summary.tracks.push(t);
                }
            }
        }),
        CHAPTERS_ID => for_each_ebml_child(payload, |eid, ep| {
            if eid == 0x45B9 {
                for_each_ebml_child(ep, |cid, cp| {
                    if cid == 0xB6 {
                        if let Some(c) = parse_chapter_atom(cp) {
                            summary.chapters.push(c);
                        }
                    }
                });
            }
        }),
        ATTACHMENTS_ID => for_each_ebml_child(payload, |aid, ap| {
            if aid == 0x61A7 {
                if let Some(a) = parse_attached_file(ap) {
                    summary.attachments.push(a);
                }
            }
        }),
        _ => {}
    });
}

fn parse_track_entry(data: &[u8]) -> Option<MediaTrackInfo> {
    let (mut num, mut t_type, mut codec, mut name, mut lang) = (0u32, 0u64, String::new(), None, None);
    let (mut is_def, mut w, mut h, mut ch, mut sr) = (true, None, None, None, None);

    for_each_ebml_child(data, |id, p| match id {
        0xD7 => num = read_uint(p) as u32,
        0x83 => t_type = read_uint(p),
        0x86 => codec = read_str(p),
        0x536E => name = Some(read_str(p)),
        0x22B5_9C | 0x22B5_9D => lang = Some(read_str(p)),
        0x88 => is_def = read_uint(p) != 0,
        0xE0 => for_each_ebml_child(p, |vid, vp| match vid {
            0xB0 => w = Some(read_uint(vp) as u32),
            0xBA => h = Some(read_uint(vp) as u32),
            _ => {}
        }),
        0xE1 => for_each_ebml_child(p, |aid, ap| match aid {
            0x9F => ch = Some(read_uint(ap) as u16),
            0xB5 => sr = read_float(ap).map(|f| f.round() as u32).or_else(|| Some(read_uint(ap) as u32)),
            _ => {}
        }),
        _ => {}
    });

    let track_type = match t_type {
        1 => MediaTrackType::Video,
        2 => MediaTrackType::Audio,
        17 => MediaTrackType::Subtitle,
        _ => return None,
    };

    let mut info = MediaTrackInfo::new(num, track_type, codec);
    info.title = name;
    info.language = lang;
    info.is_default = is_def;
    info.width = w;
    info.height = h;
    info.channels = ch;
    info.sample_rate = sr;
    Some(info)
}

fn parse_chapter_atom(data: &[u8]) -> Option<MediaChapter> {
    let (mut start_ns, mut end_ns, mut title) = (0u64, None, String::new());
    for_each_ebml_child(data, |id, p| match id {
        0x91 => start_ns = read_uint(p),
        0x92 => end_ns = Some(read_uint(p)),
        0x80 => for_each_ebml_child(p, |did, dp| {
            if did == 0x85 {
                title = read_str(dp);
            }
        }),
        _ => {}
    });
    Some(MediaChapter::new(start_ns / 1_000_000, end_ns.map(|v| v / 1_000_000), title))
}

fn parse_attached_file(data: &[u8]) -> Option<MediaAttachment> {
    let (mut name, mut mime, mut file_data) = (String::new(), String::from("application/octet-stream"), Vec::new());
    for_each_ebml_child(data, |id, p| match id {
        0x466E => name = read_str(p),
        0x4660 => mime = read_str(p),
        0x465C => file_data = p.to_vec(),
        _ => {}
    });
    if name.is_empty() && file_data.is_empty() {
        None
    } else {
        Some(MediaAttachment::new(name, mime, file_data))
    }
}

fn for_each_ebml_child(data: &[u8], mut f: impl FnMut(u32, &[u8])) {
    let mut off = 0;
    while off < data.len() {
        let (id, size) = match read_ebml_element_header(data, &mut off) {
            Some(v) => v,
            None => break,
        };
        let end = (off + size).min(data.len());
        let payload = if off <= end { &data[off..end] } else { &[] };
        f(id, payload);
        off = end;
    }
}

fn read_ebml_element_header(data: &[u8], offset: &mut usize) -> Option<(u32, usize)> {
    if *offset >= data.len() {
        return None;
    }
    let first = data[*offset];
    let id_len = first.leading_zeros() as usize + 1;
    if id_len > 4 || *offset + id_len > data.len() {
        return None;
    }
    let mut id: u32 = 0;
    for &b in &data[*offset..*offset + id_len] {
        id = (id << 8) | (b as u32);
    }
    *offset += id_len;

    if *offset >= data.len() {
        return None;
    }
    let size_first = data[*offset];
    let size_len = size_first.leading_zeros() as usize + 1;
    if size_len > 8 || *offset + size_len > data.len() {
        return None;
    }
    let mask = 0xFF >> size_len;
    let mut size: usize = (size_first & mask) as usize;
    for &b in &data[*offset + 1..*offset + size_len] {
        size = (size << 8) | (b as usize);
    }
    *offset += size_len;
    Some((id, size))
}

fn read_uint(payload: &[u8]) -> u64 {
    let mut val: u64 = 0;
    for &b in payload.iter().take(8) {
        val = (val << 8) | (b as u64);
    }
    val
}

fn read_float(payload: &[u8]) -> Option<f64> {
    match payload.len() {
        4 => {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(payload);
            Some(f32::from_be_bytes(buf) as f64)
        }
        8 => {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(payload);
            Some(f64::from_be_bytes(buf))
        }
        _ => None,
    }
}

fn read_str(payload: &[u8]) -> String {
    String::from_utf8_lossy(payload).trim_matches('\0').to_string()
}
