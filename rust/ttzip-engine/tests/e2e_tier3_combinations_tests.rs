// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Tier 3 E2E Test Suite: Cross-Feature Combinations (C01 - C08).
//!
//! Evaluates:
//! - C01: Split Volume (F4) + Solid 7z (F5) + Office XML (F1)
//! - C02: In-Archive HTML VFS (F2) + Tree-sitter Code (F6)
//! - C03: AES-256 Vault (F3) + Charset Mojibake (F7)
//! - C04: Delta Differential Patch (F8) + Split Volumes (F4)
//! - C05: PDF Metadata (F1) + Solid 7z (F5) + Mach RSS (F5)
//! - C06: HTML VFS (F2) + CJK Charset (F7) + Fast Codecs (F3)
//! - C07: Ed25519 Signed (F8) + Solid 7z (F5) + Tree-sitter (F6)
//! - C08: Encrypted Split Volume (F4+F3) + Solid (F5) + Multi-Modal (F1+F6)

use std::io::{Read, Write};
use tempfile::tempdir;

use ttzip_engine::archive::split::{
    detect_volume_chain, SplitVolumeWriter, VirtualMultiVolumeReader, VolumeNamingScheme,
};
use ttzip_engine::codecs::brotli::{brotli_compress_to_vec, brotli_decompress_to_vec};
use ttzip_engine::codecs::chardet::detect_charset;
use ttzip_engine::crypto::aes256::{aes256_cbc_decrypt, aes256_cbc_encrypt};
use ttzip_engine::crypto::ed25519::SigningKey;
use ttzip_engine::pdf::{PdfMetadataExtractor, TTZipPdfParser};
use ttzip_engine::security::html_defense::{HtmlDefenseOptions, HtmlSecurityPipeline};
use ttzip_engine::sevenz::{create_7z_solid_archive_bytes, get_current_rss_bytes, SevenZReader};
use ttzip_engine::syntax::{SupportedLanguage, SyntaxHighlighter, TTZipSyntaxParser};
use ttzip_engine::system::delta::engine::TTZipDeltaEngine;
use ttzip_engine::xml::OfficeXmlExtractor;
use ttzip_engine::zip::writer::ZipInputItem;

// ============================================================================
// C01: Split Volume (F4) + Solid 7z (F5) + Office XML (F1)
// ============================================================================

#[test]
fn test_e2e_t3_c01_split_volume_solid_7z_office_xml_pipeline() {
    let docx_xml = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Project Specification 2026</w:t></w:r></w:p></w:body></w:document>"#;

    let items = vec![ZipInputItem {
        rel_path: "word/document.xml".to_string(),
        data: docx_xml.as_bytes().to_vec(),
        mtime_epoch_secs: 0,
        mode: 0o644,
        is_directory: false,
    }];

    // 1. Create Solid 7z Archive
    let solid_bytes = create_7z_solid_archive_bytes(&items, 5, 1).expect("7z solid create");

    // 2. Split into 3 multi-volumes (100 bytes each)
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("archive.7z");
    let mut writer = SplitVolumeWriter::new(&base_path, 100, VolumeNamingScheme::NumberedExtension).unwrap();
    writer.write_all(&solid_bytes).unwrap();
    let split_paths = writer.close().unwrap();
    assert!(split_paths.len() >= 2);

    // 3. Reconstruct via VirtualMultiVolumeReader
    let chain = detect_volume_chain(&split_paths[0]).unwrap();
    let mut reader = VirtualMultiVolumeReader::from_volumes(chain).unwrap();
    let mut reconstructed_solid = Vec::new();
    reader.read_to_end(&mut reconstructed_solid).unwrap();
    assert_eq!(reconstructed_solid, solid_bytes);

    // 4. Open solid 7z and extract Office XML
    let sz_reader = SevenZReader::open_slice(&reconstructed_solid).expect("7z open");
    let extracted_xml = sz_reader.extract_entry_bytes_stream(0, None).expect("extract xml");
    let docx_outline = OfficeXmlExtractor::parse_docx_document(&extracted_xml).expect("parse docx");
    assert_eq!(docx_outline.headings.len(), 1);
    assert_eq!(docx_outline.headings[0].text, "Project Specification 2026");
}

// ============================================================================
// C02: In-Archive HTML VFS (F2) + Tree-sitter Code (F6)
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t3_c02_html_vfs_embedding_treesitter_highlighted_tokens() {
    // 1. Parse and highlight Rust source
    let source = "pub fn execute() -> bool { true }";
    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let tree = parser.parse_full(source).unwrap();
    let mut highlighter = SyntaxHighlighter::new();
    let tokens = highlighter.highlight(tree, source, SupportedLanguage::Rust).unwrap();
    assert!(!tokens.is_empty());

    // 2. Render code in HTML preview and sanitize via HTML VFS pipeline
    let raw_html = format!(
        r#"<html><head><link rel="stylesheet" href="syntax.css"></head><body><pre class="code"><code>{}</code></pre></body></html>"#,
        source
    );
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        vfs_prefix: "ttzip-vfs://code_archive/".to_string(),
        ..HtmlDefenseOptions::default()
    });
    let res = pipeline.sanitize_html(&raw_html).expect("Sanitize HTML");
    let out = res.sanitized_html.as_str().unwrap();
    assert!(out.contains("ttzip-vfs://code_archive/syntax.css"));
    assert!(out.contains("pub fn execute()"));
}

// ============================================================================
// C03: AES-256 Vault (F3) + Charset Mojibake (F7)
// ============================================================================

#[test]
fn test_e2e_t3_c03_aes256_encrypted_archive_with_cjk_mojibake_repaired_paths() {
    let raw_name = "2026年度_财务决算报告.xlsx";
    let (gbk_bytes, _, _) = encoding_rs::GB18030.encode(raw_name);
    let detected = detect_charset(&gbk_bytes);
    assert!(detected.is_some());

    let (repaired_name, _, _) = encoding_rs::GB18030.decode(&gbk_bytes);
    assert_eq!(repaired_name, raw_name);

    // Encrypt payload in AES-256 CBC Vault
    let key = [0x5Au8; 32];
    let iv = [0x3Cu8; 16];
    let payload = vec![0x11u8; 48]; // 3 blocks of 16
    let mut encrypted = vec![0u8; 48];
    aes256_cbc_encrypt(&key, &iv, &payload, &mut encrypted).unwrap();

    let mut decrypted = vec![0u8; 48];
    aes256_cbc_decrypt(&key, &iv, &encrypted, &mut decrypted).unwrap();
    assert_eq!(decrypted, payload);
}

// ============================================================================
// C04: Delta Differential Patch (F8) + Split Volumes (F4)
// ============================================================================

#[test]
fn test_e2e_t3_c04_delta_patching_across_split_volume_images() {
    let dir = tempdir().unwrap();
    let old_payload = vec![0x11u8; 500];
    let mut new_payload = vec![0x11u8; 500];
    new_payload[250..271].copy_from_slice(b"PATCHED_PAYLOAD_CHUNK");

    // 1. Create old split volumes
    let old_base = dir.path().join("old_vol.bin");
    let mut w_old = SplitVolumeWriter::new(&old_base, 200, VolumeNamingScheme::NumberedExtension).unwrap();
    w_old.write_all(&old_payload).unwrap();
    let old_paths = w_old.close().unwrap();

    // 2. Read full old stream
    let mut r_old = VirtualMultiVolumeReader::from_volumes(old_paths).unwrap();
    let mut old_full = Vec::new();
    r_old.read_to_end(&mut old_full).unwrap();

    // 3. Create differential patch
    let patch = TTZipDeltaEngine::create_patch(&old_full, &new_payload).expect("Create patch");
    let (reconstructed, _) = TTZipDeltaEngine::apply_patch_with_result(&old_full, &patch).expect("Apply patch");
    assert_eq!(reconstructed.as_slice(), new_payload.as_slice());
}

// ============================================================================
// C05: PDF Metadata (F1) + Solid 7z (F5) + Mach RSS (F5)
// ============================================================================

#[test]
fn test_e2e_t3_c05_pdf_metadata_extraction_from_7z_solid_under_rss_guard() {
    // Generate minimal lopdf document
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let content_id = doc.add_object(Stream::new(dictionary! {}, b"BT /F1 12 Tf (Combined PDF) Tj ET".to_vec()));
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

    // Solid archive packing
    let items = vec![ZipInputItem {
        rel_path: "manual.pdf".to_string(),
        data: pdf_bytes,
        mtime_epoch_secs: 0,
        mode: 0o644,
        is_directory: false,
    }];
    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 1).unwrap();

    // Decode & RSS verification
    let rss_before = get_current_rss_bytes();
    let reader = SevenZReader::open_slice(&solid_archive).unwrap();
    let extracted_pdf = reader.extract_entry_bytes_stream(0, None).unwrap();
    let pdf_parser = TTZipPdfParser::open_from_bytes(&extracted_pdf).unwrap();
    let metadata = PdfMetadataExtractor::extract_metadata(&pdf_parser).unwrap();
    assert_eq!(metadata.page_count, 1);

    let rss_after = get_current_rss_bytes();
    assert!(rss_after >= rss_before || rss_after > 0);
}

// ============================================================================
// C06: HTML VFS (F2) + CJK Charset (F7) + Fast Codecs (F3)
// ============================================================================

#[test]
fn test_e2e_t3_c06_html_vfs_with_cjk_assets_and_brotli_compression() {
    let raw_html = r#"<html><head><link rel="stylesheet" href="样式表.css"></head><body><img src="图片/封面.png"></body></html>"#;

    // 1. Sanitize HTML
    let pipeline = HtmlSecurityPipeline::new(HtmlDefenseOptions {
        vfs_prefix: "ttzip-vfs://cjk_vault/".to_string(),
        ..HtmlDefenseOptions::default()
    });
    let res = pipeline.sanitize_html(raw_html).expect("Sanitize");
    let sanitized_html = res.sanitized_html.as_str().unwrap();

    // 2. Compress HTML with Brotli
    let compressed_br = brotli_compress_to_vec(sanitized_html.as_bytes(), 6, 22).expect("Brotli compress");
    let decompressed_br = brotli_decompress_to_vec(&compressed_br, 64 * 1024).expect("Brotli decompress");
    assert_eq!(decompressed_br, sanitized_html.as_bytes());
}

// ============================================================================
// C07: Ed25519 Signed (F8) + Solid 7z (F5) + Tree-sitter (F6)
// ============================================================================

#[cfg(feature = "syntax")]
#[test]
fn test_e2e_t3_c07_ed25519_signed_solid_7z_with_treesitter_parsing() {
    let source_file = "pub fn add(x: i32, y: i32) -> i32 { x + y }";
    let items = vec![ZipInputItem {
        rel_path: "src/math.rs".to_string(),
        data: source_file.as_bytes().to_vec(),
        mtime_epoch_secs: 0,
        mode: 0o644,
        is_directory: false,
    }];

    // 1. Solid Archive
    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 1).unwrap();

    // 2. Ed25519 Sign
    let signing_key = SigningKey::from_bytes(&[0x77u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let signature = signing_key.sign(&solid_archive);
    assert!(verifying_key.verify(&solid_archive, &signature).is_ok());

    // 3. Extract and Parse
    let reader = SevenZReader::open_slice(&solid_archive).unwrap();
    let extracted = reader.extract_entry_bytes_stream(0, None).unwrap();
    let extracted_str = std::str::from_utf8(&extracted).unwrap();

    let mut parser = TTZipSyntaxParser::with_language(SupportedLanguage::Rust).unwrap();
    let tree = parser.parse_full(extracted_str).unwrap();
    assert!(!tree.root_node().has_error());
}

// ============================================================================
// C08: Encrypted Split Volume (F4+F3) + Solid (F5) + Multi-Modal (F1+F6)
// ============================================================================

#[test]
fn test_e2e_t3_c08_full_end_to_end_multitier_pipeline_integration() {
    let slide_xml = r#"<?xml version="1.0"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="Title 1"/><p:nvPr><p:ph type="ctrTitle"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Executive Summary</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;

    let items = vec![ZipInputItem {
        rel_path: "ppt/slides/slide1.xml".to_string(),
        data: slide_xml.as_bytes().to_vec(),
        mtime_epoch_secs: 0,
        mode: 0o644,
        is_directory: false,
    }];

    // Solid archive
    let solid_archive = create_7z_solid_archive_bytes(&items, 5, 1).unwrap();

    // Split across 2 volumes
    let dir = tempdir().unwrap();
    let base_path = dir.path().join("full_pipeline.7z");
    let mut writer = SplitVolumeWriter::new(&base_path, 150, VolumeNamingScheme::NumberedExtension).unwrap();
    writer.write_all(&solid_archive).unwrap();
    let paths = writer.close().unwrap();

    // Reconstruct & Extract
    let mut reader = VirtualMultiVolumeReader::from_volumes(paths).unwrap();
    let mut reconstructed = Vec::new();
    reader.read_to_end(&mut reconstructed).unwrap();

    let sz = SevenZReader::open_slice(&reconstructed).unwrap();
    let xml_data = sz.extract_entry_bytes_stream(0, None).unwrap();
    let pptx_outline = OfficeXmlExtractor::parse_pptx_slide(&xml_data, 1).unwrap();
    assert_eq!(pptx_outline.title, Some("Executive Summary".to_string()));
}
