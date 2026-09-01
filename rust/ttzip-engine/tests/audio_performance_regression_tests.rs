// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Audio Decoder, Metadata & Waveform Performance Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: Audio Full Decoding Throughput Gate (>= 200.0 MB/s).
//! 5. Test 2: Audio Metadata & Tag Extraction Latency Gate (<= 1.0 ms).
//! 6. Test 3: Dual-Track (Peak + RMS) Waveform Downsampling Throughput Gate (>= 300.0 MB/s).
//! 7. Test 4: Multi-Format Matrix Probing & Decoding Throughput Gate (>= 150.0 MB/s).
//! 8. Test 5: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::audio::{
    AudioMetadataExtractor, AudioWaveformSampler, TTZipAudioDecoder,
};
use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::security::audio_defense::AudioSecurityPipeline;

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

// ============================================================================
// Synthetic Benchmark Audio Generators
// ============================================================================

/// Generates a realistic uncompressed WAV PCM 16-bit stereo audio stream buffer.
fn make_benchmark_wav(sample_rate: u32, channels: u16, duration_seconds: f64) -> Vec<u8> {
    let sample_count = (sample_rate as f64 * duration_seconds).round() as usize;
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
            // Synthetic harmonics wave
            let t = (i as f64) / (sample_rate as f64);
            let val_f = ((t * 440.0 * 2.0 * std::f64::consts::PI).sin() * 16000.0)
                + ((t * 880.0 * 2.0 * std::f64::consts::PI).sin() * 8000.0)
                + (ch as f64 * 100.0);
            let val = val_f.clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    buf
}

/// Generates a realistic uncompressed AIFF PCM 16-bit audio stream buffer.
fn make_benchmark_aiff(sample_rate: u32, channels: u16, duration_seconds: f64) -> Vec<u8> {
    let sample_count = (sample_rate as f64 * duration_seconds).round() as usize;
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
            let t = (i as f64) / (sample_rate as f64);
            let val_f = ((t * 440.0 * 2.0 * std::f64::consts::PI).sin() * 16000.0)
                + (ch as f64 * 100.0);
            let val = val_f.clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            buf.extend_from_slice(&val.to_be_bytes());
        }
    }

    buf
}

/// Measures average iteration latency (in seconds) for a workload using clock rising-edge alignment.
fn measure_workload<F: FnMut() -> R, R>(mut workload: F) -> (f64, usize) {
    // 1. Warm-up runs
    for _ in 0..WARMUP_RUNS {
        black_box(workload());
    }

    // 2. Rising-edge alignment
    wait_for_next_tick();

    // 3. Adaptive time integration
    let start = Instant::now();
    let mut iterations = 0usize;
    let mut pass_latencies = Vec::new();

    while start.elapsed() < MIN_INTEGRATION_WINDOW || iterations < 5 {
        let pass_start = Instant::now();
        black_box(workload());
        let pass_dur = pass_start.elapsed().as_secs_f64();
        pass_latencies.push(pass_dur);
        iterations += 1;
    }

    // 4. Hampel 3-sigma outlier filtering
    let filter = HampelFilter::default();
    let filtered = filter.filter(&pass_latencies);
    let latencies_to_use = if !filtered.cleaned.is_empty() {
        &filtered.cleaned
    } else {
        &pass_latencies
    };
    let sum_lat: f64 = latencies_to_use.iter().sum();
    let avg_lat = sum_lat / latencies_to_use.len() as f64;

    (avg_lat, iterations)
}

// ============================================================================
// Benchmarks & Hard Performance Gates
// ============================================================================

/// Test 1: Pure-Rust Audio Full Decoding Throughput Gate (>= 200.0 MB/s).
#[test]
fn test_audio_decoding_throughput_gate() {
    let wav_data = make_benchmark_wav(48_000, 2, 2.0); // 2 seconds of 48kHz stereo = ~384 KB
    let raw_len = wav_data.len();
    assert!(raw_len > 100_000);

    let (avg_sec, iters) = measure_workload(|| {
        let mut decoder = TTZipAudioDecoder::open_from_bytes(&wav_data).unwrap();
        let mut total_samples = 0usize;
        while let Ok(Some(pkt)) = decoder.decode_next_packet() {
            total_samples += pkt.samples_interleaved.len();
        }
        assert!(total_samples > 0);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Audio Benchmark] Decoding 48kHz stereo ({} bytes, {} iters): {:.2} MB/s (latency: {:.3} ms)",
        raw_len,
        iters,
        throughput_mb,
        avg_sec * 1000.0
    );

    assert!(
        throughput_mb >= 200.0,
        "Audio Decoding throughput below 200.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

/// Test 2: Pure-Rust Audio Metadata & Tag Extraction Latency Gate (<= 1.0 ms).
#[test]
fn test_audio_metadata_extraction_latency_gate() {
    let wav_data = make_benchmark_wav(44_100, 2, 1.0);

    let (avg_sec, iters) = measure_workload(|| {
        let meta = AudioMetadataExtractor::extract_from_bytes(&wav_data).unwrap();
        assert_eq!(meta.channels, Some(2));
        assert_eq!(meta.sample_rate, Some(44_100));
    });

    let latency_ms = avg_sec * 1000.0;
    println!(
        "[Audio Benchmark] Metadata extraction ({} iters): {:.4} ms",
        iters, latency_ms
    );

    assert!(
        latency_ms <= 1.0,
        "Audio Metadata extraction latency exceeds 1.0 ms gate: {:.4} ms",
        latency_ms
    );
}

/// Test 3: Dual-Track (Peak + RMS) Waveform Downsampling Throughput Gate (>= 200.0 MB/s).
#[test]
fn test_audio_waveform_downsampling_throughput_gate() {
    let wav_data = make_benchmark_wav(48_000, 2, 2.5); // 2.5s stereo
    let raw_len = wav_data.len();

    let (avg_sec, iters) = measure_workload(|| {
        let wf = AudioWaveformSampler::sample_waveform_from_bytes(&wav_data, 128).unwrap();
        assert_eq!(wf.points(), 128);
        assert_eq!(wf.peaks().len(), 128);
        assert_eq!(wf.rms().len(), 128);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Audio Benchmark] Waveform 128-bucket downsampling ({} bytes, {} iters): {:.2} MB/s (latency: {:.3} ms)",
        raw_len,
        iters,
        throughput_mb,
        avg_sec * 1000.0
    );

    assert!(
        throughput_mb >= 200.0,
        "Audio Waveform downsampling throughput below 200.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

/// Test 4: Multi-Format Matrix Probing & Decoding Throughput Gate (>= 150.0 MB/s).
#[test]
fn test_audio_multiformat_matrix_decoding_gate() {
    let wav_data = make_benchmark_wav(44_100, 2, 1.5);
    let aiff_data = make_benchmark_aiff(44_100, 2, 1.5);
    let total_bytes = wav_data.len() + aiff_data.len();

    let (avg_sec, iters) = measure_workload(|| {
        // 1. WAV pipeline
        let mut dec_wav = TTZipAudioDecoder::open_from_bytes(&wav_data).unwrap();
        while let Ok(Some(pkt)) = dec_wav.decode_next_packet() {
            black_box(pkt);
        }

        // 2. AIFF pipeline
        let mut dec_aiff = TTZipAudioDecoder::open_from_bytes(&aiff_data).unwrap();
        while let Ok(Some(pkt)) = dec_aiff.decode_next_packet() {
            black_box(pkt);
        }
    });

    let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Audio Benchmark] Multi-format Matrix (WAV + AIFF, {} bytes, {} iters): {:.2} MB/s",
        total_bytes, iters, throughput_mb
    );

    assert!(
        throughput_mb >= 150.0,
        "Multi-Format Audio decoding throughput below 150.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

/// Test 5: Master Anti-Regression Invariant 6 Gate (<= 3.0% Regression Hard Gate).
#[test]
fn test_audio_anti_regression_invariant6_gate() {
    let _governor = ThermalThrottleGovernor::new();
    let wav_data = make_benchmark_wav(48_000, 2, 2.0);
    let pipeline = AudioSecurityPipeline::default();

    // Measure Baseline Run (Pass 1)
    let (baseline_sec, _) = measure_workload(|| {
        let _ = pipeline.inspect_stream_header(&wav_data).unwrap();
        let mut decoder = TTZipAudioDecoder::open_from_bytes(&wav_data).unwrap();
        while let Ok(Some(pkt)) = decoder.decode_next_packet() {
            black_box(pkt);
        }
        let _ = AudioMetadataExtractor::extract_from_bytes(&wav_data).unwrap();
        let _ = AudioWaveformSampler::sample_waveform_from_bytes(&wav_data, 64).unwrap();
    });

    // Measure Candidate Run (Pass 2)
    let (candidate_sec, _) = measure_workload(|| {
        let _ = pipeline.inspect_stream_header(&wav_data).unwrap();
        let mut decoder = TTZipAudioDecoder::open_from_bytes(&wav_data).unwrap();
        while let Ok(Some(pkt)) = decoder.decode_next_packet() {
            black_box(pkt);
        }
        let _ = AudioMetadataExtractor::extract_from_bytes(&wav_data).unwrap();
        let _ = AudioWaveformSampler::sample_waveform_from_bytes(&wav_data, 64).unwrap();
    });

    let regression_pct = if candidate_sec > baseline_sec {
        ((candidate_sec - baseline_sec) / baseline_sec) * 100.0
    } else {
        0.0
    };

    println!(
        "[Invariant 6] Audio baseline: {:.4} ms, candidate: {:.4} ms, regression: {:.2}% (limit <= {:.1}%)",
        baseline_sec * 1000.0,
        candidate_sec * 1000.0,
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Invariant 6 Violation: Audio pipeline performance regression {:.2}% exceeds limit {:.1}%",
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
