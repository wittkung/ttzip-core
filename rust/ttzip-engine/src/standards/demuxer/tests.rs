// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit tests for Matroska (MKV) and ISO BMFF (MP4) container demuxing.

use crate::types::status::TTZipStatus;
use super::types::MediaTrackType;
use super::*;

fn ebml_box(id: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if id > 0x00FF_FFFF { out.extend_from_slice(&id.to_be_bytes()); }
    else if id > 0xFFFF { out.extend_from_slice(&id.to_be_bytes()[1..]); }
    else if id > 0xFF { out.extend_from_slice(&id.to_be_bytes()[2..]); }
    else { out.push(id as u8); }
    let sz = data.len();
    if sz < 0x7F { out.push(0x80 | (sz as u8)); }
    else if sz < 0x3FFF { out.push(0x40 | ((sz >> 8) as u8)); out.push((sz & 0xFF) as u8); }
    else { out.push(0x20 | ((sz >> 16) as u8)); out.push(((sz >> 8) & 0xFF) as u8); out.push((sz & 0xFF) as u8); }
    out.extend_from_slice(data);
    out
}

fn ebml_uint(id: u32, val: u64, bytes: usize) -> Vec<u8> {
    ebml_box(id, &val.to_be_bytes()[8 - bytes..])
}

fn ebml_str(id: u32, s: &str) -> Vec<u8> {
    ebml_box(id, s.as_bytes())
}

fn ebml_float32(id: u32, val: f32) -> Vec<u8> {
    ebml_box(id, &val.to_be_bytes())
}

fn mp4_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let total_len = (payload.len() + 8) as u32;
    let mut out = Vec::with_capacity(total_len as usize);
    out.extend_from_slice(&total_len.to_be_bytes());
    out.extend_from_slice(fourcc);
    out.extend_from_slice(payload);
    out
}

#[test]
fn test_mkv_demux_synthetic_container() {
    let ebml_hdr = ebml_box(0x1A45_DFA3, &ebml_str(0x4282, "matroska"));

    let mut info_b = Vec::new();
    info_b.extend(ebml_uint(0x2AD7_B1, 1_000_000, 3));
    info_b.extend(ebml_float32(0x4489, 65000.0));
    info_b.extend(ebml_str(0x7BA9, "Synthetic Anime EP01"));
    let info = ebml_box(0x1549_A966, &info_b);

    let mut v_sub = Vec::new();
    v_sub.extend(ebml_uint(0xB0, 3840, 2));
    v_sub.extend(ebml_uint(0xBA, 2160, 2));
    let mut v_b = Vec::new();
    v_b.extend(ebml_uint(0xD7, 1, 1));
    v_b.extend(ebml_uint(0x83, 1, 1)); // Video
    v_b.extend(ebml_str(0x86, "V_MPEGH/ISO/HEVC"));
    v_b.extend(ebml_str(0x536E, "4K HDR Video"));
    v_b.extend(ebml_box(0xE0, &v_sub));
    let t1 = ebml_box(0xAE, &v_b);

    let mut a_sub = Vec::new();
    a_sub.extend(ebml_uint(0x9F, 6, 1));
    a_sub.extend(ebml_float32(0xB5, 48000.0));
    let mut a_b = Vec::new();
    a_b.extend(ebml_uint(0xD7, 2, 1));
    a_b.extend(ebml_uint(0x83, 2, 1)); // Audio
    a_b.extend(ebml_str(0x86, "A_OPUS"));
    a_b.extend(ebml_str(0x22B5_9C, "jpn"));
    a_b.extend(ebml_box(0xE1, &a_sub));
    let t2 = ebml_box(0xAE, &a_b);

    let mut s_b = Vec::new();
    s_b.extend(ebml_uint(0xD7, 3, 1));
    s_b.extend(ebml_uint(0x83, 17, 1)); // Subtitle
    s_b.extend(ebml_str(0x86, "S_TEXT/ASS"));
    s_b.extend(ebml_str(0x536E, "English Dialogue"));
    s_b.extend(ebml_str(0x22B5_9C, "eng"));
    let t3 = ebml_box(0xAE, &s_b);

    let mut trk_b = Vec::new();
    trk_b.extend(t1); trk_b.extend(t2); trk_b.extend(t3);
    let tracks = ebml_box(0x1654_AE6B, &trk_b);

    let mut c1_b = ebml_uint(0x91, 0, 1);
    c1_b.extend(ebml_box(0x80, &ebml_str(0x85, "Intro")));
    let mut c2_b = ebml_uint(0x91, 90_000_000_000, 5);
    c2_b.extend(ebml_box(0x80, &ebml_str(0x85, "Part 1")));
    let mut ed_b = ebml_box(0xB6, &c1_b);
    ed_b.extend(ebml_box(0xB6, &c2_b));
    let chaps = ebml_box(0x1043_A770, &ebml_box(0x45B9, &ed_b));

    let mut att1_b = ebml_str(0x466E, "cover.jpg");
    att1_b.extend(ebml_str(0x4660, "image/jpeg"));
    att1_b.extend(ebml_box(0x465C, &[0xFF, 0xD8, 0xFF, 0xE0]));
    let atts = ebml_box(0x1941_A469, &ebml_box(0x61A7, &att1_b));

    let mut seg_b = Vec::new();
    seg_b.extend(info); seg_b.extend(tracks); seg_b.extend(chaps); seg_b.extend(atts);
    let mut mkv = ebml_hdr;
    mkv.extend(ebml_box(0x1853_8067, &seg_b));

    let sum = demux_media_tracks_from_slice(&mkv).expect("mkv demux failed");
    assert_eq!(sum.container_format, "matroska");
    assert_eq!(sum.title.as_deref(), Some("Synthetic Anime EP01"));
    assert_eq!(sum.duration_ms, Some(65000));
    assert_eq!(sum.tracks.len(), 3);

    let v = sum.video_tracks().next().expect("video track");
    assert_eq!(v.codec, "V_MPEGH/ISO/HEVC");
    assert_eq!(v.width, Some(3840));
    assert_eq!(v.height, Some(2160));

    let a = sum.audio_tracks().next().expect("audio track");
    assert_eq!(a.codec, "A_OPUS");
    assert_eq!(a.channels, Some(6));
    assert_eq!(a.sample_rate, Some(48000));
    assert_eq!(a.language.as_deref(), Some("jpn"));

    let s = sum.subtitle_tracks().next().expect("sub track");
    assert_eq!(s.codec, "S_TEXT/ASS");
    assert_eq!(s.title.as_deref(), Some("English Dialogue"));
    assert_eq!(s.language.as_deref(), Some("eng"));

    assert_eq!(sum.chapters.len(), 2);
    assert_eq!(sum.chapters[0].title, "Intro");
    assert_eq!(sum.chapters[1].start_time_ms, 90000);
    assert_eq!(sum.chapters[1].title, "Part 1");

    assert_eq!(sum.attachments.len(), 1);
    assert_eq!(sum.attachments[0].file_name, "cover.jpg");
    assert_eq!(sum.attachments[0].mime_type, "image/jpeg");
    assert_eq!(sum.attachments[0].data, vec![0xFF, 0xD8, 0xFF, 0xE0]);
}

#[test]
fn test_mp4_demux_synthetic_container() {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\x02\0isommp42");

    let mut mvhd_p = vec![0u8; 100];
    mvhd_p[12..16].copy_from_slice(&1000u32.to_be_bytes());
    mvhd_p[16..20].copy_from_slice(&120000u32.to_be_bytes());
    let mvhd = mp4_box(b"mvhd", &mvhd_p);

    // Video Track
    let mut tkhd1_p = vec![0u8; 84];
    tkhd1_p[12..16].copy_from_slice(&1u32.to_be_bytes());
    tkhd1_p[76..80].copy_from_slice(&(1920u32 << 16).to_be_bytes());
    tkhd1_p[80..84].copy_from_slice(&(1080u32 << 16).to_be_bytes());
    let mut mdhd1_p = vec![0u8; 24];
    mdhd1_p[12..14].copy_from_slice(&0x15C7u16.to_be_bytes()); // "eng"
    let mut hdlr1_p = vec![0u8; 24];
    hdlr1_p[8..12].copy_from_slice(b"vide");
    let mut avc1_p = vec![0u8; 40];
    avc1_p[4..8].copy_from_slice(b"avc1");
    avc1_p[32..34].copy_from_slice(&1920u16.to_be_bytes());
    avc1_p[34..36].copy_from_slice(&1080u16.to_be_bytes());
    let mut stsd1_p = vec![0u8; 8];
    stsd1_p[4..8].copy_from_slice(&1u32.to_be_bytes());
    stsd1_p.extend_from_slice(&avc1_p);
    let mut mdia1_p = Vec::new();
    mdia1_p.extend(mp4_box(b"mdhd", &mdhd1_p));
    mdia1_p.extend(mp4_box(b"hdlr", &hdlr1_p));
    mdia1_p.extend(mp4_box(b"minf", &mp4_box(b"stbl", &mp4_box(b"stsd", &stsd1_p))));
    let mut trak1_p = mp4_box(b"tkhd", &tkhd1_p);
    trak1_p.extend(mp4_box(b"mdia", &mdia1_p));

    // Subtitle Track
    let mut tkhd3_p = vec![0u8; 84];
    tkhd3_p[12..16].copy_from_slice(&3u32.to_be_bytes());
    let mut hdlr3_p = vec![0u8; 24];
    hdlr3_p[8..12].copy_from_slice(b"sbtl");
    let mut tx3g_p = vec![0u8; 16];
    tx3g_p[4..8].copy_from_slice(b"tx3g");
    let mut stsd3_p = vec![0u8; 8];
    stsd3_p[4..8].copy_from_slice(&1u32.to_be_bytes());
    stsd3_p.extend_from_slice(&tx3g_p);
    let mut mdia3_p = mp4_box(b"hdlr", &hdlr3_p);
    mdia3_p.extend(mp4_box(b"minf", &mp4_box(b"stbl", &mp4_box(b"stsd", &stsd3_p))));
    let mut trak3_p = mp4_box(b"tkhd", &tkhd3_p);
    trak3_p.extend(mp4_box(b"mdia", &mdia3_p));

    // Chapters and Cover
    let mut chpl_p = vec![0u8, 0, 0, 0, 0, 2];
    chpl_p.extend_from_slice(&0u64.to_be_bytes());
    chpl_p.push(5); chpl_p.extend_from_slice(b"Start");
    chpl_p.extend_from_slice(&(300_000_000u64).to_be_bytes());
    chpl_p.push(6); chpl_p.extend_from_slice(b"Middle");

    let mut covr_data_p = vec![0u8; 8];
    covr_data_p[0..4].copy_from_slice(&13u32.to_be_bytes()); // JPEG
    covr_data_p.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xDB]);
    let mut name_data_p = vec![0u8; 8];
    name_data_p[0..4].copy_from_slice(&1u32.to_be_bytes());
    name_data_p.extend_from_slice(b"Epic Movie");

    let mut ilst_p = mp4_box(b"covr", &mp4_box(b"data", &covr_data_p));
    ilst_p.extend(mp4_box(b"\xa9nam", &mp4_box(b"data", &name_data_p)));
    let mut meta_p = vec![0u8; 4];
    meta_p.extend(mp4_box(b"ilst", &ilst_p));

    let mut udta_p = mp4_box(b"chpl", &chpl_p);
    udta_p.extend(mp4_box(b"meta", &meta_p));

    let mut moov_p = mvhd;
    moov_p.extend(mp4_box(b"trak", &trak1_p));
    moov_p.extend(mp4_box(b"trak", &trak3_p));
    moov_p.extend(mp4_box(b"udta", &udta_p));

    let mut mp4 = ftyp;
    mp4.extend(mp4_box(b"moov", &moov_p));

    let sum = demux_media_tracks_from_slice(&mp4).expect("mp4 demux failed");
    assert_eq!(sum.container_format, "MP4");
    assert_eq!(sum.title.as_deref(), Some("Epic Movie"));
    assert_eq!(sum.duration_ms, Some(120000));
    assert_eq!(sum.tracks.len(), 2);

    let v = sum.video_tracks().next().expect("video track");
    assert_eq!(v.codec, "avc1");
    assert_eq!(v.track_id, 1);
    assert_eq!(v.width, Some(1920));
    assert_eq!(v.height, Some(1080));
    assert_eq!(v.language.as_deref(), Some("eng"));

    let s = sum.subtitle_tracks().next().expect("subtitle track");
    assert_eq!(s.codec, "tx3g");
    assert_eq!(s.track_type, MediaTrackType::Subtitle);

    assert_eq!(sum.chapters.len(), 2);
    assert_eq!(sum.chapters[0].title, "Start");
    assert_eq!(sum.chapters[1].title, "Middle");
    assert_eq!(sum.chapters[1].start_time_ms, 30000);

    assert_eq!(sum.attachments.len(), 1);
    assert_eq!(sum.attachments[0].file_name, "cover.jpg");
    assert_eq!(sum.attachments[0].data, vec![0xFF, 0xD8, 0xFF, 0xDB]);
}

#[test]
fn test_webm_and_quicktime_containers() {
    let ebml_hdr = ebml_box(0x1A45_DFA3, &ebml_str(0x4282, "webm"));
    let mut webm = ebml_hdr;
    webm.extend(ebml_box(0x1853_8067, &[]));
    let sum = demux_media_tracks_from_slice(&webm).expect("webm demux");
    assert_eq!(sum.container_format, "WebM");

    let ftyp = mp4_box(b"ftyp", b"qt  \0\0\x02\0qt  ");
    let mut qt = ftyp;
    qt.extend(mp4_box(b"moov", &[]));
    let q_sum = demux_media_tracks_from_slice(&qt).expect("qt demux");
    assert_eq!(q_sum.container_format, "QuickTime");
}

#[test]
fn test_error_and_edge_cases() {
    assert_eq!(demux_media_tracks_from_slice(&[]), Err(TTZipStatus::ErrInvalidParam));
    assert_eq!(demux_media_tracks_from_slice(&[0x00, 0x00]), Err(TTZipStatus::ErrInvalidParam));
    assert_eq!(
        demux_media_tracks_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]),
        Err(TTZipStatus::ErrCorruptHeader)
    );
}
