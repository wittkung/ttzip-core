// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for PDF Metadata, Outline, and Text Search.
//!
//! Provides zero-disk-landing in-memory PDF parsing, Info dictionary extraction,
//! hierarchical outline bookmark tree traversal, per-page text extraction, and
//! full-text search for Swift 6 UI inspector and QuickLook preview pipelines.

pub mod parser;
pub mod types;

use std::fs::File;
use std::path::Path;

pub use parser::{
    extract_all_pages_text_from_slice, extract_pdf_page_text_from_slice,
    parse_pdf_metadata_from_slice, parse_pdf_outline_from_slice, search_pdf_text_from_slice,
};
pub use types::{
    UniFFIPdfMetadata, UniFFIPdfOutlineNode, UniFFIPdfPageText, UniFFIPdfSearchResult,
    UniFFIPdfService,
};
use crate::uniffi_api::types::TTZipError;

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Extracts PDF metadata (title, author, subject, keywords, page count) from a file on disk.
#[uniffi::export]
pub fn uniffi_extract_pdf_metadata(file_path: String) -> Result<UniFFIPdfMetadata, TTZipError> {
    let bytes = read_file_bytes(&file_path)?;
    parse_pdf_metadata_from_slice(&bytes)
}

/// Extracts the hierarchical bookmark and outline tree from a PDF file on disk.
#[uniffi::export]
pub fn uniffi_extract_pdf_outline(file_path: String) -> Result<Vec<UniFFIPdfOutlineNode>, TTZipError> {
    let bytes = read_file_bytes(&file_path)?;
    parse_pdf_outline_from_slice(&bytes)
}

/// Extracts plain text from a specific 1-based page number of a PDF file on disk.
#[uniffi::export]
pub fn uniffi_extract_pdf_page_text(
    file_path: String,
    page_number: u32,
) -> Result<UniFFIPdfPageText, TTZipError> {
    let bytes = read_file_bytes(&file_path)?;
    extract_pdf_page_text_from_slice(&bytes, page_number)
}

/// Searches for full-text occurrences of a query string across all pages of a PDF file on disk.
#[uniffi::export]
pub fn uniffi_search_pdf_text(
    file_path: String,
    query: String,
    max_results: u32,
    case_sensitive: bool,
) -> Result<Vec<UniFFIPdfSearchResult>, TTZipError> {
    let bytes = read_file_bytes(&file_path)?;
    search_pdf_text_from_slice(&bytes, &query, max_results, case_sensitive)
}

// ============================================================================
// Internal Helpers
// ============================================================================

pub(crate) fn read_file_bytes(path_str: &str) -> Result<Vec<u8>, TTZipError> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(TTZipError::FileNotFound {
            path: path_str.to_string(),
        });
    }
    let file = File::open(path).map_err(|e| TTZipError::IoError {
        message: format!("Failed to open file '{path_str}': {e}"),
    })?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(|e| TTZipError::IoError {
        message: format!("Failed to memory map file '{path_str}': {e}"),
    })?;
    Ok(mmap.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Document, Object, Stream};

    fn create_test_pdf_with_outline() -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let resources_id = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });

        // Page 1
        let content1 = Stream::new(dictionary! {}, b"BT /F1 18 Tf 50 750 Td (Introduction to TTZip High-Performance Archiving) Tj ET".to_vec());
        let content1_id = doc.add_object(content1);
        let page1_id = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content1_id, "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        // Page 2
        let content2 = Stream::new(dictionary! {}, b"BT /F1 18 Tf 50 750 Td (Architecture and Microkernel Stream Design) Tj ET".to_vec());
        let content2_id = doc.add_object(content2);
        let page2_id = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content2_id, "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        // Page 3
        let content3 = Stream::new(dictionary! {}, b"BT /F1 18 Tf 50 750 Td (Performance Benchmarks and Throughput) Tj ET".to_vec());
        let content3_id = doc.add_object(content3);
        let page3_id = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content3_id, "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page1_id.into(), page2_id.into(), page3_id.into()],
            "Count" => 3,
        }));

        // Outlines
        let outlines_id = doc.new_object_id();
        let item1_id = doc.new_object_id();
        let item2_id = doc.new_object_id();
        let sub_item_id = doc.new_object_id();

        // Outline item 1 -> Page 1
        doc.objects.insert(item1_id, Object::Dictionary(dictionary! {
            "Title" => Object::string_literal("1. Introduction"),
            "Parent" => outlines_id,
            "Next" => item2_id,
            "Dest" => vec![page1_id.into(), Object::Name(b"XYZ".to_vec()), 0.into(), 0.into(), 0.into()],
            "Count" => 0,
        }));

        // Outline sub-item -> Page 2
        doc.objects.insert(sub_item_id, Object::Dictionary(dictionary! {
            "Title" => Object::string_literal("2.1 Microkernel Details"),
            "Parent" => item2_id,
            "Dest" => vec![page2_id.into(), Object::Name(b"XYZ".to_vec()), 0.into(), 0.into(), 0.into()],
            "Count" => 0,
        }));

        // Outline item 2 -> Page 2 with child
        doc.objects.insert(item2_id, Object::Dictionary(dictionary! {
            "Title" => Object::string_literal("2. Architecture"),
            "Parent" => outlines_id,
            "Prev" => item1_id,
            "First" => sub_item_id,
            "Last" => sub_item_id,
            "Dest" => vec![page2_id.into(), Object::Name(b"XYZ".to_vec()), 0.into(), 0.into(), 0.into()],
            "Count" => 1,
        }));

        // Root outlines
        doc.objects.insert(outlines_id, Object::Dictionary(dictionary! {
            "Type" => "Outlines",
            "First" => item1_id,
            "Last" => item2_id,
            "Count" => 2,
        }));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "Outlines" => outlines_id,
        });
        doc.trailer.set("Root", catalog_id);

        let info_id = doc.add_object(dictionary! {
            "Title" => Object::string_literal("TTZip Engine Whitepaper"),
            "Author" => Object::string_literal("Witt Kung"),
            "Subject" => Object::string_literal("High-Performance Document Processing"),
            "Keywords" => Object::string_literal("Rust, UniFFI, Swift 6, Compression"),
            "Creator" => Object::string_literal("TTZip Automated Spec Engine"),
            "Producer" => Object::string_literal("lopdf 0.34"),
            "CustomKey" => Object::string_literal("CustomValue123"),
        });
        doc.trailer.set("Info", info_id);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn test_pdf_metadata_extraction() {
        let bytes = create_test_pdf_with_outline();
        let meta = parse_pdf_metadata_from_slice(&bytes).expect("Parse metadata failed");

        assert_eq!(meta.format_version, "PDF-1.7");
        assert_eq!(meta.page_count, 3);
        assert_eq!(meta.title.as_deref(), Some("TTZip Engine Whitepaper"));
        assert_eq!(meta.author.as_deref(), Some("Witt Kung"));
        assert_eq!(meta.subject.as_deref(), Some("High-Performance Document Processing"));
        assert_eq!(meta.keywords.as_deref(), Some("Rust, UniFFI, Swift 6, Compression"));
        assert_eq!(meta.creator.as_deref(), Some("TTZip Automated Spec Engine"));
        assert_eq!(meta.producer.as_deref(), Some("lopdf 0.34"));
        assert!(!meta.is_encrypted);
        assert!(meta.has_outline);
        assert_eq!(meta.custom_properties.get("CustomKey").map(|s| s.as_str()), Some("CustomValue123"));
    }

    #[test]
    fn test_pdf_outline_tree_extraction() {
        let bytes = create_test_pdf_with_outline();
        let outline = parse_pdf_outline_from_slice(&bytes).expect("Parse outline failed");

        assert_eq!(outline.len(), 2);
        assert_eq!(outline[0].title, "1. Introduction");
        assert_eq!(outline[0].page_number, 1);
        assert_eq!(outline[0].children.len(), 0);

        assert_eq!(outline[1].title, "2. Architecture");
        assert_eq!(outline[1].page_number, 2);
        assert_eq!(outline[1].children.len(), 1);
        assert_eq!(outline[1].children[0].title, "2.1 Microkernel Details");
        assert_eq!(outline[1].children[0].page_number, 2);
    }

    #[test]
    fn test_pdf_page_text_and_search() {
        let bytes = create_test_pdf_with_outline();

        // 1. Page text
        let p1 = extract_pdf_page_text_from_slice(&bytes, 1).expect("Page 1 failed");
        assert_eq!(p1.page_number, 1);
        assert!(p1.text.contains("Introduction to TTZip"));
        assert!(p1.character_count > 0);
        assert!(p1.word_count > 0);

        let p2 = extract_pdf_page_text_from_slice(&bytes, 2).expect("Page 2 failed");
        assert_eq!(p2.page_number, 2);
        assert!(p2.text.contains("Microkernel Stream Design"));

        // Out of bounds page
        assert!(extract_pdf_page_text_from_slice(&bytes, 99).is_err());

        // 2. All pages text
        let all_pages = extract_all_pages_text_from_slice(&bytes, None).expect("All pages failed");
        assert_eq!(all_pages.len(), 3);

        // 3. Search case insensitive
        let search_results = search_pdf_text_from_slice(&bytes, "microkernel", 10, false)
            .expect("Search failed");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].page_number, 2);
        assert!(search_results[0].match_text.contains("Microkernel"));

        // Search case sensitive
        let case_res = search_pdf_text_from_slice(&bytes, "microkernel", 10, true)
            .expect("Search failed");
        assert_eq!(case_res.len(), 0);

        let case_res2 = search_pdf_text_from_slice(&bytes, "Microkernel", 10, true)
            .expect("Search failed");
        assert_eq!(case_res2.len(), 1);
    }

    #[test]
    fn test_pdf_service_wrapper() {
        let bytes = create_test_pdf_with_outline();
        let service = UniFFIPdfService::new();

        let meta = service.extract_metadata_from_bytes(bytes.clone()).unwrap();
        assert_eq!(meta.page_count, 3);

        let outline = service.extract_outline_from_bytes(bytes.clone()).unwrap();
        assert_eq!(outline.len(), 2);

        let page1 = service.extract_page_text_from_bytes(bytes.clone(), 1).unwrap();
        assert!(page1.text.contains("TTZip"));

        let results = service.search_text_from_bytes(bytes, "TTZip".to_string(), 5, false).unwrap();
        assert!(!results.is_empty());
    }
}
