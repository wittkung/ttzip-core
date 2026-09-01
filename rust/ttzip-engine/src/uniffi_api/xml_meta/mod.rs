// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Export Layer for Document and XML Metadata Extraction.
//!
//! Provides zero-disk-landing in-memory parsing, Dublin Core introspection, Office outline
//! extraction (DOCX, XLSX, PPTX), EPUB container metadata, and Apple Property List (plist)
//! XML deserialization for Swift 6 UI inspector and QuickLook preview pipelines.

pub mod epub;
pub mod office;
pub mod plist;
pub mod types;

use std::fs::File;
use std::path::Path;

use crate::uniffi_api::types::TTZipError;
use crate::zip::reader::ZipArchive;

pub use epub::parse_epub_metadata_from_slice;
pub use office::{parse_office_metadata_from_slice, parse_office_outline_from_slice};
pub use plist::{parse_plist_xml_str, uniffi_parse_plist_from_bytes, uniffi_parse_plist_xml};
pub use types::{
    UniFFIDocumentMetadata, UniFFIEpubMetadata, UniFFIOfficeOutline, UniFFIPlistDictionary,
    UniFFIXmlMetaService,
};

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Extracts Office document metadata (DOCX, XLSX, PPTX) from a file on disk.
#[uniffi::export]
pub fn uniffi_extract_office_metadata(file_path: String) -> Result<UniFFIDocumentMetadata, TTZipError> {
    let bytes = read_file_bytes(&file_path)?;
    parse_office_metadata_from_slice(&bytes)
}

/// Extracts structural outline and preview text from an Office document on disk.
#[uniffi::export]
pub fn uniffi_extract_office_outline(file_path: String) -> Result<UniFFIOfficeOutline, TTZipError> {
    let bytes = read_file_bytes(&file_path)?;
    parse_office_outline_from_slice(&bytes)
}

/// Extracts EPUB Dublin Core publication metadata from a file on disk.
#[uniffi::export]
pub fn uniffi_extract_epub_metadata(file_path: String) -> Result<UniFFIEpubMetadata, TTZipError> {
    let bytes = read_file_bytes(&file_path)?;
    parse_epub_metadata_from_slice(&bytes)
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

pub(crate) fn find_and_extract_entry(zip: &ZipArchive, target_path: &str) -> Option<Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plist_xml_parsing() {
        let plist_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.ttzip.desktop</string>
    <key>CFBundleName</key>
    <string>TTZip</string>
    <key>CFBundleVersion</key>
    <string>100</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>CFBundleExecutable</key>
    <string>ttzip_exec</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>"#;

        let dict = parse_plist_xml_str(plist_xml).expect("Failed to parse plist");
        assert_eq!(dict.bundle_identifier.as_deref(), Some("com.ttzip.desktop"));
        assert_eq!(dict.bundle_name.as_deref(), Some("TTZip"));
        assert_eq!(dict.bundle_version.as_deref(), Some("100"));
        assert_eq!(dict.bundle_short_version.as_deref(), Some("1.0.0"));
        assert_eq!(dict.minimum_os_version.as_deref(), Some("14.0"));
        assert_eq!(dict.executable_name.as_deref(), Some("ttzip_exec"));
        assert_eq!(dict.entries.get("NSHighResolutionCapable").map(|s| s.as_str()), Some("true"));
    }
}
