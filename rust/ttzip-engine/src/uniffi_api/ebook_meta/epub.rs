// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! EPUB Standard Publication Container Resolution, OPF Manifest Parsing,
//! NCX/NAV Hierarchical TOC Extraction, Spine Ordering, and Chapter Reading.

use std::path::Path;

use super::helpers::{
    extract_html_heading, extract_resource_from_zip, find_and_extract_entry, strip_html_tags,
};
use super::types::{
    UniFFIEbookChapter, UniFFIEbookError, UniFFIEbookFormat, UniFFIEbookMetadata,
    UniFFIEbookResource, UniFFIEbookSpineItem, UniFFIEbookTocNode,
};
use crate::ebook::preview::{self, PreviewError, PreviewSpineItem, PreviewTocNode};
use crate::zip::reader::ZipArchive;

impl From<PreviewError> for UniFFIEbookError {
    fn from(error: PreviewError) -> Self {
        match error {
            PreviewError::Corrupt(message) => Self::corrupted(message),
            PreviewError::Missing(href) => Self::not_found(href),
            PreviewError::Xml(message) => Self::xml_err(message),
        }
    }
}

impl From<PreviewSpineItem> for UniFFIEbookSpineItem {
    fn from(item: PreviewSpineItem) -> Self {
        let PreviewSpineItem {
            id,
            href,
            media_type,
            play_order,
            is_linear,
        } = item;
        Self {
            id,
            href,
            media_type,
            play_order,
            is_linear,
        }
    }
}

impl From<PreviewTocNode> for UniFFIEbookTocNode {
    fn from(node: PreviewTocNode) -> Self {
        let PreviewTocNode {
            id,
            title,
            href,
            play_order,
            level,
            children,
        } = node;
        Self {
            id,
            title,
            href,
            play_order,
            level,
            children: children.into_iter().map(Into::into).collect(),
        }
    }
}

pub(crate) fn parse_epub_metadata(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIEbookMetadata, UniFFIEbookError> {
    let (zip, _, opf) = preview::open_and_parse_opf(data)?;
    let title = preview::title(&opf, file_name);
    let has_cover = opf
        .metadata
        .cover_image_href
        .as_ref()
        .is_some_and(|href| find_and_extract_entry(&zip, href).is_some());

    Ok(UniFFIEbookMetadata {
        title,
        authors: opf.metadata.authors,
        publisher: opf.metadata.publisher,
        language: opf.metadata.language,
        identifier: opf.metadata.identifiers.into_iter().next(),
        description: opf.metadata.description,
        publication_date: opf.metadata.publication_date,
        modified_date: opf.modified_date,
        rights: opf.metadata.rights,
        format: UniFFIEbookFormat::Epub,
        total_chapters: opf.spine_raw.len() as u32,
        total_resources: opf.manifest.len() as u32,
        file_size_bytes: data.len() as u64,
        has_cover,
        cover_path: opf.metadata.cover_image_href,
        extra_metadata: opf.extra_metadata,
    })
}

pub(crate) fn parse_epub_toc(data: &[u8]) -> Result<Vec<UniFFIEbookTocNode>, UniFFIEbookError> {
    let (zip, opf_dir, opf) = preview::open_and_parse_opf(data)?;
    Ok(preview::toc(&zip, &opf_dir, &opf)
        .into_iter()
        .map(Into::into)
        .collect())
}

pub(crate) fn parse_epub_spine(data: &[u8]) -> Result<Vec<UniFFIEbookSpineItem>, UniFFIEbookError> {
    let (_, _, opf) = preview::open_and_parse_opf(data)?;
    Ok(preview::spine(&opf).into_iter().map(Into::into).collect())
}

pub(crate) fn extract_epub_chapter(
    data: &[u8],
    href: &str,
) -> Result<UniFFIEbookChapter, UniFFIEbookError> {
    let zip =
        ZipArchive::open_slice(data).map_err(|e| UniFFIEbookError::corrupted(format!("{e:?}")))?;
    let clean_href = href.split('#').next().unwrap_or(href);
    let bytes = find_and_extract_entry(&zip, clean_href)
        .ok_or_else(|| UniFFIEbookError::not_found(clean_href))?;

    let content_string = String::from_utf8_lossy(&bytes).to_string();
    let plain_text = strip_html_tags(&content_string);
    let character_count = plain_text.chars().count() as u32;
    let word_count = plain_text.split_whitespace().count() as u32;

    let title = extract_html_heading(&content_string).unwrap_or_else(|| {
        Path::new(clean_href)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Chapter")
            .to_string()
    });

    Ok(UniFFIEbookChapter {
        id: clean_href.to_string(),
        title,
        href: clean_href.to_string(),
        media_type: "application/xhtml+xml".to_string(),
        play_order: 1,
        content_string,
        character_count,
        word_count,
    })
}

pub(crate) fn extract_epub_cover(
    data: &[u8],
) -> Result<Option<UniFFIEbookResource>, UniFFIEbookError> {
    let meta = parse_epub_metadata(data, None)?;
    if let Some(path) = meta.cover_path {
        match extract_resource_from_zip(data, &path) {
            Ok(res) => Ok(Some(res)),
            Err(_) => Ok(None),
        }
    } else {
        Ok(None)
    }
}
