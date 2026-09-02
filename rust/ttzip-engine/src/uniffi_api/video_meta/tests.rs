// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unit tests for Mozilla UniFFI video metadata, track probing, and cover art extraction.

use super::parser::{extract_video_cover_from_bytes, parse_video_metadata_from_bytes};
use super::service::{
    uniffi_extract_video_cover, uniffi_extract_video_metadata, uniffi_probe_video_bytes,
    UniFFIVideoService,
};
use super::types::{
    UniFFIAudioCodec, UniFFIVideoCodec, UniFFIVideoError, UniFFIVideoFormat,
};

/// Constructs a synthetic MP4 box structure with mvhd, trak (video & audio), and ilst tags.
fn create_synthetic_mp4() -> Vec<u8> {
    let mut data = Vec::new();

    // 1. ftyp box
    let ftyp_payload = b"isom\x00\x00\x02\x00isommp41";
    let ftyp_size = (8 + ftyp_payload.len()) as u32;
    data.extend_from_slice(&ftyp_size.to_be_bytes());
    data.extend_from_slice(b"ftyp");
    data.extend_from_slice(ftyp_payload);

    // 2. Build ilst tags (©nam, ©ART, covr)
    let mut ilst_payload = Vec::new();

    // ©nam (Title)
    let title_str = b"Epic 4K Journey";
    let mut name_box = Vec::new();
    let name_data_size = (16 + title_str.len()) as u32;
    name_box.extend_from_slice(&name_data_size.to_be_bytes());
    name_box.extend_from_slice(b"data");
    name_box.extend_from_slice(&1u32.to_be_bytes()); // UTF-8 flag = 1
    name_box.extend_from_slice(&0u32.to_be_bytes()); // locale
    name_box.extend_from_slice(title_str);
    let name_box_size = (8 + name_box.len()) as u32;
    ilst_payload.extend_from_slice(&name_box_size.to_be_bytes());
    ilst_payload.extend_from_slice(b"\xa9nam");
    ilst_payload.extend_from_slice(&name_box);

    // ©ART (Artist)
    let artist_str = b"Director Witt";
    let mut art_box = Vec::new();
    let art_data_size = (16 + artist_str.len()) as u32;
    art_box.extend_from_slice(&art_data_size.to_be_bytes());
    art_box.extend_from_slice(b"data");
    art_box.extend_from_slice(&1u32.to_be_bytes());
    art_box.extend_from_slice(&0u32.to_be_bytes());
    art_box.extend_from_slice(artist_str);
    let art_box_size = (8 + art_box.len()) as u32;
    ilst_payload.extend_from_slice(&art_box_size.to_be_bytes());
    ilst_payload.extend_from_slice(b"\xa9ART");
    ilst_payload.extend_from_slice(&art_box);

    // covr (Cover JPEG)
    let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0xFF, 0xD9];
    let mut covr_box = Vec::new();
    let covr_data_size = (16 + fake_jpeg.len()) as u32;
    covr_box.extend_from_slice(&covr_data_size.to_be_bytes());
    covr_box.extend_from_slice(b"data");
    covr_box.extend_from_slice(&13u32.to_be_bytes()); // JPEG flag = 13
    covr_box.extend_from_slice(&0u32.to_be_bytes());
    covr_box.extend_from_slice(&fake_jpeg);
    let covr_box_size = (8 + covr_box.len()) as u32;
    ilst_payload.extend_from_slice(&covr_box_size.to_be_bytes());
    ilst_payload.extend_from_slice(b"covr");
    ilst_payload.extend_from_slice(&covr_box);

    // Wrap ilst in meta box
    let ilst_size = (8 + ilst_payload.len()) as u32;
    let mut ilst_box = Vec::new();
    ilst_box.extend_from_slice(&ilst_size.to_be_bytes());
    ilst_box.extend_from_slice(b"ilst");
    ilst_box.extend_from_slice(&ilst_payload);

    let meta_size = (12 + ilst_box.len()) as u32;
    let mut meta_box = Vec::new();
    meta_box.extend_from_slice(&meta_size.to_be_bytes());
    meta_box.extend_from_slice(b"meta");
    meta_box.extend_from_slice(&[0u8; 4]); // version + flags
    meta_box.extend_from_slice(&ilst_box);

    // Wrap meta in udta box
    let udta_size = (8 + meta_box.len()) as u32;
    let mut udta_box = Vec::new();
    udta_box.extend_from_slice(&udta_size.to_be_bytes());
    udta_box.extend_from_slice(b"udta");
    udta_box.extend_from_slice(&meta_box);

    // 3. Build mvhd box (timescale = 1000, duration = 120_000 -> 120.0s)
    let mut mvhd_box = Vec::new();
    mvhd_box.extend_from_slice(&108u32.to_be_bytes());
    mvhd_box.extend_from_slice(b"mvhd");
    mvhd_box.push(0); // version
    mvhd_box.extend_from_slice(&[0u8; 3]); // flags
    mvhd_box.extend_from_slice(&[0u8; 8]); // creation/mod times
    mvhd_box.extend_from_slice(&1000u32.to_be_bytes()); // timescale
    mvhd_box.extend_from_slice(&120000u32.to_be_bytes()); // duration = 120s
    mvhd_box.extend_from_slice(&[0u8; 80]); // rest of mvhd

    // 4. Build video trak box (width = 3840, height = 2160, hvc1)
    let mut trak_video = Vec::new();
    // tkhd
    let mut tkhd_v = vec![0u8; 92];
    tkhd_v[0..4].copy_from_slice(&92u32.to_be_bytes());
    tkhd_v[4..8].copy_from_slice(b"tkhd");
    tkhd_v[20..24].copy_from_slice(&1u32.to_be_bytes()); // track_id = 1
    // dimensions: width = 3840 << 16, height = 2160 << 16
    let w_fixed = 3840u32 << 16;
    let h_fixed = 2160u32 << 16;
    tkhd_v[84..88].copy_from_slice(&w_fixed.to_be_bytes());
    tkhd_v[88..92].copy_from_slice(&h_fixed.to_be_bytes());

    // mdia with hdlr 'vide' and stsd 'hvc1'
    let mut mdia_v = Vec::new();
    // hdlr
    let mut hdlr_v = vec![0u8; 32];
    hdlr_v[0..4].copy_from_slice(&32u32.to_be_bytes());
    hdlr_v[4..8].copy_from_slice(b"hdlr");
    hdlr_v[16..20].copy_from_slice(b"vide");

    // stsd
    let mut stsd_v = Vec::new();
    stsd_v.extend_from_slice(&48u32.to_be_bytes());
    stsd_v.extend_from_slice(b"stsd");
    stsd_v.extend_from_slice(&[0u8; 4]);
    stsd_v.extend_from_slice(&1u32.to_be_bytes()); // entry count = 1
    // hvc1 sample entry
    stsd_v.extend_from_slice(&32u32.to_be_bytes());
    stsd_v.extend_from_slice(b"hvc1");
    stsd_v.extend_from_slice(&[0u8; 24]);

    // assemble mdia
    let mdia_v_size = (8 + hdlr_v.len() + stsd_v.len()) as u32;
    mdia_v.extend_from_slice(&mdia_v_size.to_be_bytes());
    mdia_v.extend_from_slice(b"mdia");
    mdia_v.extend_from_slice(&hdlr_v);
    mdia_v.extend_from_slice(&stsd_v);

    let trak_v_size = (8 + tkhd_v.len() + mdia_v.len()) as u32;
    trak_video.extend_from_slice(&trak_v_size.to_be_bytes());
    trak_video.extend_from_slice(b"trak");
    trak_video.extend_from_slice(&tkhd_v);
    trak_video.extend_from_slice(&mdia_v);

    // 5. Build audio trak box (sample_rate = 48000, channels = 6, mp4a)
    let mut trak_audio = Vec::new();
    let mut tkhd_a = vec![0u8; 92];
    tkhd_a[0..4].copy_from_slice(&92u32.to_be_bytes());
    tkhd_a[4..8].copy_from_slice(b"tkhd");
    tkhd_a[20..24].copy_from_slice(&2u32.to_be_bytes()); // track_id = 2

    let mut mdia_a = Vec::new();
    let mut hdlr_a = vec![0u8; 32];
    hdlr_a[0..4].copy_from_slice(&32u32.to_be_bytes());
    hdlr_a[4..8].copy_from_slice(b"hdlr");
    hdlr_a[16..20].copy_from_slice(b"soun");

    let mut stsd_a = Vec::new();
    stsd_a.extend_from_slice(&44u32.to_be_bytes());
    stsd_a.extend_from_slice(b"stsd");
    stsd_a.extend_from_slice(&[0u8; 4]);
    stsd_a.extend_from_slice(&1u32.to_be_bytes());
    // mp4a sample entry (28 bytes)
    stsd_a.extend_from_slice(&28u32.to_be_bytes());
    stsd_a.extend_from_slice(b"mp4a");
    stsd_a.extend_from_slice(&[0u8; 8]); // reserved
    stsd_a.extend_from_slice(&6u16.to_be_bytes()); // 6 channels (5.1)
    stsd_a.extend_from_slice(&16u16.to_be_bytes()); // 16 bits
    stsd_a.extend_from_slice(&[0u8; 4]); // packet size + compression id
    stsd_a.extend_from_slice(&(48000u32 << 16).to_be_bytes()); // sample rate = 48000

    let mdia_a_size = (8 + hdlr_a.len() + stsd_a.len()) as u32;
    mdia_a.extend_from_slice(&mdia_a_size.to_be_bytes());
    mdia_a.extend_from_slice(b"mdia");
    mdia_a.extend_from_slice(&hdlr_a);
    mdia_a.extend_from_slice(&stsd_a);

    let trak_a_size = (8 + tkhd_a.len() + mdia_a.len()) as u32;
    trak_audio.extend_from_slice(&trak_a_size.to_be_bytes());
    trak_audio.extend_from_slice(b"trak");
    trak_audio.extend_from_slice(&tkhd_a);
    trak_audio.extend_from_slice(&mdia_a);

    // 6. Assemble moov box
    let moov_size = (8 + mvhd_box.len() + trak_video.len() + trak_audio.len() + udta_box.len()) as u32;
    data.extend_from_slice(&moov_size.to_be_bytes());
    data.extend_from_slice(b"moov");
    data.extend_from_slice(&mvhd_box);
    data.extend_from_slice(&trak_video);
    data.extend_from_slice(&trak_audio);
    data.extend_from_slice(&udta_box);

    data
}

/// Constructs a synthetic Matroska byte buffer.
fn create_synthetic_mkv() -> Vec<u8> {
    let mut data = vec![0x1A, 0x45, 0xDF, 0xA3]; // EBML Header
    data.extend_from_slice(b"matroska");

    // Width (0xB0 0x82 -> 1920)
    data.extend_from_slice(&[0xB0, 0x82, 0x07, 0x80]);
    // Height (0xBA 0x82 -> 1080)
    data.extend_from_slice(&[0xBA, 0x82, 0x04, 0x38]);

    // Duration (0x44 0x89 0x84 -> 60_000.0 ms = 60.0s)
    data.extend_from_slice(&[0x44, 0x89, 0x84]);
    data.extend_from_slice(&60000.0f32.to_be_bytes());

    // Title (0x7B 0xA9)
    let title_str = b"Nature Documentary";
    data.extend_from_slice(&[0x7B, 0xA9, (0x80 | title_str.len() as u8)]);
    data.extend_from_slice(title_str);

    // Attachments with cover image
    let fake_cover = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\xFF\xD9";
    data.extend_from_slice(&[0x19, 0x41, 0xA4, 0x69]); // Attachments
    data.extend_from_slice(&[0x46, 0x5C]); // FileData
    data.extend_from_slice(&[(0x80 | fake_cover.len() as u8)]);
    data.extend_from_slice(fake_cover);

    data
}

/// Constructs a synthetic AVI byte buffer.
fn create_synthetic_avi() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&200u32.to_le_bytes());
    data.extend_from_slice(b"AVI ");

    // avih chunk
    data.extend_from_slice(b"avih");
    data.extend_from_slice(&56u32.to_le_bytes());
    data.extend_from_slice(&33333u32.to_le_bytes()); // ~30 fps
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(&300u32.to_le_bytes()); // 300 frames = 10s
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(&1280u32.to_le_bytes()); // width = 1280
    data.extend_from_slice(&720u32.to_le_bytes()); // height = 720
    data.extend_from_slice(&[0u8; 8]);

    // INAM tag (Title)
    let title = b"Classic Film\x00";
    data.extend_from_slice(b"INAM");
    data.extend_from_slice(&(title.len() as u32).to_le_bytes());
    data.extend_from_slice(title);

    data
}

// ============================================================================
// Test Cases
// ============================================================================

#[test]
fn test_mp4_metadata_and_cover_extraction() {
    let mp4_bytes = create_synthetic_mp4();
    let meta = parse_video_metadata_from_bytes(&mp4_bytes, Some("movie.mp4"))
        .expect("Failed to parse synthetic MP4");

    assert_eq!(meta.format, UniFFIVideoFormat::Mp4);
    assert!((meta.duration_seconds - 120.0).abs() < 0.1);
    assert_eq!(meta.title.as_deref(), Some("Epic 4K Journey"));
    assert_eq!(meta.artist_or_director.as_deref(), Some("Director Witt"));
    assert!(meta.has_cover);
    assert_eq!(meta.cover_mime_type.as_deref(), Some("image/jpeg"));

    // Check video tracks
    assert_eq!(meta.video_tracks.len(), 1);
    let vtrack = &meta.video_tracks[0];
    assert_eq!(vtrack.track_id, 1);
    assert_eq!(vtrack.codec, UniFFIVideoCodec::Hevc);
    assert_eq!(vtrack.width, 3840);
    assert_eq!(vtrack.height, 2160);
    assert_eq!(vtrack.aspect_ratio, "16:9");

    // Check audio tracks
    assert_eq!(meta.audio_tracks.len(), 1);
    let atrack = &meta.audio_tracks[0];
    assert_eq!(atrack.track_id, 2);
    assert_eq!(atrack.codec, UniFFIAudioCodec::Aac);
    assert_eq!(atrack.sample_rate, 48000);
    assert_eq!(atrack.channels, 6);
    assert_eq!(atrack.channel_layout, "5.1 Surround");

    // Check cover extraction
    let cover_bytes = extract_video_cover_from_bytes(&mp4_bytes, Some("movie.mp4"))
        .expect("Failed to extract MP4 cover");
    assert!(cover_bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
}

#[test]
fn test_mkv_metadata_and_cover_extraction() {
    let mkv_bytes = create_synthetic_mkv();
    let meta = parse_video_metadata_from_bytes(&mkv_bytes, Some("nature.mkv"))
        .expect("Failed to parse synthetic MKV");

    assert_eq!(meta.format, UniFFIVideoFormat::Mkv);
    assert!((meta.duration_seconds - 60.0).abs() < 0.1);
    assert_eq!(meta.title.as_deref(), Some("Nature Documentary"));
    assert!(meta.has_cover);

    assert_eq!(meta.video_tracks.len(), 1);
    assert_eq!(meta.video_tracks[0].width, 1920);
    assert_eq!(meta.video_tracks[0].height, 1080);

    let cover = extract_video_cover_from_bytes(&mkv_bytes, Some("nature.mkv"))
        .expect("Failed to extract MKV cover");
    assert!(cover.starts_with(&[0xFF, 0xD8]));
}

#[test]
fn test_avi_metadata_extraction() {
    let avi_bytes = create_synthetic_avi();
    let meta = parse_video_metadata_from_bytes(&avi_bytes, Some("sample.avi"))
        .expect("Failed to parse synthetic AVI");

    assert_eq!(meta.format, UniFFIVideoFormat::Avi);
    assert_eq!(meta.title.as_deref(), Some("Classic Film"));
    assert!((meta.duration_seconds - 10.0).abs() < 0.5);

    assert_eq!(meta.video_tracks.len(), 1);
    assert_eq!(meta.video_tracks[0].width, 1280);
    assert_eq!(meta.video_tracks[0].height, 720);
}

#[test]
fn test_cover_not_found_error() {
    let avi_bytes = create_synthetic_avi();
    let err = extract_video_cover_from_bytes(&avi_bytes, Some("sample.avi"))
        .expect_err("Expected CoverArtNotFound error");
    assert!(matches!(err, UniFFIVideoError::CoverArtNotFound));
}

#[test]
fn test_empty_buffer_corrupted_error() {
    let err = parse_video_metadata_from_bytes(&[], None)
        .expect_err("Expected CorruptedData error");
    assert!(matches!(err, UniFFIVideoError::CorruptedData));
}

#[test]
fn test_unsupported_format_error() {
    let random_bytes = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let err = parse_video_metadata_from_bytes(&random_bytes, None)
        .expect_err("Expected UnsupportedFormat error");
    assert!(matches!(err, UniFFIVideoError::UnsupportedFormat { .. }));
}

#[test]
fn test_service_facade_and_free_functions() {
    let mp4_bytes = create_synthetic_mp4();

    // Free functions
    let meta = uniffi_probe_video_bytes(mp4_bytes.clone(), Some("test.mp4".to_string()))
        .expect("Probe free function failed");
    assert_eq!(meta.format, UniFFIVideoFormat::Mp4);

    let extracted = uniffi_extract_video_metadata(mp4_bytes.clone(), Some("test.mp4".to_string()))
        .expect("Extract metadata free function failed");
    assert_eq!(extracted.title.as_deref(), Some("Epic 4K Journey"));

    let cover = uniffi_extract_video_cover(mp4_bytes.clone(), Some("test.mp4".to_string()))
        .expect("Extract cover free function failed");
    assert!(!cover.is_empty());

    // UniFFIVideoService Object
    let service = UniFFIVideoService::new();
    let service_meta = service
        .probe_bytes(mp4_bytes.clone(), Some("test.mp4".to_string()))
        .expect("Service probe_bytes failed");
    assert_eq!(service_meta.video_tracks.len(), 1);

    let service_cover = service
        .extract_cover(mp4_bytes, Some("test.mp4".to_string()))
        .expect("Service extract_cover failed");
    assert!(!service_cover.is_empty());
}
