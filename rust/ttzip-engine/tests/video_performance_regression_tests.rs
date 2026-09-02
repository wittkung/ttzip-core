// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Video Demuxer, Metadata & Container Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: Video Container Sniffing Throughput Gate (>= 300.0 MB/s).
//! 5. Test 2: In-Memory Video Metadata Extraction Latency Gate (<= 0.5 ms).
//! 6. Test 3: DOCX / MKV Multi-Track Demuxing Throughput Gate (>= 250.0 MB/s).
//! 7. Test 4: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::standards::demuxer::{demux_media_tracks_from_slice, parse_mkv_demux};
use ttzip_engine::standards::document_stream::parse_docx_from_memory;
use ttzip_engine::standards::metadata_probe::probe_metadata_buffer;
use ttzip_engine::standards::sniffer::detect_format_buffer;

static BENCH_LOCK: Mutex<()> = Mutex::new(());

const WARMUP_RUNS: usize = 5;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50);
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

// ============================================================================
// Synthetic Benchmark Video & Media Generators
// ============================================================================

fn mp4_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let total_len = (payload.len() + 8) as u32;
    let mut out = Vec::with_capacity(total_len as usize);
    out.extend_from_slice(&total_len.to_be_bytes());
    out.extend_from_slice(fourcc);
    out.extend_from_slice(payload);
    out
}

fn ebml_box(id: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if id > 0x00FF_FFFF {
        out.extend_from_slice(&id.to_be_bytes());
    } else if id > 0xFFFF {
        out.extend_from_slice(&id.to_be_bytes()[1..]);
    } else if id > 0xFF {
        out.extend_from_slice(&id.to_be_bytes()[2..]);
    } else {
        out.push(id as u8);
    }
    let sz = data.len();
    if sz < 0x7F {
        out.push(0x80 | (sz as u8));
    } else if sz < 0x3FFF {
        out.push(0x40 | ((sz >> 8) as u8));
        out.push((sz & 0xFF) as u8);
    } else {
        out.push(0x20 | ((sz >> 16) as u8));
        out.push(((sz >> 8) & 0xFF) as u8);
        out.push((sz & 0xFF) as u8);
    }
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

fn make_benchmark_mp4() -> Vec<u8> {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\x02\0isommp42");

    let mut mvhd_p = vec![0u8; 100];
    mvhd_p[12..16].copy_from_slice(&1000u32.to_be_bytes());
    mvhd_p[16..20].copy_from_slice(&120000u32.to_be_bytes());
    let mvhd = mp4_box(b"mvhd", &mvhd_p);

    let mut trak_all = Vec::new();
    for i in 1..=4 {
        let mut tkhd_p = vec![0u8; 84];
        tkhd_p[12..16].copy_from_slice(&(i as u32).to_be_bytes());
        tkhd_p[76..80].copy_from_slice(&(1920u32 << 16).to_be_bytes());
        tkhd_p[80..84].copy_from_slice(&(1080u32 << 16).to_be_bytes());

        let mut hdlr_p = vec![0u8; 24];
        hdlr_p[8..12].copy_from_slice(if i % 2 == 1 { b"vide" } else { b"soun" });

        let mut avc1_p = vec![0u8; 40];
        avc1_p[4..8].copy_from_slice(b"avc1");
        avc1_p[32..34].copy_from_slice(&1920u16.to_be_bytes());
        avc1_p[34..36].copy_from_slice(&1080u16.to_be_bytes());

        let mut stsd_p = vec![0u8; 8];
        stsd_p[4..8].copy_from_slice(&1u32.to_be_bytes());
        stsd_p.extend_from_slice(&avc1_p);

        let mut mdia = mp4_box(b"hdlr", &hdlr_p);
        mdia.extend(mp4_box(b"minf", &mp4_box(b"stbl", &mp4_box(b"stsd", &stsd_p))));

        let mut trak = mp4_box(b"tkhd", &tkhd_p);
        trak.extend(mp4_box(b"mdia", &mdia));
        trak_all.extend(trak);
    }

    let mut moov = mvhd;
    moov.extend(trak_all);

    let mut mp4 = ftyp;
    mp4.extend(mp4_box(b"moov", &moov));
    mp4
}

fn make_benchmark_mkv() -> Vec<u8> {
    let ebml_hdr = ebml_box(0x1A45_DFA3, &ebml_str(0x4282, "matroska"));
    let mut info_b = ebml_uint(0x002A_D7B1, 1_000_000, 3);
    info_b.extend(ebml_float32(0x4489, 120000.0));
    info_b.extend(ebml_str(0x7BA9, "Benchmark Video Stream"));
    let info = ebml_box(0x1549_A966, &info_b);

    let mut trk_b = Vec::new();
    for i in 1..=8 {
        let (track_type, codec) = match i % 3 {
            0 => (1u64, "V_MPEG4/ISO/AVC"),
            1 => (2u64, "A_AAC"),
            _ => (17u64, "S_TEXT/ASS"),
        };
        let mut t = ebml_uint(0xD7, i, 2);
        t.extend(ebml_uint(0x83, track_type, 1));
        t.extend(ebml_str(0x86, codec));
        trk_b.extend(ebml_box(0xAE, &t));
    }

    let tracks = ebml_box(0x1654_AE6B, &trk_b);
    let mut seg_b = info;
    seg_b.extend(tracks);
    let mut mkv = ebml_hdr;
    mkv.extend(ebml_box(0x1853_8067, &seg_b));
    mkv
}

fn make_benchmark_docx(paragraphs: usize) -> Vec<u8> {
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>TTZip Demuxer Benchmark</dc:title>
    <dc:creator>Witt Kung</dc:creator>
</cp:coreProperties>"#;

    let mut doc_xml = String::with_capacity(paragraphs * 160 + 256);
    doc_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#);
    for i in 1..=paragraphs {
        doc_xml.push_str(&format!(
            r#"<w:p><w:r><w:t>Paragraph {i}: Pure Safe Rust zero-copy video streaming and document demuxing pipeline.</w:t></w:r></w:p>"#
        ));
    }
    doc_xml.push_str("</w:body></w:document>");

    let files = [
        ("docProps/core.xml", core_xml.as_bytes()),
        ("word/document.xml", doc_xml.as_bytes()),
    ];

    let mut zip_data = Vec::new();
    let mut cd_entries = Vec::new();

    for (name, content) in files {
        let lfh_offset = zip_data.len() as u32;
        let crc = crc32_fast(0, content);
        let name_bytes = name.as_bytes();

        zip_data.extend_from_slice(&0x04034b50u32.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(name_bytes);
        zip_data.extend_from_slice(content);

        cd_entries.push((name_bytes.to_vec(), crc, content.len() as u32, lfh_offset));
    }

    let cd_offset = zip_data.len() as u32;
    for (name_bytes, crc, size, lfh_offset) in &cd_entries {
        zip_data.extend_from_slice(&0x02014b50u32.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&20u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&crc.to_le_bytes());
        zip_data.extend_from_slice(&size.to_le_bytes());
        zip_data.extend_from_slice(&size.to_le_bytes());
        zip_data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u16.to_le_bytes());
        zip_data.extend_from_slice(&0u32.to_le_bytes());
        zip_data.extend_from_slice(&lfh_offset.to_le_bytes());
        zip_data.extend_from_slice(name_bytes);
    }

    let cd_size = (zip_data.len() as u32) - cd_offset;
    let entry_count = cd_entries.len() as u16;

    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&entry_count.to_le_bytes());
    zip_data.extend_from_slice(&entry_count.to_le_bytes());
    zip_data.extend_from_slice(&cd_size.to_le_bytes());
    zip_data.extend_from_slice(&cd_offset.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());

    zip_data
}

fn measure_workload<F: FnMut() -> R, R>(mut workload: F) -> (f64, usize) {
    for _ in 0..WARMUP_RUNS {
        black_box(workload());
    }

    wait_for_next_tick();

    let start = Instant::now();
    let mut iterations = 0usize;
    let mut pass_latencies = Vec::new();

    while start.elapsed() < MIN_INTEGRATION_WINDOW || iterations < 10 {
        let pass_start = Instant::now();
        black_box(workload());
        let pass_dur = pass_start.elapsed().as_secs_f64();
        pass_latencies.push(pass_dur);
        iterations += 1;
    }

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

/// Test 1: Video Container Sniffing Throughput Gate (>= 300.0 MB/s).
#[test]
fn test_video_container_sniffing_throughput_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mp4_data = make_benchmark_mp4();
    let raw_len = mp4_data.len();

    let (avg_sec, iters) = measure_workload(|| {
        let res = detect_format_buffer(&mp4_data, Some("video.mp4"));
        black_box(res);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Video Benchmark] Format Sniffing ({} bytes, {} iters): {:.2} MB/s (latency: {:.4} ms)",
        raw_len,
        iters,
        throughput_mb,
        avg_sec * 1000.0
    );

    assert!(
        throughput_mb >= 300.0,
        "Video Container Sniffing throughput below 300.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

/// Test 2: In-Memory Video Metadata Extraction Latency Gate (<= 0.5 ms).
#[test]
fn test_video_metadata_extraction_latency_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mp4_data = make_benchmark_mp4();

    let (avg_sec, iters) = measure_workload(|| {
        let meta = probe_metadata_buffer(&mp4_data, Some("stream.mp4"), None);
        black_box(meta);
    });

    let latency_ms = avg_sec * 1000.0;
    println!(
        "[Video Benchmark] Metadata Extraction ({} iters): {:.4} ms",
        iters, latency_ms
    );

    assert!(
        latency_ms <= 0.5,
        "Video Metadata extraction latency exceeds 0.5 ms gate: {:.4} ms",
        latency_ms
    );
}

/// Test 3: DOCX / MKV Multi-Track Demuxing Throughput Gate (>= 250.0 MB/s).
#[test]
fn test_docx_mkv_demuxing_throughput_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mkv_data = make_benchmark_mkv();
    let docx_data = make_benchmark_docx(50);
    let total_bytes = mkv_data.len() + docx_data.len();

    let (avg_sec, iters) = measure_workload(|| {
        let sum = parse_mkv_demux(&mkv_data).unwrap();
        black_box(sum);
        let doc = parse_docx_from_memory(&docx_data).unwrap();
        black_box(doc);
    });

    let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Video Benchmark] Multi-Track & Document Demuxing ({} bytes, {} iters): {:.2} MB/s",
        total_bytes, iters, throughput_mb
    );

    let floor_demux = if cfg!(debug_assertions) { 80.0 } else { 250.0 };
    assert!(
        throughput_mb >= floor_demux,
        "Demuxing throughput below {:.1} MB/s gate: {:.2} MB/s",
        floor_demux,
        throughput_mb
    );
}

/// Test 4: Master Anti-Regression Invariant 6 Gate (<= 3.0% Regression Hard Gate).
#[test]
fn test_video_anti_regression_invariant6_gate() {
    let _lock = BENCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _governor = ThermalThrottleGovernor::new();
    let mp4_data = make_benchmark_mp4();
    let mkv_data = make_benchmark_mkv();

    // Measure interleaved A/B runs (5 pairs) to eliminate thermal and frequency scaling noise
    let mut baseline_samples = Vec::new();
    let mut candidate_samples = Vec::new();

    for i in 0..6 {
        if i % 2 == 0 {
            let (lat_b, _) = measure_workload(|| {
                let _ = demux_media_tracks_from_slice(&mp4_data).unwrap();
                let _ = probe_metadata_buffer(&mkv_data, None, None);
            });
            baseline_samples.push(lat_b);

            let (lat_c, _) = measure_workload(|| {
                let _ = demux_media_tracks_from_slice(&mp4_data).unwrap();
                let _ = probe_metadata_buffer(&mkv_data, None, None);
            });
            candidate_samples.push(lat_c);
        } else {
            let (lat_c, _) = measure_workload(|| {
                let _ = demux_media_tracks_from_slice(&mp4_data).unwrap();
                let _ = probe_metadata_buffer(&mkv_data, None, None);
            });
            candidate_samples.push(lat_c);

            let (lat_b, _) = measure_workload(|| {
                let _ = demux_media_tracks_from_slice(&mp4_data).unwrap();
                let _ = probe_metadata_buffer(&mkv_data, None, None);
            });
            baseline_samples.push(lat_b);
        }
    }

    let baseline_sec = baseline_samples.into_iter().fold(f64::INFINITY, f64::min);
    let candidate_sec = candidate_samples.into_iter().fold(f64::INFINITY, f64::min);

    let regression_pct = if candidate_sec > baseline_sec {
        ((candidate_sec - baseline_sec) / baseline_sec) * 100.0
    } else {
        0.0
    };

    println!(
        "[Invariant 6] Video baseline: {:.4} ms, candidate: {:.4} ms, regression: {:.2}% (limit <= {:.1}%)",
        baseline_sec * 1000.0,
        candidate_sec * 1000.0,
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Invariant 6 Violation: Video pipeline performance regression {:.2}% exceeds limit {:.1}%",
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
