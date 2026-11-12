// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! EPUB Dublin Core publication metadata extraction for UniFFI bindings.

use crate::ebook::container::{
    find_and_extract_zip_entry, opf_parent_dir, resolve_epub_opf_path,
};
use crate::ebook::opf::parse_opf_xml;
use crate::uniffi_api::types::TTZipError;
use crate::zip::reader::ZipArchive;
use super::types::UniFFIEpubMetadata;

/// Parses EPUB Dublin Core publication metadata from byte slice.
pub fn parse_epub_metadata_from_slice(bytes: &[u8]) -> Result<UniFFIEpubMetadata, TTZipError> {
    let zip = ZipArchive::open_slice(bytes).map_err(|e| TTZipError::CorruptHeader {
        details: format!("Failed to open EPUB archive: {e:?}"),
        offset: 0,
    })?;

    let opf_path = resolve_epub_opf_path(&zip).ok_or_else(|| TTZipError::FileNotFound {
        path: "META-INF/container.xml or *.opf".to_string(),
    })?;

    let opf_bytes =
        find_and_extract_zip_entry(&zip, &opf_path).ok_or_else(|| TTZipError::FileNotFound {
            path: opf_path.clone(),
        })?;

    let mut meta = UniFFIEpubMetadata::default();
    parse_opf_metadata_xml(&opf_bytes, opf_parent_dir(&opf_path), &mut meta);

    if meta.title.is_empty() {
        meta.title = "Untitled Book".to_string();
    }

    Ok(meta)
}

pub(crate) fn parse_opf_metadata_xml(
    xml_bytes: &[u8],
    opf_dir: &str,
    meta: &mut UniFFIEpubMetadata,
) {
    let Ok(parsed) = parse_opf_xml(xml_bytes, opf_dir) else {
        return;
    };
    if let Some(title) = parsed.metadata.title {
        meta.title = title;
    }
    meta.authors = parsed.metadata.authors;
    meta.publisher = parsed.metadata.publisher;
    meta.language = parsed.metadata.language;
    meta.identifier = parsed.metadata.identifiers.into_iter().next();
    meta.description = parsed.metadata.description;
    meta.publication_date = parsed.metadata.publication_date;
    meta.modified_date = parsed.modified_date;
    meta.rights = parsed.metadata.rights;
}
