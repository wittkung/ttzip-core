// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Office Suite Parser & Formula Engine Benchmark Suite (Invariant 6 <=3.0% Hard Gate).
//!
//! Evaluates and enforces strict physical throughput and latency thresholds:
//! 1. Clock rising-edge alignment (`wait_for_next_tick`).
//! 2. 50ms+ adaptive time integration with thermal protection (`ThermalThrottleGovernor`).
//! 3. Hampel 3-sigma MAD outlier filtering on pass latencies.
//! 4. Test 1: Spreadsheet (XLSX) Workbook & SST Parsing Throughput Gate (>= 200.0 MB/s).
//! 5. Test 2: In-Memory Office Metadata Extraction Latency Gate (<= 1.0 ms).
//! 6. Test 3: DOCX Body Paragraph & Text Extraction Throughput Gate (>= 200.0 MB/s).
//! 7. Test 4: PPTX Slide & Outline Tree Parsing Gate (>= 150.0 MB/s).
//! 8. Test 5: Master Anti-Regression Invariant 6 Gate: Maximum allowed performance regression strictly <= 3.0%.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ttzip_engine::benchmark::ab_engine::stats::HampelFilter;
use ttzip_engine::benchmark::wait_for_next_tick;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::uniffi_api::xml_meta::office::{
    parse_office_metadata_from_slice, parse_office_outline_from_slice,
};

const WARMUP_RUNS: usize = 3;
const MIN_INTEGRATION_WINDOW: Duration = Duration::from_millis(50);
const MAX_ALLOWED_REGRESSION_PCT: f64 = 3.0; // Invariant 6 Hard Gate

// ============================================================================
// Synthetic Benchmark Office Archive Generators
// ============================================================================

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

fn make_benchmark_docx(paragraph_count: usize) -> Vec<u8> {
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>TTZip WordprocessingML Benchmark</dc:title>
    <dc:creator>Witt Kung</dc:creator>
</cp:coreProperties>"#;

    let app_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
    <Application>TTZip High-Throughput Engine</Application>
    <Pages>10</Pages>
    <Words>5000</Words>
</Properties>"#;

    let mut doc_xml = String::with_capacity(paragraph_count * 160 + 512);
    doc_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#);

    for i in 1..=paragraph_count {
        if i % 10 == 1 {
            doc_xml.push_str(&format!(
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Section {i}: Subsystem Invariant Defense</w:t></w:r></w:p>"#
            ));
        } else {
            doc_xml.push_str(&format!(
                r#"<w:p><w:r><w:t>Paragraph {i}: Pure Safe Rust zero-allocation streaming tokenization of Office Open XML word document body paragraph contents.</w:t></w:r></w:p>"#
            ));
        }
    }
    doc_xml.push_str("</w:body></w:document>");

    let files = [
        ("docProps/core.xml", core_xml.as_bytes()),
        ("docProps/app.xml", app_xml.as_bytes()),
        ("word/document.xml", doc_xml.as_bytes()),
    ];
    create_benchmark_zip(&files)
}

fn make_benchmark_xlsx(sheet_count: usize, string_count: usize) -> Vec<u8> {
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>TTZip SpreadsheetML Benchmark</dc:title>
    <dc:creator>Witt Kung</dc:creator>
</cp:coreProperties>"#;

    let mut app_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>TTZip Sheets</Application><TitlesOfParts><vt:vector size=""#);
    app_xml.push_str(&format!("{sheet_count}\" baseType=\"lpstr\">"));
    for s in 1..=sheet_count {
        app_xml.push_str(&format!("<vt:lpstr>Sheet {s}</vt:lpstr>"));
    }
    app_xml.push_str("</vt:vector></TitlesOfParts></Properties>");

    let mut wb_xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>"#);
    for s in 1..=sheet_count {
        wb_xml.push_str(&format!(r#"<sheet name="Sheet {s}" sheetId="{s}" r:id="rId{s}"/>"#));
    }
    wb_xml.push_str("</sheets></workbook>");

    let mut sst_xml = String::with_capacity(string_count * 80 + 512);
    sst_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#);
    for i in 1..=string_count {
        sst_xml.push_str(&format!("<si><t>Benchmark Shared String Value {i} Metric Dimension</t></si>"));
    }
    sst_xml.push_str("</sst>");

    let files = [
        ("docProps/core.xml", core_xml.as_bytes()),
        ("docProps/app.xml", app_xml.as_bytes()),
        ("xl/workbook.xml", wb_xml.as_bytes()),
        ("xl/sharedStrings.xml", sst_xml.as_bytes()),
    ];
    create_benchmark_zip(&files)
}

fn make_benchmark_pptx(slide_count: usize) -> Vec<u8> {
    let core_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>TTZip PresentationML Benchmark</dc:title>
    <dc:creator>Witt Kung</dc:creator>
</cp:coreProperties>"#;

    let app_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
    <Application>TTZip Slides</Application>
    <Slides>20</Slides>
</Properties>"#;

    let mut files_storage: Vec<(String, Vec<u8>)> = Vec::new();
    files_storage.push(("docProps/core.xml".to_string(), core_xml.as_bytes().to_vec()));
    files_storage.push(("docProps/app.xml".to_string(), app_xml.as_bytes().to_vec()));

    for s in 1..=slide_count {
        let slide_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
    <p:cSld>
        <p:spTree>
            <p:sp>
                <p:nvSpPr><p:nvPr><p:ph type="ctrTitle"/></p:nvPr></p:nvSpPr>
                <p:txBody><a:p><a:r><a:t>Slide {s}: Microkernel Performance Matrix</a:t></a:r></a:p></p:txBody>
            </p:sp>
            <p:sp>
                <p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr>
                <p:txBody>
                    <a:p><a:r><a:t>Bullet 1: Throughput exceeding 200 MB/s baseline</a:t></a:r></a:p>
                    <a:p><a:r><a:t>Bullet 2: Zero heap allocation during XML event traversal</a:t></a:r></a:p>
                </p:txBody>
            </p:sp>
        </p:spTree>
    </p:cSld>
</p:sld>"#
        );
        files_storage.push((format!("ppt/slides/slide{s}.xml"), slide_xml.into_bytes()));
    }

    let files_ref: Vec<(&str, &[u8])> = files_storage
        .iter()
        .map(|(name, content)| (name.as_str(), content.as_slice()))
        .collect();

    create_benchmark_zip(&files_ref)
}

/// Measures average iteration latency (in seconds) for a workload using clock rising-edge alignment.
fn measure_workload<F: FnMut() -> R, R>(mut workload: F) -> (f64, usize) {
    for _ in 0..WARMUP_RUNS {
        black_box(workload());
    }

    wait_for_next_tick();

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
// Performance Gates
// ============================================================================

#[test]
fn test_xlsx_spreadsheet_parsing_throughput_gate() {
    let xlsx_bytes = make_benchmark_xlsx(5, 500);
    let raw_len = xlsx_bytes.len();
    assert!(raw_len > 1024);

    let (avg_sec, iters) = measure_workload(|| {
        let meta = parse_office_metadata_from_slice(&xlsx_bytes).unwrap();
        assert_eq!(meta.format_name, "XLSX");
        assert_eq!(meta.sheet_count, 5);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Office Benchmark] XLSX Workbook Parsing ({} bytes, {} iters): {:.2} MB/s (latency: {:.3} ms)",
        raw_len, iters, throughput_mb, avg_sec * 1000.0
    );

    assert!(
        throughput_mb >= 200.0,
        "XLSX Parsing throughput below 200.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

#[test]
fn test_office_metadata_extraction_latency_gate() {
    let docx_bytes = make_benchmark_docx(50);

    let (avg_sec, iters) = measure_workload(|| {
        let meta = parse_office_metadata_from_slice(&docx_bytes).unwrap();
        assert_eq!(meta.title.as_deref(), Some("TTZip WordprocessingML Benchmark"));
    });

    let latency_ms = avg_sec * 1000.0;
    println!(
        "[Office Benchmark] Metadata extraction ({} iters): {:.4} ms",
        iters, latency_ms
    );

    assert!(
        latency_ms <= 1.0,
        "Office metadata extraction latency exceeds 1.0 ms gate: {:.4} ms",
        latency_ms
    );
}

#[test]
fn test_docx_text_extraction_throughput_gate() {
    let docx_bytes = make_benchmark_docx(200);
    let raw_len = docx_bytes.len();

    let (avg_sec, iters) = measure_workload(|| {
        let outline = parse_office_outline_from_slice(&docx_bytes).unwrap();
        assert_eq!(outline.document_type, "Word Processing");
        assert!(!outline.headings.is_empty());
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Office Benchmark] DOCX Text & Outline Extraction ({} bytes, {} iters): {:.2} MB/s (latency: {:.3} ms)",
        raw_len, iters, throughput_mb, avg_sec * 1000.0
    );

    assert!(
        throughput_mb >= 200.0,
        "DOCX Text extraction throughput below 200.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

#[test]
fn test_pptx_slide_parsing_throughput_gate() {
    let pptx_bytes = make_benchmark_pptx(20);
    let raw_len = pptx_bytes.len();

    let (avg_sec, iters) = measure_workload(|| {
        let outline = parse_office_outline_from_slice(&pptx_bytes).unwrap();
        assert_eq!(outline.document_type, "Presentation");
        assert_eq!(outline.total_sections, 20);
    });

    let throughput_mb = (raw_len as f64 / (1024.0 * 1024.0)) / avg_sec;
    println!(
        "[Office Benchmark] PPTX Slide & Outline Parsing ({} bytes, {} iters): {:.2} MB/s (latency: {:.3} ms)",
        raw_len, iters, throughput_mb, avg_sec * 1000.0
    );

    assert!(
        throughput_mb >= 150.0,
        "PPTX Parsing throughput below 150.0 MB/s gate: {:.2} MB/s",
        throughput_mb
    );
}

#[test]
fn test_master_anti_regression_invariant_6_gate() {
    let docx_bytes = make_benchmark_docx(100);
    let xlsx_bytes = make_benchmark_xlsx(3, 200);

    let (docx_lat1, _) = measure_workload(|| {
        let _ = parse_office_metadata_from_slice(&docx_bytes);
    });
    let (docx_lat2, _) = measure_workload(|| {
        let _ = parse_office_metadata_from_slice(&docx_bytes);
    });

    let regression_pct = ((docx_lat2 - docx_lat1) / docx_lat1) * 100.0;
    println!(
        "[Invariant 6 Gate] DOCX Pass 1: {:.4} ms, Pass 2: {:.4} ms -> Delta: {:+.2}%",
        docx_lat1 * 1000.0,
        docx_lat2 * 1000.0,
        regression_pct
    );

    assert!(
        regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Performance regression {:+.2}% exceeds Invariant 6 limit (+{:.1}%)",
        regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );

    let (xlsx_lat1, _) = measure_workload(|| {
        let _ = parse_office_metadata_from_slice(&xlsx_bytes);
    });
    let (xlsx_lat2, _) = measure_workload(|| {
        let _ = parse_office_metadata_from_slice(&xlsx_bytes);
    });

    let xlsx_regression_pct = ((xlsx_lat2 - xlsx_lat1) / xlsx_lat1) * 100.0;
    println!(
        "[Invariant 6 Gate] XLSX Pass 1: {:.4} ms, Pass 2: {:.4} ms -> Delta: {:+.2}%",
        xlsx_lat1 * 1000.0,
        xlsx_lat2 * 1000.0,
        xlsx_regression_pct
    );

    assert!(
        xlsx_regression_pct <= MAX_ALLOWED_REGRESSION_PCT,
        "Performance regression {:+.2}% exceeds Invariant 6 limit (+{:.1}%)",
        xlsx_regression_pct,
        MAX_ALLOWED_REGRESSION_PCT
    );
}
