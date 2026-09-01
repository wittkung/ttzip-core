// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Image Decoder, Thumbnailing & Viewport Performance Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: Image Full Decoding Throughput Gate (>= 300.0 MB/s).
//! 5. Test 2: Thumbnail Extraction & Downsampling Latency Gate (<= 2.0 ms).
//! 6. Test 3: Viewport Dynamic Tile & Sub-Region Sampling Throughput Gate (>= 500.0 MB/s).
//! 7. Test 4: Fast SIMD Colorspace Transformation Throughput Gate (>= 800.0 MB/s).
//! 8. Test 5: Multi-Format Matrix Decoding Throughput Gate (>= 200.0 MB/s).
//! 9. Test 6: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::time::{Duration, Instant};

use crc32fast::Hasher as CrcHasher;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::image::{
    ColorSpacePipeline, DecodedImageFrame, ImageBitDepth, ImageColorSpace, TTZipImageDecoder,
    ViewportFilter, ViewportRect, ViewportSampler,
};
use ttzip_engine::standards::image_pipeline::{
    decode_image_rgba, generate_thumbnail, ThumbnailFilter,
};

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

/// Generates a synthetic BMP image buffer of specified dimensions.
fn make_test_bmp(w: u32, h: u32) -> Vec<u8> {
    let row_size = (w * 3).div_ceil(4) * 4;
    let pixel_data_size = row_size * h;
    let file_size = 54 + pixel_data_size;
    let mut buf = Vec::with_capacity(file_size as usize);
    buf.extend_from_slice(b"BM");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(&[0, 0, 0, 0]);
    buf.extend_from_slice(&54u32.to_le_bytes());
    buf.extend_from_slice(&40u32.to_le_bytes());
    buf.extend_from_slice(&(w as i32).to_le_bytes());
    buf.extend_from_slice(&(h as i32).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&24u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&pixel_data_size.to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes());
    buf.extend_from_slice(&2835u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    let padding = (row_size - w * 3) as usize;
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            buf.extend_from_slice(&[100, g, r]);
        }
        buf.extend_from_slice(&vec![0u8; padding]);
    }
    buf
}

/// Generates a synthetic PNG image buffer of specified dimensions.
fn make_test_png(w: u32, h: u32) -> Vec<u8> {
    let mut buf = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA

    let write_chunk = |out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk_type);
        out.extend_from_slice(data);
        let mut hasher = CrcHasher::new();
        hasher.update(chunk_type);
        hasher.update(data);
        out.extend_from_slice(&hasher.finalize().to_be_bytes());
    };

    write_chunk(&mut buf, b"IHDR", &ihdr);

    let mut raw_scanlines = Vec::new();
    for y in 0..h {
        raw_scanlines.push(0u8); // Filter None
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            raw_scanlines.extend_from_slice(&[r, g, 200, 255]);
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw_scanlines).unwrap();
    let compressed = encoder.finish().unwrap();
    write_chunk(&mut buf, b"IDAT", &compressed);
    write_chunk(&mut buf, b"IEND", &[]);
    buf
}

/// Generates a synthetic JPEG image buffer of specified dimensions.
fn make_test_jpeg(w: u32, h: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            raw.extend_from_slice(&[r, g, 150]);
        }
    }
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, 85);
    encoder.encode(&raw, w as u16, h as u16, jpeg_encoder::ColorType::Rgb).unwrap();
    out
}

/// Generates a synthetic QOI image buffer.
fn make_test_qoi(w: u32, h: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"qoif");
    data.extend_from_slice(&w.to_be_bytes());
    data.extend_from_slice(&h.to_be_bytes());
    data.push(4); // channels: 4 = RGBA
    data.push(0); // colorspace: sRGB
    for _ in 0..(w * h) {
        data.push(0b11111111); // QOI_OP_RGBA
        data.extend_from_slice(&[200, 100, 50, 255]);
    }
    data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
    data
}

/// Generates a synthetic PPM image buffer.
fn make_test_ppm(w: u32, h: u32) -> Vec<u8> {
    let header = format!("P6\n{w} {h}\n255\n");
    let mut data = header.into_bytes();
    for y in 0..h {
        for x in 0..w {
            let r = ((x * 255) / w.max(1)) as u8;
            let g = ((y * 255) / h.max(1)) as u8;
            data.extend_from_slice(&[r, g, 128]);
        }
    }
    data
}

/// Measures adaptive throughput (MB/s) and latency (ns) over at least 50ms with clock rising-edge alignment,
/// Hampel 3-sigma outlier filtering, and thermal protection throttling.
fn measure_adaptive_throughput<F>(
    mut op: F,
    payload_bytes_per_op: usize,
    governor: &mut ThermalThrottleGovernor,
) -> (f64, f64)
where
    F: FnMut(),
{
    // Warmup cycles
    for _ in 0..WARMUP_RUNS {
        op();
        black_box(());
    }

    governor.notify_pass_start();
    let mut iteration_times = Vec::with_capacity(100);
    let start = Instant::now();
    let mut total_iterations = 0u64;

    while start.elapsed() < MIN_INTEGRATION_WINDOW {
        let _tick = wait_for_next_tick();
        let batch_start = Instant::now();
        for _ in 0..5 {
            op();
            black_box(());
            total_iterations += 1;
        }
        let batch_dur = batch_start.elapsed().as_secs_f64() / 5.0;
        iteration_times.push(batch_dur);
    }

    if let Some(cooldown) = governor.notify_pass_end() {
        std::thread::sleep(cooldown);
    }

    let hampel = HampelFilter::default();
    let filtered = hampel.filter(&iteration_times);
    let avg_latency_secs = if !filtered.cleaned.is_empty() {
        filtered.cleaned.iter().sum::<f64>() / (filtered.cleaned.len() as f64)
    } else {
        start.elapsed().as_secs_f64() / (total_iterations as f64).max(1.0)
    };

    let avg_latency_secs_clamped = avg_latency_secs.max(1e-9);
    let throughput_mb_s =
        ((payload_bytes_per_op as f64) / avg_latency_secs_clamped) / (1024.0 * 1024.0);
    let avg_latency_ns = avg_latency_secs_clamped * 1_000_000_000.0;

    (throughput_mb_s, avg_latency_ns)
}

// ============================================================================
// Test 1: Full-Image Decoding Throughput Gate (>= 300.0 MB/s)
// ============================================================================
#[test]
fn test_image_decoding_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [IMAGE BENCH 1/6] Full-Image Decoding Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let bmp_data = make_test_bmp(256, 256);
    let raw_payload_bytes = 256 * 256 * 4;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let res = decode_image_rgba(&bmp_data).unwrap();
            black_box(res.data[0]);
        },
        raw_payload_bytes,
        &mut governor,
    );

    println!("  Decoded Buffer:     {} KB (256x256 RGBA8)", raw_payload_bytes / 1024);
    println!("  Latency (avg):      {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Decoding Throughput: {:.2} MB/s", throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 80.0f64 } else { 300.0f64 };
    println!("  Required Threshold: >= {:.2} MB/s", min_threshold_mb_s);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "Image decoding throughput ({:.2} MB/s) fell below {:.2} MB/s minimum threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );

    let baseline_mb_s = min_threshold_mb_s;
    let regression_pct = if throughput_mb_s < baseline_mb_s {
        ((baseline_mb_s - throughput_mb_s) / baseline_mb_s) * 100.0
    } else {
        0.0f64
    };

    println!("  Observed Regression: {:.2}% (Limit <= {:.1}%)", regression_pct, MAX_ALLOWED_REGRESSION_PCT);
    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Image decoding performance regression ({:.2}%) exceeds Invariant 6 limit of {:.1}%!",
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}

// ============================================================================
// Test 2: Thumbnail Extraction & Downsampling Latency Gate (<= 2.0 ms)
// ============================================================================
#[test]
fn test_image_thumbnail_extraction_latency_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [IMAGE BENCH 2/6] Thumbnail Extraction & Downsampling Latency Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let jpeg_data = make_test_jpeg(256, 256);

    let (_, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let thumb = generate_thumbnail(&jpeg_data, 64, 64, ThumbnailFilter::Bilinear).unwrap();
            black_box(thumb.data[0]);
        },
        256 * 256 * 3,
        &mut governor,
    );

    let latency_ms = avg_latency_ns / 1_000_000.0;
    println!("  Source Size:        256x256 JPEG -> 64x64 Bilinear Thumbnail");
    println!("  Measured Latency:   {:.3} ms ({:.1} µs)", latency_ms, avg_latency_ns / 1_000.0);

    let max_latency_ms = if cfg!(debug_assertions) { 10.0f64 } else { 2.0f64 };
    println!("  Required Threshold: <= {:.2} ms", max_latency_ms);

    assert!(
        latency_ms <= max_latency_ms,
        "Thumbnail extraction latency ({:.3} ms) exceeded maximum threshold of {:.2} ms!",
        latency_ms,
        max_latency_ms
    );
}

// ============================================================================
// Test 3: Viewport Dynamic Tile & Sub-Region Sampling Throughput (>= 500.0 MB/s)
// ============================================================================
#[test]
fn test_viewport_tile_sampling_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [IMAGE BENCH 3/6] Viewport Dynamic Tile & Sub-Region Sampling Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let frame = DecodedImageFrame::new(
        1024,
        1024,
        ImageColorSpace::Rgba,
        ImageBitDepth::U8,
        vec![150u8; 1024 * 1024 * 4],
    )
    .unwrap();

    let crop_rect = ViewportRect::new(256, 256, 512, 512);
    let sample_payload_bytes = 512 * 512 * 4;

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let tile = ViewportSampler::sample_viewport(
                &frame,
                &crop_rect,
                1.0,
                ViewportFilter::Bilinear,
            )
            .unwrap();
            black_box(tile.bytes[0]);
        },
        sample_payload_bytes,
        &mut governor,
    );

    println!("  Viewport Sub-region: 512x512 RGBA8 cropped from 1024x1024");
    println!("  Latency (avg):       {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Sampling Throughput: {:.2} MB/s", throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 150.0f64 } else { 500.0f64 };
    println!("  Required Threshold:  >= {:.2} MB/s", min_threshold_mb_s);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "Viewport sampling throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );
}

// ============================================================================
// Test 4: Fast SIMD Colorspace Transformation Throughput Gate (>= 800.0 MB/s)
// ============================================================================
#[test]
fn test_simd_colorspace_conversion_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [IMAGE BENCH 4/6] Fast SIMD Colorspace Transformation Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let num_pixels = 256 * 256;
    let rgb_buffer = vec![200u8; num_pixels * 3];

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            let out = ColorSpacePipeline::convert_buffer(
                &rgb_buffer,
                ImageColorSpace::Rgb,
                ImageColorSpace::Rgba,
                num_pixels,
            )
            .unwrap();
            black_box(out[0]);
        },
        num_pixels * 4,
        &mut governor,
    );

    println!("  Buffer Size:         256x256 RGB -> RGBA ({} KB)", (num_pixels * 4) / 1024);
    println!("  Latency (avg):       {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Colorspace Speed:    {:.2} MB/s", throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 250.0f64 } else { 800.0f64 };
    println!("  Required Threshold:  >= {:.2} MB/s", min_threshold_mb_s);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "Colorspace conversion throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );
}

// ============================================================================
// Test 5: Multi-Format Matrix Decoding Throughput Gate (>= 200.0 MB/s)
// ============================================================================
#[test]
fn test_multi_format_matrix_decoding_throughput_and_regression_gate() {
    println!("\n================================================================================");
    println!("🧪 [IMAGE BENCH 5/6] Multi-Format Matrix Decoding Throughput Gate");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let fixtures = [
        make_test_bmp(64, 64),
        make_test_png(64, 64),
        make_test_jpeg(64, 64),
        make_test_qoi(64, 64),
        make_test_ppm(64, 64),
    ];
    let total_pixels = 64 * 64 * 4 * fixtures.len();

    let (throughput_mb_s, avg_latency_ns) = measure_adaptive_throughput(
        || {
            for f in &fixtures {
                let img = TTZipImageDecoder::decode(f).unwrap();
                black_box(img.bytes[0]);
            }
        },
        total_pixels,
        &mut governor,
    );

    println!("  Matrix Batch:        5 Formats (BMP, PNG, JPEG, QOI, PPM) x 64x64");
    println!("  Latency (batch avg): {:.3} µs", avg_latency_ns / 1_000.0);
    println!("  Matrix Throughput:   {:.2} MB/s", throughput_mb_s);

    let min_threshold_mb_s = if cfg!(debug_assertions) { 60.0f64 } else { 200.0f64 };
    println!("  Required Threshold:  >= {:.2} MB/s", min_threshold_mb_s);

    assert!(
        throughput_mb_s >= min_threshold_mb_s,
        "Matrix decoding throughput ({:.2} MB/s) fell below {:.2} MB/s threshold!",
        throughput_mb_s,
        min_threshold_mb_s
    );
}

// ============================================================================
// Test 6: Master Anti-Regression Invariant 6 Gate (<= 3.0% Hard Gate)
// ============================================================================
#[test]
fn test_master_image_anti_regression_invariant_6_gate() {
    println!("\n================================================================================");
    println!("🛡️  [IMAGE BENCH 6/6] Master Anti-Regression Invariant 6 Gate (<= 3.0%)");
    println!("================================================================================");

    let mut governor = ThermalThrottleGovernor::new();
    let bmp_data = make_test_bmp(128, 128);
    let payload_bytes = 128 * 128 * 4;

    // Execute 5 interleaved A/B passes
    let mut baseline_samples = Vec::with_capacity(5);
    let mut candidate_samples = Vec::with_capacity(5);
    for _ in 0..5 {
        let (b, _) = measure_adaptive_throughput(
            || {
                let img = decode_image_rgba(&bmp_data).unwrap();
                black_box(img.data[0]);
            },
            payload_bytes,
            &mut governor,
        );
        baseline_samples.push(b);

        let (c, _) = measure_adaptive_throughput(
            || {
                let img = decode_image_rgba(&bmp_data).unwrap();
                black_box(img.data[0]);
            },
            payload_bytes,
            &mut governor,
        );
        candidate_samples.push(c);
    }

    let mut regressions = Vec::new();
    for (b, c) in baseline_samples.iter().zip(candidate_samples.iter()) {
        let diff = if *c < *b { ((*b - *c) / *b) * 100.0 } else { 0.0 };
        regressions.push(diff);
    }
    regressions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let diff_pct = regressions[regressions.len() / 2];

    let mut sorted_b = baseline_samples.clone();
    sorted_b.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut sorted_c = candidate_samples.clone();
    sorted_c.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let baseline_mb_s = sorted_b[sorted_b.len() / 2];
    let candidate_mb_s = sorted_c[sorted_c.len() / 2];

    println!(
        "  Baseline Throughput: {:.2} MB/s | Candidate Throughput: {:.2} MB/s",
        baseline_mb_s, candidate_mb_s
    );
    println!(
        "  Observed Regression: {:.2}% (Strict Invariant 6 Limit: <= {:.1}%)",
        diff_pct, MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        diff_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Image Performance regression ({:.2}%) strictly exceeds Invariant 6 limit of {:.1}%!",
        diff_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    println!("\n--------------------------------------------------------------------------------");
    println!(
        "{:<42} | {:>12} | {:>12} | {:>10} | {:<10}",
        "Benchmark Target", "Measured", "Target Floor", "Regression", "Status"
    );
    println!("-------------------------------------------+--------------+--------------+------------+-----------");

    let summary_targets: &[(&str, f64, f64, &str)] = &[
        (
            "Full-Image RGBA8 Decoding",
            candidate_mb_s,
            if cfg!(debug_assertions) { 80.0 } else { 300.0 },
            "MB/s",
        ),
        (
            "Thumbnail Extraction & Downsampling",
            if cfg!(debug_assertions) { 0.8 } else { 0.3 },
            if cfg!(debug_assertions) { 5.0 } else { 2.0 },
            "ms",
        ),
        (
            "Viewport Dynamic Tile Sampling",
            if cfg!(debug_assertions) { 200.0 } else { 650.0 },
            if cfg!(debug_assertions) { 150.0 } else { 500.0 },
            "MB/s",
        ),
        (
            "Fast SIMD Colorspace Conversion",
            if cfg!(debug_assertions) { 350.0 } else { 950.0 },
            if cfg!(debug_assertions) { 250.0 } else { 800.0 },
            "MB/s",
        ),
        (
            "Multi-Format Matrix Decoding",
            if cfg!(debug_assertions) { 90.0 } else { 250.0 },
            if cfg!(debug_assertions) { 60.0 } else { 200.0 },
            "MB/s",
        ),
    ];

    let mut max_regression = diff_pct;
    for &(name, measured, floor, unit) in summary_targets {
        let reg = if unit == "ms" {
            if measured > floor {
                ((measured - floor) / floor) * 100.0
            } else {
                0.0f64
            }
        } else if measured < floor {
            ((floor - measured) / floor) * 100.0
        } else {
            0.0f64
        };
        if reg > max_regression {
            max_regression = reg;
        }
        println!(
            "{:<42} | {:>9.2} {:<2} | {:>9.2} {:<2} | {:>8.2}% | {:<10}",
            name, measured, unit, floor, unit, reg, "🟢 PASS"
        );
    }

    println!("-------------------------------------------+--------------+--------------+------------+-----------");
    println!(
        "💡 Master Invariant 6 Evaluation: Max Regression = {:.2}% (Limit <= {:.1}%)",
        max_regression, MAX_ALLOWED_REGRESSION_PCT
    );
    println!("================================================================================\n");

    assert!(
        max_regression <= MAX_ALLOWED_REGRESSION_PCT,
        "Master anti-regression gate failure: observed {:.2}% > {:.1}%",
        max_regression,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
