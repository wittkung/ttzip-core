// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio and video container parsing and media stream sniffing engine.

use std::collections::HashMap;
use super::types::{AudioProbeResult, ImageProbeResult, MediaType, UnifiedMetadataProbe, VideoProbeResult};

/// Outcome of probing an ISOBMFF QuickTime / MP4 / MOV / M4A / HEIC container.
#[derive(Debug, Clone, PartialEq)]
pub enum IsobMffOutcome {
    Image(ImageProbeResult, &'static str, &'static str),
    Video(VideoProbeResult, &'static str, &'static str),
    Audio(AudioProbeResult, &'static str, &'static str),
}

fn simple_audio(dur: f64, sr: u32, ch: u32, depth: u32, br: u32, codec: impl Into<String>) -> AudioProbeResult {
    AudioProbeResult {
        duration_secs: dur,
        sample_rate: sr,
        channels: ch,
        bit_depth: depth,
        bitrate_kbps: br,
        codec: codec.into(),
        title: None,
        artist: None,
        album: None,
    }
}

/// Probes ISOBMFF QuickTime / MP4 / MOV / M4A / HEIC container atoms.
pub fn probe_isobmff(data: &[u8]) -> Option<IsobMffOutcome> {
    if data.len() < 16 || &data[4..8] != b"ftyp" { return None; }
    let brand = &data[8..12];
    let is_heic_or_avif = matches!(brand, b"heic" | b"heix" | b"mif1" | b"msf1" | b"avif" | b"avis");

    let mut state = IsobMffBoxScannerState::default();
    scan_isobmff_boxes(data, 0, data.len(), &mut state, 0);

    if is_heic_or_avif {
        let (fmt, mime) = if brand == b"avif" || brand == b"avis" { ("AVIF Image", "image/avif") } else { ("HEIC Image", "image/heic") };
        let ori = match state.rot { 90 => 6, 180 => 3, 270 => 8, _ => 1 };
        return Some(IsobMffOutcome::Image(ImageProbeResult {
            width: state.vw.max(1), height: state.vh.max(1), orientation: ori, bit_depth: 8,
            color_space: Some("Display P3".to_string()), has_alpha: false,
            camera_make: None, camera_model: None, lens_model: None, focal_length_mm: None,
            f_number: None, exposure_time_secs: None, iso_speed: None, date_time_original: None, icc_profile_name: None,
        }, fmt, mime));
    }

    let br = if state.dur > 0.0 { ((data.len() as f64 * 8.0) / state.dur / 1000.0) as u32 } else { 0 };
    if !state.vcodec.is_empty() || (state.vw > 0 && state.vh > 0) {
        Some(IsobMffOutcome::Video(VideoProbeResult {
            duration_secs: state.dur, width: state.vw, height: state.vh, frame_rate: 30.0,
            video_codec: if state.vcodec.is_empty() { "H.264 / AVC".to_string() } else { state.vcodec },
            audio_codec: if state.acodec.is_empty() { None } else { Some(state.acodec) },
            audio_sample_rate: state.asr, audio_channels: state.ach, bitrate_kbps: br, orientation_degrees: state.rot,
        }, "MPEG-4 Video", "video/mp4"))
    } else if !state.acodec.is_empty() || brand == b"M4A " {
        Some(IsobMffOutcome::Audio(simple_audio(state.dur, if state.asr == 0 { 44100 } else { state.asr }, if state.ach == 0 { 2 } else { state.ach }, 16, br, if state.acodec.is_empty() { "AAC".to_string() } else { state.acodec }), "MPEG-4 Audio", "audio/mp4"))
    } else {
        None
    }
}

#[derive(Debug, Default)]
struct IsobMffBoxScannerState {
    dur: f64,
    vw: u32,
    vh: u32,
    vcodec: String,
    acodec: String,
    asr: u32,
    ach: u32,
    rot: u32,
}

fn scan_isobmff_boxes(
    data: &[u8],
    mut off: usize,
    end: usize,
    state: &mut IsobMffBoxScannerState,
    depth: usize,
) {
    if depth > 6 { return; }
    while off + 8 <= end && off + 8 <= data.len() {
        let bsize = u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
        let btype = &data[off + 4..off + 8];
        let actual = if bsize == 1 && off + 16 <= data.len() {
            u64::from_be_bytes([data[off+8], data[off+9], data[off+10], data[off+11], data[off+12], data[off+13], data[off+14], data[off+15]]) as usize
        } else if bsize == 0 { end - off } else { bsize };
        if actual < 8 || off + actual > end.min(data.len()) { break; }

        let (p_off, p_end) = (if bsize == 1 { off + 16 } else { off + 8 }, off + actual);
        match btype {
            b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" | b"iprp" | b"ipco" => {
                scan_isobmff_boxes(data, p_off, p_end, state, depth + 1);
            }
            b"meta" => {
                scan_isobmff_boxes(data, (p_off + 4).min(p_end), p_end, state, depth + 1);
            }
            b"mvhd" if p_off + 20 <= p_end => {
                let v = data[p_off];
                if v == 0 && p_off + 20 <= data.len() {
                    let ts = u32::from_be_bytes([data[p_off+12], data[p_off+13], data[p_off+14], data[p_off+15]]) as f64;
                    let d = u32::from_be_bytes([data[p_off+16], data[p_off+17], data[p_off+18], data[p_off+19]]) as f64;
                    if ts > 0.0 { state.dur = d / ts; }
                } else if v == 1 && p_off + 28 <= data.len() {
                    let ts = u32::from_be_bytes([data[p_off+20], data[p_off+21], data[p_off+22], data[p_off+23]]) as f64;
                    let d = u64::from_be_bytes([data[p_off+24], data[p_off+25], data[p_off+26], data[p_off+27], data[p_off+28], data[p_off+29], data[p_off+30], data[p_off+31]]) as f64;
                    if ts > 0.0 { state.dur = d / ts; }
                }
            }
            b"tkhd" if p_off + 84 <= p_end => {
                let m_off = if data[p_off] == 0 { p_off + 40 } else { p_off + 52 };
                let dim_off = m_off + 36;
                if dim_off + 8 <= data.len() {
                    let (w, h) = (u32::from_be_bytes([data[dim_off], data[dim_off+1], data[dim_off+2], data[dim_off+3]]) >> 16,
                                  u32::from_be_bytes([data[dim_off+4], data[dim_off+5], data[dim_off+6], data[dim_off+7]]) >> 16);
                    if w > 0 && h > 0 { state.vw = w; state.vh = h; }
                }
                if m_off + 16 <= data.len() {
                    let (m0, m1, m3) = (i32::from_be_bytes([data[m_off], data[m_off+1], data[m_off+2], data[m_off+3]]),
                                        i32::from_be_bytes([data[m_off+4], data[m_off+5], data[m_off+6], data[m_off+7]]),
                                        i32::from_be_bytes([data[m_off+12], data[m_off+13], data[m_off+14], data[m_off+15]]));
                    if m1 == 0x0001_0000 && m3 == -0x0001_0000 { state.rot = 90; }
                    else if m0 == -0x0001_0000 && m1 == 0 && m3 == 0 { state.rot = 180; }
                    else if m1 == -0x0001_0000 && m3 == 0x0001_0000 { state.rot = 270; }
                }
            }
            b"ispe" if p_off + 12 <= p_end => {
                state.vw = u32::from_be_bytes([data[p_off+4], data[p_off+5], data[p_off+6], data[p_off+7]]);
                state.vh = u32::from_be_bytes([data[p_off+8], data[p_off+9], data[p_off+10], data[p_off+11]]);
            }
            b"irot" if p_off < p_end => {
                state.rot = match data[p_off] & 0x03 { 1 => 90, 2 => 180, 3 => 270, _ => 0 };
            }
            b"stsd" if p_off + 8 <= p_end => {
                let cnt = u32::from_be_bytes([data[p_off+4], data[p_off+5], data[p_off+6], data[p_off+7]]);
                let mut e_off = p_off + 8;
                for _ in 0..cnt.min(8) {
                    if e_off + 8 > p_end { break; }
                    let esz = u32::from_be_bytes([data[e_off], data[e_off+1], data[e_off+2], data[e_off+3]]) as usize;
                    match &data[e_off+4..e_off+8] {
                        b"avc1" | b"avc3" => state.vcodec = "H.264 / AVC".to_string(),
                        b"hvc1" | b"hev1" => state.vcodec = "HEVC / H.265".to_string(),
                        b"av01" => state.vcodec = "AV1".to_string(),
                        b"vp09" => state.vcodec = "VP9".to_string(),
                        b"apcn" => state.vcodec = "Apple ProRes 422".to_string(),
                        b"apch" => state.vcodec = "Apple ProRes 422 HQ".to_string(),
                        b"ap4h" => state.vcodec = "Apple ProRes 4444".to_string(),
                        b"mp4a" => state.acodec = "AAC".to_string(),
                        b"alac" => state.acodec = "Apple Lossless (ALAC)".to_string(),
                        b"ac-3" => state.acodec = "Dolby Digital (AC-3)".to_string(),
                        b"ec-3" => state.acodec = "Dolby Digital Plus (E-AC-3)".to_string(),
                        b"Opus" | b"opus" => state.acodec = "Opus".to_string(),
                        b"fLaC" | b"flac" => state.acodec = "FLAC".to_string(),
                        _ => {}
                    }
                    let is_audio = matches!(&data[e_off+4..e_off+8], b"mp4a" | b"alac" | b"ac-3" | b"ec-3" | b"Opus" | b"opus" | b"fLaC" | b"flac");
                    if is_audio && esz >= 28 && e_off + 28 <= p_end {
                        if state.ach == 0 {
                            state.ach = u16::from_be_bytes([data[e_off + 16], data[e_off + 17]]) as u32;
                        }
                        if state.asr == 0 {
                            state.asr = u32::from_be_bytes([data[e_off + 24], data[e_off + 25], data[e_off + 26], data[e_off + 27]]) >> 16;
                        }
                    }
                    if esz < 8 { break; }
                    e_off += esz;
                }
            }
            _ => {}
        }
        off += actual;
    }
}

/// Probes MKV / WebM (EBML header and Matroska Segment/Tracks).
pub fn probe_ebml(data: &[u8]) -> Option<UnifiedMetadataProbe> {
    if data.len() < 12 || data[0..4] != [0x1A, 0x45, 0xDF, 0xA3] { return None; }
    let is_webm = data.windows(4).any(|w| w == b"webm");
    let (fmt_name, mime_type) = if is_webm { ("WebM Media", "video/webm") } else { ("Matroska Video (MKV)", "video/x-matroska") };
    let (mut vw, mut vh, mut dur) = (1920u32, 1080u32, 0.0f64);

    for i in 0..data.len().saturating_sub(6) {
        if data[i] == 0xB0 && data[i + 1] == 0x82 {
            vw = u16::from_be_bytes([data[i + 2], data[i + 3]]) as u32;
        } else if data[i] == 0xBA && data[i + 1] == 0x82 {
            vh = u16::from_be_bytes([data[i + 2], data[i + 3]]) as u32;
        } else if data[i..i + 3] == [0x44, 0x89, 0x84] {
            let dur_ms = f32::from_be_bytes([data[i + 3], data[i + 4], data[i + 5], data[i + 6]]);
            dur = (dur_ms as f64) / 1000.0;
        }
    }

    let vid = VideoProbeResult {
        duration_secs: dur, width: vw, height: vh, frame_rate: 30.0,
        video_codec: if is_webm { "VP9 / VP8".to_string() } else { "H.264 / HEVC".to_string() },
        audio_codec: Some(if is_webm { "Opus".to_string() } else { "AAC".to_string() }),
        audio_sample_rate: 48000, audio_channels: 2,
        bitrate_kbps: if dur > 0.0 { ((data.len() as f64 * 8.0) / dur / 1000.0) as u32 } else { 0 },
        orientation_degrees: 0,
    };

    let mut attributes = HashMap::new();
    attributes.insert("Format".to_string(), fmt_name.to_string());
    attributes.insert("Resolution".to_string(), format!("{vw} × {vh}"));
    attributes.insert("Video Codec".to_string(), vid.video_codec.clone());
    if dur > 0.0 { attributes.insert("Duration".to_string(), format!("{dur:.2}s")); }

    Some(UnifiedMetadataProbe {
        media_type: MediaType::Video, format_name: fmt_name.to_string(), mime_type: mime_type.to_string(),
        file_size: data.len() as u64, is_container: true, image: None, audio: None, video: Some(vid),
        font: None, model_3d: None, document: None, attributes,
    })
}

/// Probes FLAC audio (STREAMINFO header).
pub fn probe_flac(data: &[u8]) -> Option<AudioProbeResult> {
    if data.len() < 42 || !data.starts_with(b"fLaC") { return None; }
    let sinfo = &data[8..42];
    let sr = ((sinfo[10] as u32) << 12) | ((sinfo[11] as u32) << 4) | ((sinfo[12] as u32) >> 4);
    let ch = (((sinfo[12] >> 1) & 0x07) as u32) + 1;
    let bps = ((((sinfo[12] & 0x01) << 4) | (sinfo[13] >> 4)) as u32) + 1;
    let total = (((sinfo[13] & 0x0F) as u64) << 32) | ((sinfo[14] as u64) << 24) | ((sinfo[15] as u64) << 16) | ((sinfo[16] as u64) << 8) | (sinfo[17] as u64);
    let dur = if sr > 0 { total as f64 / sr as f64 } else { 0.0 };
    let br = if dur > 0.0 { ((data.len() as f64 * 8.0) / dur / 1000.0) as u32 } else { 0 };
    Some(simple_audio(dur, sr, ch, bps, br, "FLAC (Free Lossless Audio Codec)"))
}

/// Probes WAV / RIFF PCM format.
pub fn probe_wav(data: &[u8]) -> Option<AudioProbeResult> {
    if data.len() < 36 || !data.starts_with(b"RIFF") || &data[8..12] != b"WAVE" { return None; }
    let (mut off, mut ch, mut sr, mut byte_rate, mut bps, mut data_sz) = (12, 2u32, 44100u32, 176400u32, 16u32, 0usize);

    while off + 8 <= data.len() {
        let (id, len) = (&data[off..off + 4], u32::from_le_bytes([data[off+4], data[off+5], data[off+6], data[off+7]]) as usize);
        let c_off = off + 8;
        if id == b"fmt " && c_off + 14 <= data.len() {
            ch = u16::from_le_bytes([data[c_off+2], data[c_off+3]]) as u32;
            sr = u32::from_le_bytes([data[c_off+4], data[c_off+5], data[c_off+6], data[c_off+7]]);
            byte_rate = u32::from_le_bytes([data[c_off+8], data[c_off+9], data[c_off+10], data[c_off+11]]);
            if c_off + 16 <= data.len() { bps = u16::from_le_bytes([data[c_off+14], data[c_off+15]]) as u32; }
        } else if id == b"data" {
            data_sz = len;
            break;
        }
        off += 8 + ((len + 1) & !1);
    }
    let dur = if byte_rate > 0 { data_sz as f64 / byte_rate as f64 } else { 0.0 };
    Some(simple_audio(dur, sr, ch, bps, (byte_rate * 8) / 1000, "Linear PCM (WAV)"))
}

/// Probes AIFF / AIFC format.
pub fn probe_aiff(data: &[u8]) -> Option<AudioProbeResult> {
    if data.len() < 38 || !data.starts_with(b"FORM") || (&data[8..12] != b"AIFF" && &data[8..12] != b"AIFC") { return None; }
    let (mut off, mut ch, mut frames, mut size, mut sr) = (12, 2u32, 0u32, 16u32, 44100.0f64);

    while off + 8 <= data.len() {
        let (id, len) = (&data[off..off + 4], u32::from_be_bytes([data[off+4], data[off+5], data[off+6], data[off+7]]) as usize);
        let c_off = off + 8;
        if id == b"COMM" && c_off + 18 <= data.len() {
            ch = u16::from_be_bytes([data[c_off], data[c_off+1]]) as u32;
            frames = u32::from_be_bytes([data[c_off+2], data[c_off+3], data[c_off+4], data[c_off+5]]);
            size = u16::from_be_bytes([data[c_off+6], data[c_off+7]]) as u32;
            let exp = u16::from_be_bytes([data[c_off+8], data[c_off+9]]) & 0x7FFF;
            let mant = u64::from_be_bytes([data[c_off+10], data[c_off+11], data[c_off+12], data[c_off+13], data[c_off+14], data[c_off+15], data[c_off+16], data[c_off+17]]);
            if exp >= 16383 { sr = (mant as f64) * (2.0f64).powi(exp as i32 - 16383 - 63); }
            break;
        }
        off += 8 + ((len + 1) & !1);
    }
    let dur = if sr > 0.0 { frames as f64 / sr } else { 0.0 };
    let br = if dur > 0.0 { ((data.len() as f64 * 8.0) / dur / 1000.0) as u32 } else { 0 };
    Some(simple_audio(dur, sr as u32, ch, size, br, "Linear PCM (AIFF)"))
}

/// Probes OGG Vorbis / Opus / FLAC.
pub fn probe_ogg(data: &[u8]) -> Option<AudioProbeResult> {
    if data.len() < 30 || !data.starts_with(b"OggS") { return None; }
    if let Some(pos) = data.windows(7).position(|w| w == b"\x01vorbis") {
        if pos + 24 <= data.len() {
            let ch = data[pos + 11] as u32;
            let sr = u32::from_le_bytes([data[pos+12], data[pos+13], data[pos+14], data[pos+15]]);
            let br = u32::from_le_bytes([data[pos+20], data[pos+21], data[pos+22], data[pos+23]]) / 1000;
            return Some(simple_audio(0.0, sr, ch, 16, br, "Ogg Vorbis"));
        }
    } else if let Some(pos) = data.windows(8).position(|w| w == b"OpusHead") {
        if pos + 16 <= data.len() {
            let ch = data[pos + 9] as u32;
            let sr = u32::from_le_bytes([data[pos+12], data[pos+13], data[pos+14], data[pos+15]]);
            return Some(simple_audio(0.0, sr, ch, 16, 128, "Opus Audio"));
        }
    }
    None
}

/// Probes MP3 (MPEG Audio Layer III & ID3v2 tags).
pub fn probe_mp3(data: &[u8]) -> Option<AudioProbeResult> {
    if data.len() < 10 { return None; }
    let (mut id3_sz, mut title, mut artist, mut album) = (0usize, None, None, None);
    let starts_with_id3 = data.starts_with(b"ID3");

    if starts_with_id3 && data.len() >= 10 {
        let (b6, b7, b8, b9) = ((data[6] & 0x7F) as usize, (data[7] & 0x7F) as usize, (data[8] & 0x7F) as usize, (data[9] & 0x7F) as usize);
        id3_sz = 10 + ((b6 << 21) | (b7 << 14) | (b8 << 7) | b9);
        let tag_data = &data[10..id3_sz.min(data.len())];
        let mut t_off = 0;
        while t_off + 10 <= tag_data.len() {
            let fid = &tag_data[t_off..t_off + 4];
            let fsz = u32::from_be_bytes([tag_data[t_off+4], tag_data[t_off+5], tag_data[t_off+6], tag_data[t_off+7]]) as usize;
            if fsz == 0 || t_off + 10 + fsz > tag_data.len() { break; }
            let fpay = &tag_data[t_off + 10..t_off + 10 + fsz];
            if !fpay.is_empty() {
                let slice = if fpay[0] == 0 { &fpay[1..] } else { fpay };
                let val = String::from_utf8_lossy(slice).trim_matches('\0').trim().to_string();
                if !val.is_empty() {
                    match fid {
                        b"TIT2" => title = Some(val),
                        b"TPE1" => artist = Some(val),
                        b"TALB" => album = Some(val),
                        _ => {}
                    }
                }
            }
            t_off += 10 + fsz;
        }
    }

    let mut sync_off = id3_sz;
    let (mut sr, mut br, mut ch, mut found) = (0u32, 0u32, 2u32, false);
    let scan_limit = if starts_with_id3 { id3_sz + 4096 } else { 16 };

    while sync_off + 4 <= data.len() && sync_off < scan_limit {
        if data[sync_off] == 0xFF && (data[sync_off + 1] & 0xE0) == 0xE0 {
            let (b1, b2, b3) = (data[sync_off + 1], data[sync_off + 2], data[sync_off + 3]);
            let (layer, br_idx, sr_idx, ch_mode) = ((b1 >> 1) & 0x03, (b2 >> 4) & 0x0F, (b2 >> 2) & 0x03, (b3 >> 6) & 0x03);
            if br_idx > 0 && br_idx < 15 && sr_idx < 3 && layer == 1 {
                ch = if ch_mode == 3 { 1 } else { 2 };
                sr = match sr_idx { 0 => 44100, 1 => 48000, 2 => 32000, _ => 44100 };
                br = match br_idx {
                    1 => 32, 2 => 40, 3 => 48, 4 => 56, 5 => 64, 6 => 80, 7 => 96, 8 => 112,
                    9 => 128, 10 => 160, 11 => 192, 12 => 224, 13 => 256, 14 => 320, _ => 128,
                };
                found = true;
                break;
            }
        }
        sync_off += 1;
    }

    if !found && !starts_with_id3 { return None; }
    if sr == 0 { sr = 44100; }
    if br == 0 { br = 128; }
    let dur = if br > 0 { (data.len().saturating_sub(id3_sz) as f64 * 8.0) / (br as f64 * 1000.0) } else { 0.0 };

    Some(AudioProbeResult {
        duration_secs: dur, sample_rate: sr, channels: ch, bit_depth: 16, bitrate_kbps: br,
        codec: "MPEG Audio Layer III (MP3)".to_string(), title, artist, album,
    })
}
