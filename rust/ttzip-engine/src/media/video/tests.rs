// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive unit tests and synthetic container fixtures for video demuxer microkernel.

use super::avi::TTZipAviDemuxer;
use super::demuxer::TTZipVideoDemuxer;
use super::mkv::TTZipMkvDemuxer;
use super::mp4::TTZipMp4Demuxer;
use super::types::{AudioCodec, VideoCodec, VideoFormat};

// ============================================================================
// Synthetic MP4 Container Builder Helpers
// ============================================================================

fn make_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = (payload.len() + 8) as u32;
    let mut buf = Vec::with_capacity(size as usize);
    buf.extend_from_slice(&size.to_be_bytes());
    buf.extend_from_slice(fourcc);
    buf.extend_from_slice(payload);
    buf
}

fn build_synthetic_mp4(fast_start: bool, with_cover: bool) -> Vec<u8> {
    let mut ftyp_payload = Vec::new();
    ftyp_payload.extend_from_slice(b"isom"); // Major brand
    ftyp_payload.extend_from_slice(&0x00000200u32.to_be_bytes()); // Minor version
    ftyp_payload.extend_from_slice(b"isomiso2mp41"); // Compatible brands
    let ftyp_box = make_box(b"ftyp", &ftyp_payload);

    // mvhd: version 0, timescale 1000, duration 10000 (10 seconds)
    let mut mvhd_payload = vec![0u8; 100];
    mvhd_payload[12..16].copy_from_slice(&1000u32.to_be_bytes()); // Timescale
    mvhd_payload[16..20].copy_from_slice(&10000u32.to_be_bytes()); // Duration
    let mvhd_box = make_box(b"mvhd", &mvhd_payload);

    // Track 1: Video (H.264 / avc1, 1920x1080, 60fps)
    let mut tkhd1_payload = vec![0u8; 84];
    tkhd1_payload[12..16].copy_from_slice(&1u32.to_be_bytes()); // Track ID 1
    tkhd1_payload[76..80].copy_from_slice(&(1920u32 << 16).to_be_bytes()); // Width fixed-point
    tkhd1_payload[80..84].copy_from_slice(&(1080u32 << 16).to_be_bytes()); // Height fixed-point
    let tkhd1_box = make_box(b"tkhd", &tkhd1_payload);

    let mut mdhd1_payload = vec![0u8; 24];
    mdhd1_payload[12..16].copy_from_slice(&60000u32.to_be_bytes()); // Timescale
    mdhd1_payload[16..20].copy_from_slice(&600000u32.to_be_bytes()); // Duration (10s)
    let mdhd1_box = make_box(b"mdhd", &mdhd1_payload);

    let mut hdlr1_payload = vec![0u8; 24];
    hdlr1_payload[8..12].copy_from_slice(b"vide");
    let hdlr1_box = make_box(b"hdlr", &hdlr1_payload);

    // stsd: avc1 sample entry
    let mut avc1_payload = vec![0u8; 78];
    avc1_payload[24..26].copy_from_slice(&1920u16.to_be_bytes());
    avc1_payload[26..28].copy_from_slice(&1080u16.to_be_bytes());
    let avc1_box = make_box(b"avc1", &avc1_payload);

    let mut stsd1_payload = vec![0u8; 8];
    stsd1_payload[4..8].copy_from_slice(&1u32.to_be_bytes()); // Entry count = 1
    stsd1_payload.extend_from_slice(&avc1_box);
    let stsd1_box = make_box(b"stsd", &stsd1_payload);

    // stts: 600 samples, each delta 1000 (total duration 600,000 / 60000 = 10s -> 60fps)
    let mut stts1_payload = vec![0u8; 16];
    stts1_payload[4..8].copy_from_slice(&1u32.to_be_bytes()); // 1 entry
    stts1_payload[8..12].copy_from_slice(&600u32.to_be_bytes()); // 600 samples
    stts1_payload[12..16].copy_from_slice(&1000u32.to_be_bytes()); // Delta 1000
    let stts1_box = make_box(b"stts", &stts1_payload);

    let mut stbl1_payload = Vec::new();
    stbl1_payload.extend_from_slice(&stsd1_box);
    stbl1_payload.extend_from_slice(&stts1_box);
    let stbl1_box = make_box(b"stbl", &stbl1_payload);

    let mut minf1_payload = Vec::new();
    minf1_payload.extend_from_slice(&stbl1_box);
    let minf1_box = make_box(b"minf", &minf1_payload);

    let mut mdia1_payload = Vec::new();
    mdia1_payload.extend_from_slice(&mdhd1_box);
    mdia1_payload.extend_from_slice(&hdlr1_box);
    mdia1_payload.extend_from_slice(&minf1_box);
    let mdia1_box = make_box(b"mdia", &mdia1_payload);

    let mut trak1_payload = Vec::new();
    trak1_payload.extend_from_slice(&tkhd1_box);
    trak1_payload.extend_from_slice(&mdia1_box);
    let trak1_box = make_box(b"trak", &trak1_payload);

    // Track 2: Audio (AAC / mp4a, 2 channels, 48000 Hz)
    let mut tkhd2_payload = vec![0u8; 84];
    tkhd2_payload[12..16].copy_from_slice(&2u32.to_be_bytes()); // Track ID 2
    let tkhd2_box = make_box(b"tkhd", &tkhd2_payload);

    let mut mdhd2_payload = vec![0u8; 24];
    mdhd2_payload[12..16].copy_from_slice(&48000u32.to_be_bytes()); // Timescale 48000
    mdhd2_payload[16..20].copy_from_slice(&480000u32.to_be_bytes()); // Duration 10s
    let mdhd2_box = make_box(b"mdhd", &mdhd2_payload);

    let mut hdlr2_payload = vec![0u8; 24];
    hdlr2_payload[8..12].copy_from_slice(b"soun");
    let hdlr2_box = make_box(b"hdlr", &hdlr2_payload);

    let mut mp4a_payload = vec![0u8; 28];
    mp4a_payload[16..18].copy_from_slice(&2u16.to_be_bytes()); // 2 channels
    mp4a_payload[22..24].copy_from_slice(&48000u16.to_be_bytes()); // Sample rate
    let mp4a_box = make_box(b"mp4a", &mp4a_payload);

    let mut stsd2_payload = vec![0u8; 8];
    stsd2_payload[4..8].copy_from_slice(&1u32.to_be_bytes());
    stsd2_payload.extend_from_slice(&mp4a_box);
    let stsd2_box = make_box(b"stsd", &stsd2_payload);

    let mut stbl2_payload = Vec::new();
    stbl2_payload.extend_from_slice(&stsd2_box);
    let stbl2_box = make_box(b"stbl", &stbl2_payload);

    let mut minf2_payload = Vec::new();
    minf2_payload.extend_from_slice(&stbl2_box);
    let minf2_box = make_box(b"minf", &minf2_payload);

    let mut mdia2_payload = Vec::new();
    mdia2_payload.extend_from_slice(&mdhd2_box);
    mdia2_payload.extend_from_slice(&hdlr2_box);
    mdia2_payload.extend_from_slice(&minf2_box);
    let mdia2_box = make_box(b"mdia", &mdia2_payload);

    let mut trak2_payload = Vec::new();
    trak2_payload.extend_from_slice(&tkhd2_box);
    trak2_payload.extend_from_slice(&mdia2_box);
    let trak2_box = make_box(b"trak", &trak2_payload);

    // Chapters in udta.chpl
    let mut chpl_payload = vec![0u8; 5];
    chpl_payload[4] = 2; // 2 chapters
    // Chapter 1: 0s, "Intro"
    chpl_payload.extend_from_slice(&0u64.to_be_bytes());
    chpl_payload.push(5);
    chpl_payload.extend_from_slice(b"Intro");
    // Chapter 2: 5s = 50,000,000 (100ns units), "Action"
    chpl_payload.extend_from_slice(&50_000_000u64.to_be_bytes());
    chpl_payload.push(6);
    chpl_payload.extend_from_slice(b"Action");
    let chpl_box = make_box(b"chpl", &chpl_payload);

    let mut udta_payload = Vec::new();
    udta_payload.extend_from_slice(&chpl_box);

    // Cover art in udta.meta.ilst.covr
    if with_cover {
        let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
        let mut data_payload = vec![0u8; 8];
        data_payload[3] = 13; // JPEG type indicator
        data_payload.extend_from_slice(&fake_jpeg);
        let data_box = make_box(b"data", &data_payload);

        let covr_box = make_box(b"covr", &data_box);
        let ilst_box = make_box(b"ilst", &covr_box);

        let mut meta_payload = vec![0u8; 4]; // Version + flags
        meta_payload.extend_from_slice(&ilst_box);
        let meta_box = make_box(b"meta", &meta_payload);
        udta_payload.extend_from_slice(&meta_box);
    }

    let udta_box = make_box(b"udta", &udta_payload);

    let mut moov_payload = Vec::new();
    moov_payload.extend_from_slice(&mvhd_box);
    moov_payload.extend_from_slice(&trak1_box);
    moov_payload.extend_from_slice(&trak2_box);
    moov_payload.extend_from_slice(&udta_box);
    let moov_box = make_box(b"moov", &moov_payload);

    let mdat_box = make_box(b"mdat", &[0xAA, 0xBB, 0xCC, 0xDD]);

    let mut file_buf = Vec::new();
    file_buf.extend_from_slice(&ftyp_box);
    if fast_start {
        file_buf.extend_from_slice(&moov_box);
        file_buf.extend_from_slice(&mdat_box);
    } else {
        file_buf.extend_from_slice(&mdat_box);
        file_buf.extend_from_slice(&moov_box);
    }

    file_buf
}

// ============================================================================
// Synthetic EBML (MKV/WebM) Builder Helpers
// ============================================================================

fn write_ebml_id(buf: &mut Vec<u8>, id: u32) {
    if id <= 0xFF {
        buf.push(id as u8);
    } else if id <= 0xFFFF {
        buf.extend_from_slice(&(id as u16).to_be_bytes());
    } else if id <= 0xFF_FFFF {
        buf.push((id >> 16) as u8);
        buf.push((id >> 8) as u8);
        buf.push(id as u8);
    } else {
        buf.extend_from_slice(&id.to_be_bytes());
    }
}

fn write_ebml_vint(buf: &mut Vec<u8>, val: u64) {
    if val < 0x7F {
        buf.push((0x80 | val) as u8);
    } else if val < 0x3FFF {
        buf.push((0x40 | (val >> 8)) as u8);
        buf.push((val & 0xFF) as u8);
    } else if val < 0x1F_FFFF {
        buf.push((0x20 | (val >> 16)) as u8);
        buf.push(((val >> 8) & 0xFF) as u8);
        buf.push((val & 0xFF) as u8);
    } else if val < 0x0FFF_FFFF {
        buf.push((0x10 | (val >> 24)) as u8);
        buf.push(((val >> 16) & 0xFF) as u8);
        buf.push(((val >> 8) & 0xFF) as u8);
        buf.push((val & 0xFF) as u8);
    } else {
        buf.push(0x01);
        buf.extend_from_slice(&val.to_be_bytes()[1..8]);
    }
}

fn make_ebml_element(id: u32, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_ebml_id(&mut buf, id);
    write_ebml_vint(&mut buf, payload.len() as u64);
    buf.extend_from_slice(payload);
    buf
}

fn build_synthetic_mkv(doc_type: &str, with_cover: bool) -> Vec<u8> {
    let mut ebml_header_payload = Vec::new();
    ebml_header_payload.extend_from_slice(&make_ebml_element(0x4282, doc_type.as_bytes())); // DocType
    ebml_header_payload.extend_from_slice(&make_ebml_element(0x4287, &[1])); // DocTypeVersion
    let ebml_header = make_ebml_element(0x1A45_DFA3, &ebml_header_payload);

    // Segment -> Info
    let mut info_payload = Vec::new();
    info_payload.extend_from_slice(&make_ebml_element(0x2AD7B1, &1_000_000u64.to_be_bytes())); // TimecodeScale = 1ms
    info_payload.extend_from_slice(&make_ebml_element(0x4489, &120500.0f64.to_be_bytes())); // Duration in timecode units (120,500 ms = 120.5s)
    let info_elem = make_ebml_element(0x1549_A966, &info_payload);


    // Segment -> Tracks
    let mut track1_payload = Vec::new();
    track1_payload.extend_from_slice(&make_ebml_element(0xD7, &[1])); // TrackNumber = 1
    track1_payload.extend_from_slice(&make_ebml_element(0x83, &[1])); // TrackType = 1 (Video)
    let video_codec = if doc_type == "webm" {
        "V_VP9"
    } else {
        "V_MPEGH/ISO/HEVC"
    };
    track1_payload.extend_from_slice(&make_ebml_element(0x86, video_codec.as_bytes())); // CodecID
    track1_payload
        .extend_from_slice(&make_ebml_element(0x23E383, &16_666_666u64.to_be_bytes())); // ~60fps

    let mut video_settings_payload = Vec::new();
    video_settings_payload.extend_from_slice(&make_ebml_element(0xB0, &3840u64.to_be_bytes())); // Width 3840
    video_settings_payload.extend_from_slice(&make_ebml_element(0xBA, &2160u64.to_be_bytes())); // Height 2160
    track1_payload.extend_from_slice(&make_ebml_element(0xE0, &video_settings_payload));
    let track1_elem = make_ebml_element(0xAE, &track1_payload);

    // Audio Track
    let mut track2_payload = Vec::new();
    track2_payload.extend_from_slice(&make_ebml_element(0xD7, &[2])); // TrackNumber = 2
    track2_payload.extend_from_slice(&make_ebml_element(0x83, &[2])); // TrackType = 2 (Audio)
    let audio_codec = if doc_type == "webm" {
        "A_VORBIS"
    } else {
        "A_OPUS"
    };
    track2_payload.extend_from_slice(&make_ebml_element(0x86, audio_codec.as_bytes()));
    track2_payload.extend_from_slice(&make_ebml_element(0x22B59C, b"eng"));

    let mut audio_settings_payload = Vec::new();
    audio_settings_payload.extend_from_slice(&make_ebml_element(0x9F, &[6])); // 6 channels
    audio_settings_payload.extend_from_slice(&make_ebml_element(0xB5, &48000.0f64.to_be_bytes())); // 48kHz
    track2_payload.extend_from_slice(&make_ebml_element(0xE1, &audio_settings_payload));
    let track2_elem = make_ebml_element(0xAE, &track2_payload);

    let mut tracks_payload = Vec::new();
    tracks_payload.extend_from_slice(&track1_elem);
    tracks_payload.extend_from_slice(&track2_elem);
    let tracks_elem = make_ebml_element(0x1654_AE6B, &tracks_payload);

    // Chapters
    let mut chap1_payload = Vec::new();
    chap1_payload.extend_from_slice(&make_ebml_element(0x91, &0u64.to_be_bytes())); // Start 0ms
    chap1_payload.extend_from_slice(&make_ebml_element(0x92, &30_000_000_000u64.to_be_bytes())); // End 30s
    let mut chap1_disp = Vec::new();
    chap1_disp.extend_from_slice(&make_ebml_element(0x85, b"Opening"));
    chap1_payload.extend_from_slice(&make_ebml_element(0x80, &chap1_disp));
    let chap1_atom = make_ebml_element(0xB6, &chap1_payload);

    let mut edition_payload = Vec::new();
    edition_payload.extend_from_slice(&chap1_atom);
    let edition_elem = make_ebml_element(0x45B9, &edition_payload);

    let mut chapters_payload = Vec::new();
    chapters_payload.extend_from_slice(&edition_elem);
    let chapters_elem = make_ebml_element(0x1043_A770, &chapters_payload);

    let mut segment_payload = Vec::new();
    segment_payload.extend_from_slice(&info_elem);
    segment_payload.extend_from_slice(&tracks_elem);
    segment_payload.extend_from_slice(&chapters_elem);

    // Attachments (Cover)
    if with_cover {
        let mut file_payload = Vec::new();
        file_payload.extend_from_slice(&make_ebml_element(0x466E, b"cover.jpg"));
        file_payload.extend_from_slice(&make_ebml_element(0x4660, b"image/jpeg"));
        let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x11, 0x22, 0x33, 0x44];
        file_payload.extend_from_slice(&make_ebml_element(0x465C, &fake_jpeg));
        let file_elem = make_ebml_element(0x61A7, &file_payload);

        let mut attach_payload = Vec::new();
        attach_payload.extend_from_slice(&file_elem);
        let attach_elem = make_ebml_element(0x1941_A469, &attach_payload);
        segment_payload.extend_from_slice(&attach_elem);
    }

    let segment_elem = make_ebml_element(0x1853_8067, &segment_payload);

    let mut out = Vec::new();
    out.extend_from_slice(&ebml_header);
    out.extend_from_slice(&segment_elem);
    out
}

// ============================================================================
// Synthetic RIFF AVI Builder Helpers
// ============================================================================

fn make_riff_chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(id);
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    if payload.len() % 2 != 0 {
        buf.push(0); // Pad byte
    }
    buf
}

fn make_riff_list(list_type: &[u8; 4], children: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(list_type);
    payload.extend_from_slice(children);
    make_riff_chunk(b"LIST", &payload)
}

fn build_synthetic_avi() -> Vec<u8> {
    // avih: 40 bytes
    let mut avih_payload = vec![0u8; 40];
    avih_payload[0..4].copy_from_slice(&40000u32.to_le_bytes()); // 40,000 us per frame = 25 fps
    avih_payload[16..20].copy_from_slice(&250u32.to_le_bytes()); // 250 frames -> 10,000 ms duration
    avih_payload[32..36].copy_from_slice(&640u32.to_le_bytes()); // Width 640
    avih_payload[36..40].copy_from_slice(&480u32.to_le_bytes()); // Height 480
    let avih_chunk = make_riff_chunk(b"avih", &avih_payload);

    // strl 1: Video (XVID)
    let mut strh1_payload = vec![0u8; 56];
    strh1_payload[0..4].copy_from_slice(b"vids");
    strh1_payload[4..8].copy_from_slice(b"XVID");
    strh1_payload[20..24].copy_from_slice(&1u32.to_le_bytes()); // Scale 1
    strh1_payload[24..28].copy_from_slice(&25u32.to_le_bytes()); // Rate 25 -> 25 fps
    let strh1_chunk = make_riff_chunk(b"strh", &strh1_payload);

    let mut strf1_payload = vec![0u8; 40];
    strf1_payload[4..8].copy_from_slice(&640i32.to_le_bytes()); // Width 640
    strf1_payload[8..12].copy_from_slice(&480i32.to_le_bytes()); // Height 480
    strf1_payload[16..20].copy_from_slice(b"XVID"); // Compression
    let strf1_chunk = make_riff_chunk(b"strf", &strf1_payload);

    let mut strl1_children = Vec::new();
    strl1_children.extend_from_slice(&strh1_chunk);
    strl1_children.extend_from_slice(&strf1_chunk);
    let strl1_list = make_riff_list(b"strl", &strl1_children);

    // strl 2: Audio (MP3)
    let mut strh2_payload = vec![0u8; 56];
    strh2_payload[0..4].copy_from_slice(b"auds");
    strh2_payload[20..24].copy_from_slice(&1u32.to_le_bytes());
    strh2_payload[24..28].copy_from_slice(&44100u32.to_le_bytes());
    let strh2_chunk = make_riff_chunk(b"strh", &strh2_payload);

    let mut strf2_payload = vec![0u8; 18];
    strf2_payload[0..2].copy_from_slice(&0x0055u16.to_le_bytes()); // MP3 tag
    strf2_payload[2..4].copy_from_slice(&2u16.to_le_bytes()); // 2 channels
    strf2_payload[4..8].copy_from_slice(&44100u32.to_le_bytes()); // 44.1kHz
    let strf2_chunk = make_riff_chunk(b"strf", &strf2_payload);

    let mut strl2_children = Vec::new();
    strl2_children.extend_from_slice(&strh2_chunk);
    strl2_children.extend_from_slice(&strf2_chunk);
    let strl2_list = make_riff_list(b"strl", &strl2_children);

    let mut hdrl_children = Vec::new();
    hdrl_children.extend_from_slice(&avih_chunk);
    hdrl_children.extend_from_slice(&strl1_list);
    hdrl_children.extend_from_slice(&strl2_list);
    let hdrl_list = make_riff_list(b"hdrl", &hdrl_children);

    let movi_list = make_riff_list(b"movi", &[0x00, 0x01, 0x02, 0x03]);

    let mut riff_payload = Vec::new();
    riff_payload.extend_from_slice(b"AVI ");
    riff_payload.extend_from_slice(&hdrl_list);
    riff_payload.extend_from_slice(&movi_list);

    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff_payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&riff_payload);
    out
}

// ============================================================================
// Unit Tests
// ============================================================================

#[test]
fn test_mp4_fast_start_demux_and_cover() {
    let bytes = build_synthetic_mp4(true, true);
    let format = TTZipVideoDemuxer::probe_from_bytes(&bytes);
    assert_eq!(format, VideoFormat::Mp4);

    let meta = TTZipVideoDemuxer::demux_from_bytes(&bytes).expect("Failed to demux MP4");
    assert_eq!(meta.format, VideoFormat::Mp4);
    assert_eq!(meta.duration_ms, 10000);
    assert!(meta.has_cover);

    assert_eq!(meta.video_tracks.len(), 1);
    let video = &meta.video_tracks[0];
    assert_eq!(video.track_id, 1);
    assert_eq!(video.codec, VideoCodec::H264);
    assert_eq!(video.width, 1920);
    assert_eq!(video.height, 1080);
    assert!((video.fps - 60.0).abs() < 0.1);

    assert_eq!(meta.audio_tracks.len(), 1);
    let audio = &meta.audio_tracks[0];
    assert_eq!(audio.track_id, 2);
    assert_eq!(audio.codec, AudioCodec::Aac);
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.sample_rate, 48000);

    assert_eq!(meta.chapters.len(), 2);
    assert_eq!(meta.chapters[0].title, "Intro");
    assert_eq!(meta.chapters[0].start_ms, 0);
    assert_eq!(meta.chapters[0].end_ms, 5000);
    assert_eq!(meta.chapters[1].title, "Action");
    assert_eq!(meta.chapters[1].start_ms, 5000);

    let cover = TTZipVideoDemuxer::extract_cover_from_bytes(&bytes)
        .expect("Cover query failed")
        .expect("Missing cover bytes");
    assert_eq!(&cover[0..4], &[0xFF, 0xD8, 0xFF, 0xE0]);
}

#[test]
fn test_mp4_trailing_moov() {
    let bytes = build_synthetic_mp4(false, false);
    let meta = TTZipVideoDemuxer::demux_from_bytes(&bytes).expect("Failed to demux trailing moov");
    assert_eq!(meta.format, VideoFormat::Mp4);
    assert_eq!(meta.duration_ms, 10000);
    assert!(!meta.has_cover);
    assert_eq!(meta.video_tracks.len(), 1);
}

#[test]
fn test_quicktime_mov_brand() {
    let mut ftyp_payload = Vec::new();
    ftyp_payload.extend_from_slice(b"qt  ");
    ftyp_payload.extend_from_slice(&0u32.to_be_bytes());
    ftyp_payload.extend_from_slice(b"qt  ");
    let ftyp_box = make_box(b"ftyp", &ftyp_payload);

    let mut mvhd_payload = vec![0u8; 100];
    mvhd_payload[12..16].copy_from_slice(&600u32.to_be_bytes());
    mvhd_payload[16..20].copy_from_slice(&1200u32.to_be_bytes());
    let mvhd_box = make_box(b"mvhd", &mvhd_payload);

    let mut moov_payload = Vec::new();
    moov_payload.extend_from_slice(&mvhd_box);
    let moov_box = make_box(b"moov", &moov_payload);

    let mut mov_bytes = Vec::new();
    mov_bytes.extend_from_slice(&ftyp_box);
    mov_bytes.extend_from_slice(&moov_box);

    let format = TTZipVideoDemuxer::probe_from_bytes(&mov_bytes);
    assert_eq!(format, VideoFormat::Mov);

    let meta = TTZipVideoDemuxer::demux_from_bytes(&mov_bytes).expect("Failed to demux MOV");
    assert_eq!(meta.format, VideoFormat::Mov);
    assert_eq!(meta.duration_ms, 2000);
}

#[test]
fn test_mkv_demux_and_cover() {
    let bytes = build_synthetic_mkv("matroska", true);
    let format = TTZipVideoDemuxer::probe_from_bytes(&bytes);
    assert_eq!(format, VideoFormat::Mkv);

    let meta = TTZipVideoDemuxer::demux_from_bytes(&bytes).expect("Failed to demux MKV");
    assert_eq!(meta.format, VideoFormat::Mkv);
    assert_eq!(meta.duration_ms, 120500);
    assert!(meta.has_cover);

    assert_eq!(meta.video_tracks.len(), 1);
    let video = &meta.video_tracks[0];
    assert_eq!(video.codec, VideoCodec::H265_HEVC);
    assert_eq!(video.width, 3840);
    assert_eq!(video.height, 2160);
    assert!((video.fps - 60.0).abs() < 0.1);

    assert_eq!(meta.audio_tracks.len(), 1);
    let audio = &meta.audio_tracks[0];
    assert_eq!(audio.codec, AudioCodec::Opus);
    assert_eq!(audio.channels, 6);
    assert_eq!(audio.sample_rate, 48000);
    assert_eq!(audio.language.as_deref(), Some("eng"));

    assert_eq!(meta.chapters.len(), 1);
    assert_eq!(meta.chapters[0].title, "Opening");
    assert_eq!(meta.chapters[0].start_ms, 0);
    assert_eq!(meta.chapters[0].end_ms, 30000);

    let cover = TTZipVideoDemuxer::extract_cover_from_bytes(&bytes)
        .expect("Cover extraction error")
        .expect("Missing cover");
    assert_eq!(&cover[0..4], &[0xFF, 0xD8, 0xFF, 0xE0]);
}

#[test]
fn test_webm_demux() {
    let bytes = build_synthetic_mkv("webm", false);
    let format = TTZipVideoDemuxer::probe_from_bytes(&bytes);
    assert_eq!(format, VideoFormat::Webm);

    let meta = TTZipVideoDemuxer::demux_from_bytes(&bytes).expect("Failed to demux WebM");
    assert_eq!(meta.format, VideoFormat::Webm);
    assert_eq!(meta.video_tracks[0].codec, VideoCodec::VP9);
    assert_eq!(meta.audio_tracks[0].codec, AudioCodec::Vorbis);
}

#[test]
fn test_avi_demux() {
    let bytes = build_synthetic_avi();
    let format = TTZipVideoDemuxer::probe_from_bytes(&bytes);
    assert_eq!(format, VideoFormat::Avi);

    let meta = TTZipVideoDemuxer::demux_from_bytes(&bytes).expect("Failed to demux AVI");
    assert_eq!(meta.format, VideoFormat::Avi);
    assert_eq!(meta.duration_ms, 10000);

    assert_eq!(meta.video_tracks.len(), 1);
    let video = &meta.video_tracks[0];
    assert_eq!(video.codec, VideoCodec::Mpeg4);
    assert_eq!(video.width, 640);
    assert_eq!(video.height, 480);
    assert!((video.fps - 25.0).abs() < 0.1);

    assert_eq!(meta.audio_tracks.len(), 1);
    let audio = &meta.audio_tracks[0];
    assert_eq!(audio.codec, AudioCodec::Mp3);
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.sample_rate, 44100);
}

#[test]
fn test_corrupted_and_truncated_inputs() {
    assert_eq!(TTZipVideoDemuxer::probe_from_bytes(&[]), VideoFormat::Unknown);
    assert_eq!(TTZipVideoDemuxer::probe_from_bytes(&[0x11, 0x22]), VideoFormat::Unknown);

    assert!(TTZipVideoDemuxer::demux_from_bytes(&[]).is_err());
    assert!(TTZipVideoDemuxer::demux_from_bytes(&[0xFF; 32]).is_err());
    assert!(TTZipMp4Demuxer::new(&[0x00, 0x00, 0x00, 0x08, b'f', b't', b'y', b'p']).demux().is_err());
    assert!(TTZipMkvDemuxer::new(&[0x1A, 0x45, 0xDF, 0xA3, 0x81, 0x00]).demux().is_err());
    assert!(TTZipAviDemuxer::new(b"RIFF\x04\x00\x00\x00AVI ").demux().is_ok());
}
