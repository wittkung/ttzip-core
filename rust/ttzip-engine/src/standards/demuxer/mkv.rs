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
const SEEK_HEAD_ID: u32 = 0x114D_9B74;
const INFO_ID: u32 = 0x1549_A966;
const TRACKS_ID: u32 = 0x1654_AE6B;
const CHAPTERS_ID: u32 = 0x1043_A770;
const ATTACHMENTS_ID: u32 = 0x1941_A469;
const CUES_ID: u32 = 0x1C53_BB6B;

/// Parses an MKV/WebM byte stream into a structured `MediaDemuxSummary`.
pub fn parse_mkv_demux(data: &[u8]) -> Result<MediaDemuxSummary, TTZipStatus> {
    demux_mkv_two_pass(data, None)
}

/// Zero-copy two-pass MKV demuxer supporting tail SeekHead, Chapters, Attachments, and Cues.
pub fn demux_mkv_two_pass(head: &[u8], tail: Option<&[u8]>) -> Result<MediaDemuxSummary, TTZipStatus> {
    if head.len() < 4 {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    let mut summary = MediaDemuxSummary::new("Matroska");
    let mut timecode_scale: u64 = 1_000_000;
    let mut found_ebml = false;

    for_each_ebml_child(head, |id, payload| match id {
        EBML_HEADER_ID => {
            found_ebml = true;
            for_each_ebml_child(payload, |hid, hp| {
                if hid == 0x4282 {
                    let doc = read_str(hp);
                    summary.container_format = if doc.eq_ignore_ascii_case("webm") { "WebM".into() } else { doc };
                }
            });
        }
        SEGMENT_ID => parse_segment(payload, &mut summary, &mut timecode_scale),
        _ => parse_ebml_element(id, payload, &mut summary, &mut timecode_scale),
    });

    if !found_ebml {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    if let Some(tail_data) = tail {
        parse_tail_ebml(tail_data, &mut summary, &mut timecode_scale);
    }

    Ok(summary)
}

fn parse_segment(data: &[u8], summary: &mut MediaDemuxSummary, scale: &mut u64) {
    for_each_ebml_child(data, |id, payload| {
        parse_ebml_element(id, payload, summary, scale);
    });
}

fn parse_ebml_element(id: u32, payload: &[u8], summary: &mut MediaDemuxSummary, scale: &mut u64) {
    match id {
        INFO_ID => {
            let mut raw_dur: Option<f64> = None;
            for_each_ebml_child(payload, |iid, ip| {
                match iid {
                    0x002A_D7B1 => *scale = read_uint(ip).max(1),
                    0x4489 => raw_dur = read_float(ip),
                    0x7BA9 if summary.title.is_none() => summary.title = Some(read_str(ip)),
                    _ => {}
                }
            });
            if let Some(dur) = raw_dur {
                summary.duration_ms = Some(((dur * (*scale as f64)) / 1_000_000.0) as u64);
            }
        }
        TRACKS_ID => for_each_ebml_child(payload, |tid, tp| {
            if tid == 0xAE {
                if let Some(t) = parse_track_entry(tp) {
                    if !summary.tracks.iter().any(|existing| existing.track_id == t.track_id) {
                        summary.tracks.push(t);
                    }
                }
            }
        }),
        CHAPTERS_ID => for_each_ebml_child(payload, |eid, ep| {
            if eid == 0x45B9 {
                for_each_ebml_child(ep, |cid, cp| {
                    if cid == 0xB6 {
                        if let Some(c) = parse_chapter_atom(cp) {
                            if !summary.chapters.iter().any(|ex| ex.start_time_ms == c.start_time_ms && ex.title == c.title) {
                                summary.chapters.push(c);
                            }
                        }
                    }
                });
            }
        }),
        ATTACHMENTS_ID => for_each_ebml_child(payload, |aid, ap| {
            if aid == 0x61A7 {
                if let Some(a) = parse_attached_file(ap) {
                    if !summary.attachments.iter().any(|ex| ex.file_name == a.file_name) {
                        summary.attachments.push(a);
                    }
                }
            }
        }),
        CUES_ID => parse_cues(payload, summary, *scale),
        SEEK_HEAD_ID => { let _ = parse_seek_head(payload); }
        SEGMENT_ID => parse_segment(payload, summary, scale),
        _ => {}
    }
}

fn parse_tail_ebml(tail: &[u8], summary: &mut MediaDemuxSummary, scale: &mut u64) {
    for_each_ebml_child(tail, |id, payload| {
        parse_ebml_element(id, payload, summary, scale);
    });

    const TARGETS: [(u32, &[u8]); 6] = [
        (CHAPTERS_ID, &[0x10, 0x43, 0xA7, 0x70]),
        (ATTACHMENTS_ID, &[0x19, 0x41, 0xA4, 0x69]),
        (CUES_ID, &[0x1C, 0x53, 0xBB, 0x6B]),
        (TRACKS_ID, &[0x16, 0x54, 0xAE, 0x6B]),
        (INFO_ID, &[0x15, 0x49, 0xA9, 0x66]),
        (SEEK_HEAD_ID, &[0x11, 0x4D, 0x9B, 0x74]),
    ];

    for &(tid, sig) in &TARGETS {
        let mut idx = 0;
        while idx + sig.len() <= tail.len() {
            if let Some(pos) = tail[idx..].windows(sig.len()).position(|w| w == sig) {
                let abs_pos = idx + pos;
                let mut off = abs_pos;
                if let Some((id, size)) = read_ebml_element_header(tail, &mut off) {
                    if id == tid {
                        let end = if size == usize::MAX { tail.len() } else { off.saturating_add(size).min(tail.len()) };
                        let payload = if off <= end { &tail[off..end] } else { &[] };
                        parse_ebml_element(id, payload, summary, scale);
                    }
                }
                idx = abs_pos + sig.len();
            } else {
                break;
            }
        }
    }
}

fn parse_track_entry(data: &[u8]) -> Option<MediaTrackInfo> {
    let (mut num, mut t_type, mut codec, mut name, mut lang) = (0u32, 0u64, String::new(), None, None);
    let (mut is_def, mut w, mut h, mut ch, mut sr) = (true, None, None, None, None);

    for_each_ebml_child(data, |id, p| match id {
        0xD7 => num = read_uint(p) as u32,
        0x83 => t_type = read_uint(p),
        0x86 => codec = read_str(p),
        0x536E => name = Some(read_str(p)),
        0x0022_B59C..=0x0022_B59E => lang = Some(read_str(p)),
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
            if did == 0x85 { title = read_str(dp); }
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
    if name.is_empty() && file_data.is_empty() { None } else { Some(MediaAttachment::new(name, mime, file_data)) }
}

fn parse_cues(data: &[u8], summary: &mut MediaDemuxSummary, scale: u64) {
    let mut max_cue_time = 0u64;
    for_each_ebml_child(data, |id, p| {
        if id == 0xBB {
            for_each_ebml_child(p, |cid, cp| {
                if cid == 0xB3 {
                    let t = read_uint(cp);
                    if t > max_cue_time { max_cue_time = t; }
                }
            });
        }
    });
    if max_cue_time > 0 && summary.duration_ms.is_none() {
        summary.duration_ms = Some(max_cue_time.saturating_mul(scale) / 1_000_000);
    }
}

pub(crate) fn parse_seek_head(data: &[u8]) -> Vec<(u32, u64)> {
    let mut seeks = Vec::new();
    for_each_ebml_child(data, |id, p| {
        if id == 0x4DBB {
            let (mut sid, mut spos) = (0u32, 0u64);
            for_each_ebml_child(p, |eid, ep| match eid {
                0x53AB => for &b in ep { sid = (sid << 8) | (b as u32); },
                0x53AC => spos = read_uint(ep),
                _ => {}
            });
            if sid != 0 { seeks.push((sid, spos)); }
        }
    });
    seeks
}

fn for_each_ebml_child(data: &[u8], mut f: impl FnMut(u32, &[u8])) {
    let mut off = 0;
    while off < data.len() {
        let (id, size) = match read_ebml_element_header(data, &mut off) {
            Some(v) => v,
            None => break,
        };
        let end = if size == usize::MAX { data.len() } else { off.saturating_add(size).min(data.len()) };
        let payload = if off <= end { &data[off..end] } else { &[] };
        f(id, payload);
        off = end;
    }
}

fn read_ebml_element_header(data: &[u8], offset: &mut usize) -> Option<(u32, usize)> {
    if *offset >= data.len() { return None; }
    let first = data[*offset];
    if first == 0 { return None; }
    let id_len = first.leading_zeros() as usize + 1;
    if id_len > 4 || *offset + id_len > data.len() { return None; }
    let mut id: u32 = 0;
    for &b in &data[*offset..*offset + id_len] { id = (id << 8) | (b as u32); }
    *offset += id_len;

    if *offset >= data.len() { return None; }
    let size_first = data[*offset];
    if size_first == 0 { return None; }
    let size_len = size_first.leading_zeros() as usize + 1;
    if size_len > 8 || *offset + size_len > data.len() { return None; }
    let mask = (1usize << (8 - size_len)) - 1;
    let mut size: usize = (size_first as usize) & mask;
    let mut all_ones = ((size_first as usize) & mask) == mask;
    for &b in &data[*offset + 1..*offset + size_len] {
        size = (size << 8) | (b as usize);
        if b != 0xFF { all_ones = false; }
    }
    *offset += size_len;
    if all_ones { size = usize::MAX; }
    Some((id, size))
}

fn read_uint(payload: &[u8]) -> u64 {
    let mut val: u64 = 0;
    for &b in payload.iter().take(8) { val = (val << 8) | (b as u64); }
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
