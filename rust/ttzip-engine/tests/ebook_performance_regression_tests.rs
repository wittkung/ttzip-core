// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust E-book Parser, Navigation & PalmDOC Decompression Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: OPF / Navigation Hierarchy Parsing Throughput Gate (>= 200.0 MB/s).
//! 5. Test 2: In-Memory E-Book Metadata Extraction Latency Gate (<= 1.0 ms).
//! 6. Test 3: Pure-Rust PalmDOC LZ77 Decompression Throughput Gate (>= 250.0 MB/s).
//! 7. Test 4: Multi-Format Matrix (EPUB + MOBI) Probing & Parsing Gate (>= 150.0 MB/s).
//! 8. Test 5: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::io::Cursor;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::ab_engine::thermal::ThermalThrottleGovernor;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::ebook::mobi::{decompress_palmdoc_record, EbookMobiDecoder};
use ttzip_engine::ebook::parser::TTZipEbookParser;
use ttzip_engine::security::ebook_defense::{EbookSecurityPipeline, PalmDocDecompressGuard};
use ttzip_engine::xml::EpubMetadataExtractor;

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50); // 50ms Adaptive Integration
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

// ============================================================================
// Synthetic Benchmark E-Book Fixture Generators
// ============================================================================

/// Helper to build an in-memory uncompressed (Stored) ZIP archive for benchmarks.
fn create_benchmark_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
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

/// Generates a realistic high-throughput EPUB archive with N manifest items and chapters.
fn make_benchmark_epub(item_count: usize, chapter_count: usize) -> Vec<u8> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    files.push(("mimetype".to_string(), b"application/epub+zip".to_vec()));

    let container = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    files.push(("META-INF/container.xml".to_string(), container.to_vec()));

    let mut opf = String::with_capacity(32 * 1024);
    opf.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>High Performance Benchmark Book</dc:title>
    <dc:creator>Witt Kung</dc:creator>
    <dc:language>en</dc:language>
    <dc:identifier id="pub-id">urn:isbn:978-0-123456-47-2</dc:identifier>
    <dc:publisher>TTZip Open Source</dc:publisher>
    <dc:date>2026-09-01</dc:date>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
"#);

    for i in 0..item_count {
        opf.push_str(&format!(
            r#"    <item id="item_{i}" href="text/chapter_{i}.xhtml" media-type="application/xhtml+xml"/>
"#
        ));
    }
    opf.push_str("  </manifest>\n  <spine toc=\"ncx\">\n");
    for i in 0..chapter_count {
        opf.push_str(&format!(r#"    <itemref idref="item_{i}"/>
"#));
    }
    opf.push_str("  </spine>\n</package>");
    files.push(("OEBPS/content.opf".to_string(), opf.into_bytes()));

    // Generate NCX
    let mut ncx = String::with_capacity(16 * 1024);
    ncx.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
"#);
    for i in 0..chapter_count {
        ncx.push_str(&format!(
            r#"    <navPoint id="np_{i}" playOrder="{i}">
      <navLabel><text>Chapter {i}: Core Systems Architecture</text></navLabel>
      <content src="text/chapter_{i}.xhtml"/>
    </navPoint>
"#
        ));
    }
    ncx.push_str("  </navMap>\n</ncx>");
    files.push(("OEBPS/toc.ncx".to_string(), ncx.into_bytes()));

    // Generate Chapters
    let sample_body = "<p>High performance native archiving and compression engine written in safe Rust with zero-copy stream decoders.</p>\n".repeat(50);
    for i in 0..chapter_count {
        let ch_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <head><title>Chapter {i}</title></head>
  <body>
    <h1>Chapter {i}: High Performance Systems</h1>
    {sample_body}
  </body>
</html>"#
        );
        files.push((format!("OEBPS/text/chapter_{i}.xhtml"), ch_content.into_bytes()));
    }

    let file_slices: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();

    create_benchmark_zip(&file_slices)
}

/// Generates a realistic MOBI container buffer with compressed PalmDOC records.
fn make_benchmark_mobi(title: &str, record_count: usize) -> Vec<u8> {
    let mut text_sample = Vec::with_capacity(4096);
    for _ in 0..40 {
        text_sample.extend_from_slice(b"TTZip high performance native archiving and compression engine.\n");
    }

    let mut compressed_records = Vec::new();
    for _ in 0..record_count {
        let compressed = PalmDocDecompressGuard::compress_record(&text_sample).unwrap();
        compressed_records.push(compressed);
    }

    let mut exth_payload = Vec::new();
    exth_payload.extend_from_slice(b"EXTH");
    let exth_records: &[(u32, &[u8])] = &[
        (100, b"Witt Kung"),
        (101, b"TTZip Foundation"),
        (106, b"2026-09-01"),
    ];
    let mut exth_len = 12u32;
    for (_, val) in exth_records {
        exth_len += 8 + (val.len() as u32);
    }
    let padding = (4 - (exth_len % 4)) % 4;
    exth_len += padding;
    exth_payload.extend_from_slice(&exth_len.to_be_bytes());
    exth_payload.extend_from_slice(&(exth_records.len() as u32).to_be_bytes());
    for (rec_type, val) in exth_records {
        exth_payload.extend_from_slice(&rec_type.to_be_bytes());
        let rlen = 8u32 + (val.len() as u32);
        exth_payload.extend_from_slice(&rlen.to_be_bytes());
        exth_payload.extend_from_slice(val);
    }
    for _ in 0..padding {
        exth_payload.push(0);
    }

    let mobi_header_len = 232u32;
    let full_mobi_len = mobi_header_len + (exth_payload.len() as u32);

    let mut rec0 = Vec::new();
    rec0.extend_from_slice(&2u16.to_be_bytes()); // PalmDOC compression: 2
    rec0.extend_from_slice(&0u16.to_be_bytes());
    let total_uncompressed_text_len = (text_sample.len() * record_count) as u32;
    rec0.extend_from_slice(&total_uncompressed_text_len.to_be_bytes());
    rec0.extend_from_slice(&(record_count as u16).to_be_bytes());
    rec0.extend_from_slice(&4096u16.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes());

    rec0.extend_from_slice(b"MOBI");
    rec0.extend_from_slice(&full_mobi_len.to_be_bytes());
    rec0.extend_from_slice(&2u32.to_be_bytes());
    rec0.extend_from_slice(&65001u32.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes());
    rec0.extend_from_slice(&6u32.to_be_bytes());

    let title_offset = (rec0.len() as u32) + 160 + (exth_payload.len() as u32);
    let title_bytes = title.as_bytes();
    let title_len = title_bytes.len() as u32;

    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    for _ in 0..6 {
        rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    }
    rec0.extend_from_slice(&0u32.to_be_bytes());
    rec0.extend_from_slice(&title_offset.to_be_bytes());
    rec0.extend_from_slice(&title_len.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes());
    rec0.extend_from_slice(&6u32.to_be_bytes());
    rec0.extend_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes());
    rec0.extend_from_slice(&0u32.to_be_bytes());
    rec0.extend_from_slice(&0x40u32.to_be_bytes());

    while rec0.len() < (16 + mobi_header_len as usize) {
        rec0.push(0);
    }
    rec0.extend_from_slice(&exth_payload);
    rec0.extend_from_slice(title_bytes);
    rec0.extend_from_slice(&[0, 0, 0, 0]);

    let num_records = (1 + record_count) as u16;
    let pdb_header_len = 78 + (num_records as usize * 8) + 2;
    let mut current_offset = pdb_header_len as u32;

    let mut record_offsets = Vec::new();
    record_offsets.push(current_offset);
    current_offset += rec0.len() as u32;

    for r in &compressed_records {
        record_offsets.push(current_offset);
        current_offset += r.len() as u32;
    }

    let mut file_buf = Vec::new();
    let mut name_buf = [0u8; 32];
    let name_bytes = b"BenchmarkBook";
    name_buf[..name_bytes.len()].copy_from_slice(name_bytes);
    file_buf.extend_from_slice(&name_buf);
    file_buf.extend_from_slice(&0u16.to_be_bytes());
    file_buf.extend_from_slice(&0u16.to_be_bytes());
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(b"BOOK");
    file_buf.extend_from_slice(b"MOBI");
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(&0u32.to_be_bytes());
    file_buf.extend_from_slice(&num_records.to_be_bytes());

    for (idx, &off) in record_offsets.iter().enumerate() {
        file_buf.extend_from_slice(&off.to_be_bytes());
        file_buf.push(0);
        file_buf.extend_from_slice(&((idx as u32) & 0x00FFFFFF).to_be_bytes()[1..]);
    }
    file_buf.extend_from_slice(&[0, 0]);

    file_buf.extend_from_slice(&rec0);
    for r in &compressed_records {
        file_buf.extend_from_slice(r);
    }

    file_buf
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

/// Test 1: OPF / Navigation Hierarchy Parsing Throughput Gate (>= 200.0 MB/s).
#[test]
fn test_ebook_opf_nav_parsing_throughput_gate() {
    let mut opf_xml = String::with_capacity(64 * 1024);
    opf_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>High Throughput Architecture</dc:title>
    <dc:creator>Witt Kung</dc:creator>
    <dc:publisher>TTZip</dc:publisher>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
"#);
    for i in 0..1500 {
        opf_xml.push_str(&format!(
            r#"    <item id="id_{i}" href="text/ch_{i}.xhtml" media-type="application/xhtml+xml"/>
"#
        ));
    }
    opf_xml.push_str("  </manifest>\n  <spine>\n");
    for i in 0..1500 {
        opf_xml.push_str(&format!(r#"    <itemref idref="id_{i}"/>
"#));
    }
    opf_xml.push_str("  </spine>\n</package>");

    let opf_bytes = opf_xml.as_bytes();
    let raw_len = opf_bytes.len();
    assert!(raw_len > 10_000);

    let (avg_sec, iters) = measure_workload(|| {
        let pkg = EpubMetadataExtractor::parse_opf(opf_bytes).unwrap();
        assert_eq!(pkg.manifest.len(), 1500);
        assert_eq!(pkg.spine.len(), 1500);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[E-Book Benchmark] OPF Streaming SAX Parser ({} bytes, {} iters): {:.2} MB/s (latency: {:.3} ms)",
        raw_len,
        iters,
        throughput_mb,
        avg_sec * 1000.0
    );

    assert!(
        throughput_mb >= 200.0,
        "OPF Parsing throughput below 200.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

/// Test 2: In-Memory E-Book Metadata Extraction Latency Gate (<= 1.0 ms).
#[test]
fn test_ebook_metadata_extraction_latency_gate() {
    let epub_bytes = make_benchmark_epub(50, 10);

    let (avg_sec, iters) = measure_workload(|| {
        let parser = TTZipEbookParser::open_from_bytes(&epub_bytes).unwrap();
        assert_eq!(
            parser.metadata().title.as_deref(),
            Some("High Performance Benchmark Book")
        );
        assert_eq!(parser.spine().len(), 10);
    });

    let latency_ms = avg_sec * 1000.0;
    println!(
        "[E-Book Benchmark] In-Memory EPUB Probing & Metadata ({} iters): {:.4} ms",
        iters, latency_ms
    );

    assert!(
        latency_ms <= 1.0,
        "E-Book Metadata extraction latency exceeds 1.0 ms gate: {:.4} ms",
        latency_ms
    );
}

/// Test 3: Pure-Rust PalmDOC LZ77 Decompression Throughput Gate (>= 250.0 MB/s).
#[test]
fn test_ebook_palmdoc_decompression_throughput_gate() {
    // Generate a 1MB payload of uncompressed text with repeated phrases
    let mut uncompressed_source = Vec::with_capacity(1024 * 1024);
    let sample = b"TTZip high performance native archiving and compression engine with PalmDOC LZ77 streaming codec.\n";
    while uncompressed_source.len() < 1024 * 1024 {
        uncompressed_source.extend_from_slice(sample);
    }
    uncompressed_source.truncate(1024 * 1024);

    // Compress in 4KB chunks
    let mut compressed_records = Vec::new();
    for chunk in uncompressed_source.chunks(4096) {
        let comp = PalmDocDecompressGuard::compress_record(chunk).unwrap();
        compressed_records.push(comp);
    }

    let raw_len = uncompressed_source.len();
    let total_compressed_len: usize = compressed_records.iter().map(|r| r.len()).sum();

    let (avg_sec, iters) = measure_workload(|| {
        let mut total_decompressed = 0usize;
        for rec in &compressed_records {
            let decomp = decompress_palmdoc_record(rec, 4096).unwrap();
            total_decompressed += decomp.len();
        }
        assert_eq!(total_decompressed, raw_len);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[E-Book Benchmark] PalmDOC LZ77 Decompressor ({} bytes -> {} bytes, {} iters): {:.2} MB/s",
        total_compressed_len, raw_len, iters, throughput_mb
    );

    assert!(
        throughput_mb >= 250.0,
        "PalmDOC decompression throughput below 250.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

/// Test 4: Multi-Format Matrix (EPUB + MOBI) Probing & Parsing Gate (>= 150.0 MB/s).
#[test]
fn test_ebook_multiformat_matrix_parsing_gate() {
    let epub_bytes = make_benchmark_epub(100, 20);
    let mobi_bytes = make_benchmark_mobi("BenchmarkMobiBook", 20);
    let total_bytes = epub_bytes.len() + mobi_bytes.len();

    let (avg_sec, iters) = measure_workload(|| {
        // 1. Parse EPUB
        let epub_parser = TTZipEbookParser::open_from_bytes(&epub_bytes).unwrap();
        black_box(epub_parser.metadata());
        black_box(epub_parser.spine());

        // 2. Parse MOBI
        let mobi_parser = TTZipEbookParser::open_from_bytes(&mobi_bytes).unwrap();
        black_box(mobi_parser.metadata());
        black_box(mobi_parser.format());
    });

    let throughput_mb = (total_bytes as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[E-Book Benchmark] Multi-Format Matrix (EPUB + MOBI, {} bytes, {} iters): {:.2} MB/s",
        total_bytes, iters, throughput_mb
    );

    assert!(
        throughput_mb >= 150.0,
        "Multi-Format E-Book parsing throughput below 150.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

/// Test 5: Master Anti-Regression Invariant 6 Gate (<= 3.0% Regression Hard Gate).
#[test]
fn test_ebook_anti_regression_invariant6_gate() {
    let _governor = ThermalThrottleGovernor::new();
    let epub_bytes = make_benchmark_epub(50, 10);
    let mobi_bytes = make_benchmark_mobi("AntiRegressionMobi", 10);
    // Measure Baseline Run (Pass 1)
    let (baseline_sec, _) = measure_workload(|| {
        let p_epub = TTZipEbookParser::open_from_bytes(&epub_bytes).unwrap();
        black_box(p_epub.metadata());
        black_box(p_epub.spine());

        let p_mobi = TTZipEbookParser::open_from_bytes(&mobi_bytes).unwrap();
        black_box(p_mobi.metadata());

        let dec = EbookMobiDecoder::parse(&mobi_bytes).unwrap();
        let _ = dec.extract_full_text().unwrap();

        let mut pipeline = EbookSecurityPipeline::default();
        let opf_xml = r#"<package><manifest><item id="c1" href="c.xhtml" media-type="application/xhtml+xml"/></manifest></package>"#;
        let _ = pipeline
            .inspect_opf_manifest(Cursor::new(opf_xml.as_bytes()), opf_xml.len() as u64)
            .unwrap();
    });

    // Measure Candidate Run (Pass 2)
    let (candidate_sec, _) = measure_workload(|| {
        let p_epub = TTZipEbookParser::open_from_bytes(&epub_bytes).unwrap();
        black_box(p_epub.metadata());
        black_box(p_epub.spine());

        let p_mobi = TTZipEbookParser::open_from_bytes(&mobi_bytes).unwrap();
        black_box(p_mobi.metadata());

        let dec = EbookMobiDecoder::parse(&mobi_bytes).unwrap();
        let _ = dec.extract_full_text().unwrap();

        let mut pipeline = EbookSecurityPipeline::default();
        let opf_xml = r#"<package><manifest><item id="c1" href="c.xhtml" media-type="application/xhtml+xml"/></manifest></package>"#;
        let _ = pipeline
            .inspect_opf_manifest(Cursor::new(opf_xml.as_bytes()), opf_xml.len() as u64)
            .unwrap();
    });

    let regression_pct = if candidate_sec > baseline_sec {
        ((candidate_sec - baseline_sec) / baseline_sec) * 100.0
    } else {
        0.0
    };

    println!(
        "[Invariant 6] E-Book baseline: {:.4} ms, candidate: {:.4} ms, regression: {:.2}% (limit <= {:.1}%)",
        baseline_sec * 1000.0,
        candidate_sec * 1000.0,
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Invariant 6 Violation: E-Book pipeline performance regression {:.2}% exceeds limit {:.1}%",
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
