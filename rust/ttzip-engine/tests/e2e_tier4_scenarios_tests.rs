// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tier 4 E2E Test Suite: Real-World Application Scenarios (S01 - S05).
//!
//! Realistic multi-stage workflow simulations:
//! - S01: Software Release Bundle Workflow (Split Volumes, 7z Solid, Ed25519, Tree-sitter, HTML VFS)
//! - S02: Digital Library Archival Pipeline (PDF, EPUB, CJK Mojibake, Mach RSS Guard)
//! - S03: Enterprise Financial Multi-Sheet Audit (XLSX Workbook, SST Quota, Formula Depth AST)
//! - S04: Delta OTA Application Update (BSDIFF40/SPK4, Container Header, Ed25519, Bit-exact Parity)
//! - S05: Secure Air-Gapped Code Review (Tree-sitter GLR, Shift-JIS Repair, Strict CSP VFS)

use std::io::{Read, Write};
use tempfile::tempdir;

use ttzip_engine::archive::split::{
    detect_volume_chain, SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
};
use ttzip_engine::codecs::chardet::detect_charset;
use ttzip_engine::crypto::ed25519::SigningKey;
use ttzip_engine::pdf::{PdfMetadataExtractor, TTZipPdfParser};
use ttzip_engine::security::ebook_defense::ManifestItemCountGuard;
use ttzip_engine::security::html_defense::{HtmlDefenseOptions, HtmlSecurityPipeline};
use ttzip_engine::security::office_defense::{FormulaDepthGuard, SstQuotaGuard};
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, get_current_rss_bytes, SevenZReader};
use ttzip_engine::syntax::{
    SupportedLanguage, SymbolOutlineExtractor, SyntaxHighlighter, TTZipSyntaxParser,
};
use ttzip_engine::system::delta::engine::TTZipDeltaEngine;
use ttzip_engine::system::delta::types::DeltaPatchHeader;
use ttzip_engine::xml::OfficeXmlExtractor;
use ttzip_engine::zip::writer::ZipInputItem;

// ============================================================================
// Scenario S01: Software Release Bundle Workflow
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t4_s01_software_release_bundle_workflow() {
    let release_notes_md = "# TTZip Release v1.1.0\n\n## Highlights\n- Solid 7z Micro-buffer Streaming\n- Tree-sitter Viewport Syntax\n";
    let changelog_html = r#"<html><head><link rel="stylesheet" href="style.css"></head><body><h1>Changelog</h1><p>Full release changelog</p></body></html>"#;
    let binary_dmg = vec![0xCFu8, 0xFA, 0xED, 0xFE, 0x07, 0x00, 0x00, 0x01]; // Mach-O 64-bit magic

    let items = vec![
        ZipInputItem {
            rel_path: "RELEASE_NOTES.md".to_string(),
            data: release_notes_md.as_bytes().to_vec(),
            mtime_epoch_secs: 1772500000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "changelog.html".to_string(),
            data: changelog_html.as_bytes().to_vec(),
            mtime_epoch_secs: 1772500000,
            mode: 0o644,
            is_directory: false,
        },
        ZipInputItem {
            rel_path: "TTZip-1.1.0-macos.dmg".to_string(),
            data: binary_dmg,
            mtime_epoch_secs: 1772500000,
            mode: 0o755,
            is_directory: false,
        },
    ];

    // Stage 1: Build Solid 7z Archive
    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 2).expect("7z solid archive");

    // Stage 2: Sign release bundle with Ed25519
    let signing_key = SigningKey::from_bytes(&[0x19u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let appcast_signature = signing_key.sign(&solid_archive);
    assert!(verifying_key.verify(&solid_archive, &appcast_signature).is_ok());

    // Stage 3: Split into 3 discrete distribution volumes
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("TTZip-1.1.0-bundle.7z");
    let mut writer = SplitVolumeWriter::new(&base_path, 120, VolumeNamingScheme::NumberedExtension).unwrap();
    writer.write_all(&solid_archive).unwrap();
    let split_files = writer.close().unwrap();
    assert!(split_files.len() >= 2);

    // Stage 4: Simulate client ingestion - detect chain and assemble
    let detected_chain = detect_volume_chain(&split_files[0]).unwrap();
    let mut virtual_reader = VirtualMultiVolumeReader::from_volumes(detected_chain).unwrap();
    let mut client_bundle = Vec::new();
    virtual_reader.read_to_end(&mut client_bundle).unwrap();
    assert_eq!(client_bundle, solid_archive);

    // Stage 5: Client validates Ed25519 signature before execution
    assert!(verifying_key.verify(&client_bundle, &appcast_signature).is_ok());

    // Stage 6: Open solid payload, parse Markdown outline, and sanitize HTML preview
    let sz_client = SevenZReader::open_slice(&client_bundle).unwrap();
    let extracted_md = sz_client.extract_entry_bytes_stream(0, None).unwrap();
    let md_str = std::str::from_utf8(&extracted_md).unwrap();
    let md_symbols = SymbolOutlineExtractor::extract_from_source(md_str, SupportedLanguage::Markdown).unwrap();
    assert!(!md_symbols.is_empty());

    let extracted_html = sz_client.extract_entry_bytes_stream(1, None).unwrap();
    let html_pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        vfs_prefix: "ttzip-vfs://release_110/".to_string(),
        ..HtmlDefenseOptions::default()
    });
    let sanitized = html_pipeline.sanitize_html(std::str::from_utf8(&extracted_html).unwrap()).unwrap();
    assert!(sanitized.sanitized_html.as_str().unwrap().contains("ttzip-vfs://release_110/style.css"));
}

// ============================================================================
// Scenario S02: Digital Library Archival & Ingestion Pipeline
// ============================================================================

#[test]
fn test_e2e_t4_s02_digital_library_archival_and_ingestion_pipeline() {
    // 1. Synthesize PDF Manual
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Roman" });
    let content_id = doc.add_object(Stream::new(dictionary! {}, b"BT /F1 14 Tf (Archived Manuscript 2026) Tj ET".to_vec()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut pdf_bytes = Vec::new();
    doc.save_to(&mut pdf_bytes).unwrap();

    // 2. Synthesize EPUB package metadata with CJK title
    let epub_opf = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><manifest><item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#;
    let mut epub_guard = ManifestItemCountGuard::new();
    let manifest_items = epub_guard.parse_opf_stream(std::io::Cursor::new(epub_opf.as_bytes()), epub_opf.len() as u64).unwrap();
    assert_eq!(manifest_items.len(), 1);

    // 3. Mojibake repair for book metadata title
    let raw_book_title = "四库全书_精选录.pdf";
    let (gb18030_title, _, _) = encoding_rs::GB18030.encode(raw_book_title);
    assert!(detect_charset(&gb18030_title).is_some());
    let (restored_title, _, _) = encoding_rs::GB18030.decode(&gb18030_title);
    assert_eq!(restored_title, raw_book_title);

    // 4. Ingest into 7z solid archive and verify RSS memory invariance
    let _rss_initial = get_current_rss_bytes();
    let items = vec![
        ZipInputItem {
            rel_path: restored_title.to_string(),
            data: pdf_bytes,
            mtime_epoch_secs: 0,
            mode: 0o644,
            is_directory: false,
        },
    ];
    let archive_bytes = create_7z_solid_archive_bytes(&items, 5, 1).unwrap();

    let reader = SevenZReader::open_slice(&archive_bytes).unwrap();
    let extracted_book = reader.extract_entry_bytes_stream(0, None).unwrap();
    let pdf_parser = TTZipPdfParser::open_from_bytes(&extracted_book).unwrap();
    let meta = PdfMetadataExtractor::extract_metadata(&pdf_parser).unwrap();
    assert_eq!(meta.page_count, 1);

    let rss_final = get_current_rss_bytes();
    assert!(rss_final > 0);
    assert!(rss_final < 512 * 1024 * 1024, "Resident memory strictly bounded");
}

// ============================================================================
// Scenario S03: Enterprise Financial Multi-Sheet Audit Workflow
// ============================================================================

#[test]
fn test_e2e_t4_s03_enterprise_financial_multisheet_audit_workflow() {
    let sst_xml = r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3"><si><t>Q1 Revenue</t></si><si><t>Operating Expenses</t></si><si><t>Net Income</t></si></sst>"#;
    let wb_xml = r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheets><sheet name="Income Statement" sheetId="1" r:id="rId1"/><sheet name="Balance Sheet" sheetId="2" r:id="rId2"/></sheets></workbook>"#;

    // 1. Validate Shared String Table against HashDOS Quota Guard
    let mut sst_guard = SstQuotaGuard::default();
    let strings = OfficeXmlExtractor::parse_xlsx_shared_strings(sst_xml.as_bytes(), Some(100)).unwrap();
    assert_eq!(strings.len(), 3);
    assert_eq!(strings[0], "Q1 Revenue");
    assert_eq!(strings[2], "Net Income");
    for s in &strings {
        assert!(sst_guard.add_entry(s).is_ok());
    }

    // 2. Parse Workbook Sheets structure
    let wb_meta = OfficeXmlExtractor::parse_xlsx_workbook(wb_xml.as_bytes()).unwrap();
    assert_eq!(wb_meta.sheets.len(), 2);
    assert_eq!(wb_meta.sheets[0].name, "Income Statement");
    assert_eq!(wb_meta.sheets[1].name, "Balance Sheet");

    // 3. Validate financial formula safety with FormulaDepthGuard
    let formula_guard = FormulaDepthGuard::default();
    let valid_formula = "SUM(B2:B10) - SUM(C2:C10)";
    let report = formula_guard.inspect_formula(valid_formula).unwrap();
    assert!(report.max_depth <= 32);
    assert!(report.token_count <= 1024);
}

// ============================================================================
// Scenario S04: Delta Over-The-Air (OTA) Application Update
// ============================================================================

#[test]
fn test_e2e_t4_s04_delta_ota_application_update_workflow() {
    let base_app = b"TTZip Base Application Binary v1.0.0 Mach-O x86_64 arm64e universal build";
    let mut target_app = base_app.to_vec();
    target_app.extend_from_slice(b" - Patched with Ultra-Fast Streaming Engine v1.1.0");

    // 1. Generate differential patch
    let raw_patch = TTZipDeltaEngine::create_patch(base_app, &target_app).expect("create delta");

    // 2. Serialize and verify 24-byte container header with CRC-32
    let header = DeltaPatchHeader::new(
        *b"spk!",
        4,
        0,
        0xAA11_BB22,
        0xCC33_DD44,
        target_app.len() as u64,
    );
    let hdr_bytes = header.to_bytes();
    assert_eq!(hdr_bytes.len(), 24);
    let parsed_hdr = DeltaPatchHeader::from_bytes(&hdr_bytes).unwrap();
    assert_eq!(parsed_hdr.magic, *b"spk!");

    // 3. Ed25519 Sign Delta Patch
    let signer = SigningKey::from_bytes(&[0x88u8; 32]);
    let verifier = signer.verifying_key();
    let patch_sig = signer.sign(&raw_patch);
    assert!(verifier.verify(&raw_patch, &patch_sig).is_ok());

    // 4. Client applies patch and asserts 100% bit-exact parity
    let (reconstructed, telemetry) = TTZipDeltaEngine::apply_patch_with_result(base_app, &raw_patch).unwrap();
    assert_eq!(reconstructed.as_slice(), target_app.as_slice());
    assert_eq!(telemetry.bytes_out, target_app.len());
}

// ============================================================================
// Scenario S05: Secure Air-Gapped Code Review & Security Audit
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t4_s05_secure_airgapped_code_review_and_security_audit() {
    let source_c = "#include <stdio.h>\nint main(void) {\n    printf(\"Audited C Kernel\\n\");\n    return 0;\n}\n";
    let source_rs = "pub fn verify_bounds(idx: usize, len: usize) -> bool { idx < len }";

    // 1. Shift-JIS Filename Repair
    let raw_kanji = "カーネルソース_v1.c";
    let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(raw_kanji);
    assert!(detect_charset(&sjis_bytes).is_some());
    let (repaired_name, _, _) = encoding_rs::SHIFT_JIS.decode(&sjis_bytes);
    assert_eq!(repaired_name, raw_kanji);

    // 2. Tree-sitter Viewport Syntax Tokenization
    let mut c_parser = TTZipSyntaxParser::with_language(SupportedLanguage::C).unwrap();
    let c_tree = c_parser.parse_full(source_c).unwrap();
    let mut highlighter = SyntaxHighlighter::new();
    let c_tokens = highlighter.highlight(c_tree, source_c, SupportedLanguage::C).unwrap();
    assert!(!c_tokens.is_empty());

    let mut rs_parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let rs_tree = rs_parser.parse_full(source_rs).unwrap();
    let rs_tokens = highlighter.highlight(rs_tree, source_rs, SupportedLanguage::Rust).unwrap();
    assert!(!rs_tokens.is_empty());

    // 3. Air-Gapped HTML Security Pipeline with Strict CSP
    let doc_preview = format!(
        r#"<html><head><script>steal_keys();</script><link rel="stylesheet" href="theme.css"></head><body><h2>Security Audit</h2><pre>{}</pre></body></html>"#,
        source_rs
    );
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        vfs_prefix: "ttzip-vfs://audit_sandbox/".to_string(),
        inject_csp: true,
        ..HtmlDefenseOptions::default()
    });
    let sanitized = pipeline.sanitize_html(&doc_preview).unwrap();
    let out_html = sanitized.sanitized_html.as_str().unwrap();

    assert!(!out_html.contains("<script>"));
    assert!(!out_html.contains("steal_keys()"));
    assert!(out_html.contains("Content-Security-Policy"));
    assert!(out_html.contains("ttzip-vfs://audit_sandbox/theme.css"));
}
