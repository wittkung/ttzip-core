// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Service and Pipeline Implementations for Ebook Introspection,
//! Metadata Extraction, Hierarchical TOC Navigation, and Streaming Chapter Reading.

use std::path::Path;
use std::sync::Arc;

use super::cbz::{
    extract_cbz_chapter, extract_cbz_cover, parse_cbz_metadata, parse_cbz_spine, parse_cbz_toc,
};
use super::epub::{
    extract_epub_chapter, extract_epub_cover, parse_epub_metadata, parse_epub_spine,
    parse_epub_toc,
};
use super::helpers::{extract_resource_from_zip, is_image_path, read_file_bytes};
use super::types::{
    UniFFIEbookChapter, UniFFIEbookError, UniFFIEbookFormat, UniFFIEbookMetadata,
    UniFFIEbookResource, UniFFIEbookSpineItem, UniFFIEbookTocNode,
};
use crate::zip::reader::ZipArchive;

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Probes and identifies the ebook container format directly from in-memory bytes.
#[uniffi::export]
pub fn uniffi_probe_ebook_bytes(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIEbookFormat, UniFFIEbookError> {
    probe_format_internal(&data, file_name.as_deref())
}

/// Extracts publication metadata and cover presence from in-memory ebook bytes.
#[uniffi::export]
pub fn uniffi_extract_ebook_metadata(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIEbookMetadata, UniFFIEbookError> {
    let format = probe_format_internal(&data, file_name.as_deref())?;
    match format {
        UniFFIEbookFormat::Epub => parse_epub_metadata(&data, file_name.as_deref()),
        UniFFIEbookFormat::Cbz => parse_cbz_metadata(&data, file_name.as_deref()),
        _ => Err(UniFFIEbookError::UnsupportedFormat {
            format: format!("{format:?}"),
        }),
    }
}

/// Extracts the hierarchical Table of Contents (TOC) tree from in-memory ebook bytes.
#[uniffi::export]
pub fn uniffi_extract_ebook_toc(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<Vec<UniFFIEbookTocNode>, UniFFIEbookError> {
    let format = probe_format_internal(&data, file_name.as_deref())?;
    match format {
        UniFFIEbookFormat::Epub => parse_epub_toc(&data),
        UniFFIEbookFormat::Cbz => parse_cbz_toc(&data),
        _ => Err(UniFFIEbookError::UnsupportedFormat {
            format: format!("{format:?}"),
        }),
    }
}

/// Returns the ordered reading spine items from in-memory ebook bytes.
#[uniffi::export]
pub fn uniffi_get_ebook_spine(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<Vec<UniFFIEbookSpineItem>, UniFFIEbookError> {
    let format = probe_format_internal(&data, file_name.as_deref())?;
    match format {
        UniFFIEbookFormat::Epub => parse_epub_spine(&data),
        UniFFIEbookFormat::Cbz => parse_cbz_spine(&data),
        _ => Err(UniFFIEbookError::UnsupportedFormat {
            format: format!("{format:?}"),
        }),
    }
}

/// Extracts an embedded asset resource (image, stylesheet, font) by href path.
#[uniffi::export]
pub fn uniffi_extract_ebook_resource(
    data: Vec<u8>,
    href: String,
    file_name: Option<String>,
) -> Result<UniFFIEbookResource, UniFFIEbookError> {
    let format = probe_format_internal(&data, file_name.as_deref())?;
    match format {
        UniFFIEbookFormat::Epub | UniFFIEbookFormat::Cbz => extract_resource_from_zip(&data, &href),
        _ => Err(UniFFIEbookError::UnsupportedFormat {
            format: format!("{format:?}"),
        }),
    }
}

/// Extracts and parses a single chapter document into structured XHTML content and word metrics.
#[uniffi::export]
pub fn uniffi_extract_ebook_chapter(
    data: Vec<u8>,
    href: String,
    file_name: Option<String>,
) -> Result<UniFFIEbookChapter, UniFFIEbookError> {
    let format = probe_format_internal(&data, file_name.as_deref())?;
    match format {
        UniFFIEbookFormat::Epub => extract_epub_chapter(&data, &href),
        UniFFIEbookFormat::Cbz => extract_cbz_chapter(&data, &href),
        _ => Err(UniFFIEbookError::UnsupportedFormat {
            format: format!("{format:?}"),
        }),
    }
}

/// Extracts the embedded cover artwork resource if available.
#[uniffi::export]
pub fn uniffi_extract_ebook_cover(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<Option<UniFFIEbookResource>, UniFFIEbookError> {
    let format = probe_format_internal(&data, file_name.as_deref())?;
    match format {
        UniFFIEbookFormat::Epub => extract_epub_cover(&data),
        UniFFIEbookFormat::Cbz => extract_cbz_cover(&data),
        _ => Ok(None),
    }
}

// ============================================================================
// Stateful UniFFI Service Object
// ============================================================================

/// High-performance thread-safe Ebook Introspection Service.
#[derive(uniffi::Object, Default)]
pub struct UniFFIEbookService {}

#[uniffi::export]
impl UniFFIEbookService {
    /// Constructs a new thread-safe ebook service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Probes the ebook format from an in-memory byte buffer.
    pub fn probe_bytes(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIEbookFormat, UniFFIEbookError> {
        uniffi_probe_ebook_bytes(data, file_name)
    }

    /// Probes the ebook format from a file on disk.
    pub fn probe_file(&self, file_path: String) -> Result<UniFFIEbookFormat, UniFFIEbookError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_probe_ebook_bytes(bytes, name)
    }

    /// Extracts publication metadata from an in-memory byte buffer.
    pub fn extract_metadata(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIEbookMetadata, UniFFIEbookError> {
        uniffi_extract_ebook_metadata(data, file_name)
    }

    /// Extracts publication metadata from a file on disk.
    pub fn extract_metadata_from_file(
        &self,
        file_path: String,
    ) -> Result<UniFFIEbookMetadata, UniFFIEbookError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_ebook_metadata(bytes, name)
    }

    /// Extracts hierarchical Table of Contents (TOC) from an in-memory byte buffer.
    pub fn extract_toc(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<Vec<UniFFIEbookTocNode>, UniFFIEbookError> {
        uniffi_extract_ebook_toc(data, file_name)
    }

    /// Extracts hierarchical Table of Contents (TOC) from a file on disk.
    pub fn extract_toc_from_file(
        &self,
        file_path: String,
    ) -> Result<Vec<UniFFIEbookTocNode>, UniFFIEbookError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_ebook_toc(bytes, name)
    }

    /// Retrieves ordered spine reading items from an in-memory byte buffer.
    pub fn get_spine(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<Vec<UniFFIEbookSpineItem>, UniFFIEbookError> {
        uniffi_get_ebook_spine(data, file_name)
    }

    /// Retrieves ordered spine reading items from a file on disk.
    pub fn get_spine_from_file(
        &self,
        file_path: String,
    ) -> Result<Vec<UniFFIEbookSpineItem>, UniFFIEbookError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_get_ebook_spine(bytes, name)
    }

    /// Extracts an embedded asset resource by href from an in-memory byte buffer.
    pub fn extract_resource(
        &self,
        data: Vec<u8>,
        href: String,
        file_name: Option<String>,
    ) -> Result<UniFFIEbookResource, UniFFIEbookError> {
        uniffi_extract_ebook_resource(data, href, file_name)
    }

    /// Extracts an embedded asset resource by href from a file on disk.
    pub fn extract_resource_from_file(
        &self,
        file_path: String,
        href: String,
    ) -> Result<UniFFIEbookResource, UniFFIEbookError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_ebook_resource(bytes, href, name)
    }

    /// Extracts and parses a single chapter document from an in-memory byte buffer.
    pub fn extract_chapter(
        &self,
        data: Vec<u8>,
        href: String,
        file_name: Option<String>,
    ) -> Result<UniFFIEbookChapter, UniFFIEbookError> {
        uniffi_extract_ebook_chapter(data, href, file_name)
    }

    /// Extracts and parses a single chapter document from a file on disk.
    pub fn extract_chapter_from_file(
        &self,
        file_path: String,
        href: String,
    ) -> Result<UniFFIEbookChapter, UniFFIEbookError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_ebook_chapter(bytes, href, name)
    }

    /// Extracts the embedded cover artwork from an in-memory byte buffer.
    pub fn extract_cover(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<Option<UniFFIEbookResource>, UniFFIEbookError> {
        uniffi_extract_ebook_cover(data, file_name)
    }

    /// Extracts the embedded cover artwork from a file on disk.
    pub fn extract_cover_from_file(
        &self,
        file_path: String,
    ) -> Result<Option<UniFFIEbookResource>, UniFFIEbookError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_ebook_cover(bytes, name)
    }
}

// ============================================================================
// Internal Probing Implementation
// ============================================================================

fn probe_format_internal(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIEbookFormat, UniFFIEbookError> {
    if data.len() < 4 {
        return Err(UniFFIEbookError::corrupted("Payload too small to probe"));
    }

    if data.starts_with(b"%PDF") {
        return Ok(UniFFIEbookFormat::Pdf);
    }
    if data.len() > 68 && &data[60..68] == b"BOOKMOBI" {
        return Ok(UniFFIEbookFormat::Mobi);
    }
    if data.starts_with(b"<?xml")
        && std::str::from_utf8(&data[..data.len().min(512)])
            .map(|s| s.contains("<FictionBook"))
            .unwrap_or(false)
    {
        return Ok(UniFFIEbookFormat::Fb2);
    }

    // ZIP container inspection
    if data.starts_with(b"PK\x03\x04")
        || data.starts_with(b"PK\x05\x06")
        || data.starts_with(b"PK\x07\x08")
    {
        let zip = ZipArchive::open_slice(data)
            .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;

        let has_epub_container = zip.entries().iter().any(|e| {
            let p = e.rel_path.to_lowercase();
            p == "meta-inf/container.xml" || p.ends_with(".opf")
        });
        if has_epub_container {
            return Ok(UniFFIEbookFormat::Epub);
        }

        let is_cbz_ext = file_name
            .map(|n| n.to_lowercase().ends_with(".cbz"))
            .unwrap_or(false);
        let has_images = zip.entries().iter().any(|e| is_image_path(&e.rel_path));
        if is_cbz_ext || has_images {
            return Ok(UniFFIEbookFormat::Cbz);
        }
    }

    if let Some(name) = file_name {
        let l = name.to_lowercase();
        if l.ends_with(".epub") {
            return Ok(UniFFIEbookFormat::Epub);
        }
        if l.ends_with(".cbz") {
            return Ok(UniFFIEbookFormat::Cbz);
        }
        if l.ends_with(".fb2") {
            return Ok(UniFFIEbookFormat::Fb2);
        }
        if l.ends_with(".mobi") {
            return Ok(UniFFIEbookFormat::Mobi);
        }
        if l.ends_with(".azw3") || l.ends_with(".azw") {
            return Ok(UniFFIEbookFormat::Azw3);
        }
    }

    Ok(UniFFIEbookFormat::Unknown)
}
