// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Comic Book ZIP (CBZ) Archive Scanning, Natural Ordering, Virtual Spine, and Cover Art.

use std::collections::HashMap;
use std::path::Path;

use super::helpers::{
    extract_resource_from_zip, find_and_extract_entry, guess_mime_type, is_image_path,
};
use super::types::{
    UniFFIEbookChapter, UniFFIEbookError, UniFFIEbookFormat, UniFFIEbookMetadata,
    UniFFIEbookResource, UniFFIEbookSpineItem, UniFFIEbookTocNode,
};
use crate::zip::reader::ZipArchive;

pub(crate) fn parse_cbz_metadata(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIEbookMetadata, UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;

    let mut images: Vec<String> = zip
        .entries()
        .iter()
        .filter(|e| is_image_path(&e.rel_path))
        .map(|e| e.rel_path.clone())
        .collect();
    images.sort();

    let title = file_name
        .map(|f| {
            Path::new(f)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Comic Book")
        })
        .unwrap_or("Comic Book")
        .to_string();

    let cover_path = images.first().cloned();
    let has_cover = cover_path.is_some();
    let count = images.len() as u32;

    Ok(UniFFIEbookMetadata {
        title,
        authors: Vec::new(),
        publisher: None,
        language: None,
        identifier: None,
        description: Some("Comic Book Archive".to_string()),
        publication_date: None,
        modified_date: None,
        rights: None,
        format: UniFFIEbookFormat::Cbz,
        total_chapters: count,
        total_resources: count,
        file_size_bytes: data.len() as u64,
        has_cover,
        cover_path,
        extra_metadata: HashMap::new(),
    })
}

pub(crate) fn parse_cbz_toc(data: &[u8]) -> Result<Vec<UniFFIEbookTocNode>, UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;

    let mut images: Vec<String> = zip
        .entries()
        .iter()
        .filter(|e| is_image_path(&e.rel_path))
        .map(|e| e.rel_path.clone())
        .collect();
    images.sort();

    Ok(images
        .into_iter()
        .enumerate()
        .map(|(idx, href)| {
            let order = (idx + 1) as u32;
            UniFFIEbookTocNode {
                id: format!("page_{order}"),
                title: format!("Page {order}"),
                href,
                play_order: order,
                level: 0,
                children: Vec::new(),
            }
        })
        .collect())
}

pub(crate) fn parse_cbz_spine(data: &[u8]) -> Result<Vec<UniFFIEbookSpineItem>, UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;

    let mut images: Vec<String> = zip
        .entries()
        .iter()
        .filter(|e| is_image_path(&e.rel_path))
        .map(|e| e.rel_path.clone())
        .collect();
    images.sort();

    Ok(images
        .into_iter()
        .enumerate()
        .map(|(idx, href)| {
            let order = (idx + 1) as u32;
            let mime = guess_mime_type(&href);
            UniFFIEbookSpineItem {
                id: format!("page_{order}"),
                href,
                media_type: mime,
                play_order: order,
                is_linear: true,
            }
        })
        .collect())
}

pub(crate) fn extract_cbz_chapter(
    data: &[u8],
    href: &str,
) -> Result<UniFFIEbookChapter, UniFFIEbookError> {
    let zip = ZipArchive::open_slice(data)
        .map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;
    let clean = href.split('#').next().unwrap_or(href);
    let bytes = find_and_extract_entry(&zip, clean)
        .ok_or_else(|| UniFFIEbookError::not_found(clean))?;

    let html_content = format!(
        "<!DOCTYPE html><html><body><img src=\"{}\" alt=\"Comic Page\"/></body></html>",
        clean
    );

    Ok(UniFFIEbookChapter {
        id: clean.to_string(),
        title: Path::new(clean)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Page")
            .to_string(),
        href: clean.to_string(),
        media_type: "application/xhtml+xml".to_string(),
        play_order: 1,
        content_string: html_content,
        character_count: bytes.len() as u32,
        word_count: 1,
    })
}

pub(crate) fn extract_cbz_cover(
    data: &[u8],
) -> Result<Option<UniFFIEbookResource>, UniFFIEbookError> {
    let meta = parse_cbz_metadata(data, None)?;
    if let Some(path) = meta.cover_path {
        Ok(Some(extract_resource_from_zip(data, &path)?))
    } else {
        Ok(None)
    }
}
