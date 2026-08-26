// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Document In-Memory Streaming and Introspection Layer.
//!
//! Provides zero-disk-landing in-memory parsing, metadata extraction, and text streaming
//! for DOCX, EPUB, and PDF documents.

use std::fs::File;
use std::path::Path;

use super::types::TTZipError;
use crate::standards::document_stream::{
    extract_epub_chapter_text, parse_docx_from_memory, parse_epub_from_memory,
    parse_pdf_from_memory, DocxCoreProperties, DocxDocument, DocumentStreamError,
    EpubBook, EpubChapter, EpubCover, EpubMetadata, PdfDocumentInfo,
};

// ============================================================================
// 1. DOCX UniFFI Records
// ============================================================================

/// Metadata properties extracted from DOCX document.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIDocxProperties {
    pub title: Option<String>,
    pub creator: Option<String>,
    pub description: Option<String>,
    pub last_modified_by: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub revision: Option<String>,
    pub word_count: u32,
    pub character_count: u32,
    pub paragraph_count: u32,
}

impl From<DocxCoreProperties> for UniFFIDocxProperties {
    fn from(p: DocxCoreProperties) -> Self {
        Self {
            title: p.title,
            creator: p.creator,
            description: p.description,
            last_modified_by: p.last_modified_by,
            created: p.created,
            modified: p.modified,
            revision: p.revision,
            word_count: p.word_count,
            character_count: p.character_count,
            paragraph_count: p.paragraph_count,
        }
    }
}

/// Extracted DOCX plain text, paragraph list, and metadata.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIDocxExtractResult {
    pub full_text: String,
    pub paragraphs: Vec<String>,
    pub properties: UniFFIDocxProperties,
}

impl From<DocxDocument> for UniFFIDocxExtractResult {
    fn from(d: DocxDocument) -> Self {
        Self {
            full_text: d.full_text,
            paragraphs: d.paragraphs,
            properties: d.properties.into(),
        }
    }
}

// ============================================================================
// 2. EPUB UniFFI Records
// ============================================================================

/// Metadata record of an EPUB book.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIEpubMetadata {
    pub title: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub publication_date: Option<String>,
    pub modified_date: Option<String>,
    pub rights: Option<String>,
}

impl From<EpubMetadata> for UniFFIEpubMetadata {
    fn from(m: EpubMetadata) -> Self {
        Self {
            title: m.title,
            authors: m.authors,
            publisher: m.publisher,
            language: m.language,
            identifier: m.identifier,
            description: m.description,
            publication_date: m.publication_date,
            modified_date: m.modified_date,
            rights: m.rights,
        }
    }
}

/// A chapter in an EPUB book exposed to UniFFI.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIEpubChapterItem {
    pub id: String,
    pub title: String,
    pub href: String,
    pub media_type: String,
    pub play_order: u32,
}

impl From<EpubChapter> for UniFFIEpubChapterItem {
    fn from(c: EpubChapter) -> Self {
        Self {
            id: c.id,
            title: c.title,
            href: c.href,
            media_type: c.media_type,
            play_order: c.play_order,
        }
    }
}

/// Raw cover image data and MIME type extracted in memory.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIEpubCoverData {
    pub file_path: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

impl From<EpubCover> for UniFFIEpubCoverData {
    fn from(c: EpubCover) -> Self {
        Self {
            file_path: c.file_path,
            mime_type: c.mime_type,
            data: c.data,
        }
    }
}

/// Comprehensive EPUB parse result containing metadata, chapters, and cover image.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIEpubParseResult {
    pub metadata: UniFFIEpubMetadata,
    pub chapters: Vec<UniFFIEpubChapterItem>,
    pub cover: Option<UniFFIEpubCoverData>,
    pub total_chapters: u32,
    pub manifest_items_count: u32,
}

impl From<EpubBook> for UniFFIEpubParseResult {
    fn from(b: EpubBook) -> Self {
        Self {
            metadata: b.metadata.into(),
            chapters: b.chapters.into_iter().map(Into::into).collect(),
            cover: b.cover.map(Into::into),
            total_chapters: b.total_chapters,
            manifest_items_count: b.manifest_items_count,
        }
    }
}

// ============================================================================
// 3. PDF UniFFI Records
// ============================================================================

/// Metadata and extracted content of a PDF document exposed to Swift.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIPdfDocumentInfo {
    pub format_version: String,
    pub page_count: u32,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
    pub is_encrypted: bool,
    pub extracted_text: Option<String>,
    pub extracted_page_count: u32,
}

impl From<PdfDocumentInfo> for UniFFIPdfDocumentInfo {
    fn from(p: PdfDocumentInfo) -> Self {
        Self {
            format_version: p.format_version,
            page_count: p.page_count,
            title: p.title,
            author: p.author,
            subject: p.subject,
            keywords: p.keywords,
            creator: p.creator,
            producer: p.producer,
            creation_date: p.creation_date,
            modification_date: p.modification_date,
            is_encrypted: p.is_encrypted,
            extracted_text: p.extracted_text,
            extracted_page_count: p.extracted_page_count,
        }
    }
}

// ============================================================================
// 4. Error Mapping Helper
// ============================================================================

fn map_doc_error(err: DocumentStreamError) -> TTZipError {
    match err {
        DocumentStreamError::ZipError(msg) => TTZipError::CorruptHeader {
            details: msg,
            offset: 0,
        },
        DocumentStreamError::EntryNotFound(path) => TTZipError::FileNotFound { path },
        DocumentStreamError::XmlError(msg) => TTZipError::IoError {
            message: format!("XML Parse Error: {msg}"),
        },
        DocumentStreamError::PdfError(msg) => TTZipError::IoError {
            message: format!("PDF Parse Error: {msg}"),
        },
        DocumentStreamError::DecodeError(msg) => TTZipError::IoError {
            message: format!("Document Decode Error: {msg}"),
        },
        DocumentStreamError::UnsupportedFormat(msg) => TTZipError::IoError {
            message: format!("Unsupported Document Format: {msg}"),
        },
    }
}

fn read_file_bytes(path_str: &str) -> Result<Vec<u8>, TTZipError> {
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
        message: format!("Failed to mmap file '{path_str}': {e}"),
    })?;
    Ok(mmap.to_vec())
}

// ============================================================================
// 5. UniFFI Export Functions
// ============================================================================

/// Extracts DOCX full text, paragraph structure, and core metadata directly from in-memory byte buffer.
#[uniffi::export]
pub fn extract_docx_text_from_memory(
    docx_bytes: Vec<u8>,
) -> Result<UniFFIDocxExtractResult, TTZipError> {
    parse_docx_from_memory(&docx_bytes)
        .map(Into::into)
        .map_err(map_doc_error)
}

/// Convenience function to extract plain text string from DOCX in memory.
#[uniffi::export]
pub fn extract_docx_full_text(docx_bytes: Vec<u8>) -> Result<String, TTZipError> {
    parse_docx_from_memory(&docx_bytes)
        .map(|doc| doc.full_text)
        .map_err(map_doc_error)
}

/// Extracts DOCX document content from a file path using zero-copy memory mapping.
#[uniffi::export]
pub fn extract_docx_text_from_file(docx_path: String) -> Result<UniFFIDocxExtractResult, TTZipError> {
    let bytes = read_file_bytes(&docx_path)?;
    extract_docx_text_from_memory(bytes)
}

/// Parses EPUB book metadata, spine chapters, TOC, and in-memory cover image bytes directly from memory.
#[uniffi::export]
pub fn parse_epub_book_from_memory(epub_bytes: Vec<u8>) -> Result<UniFFIEpubParseResult, TTZipError> {
    parse_epub_from_memory(&epub_bytes)
        .map(Into::into)
        .map_err(map_doc_error)
}

/// Parses EPUB book metadata, chapters, and cover image from a file on disk using memory mapping.
#[uniffi::export]
pub fn parse_epub_book_from_file(epub_path: String) -> Result<UniFFIEpubParseResult, TTZipError> {
    let bytes = read_file_bytes(&epub_path)?;
    parse_epub_book_from_memory(bytes)
}

/// Extracts clean plain text of a specific EPUB chapter directly from memory without disk extraction.
#[uniffi::export]
pub fn extract_epub_chapter_text_from_memory(
    epub_bytes: Vec<u8>,
    chapter_href: String,
) -> Result<String, TTZipError> {
    extract_epub_chapter_text(&epub_bytes, &chapter_href).map_err(map_doc_error)
}

/// Extracts PDF metadata (title, author, creation date, etc.) and page text stream directly from memory.
#[uniffi::export]
pub fn extract_pdf_info_from_memory(
    pdf_bytes: Vec<u8>,
    max_pages_text: Option<u32>,
) -> Result<UniFFIPdfDocumentInfo, TTZipError> {
    parse_pdf_from_memory(&pdf_bytes, max_pages_text)
        .map(Into::into)
        .map_err(map_doc_error)
}

/// Extracts PDF metadata and page text from a file on disk using memory mapping.
#[uniffi::export]
pub fn extract_pdf_info_from_file(
    pdf_path: String,
    max_pages_text: Option<u32>,
) -> Result<UniFFIPdfDocumentInfo, TTZipError> {
    let bytes = read_file_bytes(&pdf_path)?;
    extract_pdf_info_from_memory(bytes, max_pages_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

    fn build_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let items: Vec<ZipInputItem> = files
            .iter()
            .map(|(name, content)| ZipInputItem {
                rel_path: name.to_string(),
                data: content.to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            })
            .collect();
        let compressed = compress_items_parallel(
            items,
            6,
            crate::types::TTZipEncryptionMethod::None,
            None,
            1,
        )
        .unwrap();
        assemble_zip_archive(&compressed).unwrap()
    }

    #[test]
    fn test_uniffi_docx_exports() {
        let doc_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>UniFFI DOCX Export Line 1</w:t></w:r></w:p>
    <w:p><w:r><w:t>UniFFI DOCX Export Line 2</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let docx_bytes = build_test_zip(&[("word/document.xml", doc_xml)]);

        let res = extract_docx_text_from_memory(docx_bytes.clone()).expect("UniFFI docx failed");
        assert_eq!(res.paragraphs.len(), 2);
        assert_eq!(res.paragraphs[0], "UniFFI DOCX Export Line 1");

        let full_text = extract_docx_full_text(docx_bytes).expect("Full text failed");
        assert!(full_text.contains("UniFFI DOCX Export Line 1"));
        assert!(full_text.contains("UniFFI DOCX Export Line 2"));
    }

    #[test]
    fn test_uniffi_epub_and_pdf_exports() {
        let container_xml = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;

        let opf_xml = br#"<?xml version="1.0" encoding="utf-8"?>
<package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>UniFFI EPUB Test</dc:title>
    <dc:creator>Witt Kung</dc:creator>
  </metadata>
  <manifest>
    <item id="ch1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"#;

        let ch1 = b"<html><body><p>Chapter Content</p></body></html>";

        let epub_bytes = build_test_zip(&[
            ("META-INF/container.xml", container_xml),
            ("OEBPS/content.opf", opf_xml),
            ("OEBPS/text/ch1.xhtml", ch1),
        ]);

        let parsed_epub = parse_epub_book_from_memory(epub_bytes.clone()).expect("parse epub failed");
        assert_eq!(parsed_epub.metadata.title, "UniFFI EPUB Test");
        assert_eq!(parsed_epub.chapters.len(), 1);

        let ch_text = extract_epub_chapter_text_from_memory(epub_bytes, "OEBPS/text/ch1.xhtml".to_string()).unwrap();
        assert_eq!(ch_text, "Chapter Content");
    }
}
