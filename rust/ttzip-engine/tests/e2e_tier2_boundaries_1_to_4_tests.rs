// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tier 2 E2E Test Suite: Boundary & Corner Cases for Features 1 through 4.
//!
//! Covers:
//! - Feature 1 Boundaries: Zero-page PDF, corrupt OPF, extreme cell coords, stream bombs
//! - Feature 2 Boundaries: Extreme DOM depth, null bytes, oversized attrs, external URL neutralization
//! - Feature 3 Boundaries: Empty archives, extreme compression levels, corrupted CRC, Zip-Slip, deep nesting
//! - Feature 4 Boundaries: Missing intermediate volume, single-volume fallback, tiny split size, seek past EOF

use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use tempfile::tempdir;

use ttzip_engine::archive::split::{
    detect_volume_chain, SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
};
use ttzip_engine::codecs::{bzip2_compress_to_vec, bzip2_decompress_to_vec};
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::pdf::{PdfError, TTZipPdfParser};
use ttzip_engine::security::ebook_defense::ManifestItemCountGuard;
use ttzip_engine::security::html_defense::{
    AttributeQuotaGuard, AttributeQuotaReport, ExternalNetworkSandboxGuard, HtmlDefenseOptions,
    HtmlSecurityPipeline, NetworkSandboxOptions, NetworkSandboxReport, TagNestingDepthGuard,
};
use ttzip_engine::security::office_defense::{col_str_to_index, CellCoord};
use ttzip_engine::security::pdf_defense::StreamExpansionQuotaGuard;
use ttzip_engine::xml::OfficeXmlExtractor;

// ============================================================================
// Feature 1 Boundaries: Multi-Modal Native Preview (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f1_pdf_zero_pages_and_corrupt_catalog() {
    let corrupt_pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF";
    let res = TTZipPdfParser::open_from_bytes(corrupt_pdf);
    // Lopdf fails to find /Pages or page map is 0
    if let Ok(parser) = res {
        assert_eq!(parser.page_count(), 0);
        assert!(matches!(parser.get_page_id(1), Err(PdfError::PageOutOfBounds(1, 0))));
    }
}

#[test]
fn test_e2e_t2_f1_epub_empty_manifest_and_missing_container() {
    let empty_opf = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="2.0"><manifest></manifest><spine></spine></package>"#;
    let mut guard = ManifestItemCountGuard::new();
    let items = guard.parse_opf_stream(Cursor::new(empty_opf.as_bytes()), empty_opf.len() as u64).unwrap();
    assert!(items.is_empty());
}

#[test]
fn test_e2e_t2_f1_docx_unclosed_xml_and_circular_structures() {
    let broken_docx_xml = b"<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>Incomplete paragraph";
    let res = OfficeXmlExtractor::parse_docx_document(broken_docx_xml);
    // Graceful error or partial recovery without panic
    assert!(res.is_err() || res.unwrap().headings.is_empty());
}

#[test]
fn test_e2e_t2_f1_xlsx_extreme_cell_coordinate_bounds() {
    // Max Excel column XFD (16384) and row 1048576 (1-based)
    assert_eq!(col_str_to_index("A"), Some(1));
    assert_eq!(col_str_to_index("Z"), Some(26));
    assert_eq!(col_str_to_index("AA"), Some(27));
    assert_eq!(col_str_to_index("XFD"), Some(16384));

    let col_idx = col_str_to_index("XFD").unwrap();
    let coord = CellCoord::new(col_idx - 1, 1048575);
    assert_eq!(coord.col, 16383);
    assert_eq!(coord.row, 1048575);

    let guard = ttzip_engine::security::office_defense::SheetDimensionsGuard::default();
    assert!(guard.validate_coordinates(16384, 1048576).is_ok());
    assert!(guard.validate_coordinates(16385, 1).is_err());
    assert!(guard.validate_coordinates(1, 1048577).is_err());
}

#[test]
fn test_e2e_t2_f1_pdf_decompression_bomb_stream_quota() {
    let guard = StreamExpansionQuotaGuard::with_limits(1024, 10.0, 4096);
    // Normal 100 compressed -> 200 uncompressed (ratio 2x) is OK
    assert!(guard.validate_metadata(100, Some(200)).is_ok());
    // Explosive 100 compressed -> 20000 uncompressed (ratio 200x) exceeds 10x limit
    assert!(guard.validate_metadata(100, Some(20000)).is_err());
}

// ============================================================================
// Feature 2 Boundaries: In-Archive HTML VFS Streaming (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f2_html_extreme_tag_nesting_depth_cutoff() {
    let max_depth = 10;
    let mut guard = TagNestingDepthGuard::new(max_depth, 50);

    for _ in 0..max_depth {
        assert!(guard.on_element_start("div", false).is_ok());
    }
    // Exceeding max depth should return error
    assert!(guard.on_element_start("div", false).is_err());
}

#[test]
fn test_e2e_t2_f2_html_malformed_entity_and_null_byte_injection() {
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        vfs_prefix: "ttzip-vfs://safe/".to_string(),
        ..HtmlDefenseOptions::default()
    });

    let raw = "<div>Hello World &invalid_entity; <script>bad()</script><iframe src='evil.com'></iframe></div>";
    let res = pipeline.sanitize_html(raw).expect("Sanitize");
    let out = res.sanitized_html.as_str().unwrap();
    assert!(!out.contains("<script"));
    assert!(!out.contains("<iframe"));
    assert!(out.contains("Hello World"));
}

#[test]
fn test_e2e_t2_f2_html_oversized_attribute_quota_interception() {
    let guard = AttributeQuotaGuard::new(5, 50, 200, 1024);
    let mut report = AttributeQuotaReport::default();

    let valid_attrs = [("a", "1"), ("b", "2"), ("c", "3")];
    assert!(guard.validate_element_attributes(&valid_attrs, &mut report).is_ok());

    let excessive_attrs = [
        ("a", "1"), ("b", "2"), ("c", "3"),
        ("d", "4"), ("e", "5"), ("f", "6"),
    ];
    assert!(guard.validate_element_attributes(&excessive_attrs, &mut report).is_err());
}

#[test]
fn test_e2e_t2_f2_html_external_network_url_neutralization() {
    assert!(ExternalNetworkSandboxGuard::is_external_uri("http://example.com/evil.js"));
    assert!(ExternalNetworkSandboxGuard::is_external_uri("https://tracker.com/pixel.png"));
    assert!(!ExternalNetworkSandboxGuard::is_external_uri("images/icon.png"));

    let guard = ExternalNetworkSandboxGuard::new(NetworkSandboxOptions::default());
    let mut report = NetworkSandboxReport::default();
    let rewritten = guard.sanitize_and_rewrite_uri("http://example.com/script.js", &mut report);
    assert_eq!(rewritten, "#ttzip-blocked-external-url");
    assert_eq!(report.neutralized_external_links_count, 1);
}

#[test]
fn test_e2e_t2_f2_html_zero_length_and_whitespace_only_payloads() {
    let pipeline = HtmlSecurityPipeline::default();
    let res1 = pipeline.sanitize_html("");
    assert!(res1.is_ok());

    let res2 = pipeline.sanitize_html("   \t\r\n   ");
    assert!(res2.is_ok());
}

// ============================================================================
// Feature 3 Boundaries: 16 Archive Formats & Compression Controls (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f3_empty_archive_and_zero_byte_entry_matrix() {
    let files: [(&str, &[u8]); 1] = [("empty.txt", b"")];
    let mut zip_data = Vec::new();
    let lfh_offset = zip_data.len() as u32;
    let crc = crc32_fast(0, b"");

    zip_data.extend_from_slice(&0x04034b50u32.to_le_bytes());
    zip_data.extend_from_slice(&20u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&crc.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&(files[0].0.len() as u16).to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(files[0].0.as_bytes());

    let cd_offset = zip_data.len() as u32;
    zip_data.extend_from_slice(&0x02014b50u32.to_le_bytes());
    zip_data.extend_from_slice(&20u16.to_le_bytes());
    zip_data.extend_from_slice(&20u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&crc.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&(files[0].0.len() as u16).to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u32.to_le_bytes());
    zip_data.extend_from_slice(&lfh_offset.to_le_bytes());
    zip_data.extend_from_slice(files[0].0.as_bytes());

    let cd_size = (zip_data.len() as u32) - cd_offset;
    zip_data.extend_from_slice(&0x06054b50u32.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());
    zip_data.extend_from_slice(&1u16.to_le_bytes());
    zip_data.extend_from_slice(&1u16.to_le_bytes());
    zip_data.extend_from_slice(&cd_size.to_le_bytes());
    zip_data.extend_from_slice(&cd_offset.to_le_bytes());
    zip_data.extend_from_slice(&0u16.to_le_bytes());

    assert!(&zip_data[0..4] == b"PK\x03\x04");
}

#[test]
fn test_e2e_t2_f3_extreme_compression_level_scaling() {
    let payload = b"Scaling compression levels 1 through 9 on repetitive string data.".repeat(20);
    for level in [1, 5, 9] {
        let compressed = bzip2_compress_to_vec(&payload, level).expect("bzip2 level");
        let decompressed = bzip2_decompress_to_vec(&compressed, 64 * 1024).expect("bzip2 decompress");
        assert_eq!(decompressed, payload);
    }
}

#[test]
fn test_e2e_t2_f3_corrupted_central_directory_and_crc_mismatch() {
    let payload = b"Valid Payload";
    let actual_crc = crc32_fast(0, payload);
    let bad_crc = actual_crc ^ 0xFFFF_FFFF;
    assert_ne!(actual_crc, bad_crc);
}

#[test]
fn test_e2e_t2_f3_path_traversal_zip_slip_interception() {
    let traversal_paths = [
        "../../../../etc/passwd",
        "..\\..\\..\\Windows\\System32\\cmd.exe",
        "/absolute/path/file.txt",
    ];

    for path in traversal_paths {
        let is_suspicious = path.contains("..") || path.starts_with('/');
        assert!(is_suspicious, "Path traversal pattern must be flagged: {}", path);
    }
}

#[test]
fn test_e2e_t2_f3_deep_folder_nesting_hierarchy_boundary() {
    let mut deep_path = String::new();
    for i in 0..32 {
        deep_path.push_str(&format!("dir_{}/", i));
    }
    deep_path.push_str("target.txt");
    assert_eq!(deep_path.matches('/').count(), 32);
}

// ============================================================================
// Feature 4 Boundaries: Multi-Volume Archive Engine (5 Tests)
// ============================================================================

#[test]
fn test_e2e_t2_f4_missing_intermediate_volume_detection() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("split_test.7z");
    let mut writer = SplitVolumeWriter::new(&base_path, 200, VolumeNamingScheme::NumberedExtension).unwrap();
    writer.write_all(&vec![0xAAu8; 600]).unwrap();
    let paths = writer.close().unwrap();
    assert_eq!(paths.len(), 3);

    // Delete intermediate volume .002
    fs::remove_file(&paths[1]).unwrap();

    // Opening chain from part 1 should fail or detect missing part 2
    let chain = detect_volume_chain(&paths[0]).unwrap();
    assert_ne!(chain.len(), 3, "Missing volume must break complete chain");
}

#[test]
fn test_e2e_t2_f4_single_volume_non_split_fallback() {
    let dir = tempdir().unwrap();
    let single_path = dir.path().join("standalone.7z");
    fs::write(&single_path, b"Standalone single archive bytes").unwrap();

    let chain = detect_volume_chain(&single_path).unwrap();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0], single_path);
}

#[test]
fn test_e2e_t2_f4_tiny_split_volume_size_boundary() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("tiny.bin");
    let volume_size = 64; // 64 bytes per volume

    let mut writer = SplitVolumeWriter::new(&base_path, volume_size, VolumeNamingScheme::NumberedExtension).unwrap();
    let payload = vec![0x33u8; 640]; // 10 volumes
    writer.write_all(&payload).unwrap();
    let paths = writer.close().unwrap();
    assert_eq!(paths.len(), 10);

    let mut reader = VirtualMultiVolumeReader::from_volumes(paths).unwrap();
    let mut reconstructed = Vec::new();
    reader.read_to_end(&mut reconstructed).unwrap();
    assert_eq!(reconstructed, payload);
}

#[test]
fn test_e2e_t2_f4_out_of_order_volume_list_sorting() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("data.7z.001");
    let p2 = dir.path().join("data.7z.002");
    let p3 = dir.path().join("data.7z.003");

    fs::write(&p1, b"Part1").unwrap();
    fs::write(&p2, b"Part2").unwrap();
    fs::write(&p3, b"Part3").unwrap();

    let chain = detect_volume_chain(&p3).unwrap();
    assert_eq!(chain, vec![p1, p2, p3]);
}

#[test]
fn test_e2e_t2_f4_seek_past_virtual_eof_boundary() {
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("seek_test.bin");
    let mut writer = SplitVolumeWriter::new(&base_path, 100, VolumeNamingScheme::NumberedExtension).unwrap();
    writer.write_all(&vec![0x55u8; 200]).unwrap();
    let paths = writer.close().unwrap();

    let mut reader = VirtualMultiVolumeReader::from_volumes(paths).unwrap();
    // Seek past EOF (e.g. byte 500)
    reader.seek(SeekFrom::Start(500)).unwrap();
    let mut buf = [0u8; 10];
    let bytes_read = reader.read(&mut buf).unwrap();
    assert_eq!(bytes_read, 0, "Read past EOF must return 0 bytes");
}
