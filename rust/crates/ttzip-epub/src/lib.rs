// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::fs::File;
use std::path::Path;

use ttzip_archive::mmap_guard::safe_map_file;
use ttzip_ebook::preview::{self, PreviewError, PreviewTocNode};

uniffi::setup_scaffolding!("ttzip_epub");

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum EpubError {
    #[error("Could not read EPUB file: {message}")]
    Io { message: String },
    #[error("Invalid EPUB: {message}")]
    Invalid { message: String },
    #[error("Missing EPUB resource: {href}")]
    Missing { href: String },
}

impl From<PreviewError> for EpubError {
    fn from(error: PreviewError) -> Self {
        match error {
            PreviewError::Corrupt(message) | PreviewError::Xml(message) => {
                Self::Invalid { message }
            }
            PreviewError::Missing(href) => Self::Missing { href },
        }
    }
}

#[derive(uniffi::Record)]
pub struct EpubTocNode {
    pub title: String,
    pub href: String,
    pub children: Vec<EpubTocNode>,
}

impl From<PreviewTocNode> for EpubTocNode {
    fn from(node: PreviewTocNode) -> Self {
        Self {
            title: node.title,
            href: node.href,
            children: node.children.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(uniffi::Record)]
pub struct EpubChapter {
    pub title: String,
    pub href: String,
}

#[derive(uniffi::Record)]
pub struct EpubOutline {
    pub title: String,
    pub cover_path: Option<String>,
    pub spine: Vec<String>,
    pub toc: Vec<EpubTocNode>,
    pub disk_fallback: Vec<EpubChapter>,
}

fn outline(
    data: &[u8],
    file_name: Option<&str>,
    allow_disk_fallback: bool,
) -> Result<EpubOutline, EpubError> {
    let (zip, opf_dir, opf) = preview::open_and_parse_opf(data)?;
    let spine = preview::spine(&opf);
    let disk_fallback = if spine.is_empty() && allow_disk_fallback {
        preview::fallback_chapters(&zip)
            .into_iter()
            .map(|chapter| EpubChapter {
                title: chapter.title,
                href: chapter.href,
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(EpubOutline {
        title: preview::title(&opf, file_name),
        spine: spine.into_iter().map(|item| item.href).collect(),
        toc: preview::toc(&zip, &opf_dir, &opf)
            .into_iter()
            .map(Into::into)
            .collect(),
        cover_path: opf.metadata.cover_image_href,
        disk_fallback,
    })
}

#[uniffi::export]
pub fn inspect_epub_bytes(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<EpubOutline, EpubError> {
    outline(&data, file_name.as_deref(), false)
}

#[uniffi::export]
pub fn inspect_epub_file(path: String) -> Result<EpubOutline, EpubError> {
    let path = Path::new(&path);
    let file = File::open(path).map_err(|error| EpubError::Io {
        message: error.to_string(),
    })?;
    let bytes = safe_map_file(&file).map_err(|error| EpubError::Io {
        message: error.to_string(),
    })?;
    let file_name = path.file_name().and_then(|name| name.to_str());
    outline(&bytes, file_name, true)
}

#[uniffi::export]
pub fn read_epub_resource(data: Vec<u8>, href: String) -> Result<Vec<u8>, EpubError> {
    Ok(preview::resource(&data, &href)?)
}
