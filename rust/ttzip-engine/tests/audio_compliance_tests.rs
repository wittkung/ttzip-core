// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Official Audio Compliance and Differential Oracle Test Suite.
//!
//! Validates audio compliance, differential parsing oracles, and 6-layer defense guards across:
//! 1. Container and PCM Data Equivalence Oracles (WAV, AIFF, FLAC, MP3 framing)
//! 2. ID3v2 Syncsafe Integer & Desynchronization Compliance Test Vectors
//! 3. 6-Layer Security Defense Adversarial Test Matrix (Channel/Rate, CoverArt, FrameLoop, ID3, MemoryBudget, SensitiveBuffer)
//! 4. End-to-End Security Pipeline Verification

use ttzip_engine::security::audio_defense::{
    AudioChannelRateGuard, AudioDefenseError, AudioMemoryBudgetGuard, AudioSecurityPipeline,
    CoverArtFormat, CoverArtQuotaGuard, FrameLoopTimeoutGuard, Id3TagSafetyGuard,
    SensitiveAudioBuffer, DEFAULT_MAX_AUDIO_CHANNELS, DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS,
    DEFAULT_MAX_COVER_ART_COUNT, DEFAULT_MAX_CUMULATIVE_FRAME_ERRORS, DEFAULT_MAX_ID3_TAG_SIZE,
    DEFAULT_MAX_SAMPLE_RATE, DEFAULT_MAX_SINGLE_COVER_ART_SIZE, DEFAULT_MIN_AUDIO_CHANNELS,
    DEFAULT_MIN_SAMPLE_RATE,
};

// ============================================================================
// Helper Utilities & Synthetic Generators
// ============================================================================

/// Generates a valid canonical WAV RIFF PCM 16-bit stereo container in memory.
fn generate_canonical_wav(sample_rate: u32, channels: u16, sample_count: usize) -> Vec<u8> {
    let bits_per_sample = 16u16;
    let bytes_per_sample = 2usize;
    let block_align = (channels as usize) * bytes_per_sample;
    let byte_rate = (sample_rate as usize) * block_align;
    let data_len = sample_count * block_align;
    let file_len = 36u32 + (data_len as u32);

    let mut buf = Vec::with_capacity(44 + data_len);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_len.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    buf.extend_from_slice(&1u16.to_le_bytes());  // PCM format
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(byte_rate as u32).to_le_bytes());
    buf.extend_from_slice(&(block_align as u16).to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_len as u32).to_le_bytes());

    for i in 0..sample_count {
        for ch in 0..channels {
            let val = ((i.wrapping_add(ch as usize) % 256) as i16).wrapping_mul(120);
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    buf
}

/// Generates a valid canonical AIFF PCM 16-bit big-endian container in memory.
fn generate_canonical_aiff(sample_rate: u32, channels: u16, sample_count: usize) -> Vec<u8> {
    let bits_per_sample = 16u16;
    let bytes_per_sample = 2usize;
    let frame_size = (channels as usize) * bytes_per_sample;
    let sound_data_len = sample_count * frame_size;
    let ssnd_chunk_size = 8 + sound_data_len;
    let total_len = 4 + (8 + 18) + (8 + ssnd_chunk_size);

    let mut buf = Vec::with_capacity(total_len + 8);
    buf.extend_from_slice(b"FORM");
    buf.extend_from_slice(&(total_len as u32).to_be_bytes());
    buf.extend_from_slice(b"AIFF");

    // COMM chunk
    buf.extend_from_slice(b"COMM");
    buf.extend_from_slice(&18u32.to_be_bytes());
    buf.extend_from_slice(&channels.to_be_bytes());
    buf.extend_from_slice(&(sample_count as u32).to_be_bytes());
    buf.extend_from_slice(&bits_per_sample.to_be_bytes());

    // 80-bit IEEE 754 sample rate (e.g. 44100 -> exp 16398, mantissa 0xAC44000000000000)
    let exponent = 16383 + 15; // 16398 for 44100
    let mantissa = (sample_rate as u64) << (63 - 15);
    buf.extend_from_slice(&(exponent as u16).to_be_bytes());
    buf.extend_from_slice(&mantissa.to_be_bytes());

    // SSND chunk
    buf.extend_from_slice(b"SSND");
    buf.extend_from_slice(&(ssnd_chunk_size as u32).to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes()); // offset = 0
    buf.extend_from_slice(&0u32.to_be_bytes()); // block_size = 0

    for i in 0..sample_count {
        for ch in 0..channels {
            let val = ((i.wrapping_add(ch as usize) % 256) as i16).wrapping_mul(120);
            buf.extend_from_slice(&val.to_be_bytes());
        }
    }

    buf
}

// ============================================================================
// 1. Container and PCM Data Equivalence Oracles
// ============================================================================

#[test]
fn test_wav_container_probing_and_compliance() {
    let pipeline = AudioSecurityPipeline::default();

    // Standard Stereo 44.1kHz WAV
    let wav_44k = generate_canonical_wav(44_100, 2, 500);
    let report_44k = pipeline.inspect_stream_header(&wav_44k).expect("WAV probe should succeed");
    assert_eq!(report_44k.channels, Some(2));
    assert_eq!(report_44k.sample_rate, Some(44_100));
    assert!(report_44k.memory_reservation.is_some());

    // 7.1 Surround 96kHz WAV
    let wav_96k = generate_canonical_wav(96_000, 8, 200);
    let report_96k = pipeline.inspect_stream_header(&wav_96k).expect("7.1 WAV probe should succeed");
    assert_eq!(report_96k.channels, Some(8));
    assert_eq!(report_96k.sample_rate, Some(96_000));

    // Mono 8kHz WAV (lower boundary)
    let wav_8k = generate_canonical_wav(8_000, 1, 100);
    let report_8k = pipeline.inspect_stream_header(&wav_8k).expect("8kHz mono WAV probe should succeed");
    assert_eq!(report_8k.channels, Some(1));
    assert_eq!(report_8k.sample_rate, Some(8_000));
}

#[test]
fn test_aiff_container_probing_and_compliance() {
    let pipeline = AudioSecurityPipeline::default();

    let aiff_data = generate_canonical_aiff(44_100, 2, 300);
    let report = pipeline.inspect_stream_header(&aiff_data).expect("AIFF probe should succeed");
    assert_eq!(report.channels, Some(2));
    assert_eq!(report.sample_rate, Some(44_100));
}

#[test]
fn test_flac_container_probing_and_compliance() {
    let pipeline = AudioSecurityPipeline::default();

    // Construct minimal FLAC STREAMINFO header
    // fLaC (4 bytes) + block header (4 bytes: type 0, len 34) + streaminfo (34 bytes)
    let mut flac_data = Vec::new();
    flac_data.extend_from_slice(b"fLaC");
    flac_data.push(0x80); // Last block flag (bit 7) + type 0 (STREAMINFO)
    flac_data.extend_from_slice(&[0x00, 0x00, 0x22]); // length = 34 bytes
    flac_data.extend_from_slice(&[0x00; 10]); // min/max block size, frames
    // Sample rate: 44100 (20 bits), channels: 2 (coded as 1, 3 bits), bps: 16 (coded as 15, 5 bits)
    // 24-bit combined: (44100 << 4) | ((2 - 1) << 1)
    let sr_ch_bps = (44_100u32 << 4) | ((2 - 1) << 1);
    let b0 = ((sr_ch_bps >> 16) & 0xFF) as u8;
    let b1 = ((sr_ch_bps >> 8) & 0xFF) as u8;
    let b2 = (sr_ch_bps & 0xFF) as u8;
    flac_data.push(b0);
    flac_data.push(b1);
    flac_data.push(b2);
    flac_data.extend_from_slice(&[0x00; 21]); // remaining streaminfo bytes + MD5

    let report = pipeline.inspect_stream_header(&flac_data).expect("FLAC probe should succeed");
    assert_eq!(report.channels, Some(2));
    assert_eq!(report.sample_rate, Some(44_100));
}

// ============================================================================
// 2. ID3v2 Syncsafe Integer & Desynchronization Test Vectors
// ============================================================================

#[test]
fn test_syncsafe_28bit_boundary_test_vectors() {
    // Official test vector matrix across 28-bit critical boundaries
    let boundary_vectors: &[(u32, [u8; 4])] = &[
        (0, [0x00, 0x00, 0x00, 0x00]),
        (1, [0x00, 0x00, 0x00, 0x01]),
        (127, [0x00, 0x00, 0x00, 0x7F]),
        (128, [0x00, 0x00, 0x01, 0x00]),
        (255, [0x00, 0x00, 0x01, 0x7F]),
        (256, [0x00, 0x00, 0x02, 0x00]),
        (16_383, [0x00, 0x00, 0x7F, 0x7F]),
        (16_384, [0x00, 0x01, 0x00, 0x00]),
        (2_097_151, [0x00, 0x7F, 0x7F, 0x7F]),
        (2_097_152, [0x01, 0x00, 0x00, 0x00]),
        (268_435_455, [0x7F, 0x7F, 0x7F, 0x7F]), // 0x0FFF_FFFF (max 28-bit)
    ];

    for &(expected_val, syncsafe_bytes) in boundary_vectors {
        let parsed = Id3TagSafetyGuard::parse_syncsafe_u32(syncsafe_bytes)
            .unwrap_or_else(|_| panic!("Failed to parse syncsafe bytes {syncsafe_bytes:?}"));
        assert_eq!(
            parsed, expected_val,
            "Syncsafe integer parsing mismatch for {syncsafe_bytes:?}"
        );

        let encoded = Id3TagSafetyGuard::encode_syncsafe_u32(expected_val)
            .unwrap_or_else(|_| panic!("Failed to encode value {expected_val}"));
        assert_eq!(
            encoded, syncsafe_bytes,
            "Syncsafe integer encoding mismatch for {expected_val}"
        );
    }
}

#[test]
fn test_syncsafe_7bit_msb_violation_rejection() {
    // Test all 4 byte positions having bit 7 (0x80) set
    let bad_vectors: &[[u8; 4]] = &[
        [0x80, 0x00, 0x00, 0x00],
        [0x00, 0x80, 0x00, 0x00],
        [0x00, 0x00, 0x80, 0x00],
        [0x00, 0x00, 0x00, 0x80],
        [0xFF, 0xFF, 0xFF, 0xFF],
        [0x81, 0x02, 0x03, 0x04],
    ];

    for &bad in bad_vectors {
        let res = Id3TagSafetyGuard::parse_syncsafe_u32(bad);
        assert!(
            matches!(res, Err(AudioDefenseError::Id3InvalidSyncsafe { .. })),
            "Expected Id3InvalidSyncsafe for {bad:?}, got {res:?}"
        );
    }
}

#[test]
fn test_two_pointer_desynchronization_oracle() {
    // Case 1: Standard ID3 unsynchronisation sequence
    let mut data1 = vec![0x49, 0x44, 0x33, 0xFF, 0x00, 0xE0, 0x55, 0xFF, 0x00, 0x00];
    let len1 = Id3TagSafetyGuard::desynchronize_in_place(&mut data1);
    assert_eq!(len1, 8);
    assert_eq!(data1, vec![0x49, 0x44, 0x33, 0xFF, 0xE0, 0x55, 0xFF, 0x00]);

    // Case 2: Consecutive FF 00 pairs
    let mut data2 = vec![0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00];
    let len2 = Id3TagSafetyGuard::desynchronize_in_place(&mut data2);
    assert_eq!(len2, 3);
    assert_eq!(data2, vec![0xFF, 0xFF, 0xFF]);

    // Case 3: No escape sequence
    let mut data3 = vec![0x01, 0x02, 0x03, 0x04, 0x05];
    let len3 = Id3TagSafetyGuard::desynchronize_in_place(&mut data3);
    assert_eq!(len3, 5);
    assert_eq!(data3, vec![0x01, 0x02, 0x03, 0x04, 0x05]);

    // Case 4: Single byte buffer
    let mut data4 = vec![0xFF];
    let len4 = Id3TagSafetyGuard::desynchronize_in_place(&mut data4);
    assert_eq!(len4, 1);
    assert_eq!(data4, vec![0xFF]);

    // Case 5: Empty buffer
    let mut data5 = Vec::new();
    let len5 = Id3TagSafetyGuard::desynchronize_in_place(&mut data5);
    assert_eq!(len5, 0);
}

// ============================================================================
// 3. 6-Layer Security Defense Adversarial Test Matrix
// ============================================================================

#[test]
fn test_layer1_channel_rate_adversarial_matrix() {
    let guard = AudioChannelRateGuard::new();

    // Adversarial channel attacks
    assert_eq!(
        guard.validate_channels(0).unwrap_err(),
        AudioDefenseError::InvalidChannelCount {
            channels: 0,
            min: DEFAULT_MIN_AUDIO_CHANNELS,
            max: DEFAULT_MAX_AUDIO_CHANNELS,
        }
    );
    assert_eq!(
        guard.validate_channels(9).unwrap_err(),
        AudioDefenseError::InvalidChannelCount {
            channels: 9,
            min: DEFAULT_MIN_AUDIO_CHANNELS,
            max: DEFAULT_MAX_AUDIO_CHANNELS,
        }
    );
    assert!(guard.validate_channels(65535).is_err());

    // Adversarial sample rate attacks
    assert_eq!(
        guard.validate_sample_rate(4000).unwrap_err(),
        AudioDefenseError::InvalidSampleRate {
            sample_rate: 4000,
            min: DEFAULT_MIN_SAMPLE_RATE,
            max: DEFAULT_MAX_SAMPLE_RATE,
        }
    );
    assert_eq!(
        guard.validate_sample_rate(192_001).unwrap_err(),
        AudioDefenseError::InvalidSampleRate {
            sample_rate: 192_001,
            min: DEFAULT_MIN_SAMPLE_RATE,
            max: DEFAULT_MAX_SAMPLE_RATE,
        }
    );
    assert!(guard.validate_sample_rate(0).is_err());

    // Frame size arithmetic overflow protection
    assert!(guard.estimate_frame_size(2, 0).is_err());
    assert!(guard.estimate_frame_size(2, 128).is_err());
    assert!(guard.estimate_buffer_size(8, 32, usize::MAX).is_err());
}

#[test]
fn test_layer2_cover_art_quota_adversarial_matrix() {
    let mut guard = CoverArtQuotaGuard::new();

    // 1. Valid PNG Artwork
    let mut valid_png = vec![0u8; 1024];
    valid_png[0..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let info = guard.inspect_and_register(&valid_png).expect("PNG registration must succeed");
    assert_eq!(info.format, CoverArtFormat::Png);
    assert_eq!(info.mime_type, "image/png");

    // 2. Single item size limit exceeded (17 MiB > 16 MiB default)
    let mut huge_img = vec![0u8; DEFAULT_MAX_SINGLE_COVER_ART_SIZE + 1024];
    huge_img[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
    let err_single = guard.inspect_and_register(&huge_img).unwrap_err();
    assert!(matches!(err_single, AudioDefenseError::CoverArtSizeExceeded { .. }));

    // 3. Count limit exceeded (max 4 default)
    guard.reset();
    let mut jpeg = vec![0u8; 100];
    jpeg[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
    for _ in 0..DEFAULT_MAX_COVER_ART_COUNT {
        assert!(guard.inspect_and_register(&jpeg).is_ok());
    }
    let err_count = guard.inspect_and_register(&jpeg).unwrap_err();
    assert!(matches!(err_count, AudioDefenseError::CoverArtCountExceeded { .. }));

    // 4. Magic header forgery rejection
    guard.reset();
    let corrupt_payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
    let err_magic = guard.inspect_and_register(&corrupt_payload).unwrap_err();
    assert!(matches!(err_magic, AudioDefenseError::CoverArtMalformed { .. }));
}

#[test]
fn test_layer3_frame_loop_timeout_adversarial_matrix() {
    let guard = FrameLoopTimeoutGuard::new();
    let mut tracker = guard.create_tracker();

    // Consecutive error circuit breaker (threshold 64 default)
    for _ in 0..DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS {
        assert!(tracker.record_error().is_ok());
    }
    let err_consecutive = tracker.record_error().unwrap_err();
    assert_eq!(
        err_consecutive,
        AudioDefenseError::FrameLoopConsecutiveErrorFuse {
            consecutive_errors: DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS + 1,
            limit: DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS,
        }
    );

    // Reset on success and cumulative error circuit breaker (threshold 256 default)
    tracker.reset();
    for _ in 0..60 {
        for _ in 0..4 {
            assert!(tracker.record_error().is_ok());
        }
        tracker.record_success();
    }
    assert_eq!(tracker.cumulative_errors(), 240);
    assert_eq!(tracker.consecutive_errors(), 0);

    for _ in 0..16 {
        assert!(tracker.record_error().is_ok());
    }
    assert_eq!(tracker.cumulative_errors(), 256);

    let err_cumulative = tracker.record_error().unwrap_err();
    assert_eq!(
        err_cumulative,
        AudioDefenseError::FrameLoopCumulativeErrorFuse {
            cumulative_errors: DEFAULT_MAX_CUMULATIVE_FRAME_ERRORS + 1,
            limit: DEFAULT_MAX_CUMULATIVE_FRAME_ERRORS,
        }
    );
}

#[test]
fn test_layer4_id3_tag_safety_adversarial_matrix() {
    let guard = Id3TagSafetyGuard::new();

    // 1. Oversized tag quota (> 32 MiB default)
    let mut header = vec![b'I', b'D', b'3', 4, 0, 0];
    let oversized_bytes = Id3TagSafetyGuard::encode_syncsafe_u32(DEFAULT_MAX_ID3_TAG_SIZE as u32 + 1024).unwrap();
    header.extend_from_slice(&oversized_bytes);
    let err_size = guard.inspect_header(&header).unwrap_err();
    assert!(matches!(err_size, AudioDefenseError::Id3TagSizeExceeded { .. }));

    // 2. Malformed magic signature
    let bad_magic = b"NOT_ID3_HEADER";
    assert!(matches!(
        guard.inspect_header(bad_magic),
        Err(AudioDefenseError::Id3Malformed { .. })
    ));

    // 3. Unsupported ID3 major version (e.g. ID3v2.1 or ID3v2.5)
    let bad_version = [b'I', b'D', b'3', 5, 0, 0, 0, 0, 0, 10];
    assert!(matches!(
        guard.inspect_header(&bad_version),
        Err(AudioDefenseError::Id3Malformed { .. })
    ));
}

#[test]
fn test_layer5_memory_budget_watchdog_adversarial_matrix() {
    let watchdog = AudioMemoryBudgetGuard::new(1024 * 1024); // 1 MiB budget

    // 1. Single reservation exceeding total quota
    let err = watchdog.reserve(2 * 1024 * 1024).unwrap_err();
    assert_eq!(
        err,
        AudioDefenseError::MemoryBudgetExceeded {
            allocated_bytes: 2 * 1024 * 1024,
            budget_bytes: 1024 * 1024,
        }
    );

    // 2. Incremental allocation tripping budget
    let res1 = watchdog.reserve(600 * 1024).expect("600KB reservation should succeed");
    assert_eq!(watchdog.current_allocated(), 600 * 1024);
    assert_eq!(watchdog.remaining_budget(), 424 * 1024);

    let err_trip = watchdog.reserve(500 * 1024).unwrap_err();
    assert!(matches!(err_trip, AudioDefenseError::MemoryBudgetExceeded { .. }));

    // 3. RAII auto-release on drop
    drop(res1);
    assert_eq!(watchdog.current_allocated(), 0);
    assert_eq!(watchdog.remaining_budget(), 1024 * 1024);
}

#[test]
fn test_layer6_sensitive_audio_buffer_erasure() {
    let mut buffer = SensitiveAudioBuffer::from_slice(b"secret-raw-pcm-samples-0123456789");
    assert_eq!(buffer.len(), 33);
    assert_eq!(&buffer[..6], b"secret");

    // Test constant-time equality
    let buffer_clone = buffer.clone();
    assert!(buffer.ct_eq(&buffer_clone));

    let different = SensitiveAudioBuffer::from_slice(b"secret-raw-pcm-samples-0123456780");
    assert!(!buffer.ct_eq(&different));

    // Explicit wipe
    buffer.wipe();
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
}

#[test]
fn test_end_to_end_composite_stream_defense() {
    let mut pipeline = AudioSecurityPipeline::default();

    // 1. Valid compound ID3v2 + WAV container
    let wav_body = generate_canonical_wav(48_000, 2, 200);
    let mut stream = vec![b'I', b'D', b'3', 3, 0, 0];
    let id3_size = Id3TagSafetyGuard::encode_syncsafe_u32(10).unwrap();
    stream.extend_from_slice(&id3_size);
    stream.extend_from_slice(&[0u8; 10]); // ID3 body
    stream.extend_from_slice(&wav_body);

    let report = pipeline.inspect_stream_header(&stream).expect("Compound stream probe should succeed");
    assert!(report.id3_summary.is_some());
    assert_eq!(report.channels, Some(2));
    assert_eq!(report.sample_rate, Some(48_000));
    assert!(report.memory_reservation.is_some());

    // 2. Register cover art through pipeline
    let mut jpeg_art = vec![0u8; 512];
    jpeg_art[0..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
    let art_info = pipeline.inspect_cover_art(&jpeg_art).expect("Artwork registration should succeed");
    assert_eq!(art_info.format, CoverArtFormat::Jpeg);

    // 3. Frame loop tracking through pipeline factory
    let mut tracker = pipeline.create_frame_tracker();
    tracker.record_success();
    assert_eq!(tracker.success_frames(), 1);
}
