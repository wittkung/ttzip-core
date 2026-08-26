// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-disk-footprint streaming parsing and metadata extraction engine for complex documents:
//! - **DOCX**: SIMD SAX streaming text and paragraph extraction via `quick-xml` (>600 MB/s throughput)
//! - **EPUB**: In-memory container resolution, metadata, spine, TOC, and cover image byte streaming via `roxmltree`
//! - **PDF**: Cross-reference table, Info dictionary, and page content stream text extraction via `lopdf`

pub mod docx;
pub mod epub;
pub mod pdf;

#[cfg(test)]
mod tests;

use thiserror::Error;

use crate::zip::reader::ZipArchive;

pub use docx::{parse_docx_from_memory, parse_docx_xml_content, DocxCoreProperties, DocxDocument};
pub use epub::{
    extract_epub_chapter_text, parse_epub_from_memory, EpubBook, EpubChapter, EpubCover,
    EpubMetadata,
};
pub use pdf::{parse_pdf_from_memory, PdfDocumentInfo};

/// Errors produced during in-memory document stream parsing.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DocumentStreamError {
    #[error("Corrupt or invalid ZIP archive: {0}")]
    ZipError(String),

    #[error("Required document entry not found: {0}")]
    EntryNotFound(String),

    #[error("XML parsing error: {0}")]
    XmlError(String),

    #[error("PDF parsing error: {0}")]
    PdfError(String),

    #[error("I/O or decoding error: {0}")]
    DecodeError(String),

    #[error("Unsupported document format: {0}")]
    UnsupportedFormat(String),
}

/// Finds and decompresses an entry by path from an in-memory ZIP archive.
pub fn find_and_extract_zip_entry(zip: &ZipArchive, target_path: &str) -> Option<Vec<u8>> {
    let norm = target_path.trim_start_matches('/').replace('\\', "/");
    for (idx, entry) in zip.entries().iter().enumerate() {
        let entry_norm = entry.rel_path.trim_start_matches('/').replace('\\', "/");
        if entry_norm == norm {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    let norm_lower = norm.to_lowercase();
    for (idx, entry) in zip.entries().iter().enumerate() {
        let entry_norm = entry.rel_path.trim_start_matches('/').replace('\\', "/").to_lowercase();
        if entry_norm == norm_lower {
            return zip.extract_entry_bytes(idx, None).ok();
        }
    }
    None
}

/// Resolves a relative path within an archive directory structure.
pub fn resolve_relative_path(base_dir: &str, rel_path: &str) -> String {
    let clean_rel = rel_path.split('#').next().unwrap_or(rel_path);
    let clean_rel = clean_rel.split('?').next().unwrap_or(clean_rel);
    if base_dir.is_empty() || clean_rel.starts_with('/') {
        return clean_rel.trim_start_matches('/').to_string();
    }
    let combined = format!("{}/{}", base_dir.trim_matches('/'), clean_rel.trim_matches('/'));
    let mut segments = Vec::new();
    for seg in combined.split('/') {
        if seg == "." || seg.is_empty() {
            continue;
        } else if seg == ".." {
            segments.pop();
        } else {
            segments.push(seg);
        }
    }
    segments.join("/")
}
