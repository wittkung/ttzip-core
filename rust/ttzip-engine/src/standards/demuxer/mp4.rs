// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust ISO Base Media File Format (MP4 / MOV / M4A) demuxer.

use crate::types::status::TTZipStatus;
use super::types::{MediaAttachment, MediaChapter, MediaDemuxSummary, MediaTrackInfo, MediaTrackType};

/// Parses an MP4/MOV byte stream into a structured `MediaDemuxSummary`.
pub fn parse_mp4_demux(data: &[u8]) -> Result<MediaDemuxSummary, TTZipStatus> {
    if data.len() < 8 {
        return Err(TTZipStatus::ErrInvalidParam);
    }
    let mut summary = MediaDemuxSummary::new("MP4");
    let mut found_valid_box = false;

    for_each_mp4_box(data, |fourcc, payload| match &fourcc {
        b"ftyp" => {
            found_valid_box = true;
            if payload.len() >= 4 {
                let brand = &payload[0..4];
                if brand == b"qt  " { summary.container_format = "QuickTime".into(); }
                else if brand == b"M4A " || brand == b"M4B " { summary.container_format = "M4A".into(); }
            }
        }
        b"moov" => {
            found_valid_box = true;
            parse_moov_box(payload, &mut summary);
        }
        b"mdat" | b"free" | b"skip" | b"wide" => found_valid_box = true,
        _ => {}
    });

    if !found_valid_box {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    Ok(summary)
}

fn parse_moov_box(data: &[u8], summary: &mut MediaDemuxSummary) {
    for_each_mp4_box(data, |fourcc, payload| match &fourcc {
        b"mvhd" => parse_mvhd(payload, summary),
        b"trak" => if let Some(t) = parse_trak_box(payload, summary) { summary.tracks.push(t); },
        b"udta" => parse_udta_box(payload, summary),
        _ => {}
    });
}

fn parse_mvhd(data: &[u8], summary: &mut MediaDemuxSummary) {
    if data.len() < 20 { return; }
    let (timescale, duration) = if data[0] == 1 && data.len() >= 32 {
        (u32::from_be_bytes([data[20], data[21], data[22], data[23]]) as u64,
         u64::from_be_bytes([data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31]]))
    } else if data[0] == 0 {
        (u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as u64,
         u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as u64)
    } else { return; };
    if timescale > 0 { summary.duration_ms = Some(duration.saturating_mul(1000) / timescale); }
}

fn parse_trak_box(data: &[u8], summary: &mut MediaDemuxSummary) -> Option<MediaTrackInfo> {
    let (mut track_id, mut width, mut height) = (0u32, None, None);
    let (mut hdlr_type, mut codec_str, mut language) = ([0u8; 4], String::new(), None);
    let (mut channels, mut sample_rate, mut track_name) = (None, None, None);

    for_each_mp4_box(data, |fourcc, payload| match &fourcc {
        b"tkhd" => if payload.len() >= 24 {
            let (tid, w, h) = if payload[0] == 1 && payload.len() >= 92 {
                (u32::from_be_bytes([payload[20], payload[21], payload[22], payload[23]]),
                 u32::from_be_bytes([payload[84], payload[85], payload[86], payload[87]]) >> 16,
                 u32::from_be_bytes([payload[88], payload[89], payload[90], payload[91]]) >> 16)
            } else if payload[0] == 0 && payload.len() >= 84 {
                (u32::from_be_bytes([payload[12], payload[13], payload[14], payload[15]]),
                 u32::from_be_bytes([payload[76], payload[77], payload[78], payload[79]]) >> 16,
                 u32::from_be_bytes([payload[80], payload[81], payload[82], payload[83]]) >> 16)
            } else { (0, 0, 0) };
            track_id = tid;
            if w > 0 { width = Some(w); }
            if h > 0 { height = Some(h); }
        },
        b"mdia" => parse_mdia_box(payload, &mut hdlr_type, &mut codec_str, &mut language, &mut channels, &mut sample_rate, &mut width, &mut height),
        b"udta" => parse_udta_box(payload, summary),
        b"name" => track_name = Some(String::from_utf8_lossy(payload).trim_matches('\0').to_string()),
        _ => {}
    });

    let track_type = match &hdlr_type {
        b"vide" => MediaTrackType::Video,
        b"soun" => MediaTrackType::Audio,
        b"sbtl" | b"subt" | b"text" | b"clcp" => MediaTrackType::Subtitle,
        _ => classify_by_codec(&codec_str)?,
    };

    let mut info = MediaTrackInfo::new(track_id, track_type, codec_str);
    info.language = language;
    info.title = track_name;
    info.width = width;
    info.height = height;
    info.channels = channels;
    info.sample_rate = sample_rate;
    info.is_default = track_id == 1;
    Some(info)
}

fn parse_mdia_box(
    data: &[u8],
    hdlr: &mut [u8; 4],
    codec: &mut String,
    lang: &mut Option<String>,
    ch: &mut Option<u16>,
    sr: &mut Option<u32>,
    w: &mut Option<u32>,
    h: &mut Option<u32>,
) {
    for_each_mp4_box(data, |fourcc, payload| match &fourcc {
        b"mdhd" => *lang = parse_mdhd_lang(payload),
        b"hdlr" => if payload.len() >= 12 { hdlr.copy_from_slice(&payload[8..12]); },
        b"minf" => for_each_mp4_box(payload, |m_fc, m_p| if &m_fc == b"stbl" {
            for_each_mp4_box(m_p, |s_fc, s_p| if &s_fc == b"stsd" { parse_stsd_box(s_p, codec, ch, sr, w, h); });
        }),
        _ => {}
    });
}

fn parse_mdhd_lang(data: &[u8]) -> Option<String> {
    if data.len() < 4 { return None; }
    let offset = if data[0] == 1 { 20 } else { 12 };
    if data.len() < offset + 2 { return None; }
    let raw = u16::from_be_bytes([data[offset], data[offset + 1]]);
    let (c1, c2, c3) = (((raw >> 10) & 0x1F) as u8, ((raw >> 5) & 0x1F) as u8, (raw & 0x1F) as u8);
    if c1 > 0 && c2 > 0 && c3 > 0 && c1 <= 26 && c2 <= 26 && c3 <= 26 {
        let code = String::from_utf8(vec![c1 + 0x60, c2 + 0x60, c3 + 0x60]).ok();
        if code.as_deref() != Some("und") { return code; }
    }
    None
}

fn parse_stsd_box(
    data: &[u8],
    codec: &mut String,
    ch: &mut Option<u16>,
    sr: &mut Option<u32>,
    w: &mut Option<u32>,
    h: &mut Option<u32>,
) {
    if data.len() < 16 { return; }
    let entry = &data[8..];
    *codec = String::from_utf8_lossy(&entry[4..8]).to_string();

    if entry.len() >= 36 {
        let channels = u16::from_be_bytes([entry[24], entry[25]]);
        let sample_rate = u32::from_be_bytes([entry[30], entry[31], entry[32], entry[33]]) >> 16;
        if channels > 0 && sample_rate > 0 {
            *ch = Some(channels);
            *sr = Some(sample_rate);
        }
        let width = u16::from_be_bytes([entry[32], entry[33]]) as u32;
        let height = u16::from_be_bytes([entry[34], entry[35]]) as u32;
        if width > 0 && height > 0 {
            *w = Some(width);
            *h = Some(height);
        }
    }
}

fn parse_udta_box(data: &[u8], summary: &mut MediaDemuxSummary) {
    for_each_mp4_box(data, |fourcc, payload| match &fourcc {
        b"chpl" => parse_chpl_box(payload, summary),
        b"meta" => {
            let meta_p = if payload.len() >= 4 { &payload[4..] } else { payload };
            for_each_mp4_box(meta_p, |m_fc, m_p| if &m_fc == b"ilst" { parse_ilst_box(m_p, summary); });
        }
        _ => {}
    });
}

fn parse_chpl_box(data: &[u8], summary: &mut MediaDemuxSummary) {
    if data.len() < 6 { return; }
    let (count, mut off) = (data[5] as usize, 6);
    for _ in 0..count {
        if off + 9 > data.len() { break; }
        let ts = u64::from_be_bytes([data[off], data[off+1], data[off+2], data[off+3], data[off+4], data[off+5], data[off+6], data[off+7]]);
        let len = data[off + 8] as usize;
        off += 9;
        if off + len > data.len() { break; }
        let title = String::from_utf8_lossy(&data[off..off + len]).to_string();
        off += len;
        summary.chapters.push(MediaChapter::new(ts / 10_000, None, title));
    }
}

fn parse_ilst_box(data: &[u8], summary: &mut MediaDemuxSummary) {
    for_each_mp4_box(data, |fourcc, p| {
        if &fourcc == b"covr" {
            for_each_mp4_box(p, |d_fc, dp| if &d_fc == b"data" && dp.len() >= 8 {
                let is_png = u32::from_be_bytes([dp[0], dp[1], dp[2], dp[3]]) == 14;
                let (ext, mime) = if is_png { ("png", "image/png") } else { ("jpg", "image/jpeg") };
                summary.attachments.push(MediaAttachment::new(format!("cover.{}", ext), mime, dp[8..].to_vec()));
            });
        } else if fourcc == [0xa9, b'n', b'a', b'm'] || &fourcc == b"\xa9nam" {
            for_each_mp4_box(p, |d_fc, dp| if &d_fc == b"data" && dp.len() >= 8 {
                summary.title = Some(String::from_utf8_lossy(&dp[8..]).trim_matches('\0').to_string());
            });
        }
    });
}

fn classify_by_codec(codec: &str) -> Option<MediaTrackType> {
    match codec {
        "avc1" | "hvc1" | "vp09" | "av01" | "mp4v" | "hev1" | "dvh1" | "dvhe" => Some(MediaTrackType::Video),
        "mp4a" | "ac-3" | "ec-3" | "opus" | "flac" | "alac" | "lpcm" | "samr" => Some(MediaTrackType::Audio),
        "tx3g" | "mov_text" | "wvtt" | "c608" | "c708" | "stpp" | "sbtt" => Some(MediaTrackType::Subtitle),
        _ => None,
    }
}

fn for_each_mp4_box(data: &[u8], mut f: impl FnMut([u8; 4], &[u8])) {
    let mut off = 0;
    while off + 8 <= data.len() {
        let size_32 = u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
        let mut fourcc = [0u8; 4];
        fourcc.copy_from_slice(&data[off + 4..off + 8]);
        let (hdr, tot) = if size_32 == 1 {
            if off + 16 > data.len() { break; }
            (16, u64::from_be_bytes([data[off+8], data[off+9], data[off+10], data[off+11], data[off+12], data[off+13], data[off+14], data[off+15]]) as usize)
        } else if size_32 == 0 {
            (8, data.len() - off)
        } else {
            (8, size_32)
        };
        if tot < hdr { break; }
        let end = (off + tot).min(data.len());
        f(fourcc, if off + hdr <= end { &data[off + hdr..end] } else { &[] });
        off = end;
    }
}
