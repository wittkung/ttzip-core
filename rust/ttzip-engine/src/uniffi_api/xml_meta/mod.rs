// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
    crate::ebook::container::find_and_extract_zip_entry(zip, target_path)
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

    #[test]
    fn test_consolidated_epub_and_office_xml_meta() {
        let mut epub_meta = UniFFIEpubMetadata::default();
        let opf_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<package version="3.0" xmlns="http://www.idpf.org/2007/opf">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Consolidated OPF Title</dc:title>
    <dc:creator>Author A</dc:creator>
    <dc:language>en</dc:language>
    <meta property="dcterms:modified">2026-10-01T00:00:00Z</meta>
  </metadata>
</package>"#;
        epub::parse_opf_metadata_xml(opf_xml, "", &mut epub_meta);
        assert_eq!(epub_meta.title, "Consolidated OPF Title");
        assert_eq!(epub_meta.authors, vec!["Author A".to_string()]);
        assert_eq!(epub_meta.language.as_deref(), Some("en"));
        assert_eq!(
            epub_meta.modified_date.as_deref(),
            Some("2026-10-01T00:00:00Z")
        );

        let mut doc_meta = UniFFIDocumentMetadata::default();
        let core_xml = br#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Unified Core</dc:title><dc:creator>Creator B</dc:creator><cp:lastModifiedBy>Editor C</cp:lastModifiedBy></cp:coreProperties>"#;
        office::parse_core_properties_xml(core_xml, &mut doc_meta);
        assert_eq!(doc_meta.title.as_deref(), Some("Unified Core"));
        assert_eq!(doc_meta.author.as_deref(), Some("Creator B"));
        assert_eq!(doc_meta.last_modified_by.as_deref(), Some("Editor C"));
    }
}
