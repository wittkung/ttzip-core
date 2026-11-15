// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Shared Helper Functions for Ebook Parsing, Path Resolution, MIME Detection, and Markup Cleaning.

use std::fs::File;
use std::path::Path;

use super::types::{UniFFIEbookError, UniFFIEbookResource};
use crate::ebook::container::{
    extract_html_heading as canonical_extract_html_heading, find_and_extract_zip_entry,
    strip_html_tags as canonical_strip_html_tags,
};
use crate::zip::reader::ZipArchive;

/// Reads full bytes of a local file via memory mapping.
pub(crate) fn read_file_bytes(path_str: &str) -> Result<Vec<u8>, UniFFIEbookError> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(UniFFIEbookError::not_found(path_str));
    }
    let file = File::open(path).map_err(UniFFIEbookError::io_err)?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(UniFFIEbookError::io_err)?;
    Ok(mmap.to_vec())
}

/// Finds and extracts raw decompressed entry bytes from an in-memory ZIP archive.
pub(crate) fn find_and_extract_entry(zip: &ZipArchive<'_>, target: &str) -> Option<Vec<u8>> {
    find_and_extract_zip_entry(zip, target)
}

/// Extracts an embedded resource by href from an in-memory ZIP archive.
pub(crate) fn extract_resource_from_zip(
    data: &[u8],
    href: &str,
) -> Result<UniFFIEbookResource, UniFFIEbookError> {
    let clean = href.split('#').next().unwrap_or(href);
    let bytes = crate::ebook::preview::resource(data, href)?;

    let media_type = guess_mime_type(clean);
    let size_bytes = bytes.len() as u64;

    Ok(UniFFIEbookResource {
        href: clean.to_string(),
        media_type,
        data: bytes,
        size_bytes,
    })
}

/// Returns true if the relative path ends with a standard comic / graphic image extension.
pub(crate) fn is_image_path(p: &str) -> bool {
    let l = p.to_lowercase();
    l.ends_with(".jpg")
        || l.ends_with(".jpeg")
        || l.ends_with(".png")
        || l.ends_with(".webp")
        || l.ends_with(".gif")
}

const MIME_BY_EXTENSION: &[(&str, &str)] = &[
    (".jpg", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".png", "image/png"),
    (".webp", "image/webp"),
    (".gif", "image/gif"),
    (".svg", "image/svg+xml"),
    (".css", "text/css"),
    (".xhtml", "application/xhtml+xml"),
    (".html", "application/xhtml+xml"),
    (".htm", "application/xhtml+xml"),
    (".ncx", "application/x-dtbncx+xml"),
    (".opf", "application/oebps-package+xml"),
    (".woff2", "font/woff2"),
    (".woff", "font/woff"),
    (".ttf", "font/ttf"),
];

/// Heuristically determines the MIME type from a file path extension.
pub(crate) fn guess_mime_type(path: &str) -> String {
    let l = path.to_lowercase();
    MIME_BY_EXTENSION
        .iter()
        .find_map(|&(ext, mime)| l.ends_with(ext).then_some(mime))
        .unwrap_or("application/octet-stream")
        .to_string()
}

/// Extracts primary heading or title string from raw HTML/XHTML content.
pub(crate) fn extract_html_heading(html: &str) -> Option<String> {
    canonical_extract_html_heading(html)
}

/// Strips markup tags and unescapes standard XML/HTML entities.
pub(crate) fn strip_html_tags(s: &str) -> String {
    canonical_strip_html_tags(s)
}
