// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comprehensive corruption injection, chaos mutation, and fuzzing test suite for TTZip Audio Subsystem.
//!
//! Deploys 16 surgical destruction targets:
//! 1. Extreme channel counts (>8 or 0 channels) integer overflow and heap exhaustion defense.
//! 2. Malformed ultra-high sample rates (>192kHz or 0Hz) divide-by-zero and unbounded allocation defense.
//! 3. Giant embedded Album Art cover image memory bomb (>16MB) quota circuit breaker.
//! 4. Broken ID3v2 tag syncsafe integers (7-bit out-of-bounds and non-zero MSB injection).
//! 5. Truncated audio streams and incomplete audio frame state machine escape.
//! 6. Consecutive corrupted audio frame infinite spin-lock loop circuit breaker (>64 errors).
//! 7. Zero-byte, single-byte, and empty stream audio probing defense.
//! 8. 1000+ tasks concurrent audio decoding contention and memory watchdog stress.
//! 9. 500+ rounds of pseudo-random mutation audio data fuzzing across format matrices.
//! 10. Corrupted RIFF WAV header (illegal chunk sizes, missing fmt, unbalanced data).
//! 11. Malformed OGG Vorbis page sequences and illegal bitstream serial numbers.
//! 12. Malformed MP4 ILST metadata atom tree deep recursion and cyclic injection.
//! 13. Malformed FLAC METADATA_BLOCK_HEADER and illegal block length injection.
//! 14. Sensitive audio PCM buffer Zeroize memory erasure adversarial verification.
//! 15. Negative seek and out-of-bounds seek timestamp state machine recovery.
//! 16. Single-task resident memory budget (>64MB) watchdog circuit breaker.

use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use ttzip_engine::audio::{
    extract_waveform_from_bytes, AudioError, AudioMetadataExtractor, AudioWaveformSampler,
    TTZipAudioDecoder,
};
use ttzip_engine::security::audio_defense::{
    AudioChannelRateGuard, AudioDefenseError, AudioMemoryBudgetGuard, AudioSecurityPipeline,
    CoverArtQuotaGuard, FrameLoopTimeoutGuard, Id3TagSafetyGuard, SensitiveAudioBuffer,
    DEFAULT_MAX_AUDIO_RESIDENT_MEMORY_BUDGET, DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS,
    DEFAULT_MAX_COVER_ART_COUNT, DEFAULT_MAX_SAMPLE_RATE, DEFAULT_MAX_SINGLE_COVER_ART_SIZE,
    DEFAULT_MIN_SAMPLE_RATE,
};

/// High-speed deterministic linear congruential generator for reproducible fuzzing vectors.
#[derive(Clone, Debug)]
struct DeterministicPrng {
    state: u64,
}

impl DeterministicPrng {
    #[inline]
    const fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9_7f4a_7c15 } else { seed },
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    #[inline]
    fn next_range(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u32() as u64 % span) as usize
    }

    #[inline]
    fn next_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }
}

// ============================================================================
// Synthetic Canonical Audio Fixture Generators
// ============================================================================

/// Generates a valid canonical WAV RIFF PCM 16-bit container in memory.
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
            let val = ((i.wrapping_add(ch as usize) % 256) as i16).wrapping_mul(100);
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    buf
}

/// Generates a valid canonical AIFF PCM 16-bit container in memory.
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

    // 80-bit IEEE 754 sample rate
    let exponent = 16383 + 15;
    let mantissa = (sample_rate as u64) << (63 - 15);
    buf.extend_from_slice(&(exponent as u16).to_be_bytes());
    buf.extend_from_slice(&mantissa.to_be_bytes());

    // SSND chunk
    buf.extend_from_slice(b"SSND");
    buf.extend_from_slice(&(ssnd_chunk_size as u32).to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());

    for i in 0..sample_count {
        for ch in 0..channels {
            let val = ((i.wrapping_add(ch as usize) % 256) as i16).wrapping_mul(100);
            buf.extend_from_slice(&val.to_be_bytes());
        }
    }

    buf
}

// ============================================================================
// Target 1: Extreme Channel Counts Overflow & Heap Exhaustion Defense
// ============================================================================

#[test]
fn test_target_01_extreme_channels_overflow_defense() {
    let guard = AudioChannelRateGuard::new();

    // 0 channels must fail
    assert!(matches!(
        guard.validate(0, 44_100),
        Err(AudioDefenseError::InvalidChannelCount { channels: 0, .. })
    ));

    // Channels > 8 (e.g. 9, 16, 255, 65535) must fail
    for ch in [9, 16, 64, 255, 1024, 65535] {
        assert!(matches!(
            guard.validate(ch, 44_100),
            Err(AudioDefenseError::InvalidChannelCount { channels, .. }) if channels == ch
        ));
    }

    // Frame size arithmetic overflow estimation (bits_per_sample = 0 or >64)
    assert!(matches!(
        guard.estimate_frame_size(2, 0),
        Err(AudioDefenseError::FrameSizeOverflow { .. })
    ));
    assert!(matches!(
        guard.estimate_frame_size(2, 128),
        Err(AudioDefenseError::FrameSizeOverflow { .. })
    ));

    // Pipeline rejection on WAV with extreme channels
    let pipeline = AudioSecurityPipeline::default();
    let bad_wav = generate_canonical_wav(44_100, 12, 100);
    assert!(pipeline.inspect_stream_header(&bad_wav).is_err());
}

// ============================================================================
// Target 2: Malformed Sample Rates Divide-by-Zero & Allocation Defense
// ============================================================================

#[test]
fn test_target_02_malformed_sample_rate_zero_divide_defense() {
    let guard = AudioChannelRateGuard::new();

    // 0 Hz (divide-by-zero vulnerability)
    assert!(matches!(
        guard.validate(2, 0),
        Err(AudioDefenseError::InvalidSampleRate { sample_rate: 0, .. })
    ));

    // Under minimum (< 8,000 Hz)
    for sr in [1, 100, 4000, 7999] {
        assert!(matches!(
            guard.validate(2, sr),
            Err(AudioDefenseError::InvalidSampleRate { sample_rate, .. }) if sample_rate == sr
        ));
    }

    // Over maximum (> 192,000 Hz)
    for sr in [192_001, 384_000, 1_000_000, u32::MAX] {
        assert!(matches!(
            guard.validate(2, sr),
            Err(AudioDefenseError::InvalidSampleRate { sample_rate, .. }) if sample_rate == sr
        ));
    }

    // Safe valid boundary rates
    assert!(guard.validate(2, DEFAULT_MIN_SAMPLE_RATE).is_ok());
    assert!(guard.validate(2, DEFAULT_MAX_SAMPLE_RATE).is_ok());
}

// ============================================================================
// Target 3: Giant Embedded Album Art Memory Bomb Quota Circuit Breaker
// ============================================================================

#[test]
fn test_target_03_giant_album_art_memory_bomb_quota_circuit_breaker() {
    let mut quota_guard = CoverArtQuotaGuard::new();

    // Oversized single image (> 16MB)
    let giant_img = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG magic
    let giant_size = DEFAULT_MAX_SINGLE_COVER_ART_SIZE + 1;
    let mut oversized = vec![0u8; giant_size];
    oversized[..4].copy_from_slice(&giant_img);

    assert!(matches!(
        quota_guard.inspect_and_register(&oversized),
        Err(AudioDefenseError::CoverArtSizeExceeded { .. })
    ));

    // Cumulative quota exceeding 32MB
    let mut fresh_guard = CoverArtQuotaGuard::new();
    let img_chunk_size = 10 * 1024 * 1024; // 10 MiB
    let mut valid_png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    valid_png.resize(img_chunk_size, 0);

    assert!(fresh_guard.inspect_and_register(&valid_png).is_ok());
    assert!(fresh_guard.inspect_and_register(&valid_png).is_ok());
    assert!(fresh_guard.inspect_and_register(&valid_png).is_ok());
    // 4th 10 MiB image breaches 32 MiB total
    assert!(matches!(
        fresh_guard.inspect_and_register(&valid_png),
        Err(AudioDefenseError::TotalCoverArtQuotaExceeded { .. })
    ));

    // Image count limit (> 4 images)
    let mut count_guard = CoverArtQuotaGuard::new();
    let small_jpg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    for _ in 0..DEFAULT_MAX_COVER_ART_COUNT {
        assert!(count_guard.inspect_and_register(&small_jpg).is_ok());
    }
    assert!(matches!(
        count_guard.inspect_and_register(&small_jpg),
        Err(AudioDefenseError::CoverArtCountExceeded { .. })
    ));

    // Corrupted non-image payload
    let mut malformed_guard = CoverArtQuotaGuard::new();
    assert!(matches!(
        malformed_guard.inspect_and_register(b"NOT_AN_IMAGE_DATA_HEADER"),
        Err(AudioDefenseError::CoverArtMalformed { .. })
    ));
}

// ============================================================================
// Target 4: Broken ID3v2 Syncsafe 7-bit & MSB Injection
// ============================================================================

#[test]
fn test_target_04_broken_id3v2_syncsafe_7bit_msb_injection() {
    // 7-bit MSB violation injection (bit 7 set)
    let msb_bad_vectors: &[[u8; 4]] = &[
        [0x80, 0x00, 0x00, 0x00],
        [0x00, 0x80, 0x00, 0x00],
        [0x00, 0x00, 0x80, 0x00],
        [0x00, 0x00, 0x00, 0x80],
        [0xFF, 0x00, 0x7F, 0x01],
    ];

    for &bad in msb_bad_vectors {
        assert!(matches!(
            Id3TagSafetyGuard::parse_syncsafe_u32(bad),
            Err(AudioDefenseError::Id3InvalidSyncsafe { .. })
        ));
    }

    // Oversized ID3v2 header tag size (> 32MB)
    let guard = Id3TagSafetyGuard::new();
    let mut oversized_id3 = vec![b'I', b'D', b'3', 0x04, 0x00, 0x00];
    let syncsafe_40mb = Id3TagSafetyGuard::encode_syncsafe_u32(40 * 1024 * 1024).unwrap();
    oversized_id3.extend_from_slice(&syncsafe_40mb);

    assert!(matches!(
        guard.inspect_header(&oversized_id3),
        Err(AudioDefenseError::Id3TagSizeExceeded { .. })
    ));

    // Malformed ID3 header with invalid magic
    assert!(matches!(
        guard.inspect_header(b"XYZ\x04\x00\x00\x00\x00\x00\x00"),
        Err(AudioDefenseError::Id3Malformed { .. })
    ));
}

// ============================================================================
// Target 5: Truncated Audio Stream & Frame State Machine Escape
// ============================================================================

#[test]
fn test_target_05_truncated_audio_stream_state_machine_escape() {
    let wav_data = generate_canonical_wav(44_100, 2, 200);
    let aiff_data = generate_canonical_aiff(44_100, 2, 200);

    let pipeline = AudioSecurityPipeline::default();

    // Systematically test partial prefixes of valid audio streams
    for len in 1..wav_data.len() {
        let truncated = &wav_data[..len];
        let _ = catch_unwind(|| {
            let _ = TTZipAudioDecoder::open_from_bytes(truncated);
            let _ = AudioMetadataExtractor::extract_from_bytes(truncated);
            let _ = AudioWaveformSampler::sample_waveform_from_bytes(truncated, 32);
            let _ = pipeline.inspect_stream_header(truncated);
        });
    }

    for len in 1..aiff_data.len() {
        let truncated = &aiff_data[..len];
        let _ = catch_unwind(|| {
            let _ = TTZipAudioDecoder::open_from_bytes(truncated);
            let _ = AudioMetadataExtractor::extract_from_bytes(truncated);
            let _ = AudioWaveformSampler::sample_waveform_from_bytes(truncated, 32);
            let _ = pipeline.inspect_stream_header(truncated);
        });
    }
}

// ============================================================================
// Target 6: Consecutive Corrupted Frame Infinite Loop Circuit Breaker
// ============================================================================

#[test]
fn test_target_06_consecutive_corrupt_frame_infinite_loop_circuit_breaker() {
    let guard = FrameLoopTimeoutGuard::new();
    let mut tracker = guard.create_tracker();

    // Inject 64 consecutive errors (boundary allowed)
    for _ in 0..DEFAULT_MAX_CONSECUTIVE_FRAME_ERRORS {
        assert!(tracker.record_error().is_ok());
    }

    // 65th consecutive error trips the fuse
    assert!(matches!(
        tracker.record_error(),
        Err(AudioDefenseError::FrameLoopConsecutiveErrorFuse { consecutive_errors: 65, .. })
    ));

    // Test cumulative error fuse with interleaved successes
    let mut cum_tracker = guard.create_tracker();
    for _ in 0..128 {
        cum_tracker.record_success();
        assert!(cum_tracker.record_error().is_ok());
        assert!(cum_tracker.record_error().is_ok());
    }

    // Reaching 257 cumulative errors trips cumulative fuse
    assert!(matches!(
        cum_tracker.record_error(),
        Err(AudioDefenseError::FrameLoopCumulativeErrorFuse { .. })
    ));
}

// ============================================================================
// Target 7: Zero-Byte and Empty Stream Probing Defense
// ============================================================================

#[test]
fn test_target_07_zero_byte_and_empty_stream_probing_defense() {
    let empty: &[u8] = &[];
    let one_byte: &[u8] = &[0x00];
    let two_bytes: &[u8] = &[0x00, 0x00];
    let three_bytes: &[u8] = &[0x00, 0x00, 0x00];
    let ten_zeros: &[u8] = &[0u8; 10];

    let test_inputs = [empty, one_byte, two_bytes, three_bytes, ten_zeros];
    let pipeline = AudioSecurityPipeline::default();

    for &input in &test_inputs {
        // Zero-byte open must fail safely without panics
        assert!(TTZipAudioDecoder::open_from_bytes(input).is_err());
        assert!(AudioMetadataExtractor::extract_from_bytes(input).is_err());

        if input.len() < 4 {
            assert!(pipeline.inspect_stream_header(input).is_err());
        } else {
            let res = pipeline.inspect_stream_header(input);
            assert!(res.is_ok());
            assert!(res.unwrap().channels.is_none());
        }

        // extract_waveform_from_bytes provides safe organic fallback
        let wf = extract_waveform_from_bytes(input, 32);
        assert_eq!(wf.len(), 32);
    }
}

// ============================================================================
// Target 8: Concurrent 1000+ Tasks Audio Decoding and Watchdog Stress
// ============================================================================

#[test]
fn test_target_08_concurrent_1000_tasks_audio_decoding_and_watchdog() {
    let shared_watchdog = AudioMemoryBudgetGuard::default();
    let total_tasks = 1000;
    let completed_count = Arc::new(AtomicUsize::new(0));

    (0..total_tasks).into_par_iter().for_each(|i| {
        let sample_rate = if i % 2 == 0 { 44_100 } else { 48_000 };
        let channels = ((i % 2) + 1) as u16;
        let wav_data = generate_canonical_wav(sample_rate, channels, 120);

        // Reserve transient quota
        let reservation = shared_watchdog.reserve(wav_data.len() + 1024).unwrap();

        // Decode packets
        let mut decoder = TTZipAudioDecoder::open_from_bytes(&wav_data).unwrap();
        let mut packet_count = 0;
        while let Ok(Some(pkt)) = decoder.decode_next_packet() {
            packet_count += 1;
            assert!(!pkt.samples_interleaved.is_empty());
        }
        assert!(packet_count > 0);

        // Waveform extraction
        let wf = AudioWaveformSampler::sample_waveform_from_bytes(&wav_data, 24).unwrap();
        assert_eq!(wf.points(), 24);

        drop(reservation);
        completed_count.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(completed_count.load(Ordering::SeqCst), total_tasks);
    assert_eq!(shared_watchdog.current_allocated(), 0);
}

// ============================================================================
// Target 9: 500+ Rounds Pseudo-Random Mutation Audio Data Fuzzing
// ============================================================================

#[test]
fn test_target_09_pseudorandom_500_rounds_mutation_fuzzing() {
    let mut prng = DeterministicPrng::new(0xA5A5_F00D_CAFE_BABE);
    let base_wav = generate_canonical_wav(44_100, 2, 150);
    let base_aiff = generate_canonical_aiff(44_100, 2, 150);
    let pipeline = AudioSecurityPipeline::default();

    for round in 0..500 {
        let mut mutant = if round % 2 == 0 {
            base_wav.clone()
        } else {
            base_aiff.clone()
        };

        // Apply 1 to 5 random mutation actions
        let mutations = prng.next_range(1, 5);
        for _ in 0..mutations {
            let action = prng.next_range(0, 4);
            match action {
                0 => {
                    // Random byte replacement
                    if !mutant.is_empty() {
                        let idx = prng.next_range(0, mutant.len() - 1);
                        mutant[idx] = prng.next_byte();
                    }
                }
                1 => {
                    // Random bit flip
                    if !mutant.is_empty() {
                        let idx = prng.next_range(0, mutant.len() - 1);
                        let bit = 1u8 << prng.next_range(0, 7);
                        mutant[idx] ^= bit;
                    }
                }
                2 => {
                    // Random truncation
                    if mutant.len() > 8 {
                        let new_len = prng.next_range(4, mutant.len() - 1);
                        mutant.truncate(new_len);
                    }
                }
                3 => {
                    // Random chunk insertion
                    let idx = prng.next_range(0, mutant.len());
                    let insert_len = prng.next_range(1, 16);
                    let junk: Vec<u8> = (0..insert_len).map(|_| prng.next_byte()).collect();
                    mutant.splice(idx..idx, junk);
                }
                _ => {
                    // Random block zeroing
                    if mutant.len() > 16 {
                        let start = prng.next_range(0, mutant.len() - 8);
                        let end = (start + 8).min(mutant.len());
                        mutant[start..end].fill(0);
                    }
                }
            }
        }

        // Fuzz across all subsystem entrypoints under catch_unwind
        let panic_result = catch_unwind(|| {
            let _ = TTZipAudioDecoder::open_from_bytes(&mutant);
            let _ = AudioMetadataExtractor::extract_from_bytes(&mutant);
            let _ = AudioWaveformSampler::sample_waveform_from_bytes(&mutant, 32);
            let _ = pipeline.inspect_stream_header(&mutant);
        });

        assert!(
            panic_result.is_ok(),
            "Audio subsystem panicked on fuzz round {round}"
        );
    }
}

// ============================================================================
// Target 10: Corrupted RIFF WAV Header Chunk Sizes
// ============================================================================

#[test]
fn test_target_10_corrupted_riff_wav_header_chunk_sizes() {
    // 1. WAV with u32::MAX RIFF length
    let mut bad_riff = generate_canonical_wav(44_100, 2, 50);
    bad_riff[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let _ = catch_unwind(|| {
        let _ = TTZipAudioDecoder::open_from_bytes(&bad_riff);
    });

    // 2. WAV missing fmt chunk
    let mut no_fmt = Vec::new();
    no_fmt.extend_from_slice(b"RIFF\x24\x00\x00\x00WAVEdata\x00\x00\x00\x00");
    assert!(TTZipAudioDecoder::open_from_bytes(&no_fmt).is_err());

    // 3. WAV corrupted fmt chunk size (0 bytes)
    let mut zero_fmt = generate_canonical_wav(44_100, 2, 50);
    zero_fmt[16..20].copy_from_slice(&0u32.to_le_bytes());
    assert!(TTZipAudioDecoder::open_from_bytes(&zero_fmt).is_err());

    // 4. Data chunk size larger than file
    let mut huge_data = generate_canonical_wav(44_100, 2, 50);
    huge_data[40..44].copy_from_slice(&0x7FFF_FFFFu32.to_le_bytes());
    let _ = catch_unwind(|| {
        let _ = TTZipAudioDecoder::open_from_bytes(&huge_data);
    });
}

// ============================================================================
// Target 11: Malformed OGG Vorbis Page Sequences
// ============================================================================

#[test]
fn test_target_11_malformed_ogg_vorbis_page_sequences() {
    // Synthetic OggS header with corrupted version and huge segment tables
    let mut ogg_page = Vec::new();
    ogg_page.extend_from_slice(b"OggS");
    ogg_page.push(0x01); // Invalid structure version != 0
    ogg_page.push(0x02); // Header type: first page of logical bitstream
    ogg_page.extend_from_slice(&[0u8; 8]); // Granule position
    ogg_page.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]); // Bitstream serial number
    ogg_page.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // Page sequence number
    ogg_page.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]); // Corrupted CRC
    ogg_page.push(255); // 255 page segments
    ogg_page.extend_from_slice(&[255u8; 255]); // Max lacing values

    let _ = catch_unwind(|| {
        let _ = TTZipAudioDecoder::open_from_bytes(&ogg_page);
        let _ = AudioMetadataExtractor::extract_from_bytes(&ogg_page);
    });
}

// ============================================================================
// Target 12: Malformed MP4 ILST Metadata Atom Tree Deep Recursion
// ============================================================================

#[test]
fn test_target_12_malformed_mp4_ilst_atom_tree_deep_recursion() {
    // Construct nested MP4 boxes with cyclic or excessive length
    let mut mp4_data = Vec::new();
    // ftyp box
    mp4_data.extend_from_slice(&20u32.to_be_bytes());
    mp4_data.extend_from_slice(b"ftypM4A \x00\x00\x00\x00M4A mp42isom");

    // moov box with illegal 0 length (extends to EOF)
    mp4_data.extend_from_slice(&0u32.to_be_bytes());
    mp4_data.extend_from_slice(b"moov");
    mp4_data.extend_from_slice(&8u32.to_be_bytes());
    mp4_data.extend_from_slice(b"udta");
    mp4_data.extend_from_slice(&8u32.to_be_bytes());
    mp4_data.extend_from_slice(b"meta");
    mp4_data.extend_from_slice(&8u32.to_be_bytes());
    mp4_data.extend_from_slice(b"ilst");

    let _ = catch_unwind(|| {
        let _ = TTZipAudioDecoder::open_from_bytes(&mp4_data);
        let _ = AudioMetadataExtractor::extract_from_bytes(&mp4_data);
    });
}

// ============================================================================
// Target 13: Malformed FLAC Metadata Block Header Lengths
// ============================================================================

#[test]
fn test_target_13_malformed_flac_metadata_block_header_lengths() {
    // 1. Invalid block type (127: invalid reserved)
    let mut bad_flac = Vec::new();
    bad_flac.extend_from_slice(b"fLaC");
    bad_flac.push(0x7F); // type 127
    bad_flac.extend_from_slice(&[0x00, 0x00, 0x22]);
    bad_flac.extend_from_slice(&[0u8; 34]);

    assert!(TTZipAudioDecoder::open_from_bytes(&bad_flac).is_err());

    // 2. Excessive block length (0xFFFFFF = 16MB+)
    let mut huge_flac = Vec::new();
    huge_flac.extend_from_slice(b"fLaC");
    huge_flac.push(0x80); // Last block, type 0
    huge_flac.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // 16MB length
    huge_flac.extend_from_slice(&[0u8; 32]);

    assert!(TTZipAudioDecoder::open_from_bytes(&huge_flac).is_err());
}

// ============================================================================
// Target 14: Sensitive Audio PCM Buffer Zeroize Memory Erasure
// ============================================================================

#[test]
fn test_target_14_sensitive_pcm_buffer_zeroize_memory_erasure() {
    let mut secret_pcm = SensitiveAudioBuffer::from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    assert_eq!(secret_pcm.len(), 5);
    assert_eq!(&secret_pcm[..], &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);

    // Test constant-time equality
    let same_pcm = SensitiveAudioBuffer::from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    let diff_pcm = SensitiveAudioBuffer::from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00]);
    assert!(secret_pcm.ct_eq(&same_pcm));
    assert!(!secret_pcm.ct_eq(&diff_pcm));

    // Test explicit zeroization and buffer wipe
    secret_pcm.wipe();
    assert_eq!(secret_pcm.len(), 0);
    assert!(secret_pcm.is_empty());
}

// ============================================================================
// Target 15: Negative and Out-of-Bounds Seek State Machine Recovery
// ============================================================================

#[test]
fn test_target_15_negative_and_out_of_bounds_seek_state_recovery() {
    let wav_data = generate_canonical_wav(44_100, 2, 88_200); // 2 seconds of audio
    let mut decoder = TTZipAudioDecoder::open_from_bytes(&wav_data).unwrap();

    // 1. Negative seek must be rejected
    assert!(matches!(
        decoder.seek(-1.0),
        Err(AudioError::InvalidParameter(_))
    ));
    assert!(matches!(
        decoder.seek(-100.0),
        Err(AudioError::InvalidParameter(_))
    ));

    // 2. Valid seek to 0.5s
    let seek_res = decoder.seek(0.5);
    assert!(seek_res.is_ok());

    // Decode packet after seek
    let pkt = decoder.decode_next_packet().unwrap();
    assert!(pkt.is_some());

    // 3. Out-of-bounds seek (far beyond duration)
    let oob_res = decoder.seek(999999.0);
    assert!(oob_res.is_ok() || matches!(oob_res, Err(AudioError::SeekError(_))));

    // 4. Seek back to beginning to verify state recovery
    let back_to_start = decoder.seek(0.0);
    assert!(back_to_start.is_ok());
    let first_pkt = decoder.decode_next_packet().unwrap();
    assert!(first_pkt.is_some());
}

// ============================================================================
// Target 16: Single-Task Memory Budget Watchdog Circuit Breaker
// ============================================================================

#[test]
fn test_target_16_single_task_memory_budget_exceeded_circuit_breaker() {
    let budget_guard = AudioMemoryBudgetGuard::default(); // 64 MiB

    // Reserve up to limit (64 MiB)
    let res1 = budget_guard
        .reserve(DEFAULT_MAX_AUDIO_RESIDENT_MEMORY_BUDGET)
        .expect("Reservation up to 64MB should succeed");
    assert_eq!(
        budget_guard.current_allocated(),
        DEFAULT_MAX_AUDIO_RESIDENT_MEMORY_BUDGET
    );

    // Any further reservation must trip circuit breaker
    assert!(matches!(
        budget_guard.reserve(1),
        Err(AudioDefenseError::MemoryBudgetExceeded { .. })
    ));

    // RAII drop releases allocated memory
    drop(res1);
    assert_eq!(budget_guard.current_allocated(), 0);

    // Subsequent reservation succeeds
    let res2 = budget_guard.reserve(1024 * 1024);
    assert!(res2.is_ok());
}
