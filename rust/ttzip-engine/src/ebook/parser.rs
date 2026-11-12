// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! TTZip E-book Format Detector, OPF Package Parser, and Unified Parsing Microkernel.
//!
//! Handles EPUB 2/3, MOBI, and AZW3 container recognition, Dublin Core metadata extraction,
//! and TOC/Spine graph construction.

use std::collections::HashMap;

use crate::ebook::mobi::EbookMobiDecoder;
use crate::ebook::navigation::{EbookTocNode, SpineItem};
use crate::ebook::opf::{
    extract_epub_toc, is_epub_container, parse_container_rootfile, parse_opf_xml,
    resolve_cover_href, resolve_epub_spine,
};
use crate::ebook::resource::{EbookResource, EbookResourceExtractor};
use crate::ebook::{EbookError, EbookResult};
use crate::zip::ZipArchive;

/// Recognized electronic book container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EbookFormat {
    /// EPUB 2.0 / 2.0.1 Open eBook Publication structure.
    Epub2,
    /// EPUB 3.0 / 3.2 / 3.3 Modern Web Standards Publication structure.
    Epub3,
    /// Mobipocket standard format (MOBI 6/7).
    Mobi,
    /// Amazon Kindle Format 8 (AZW3 / KF8).
    Azw3,
    /// Unrecognized e-book format.
    #[default]
    Unknown,
}

/// Extracted publication metadata (Dublin Core and EXTH extensions).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EbookMetadata {
    /// Main publication title.
    pub title: Option<String>,
    /// List of creators / authors.
    pub authors: Vec<String>,
    /// Publishing house or entity.
    pub publisher: Option<String>,
    /// Primary BCP-47 language tag (e.g., `en`, `zh-CN`).
    pub language: Option<String>,
    /// Book synopsis or description.
    pub description: Option<String>,
    /// Copyright or distribution rights statement.
    pub rights: Option<String>,
    /// Canonical unique identifiers (ISBN, UUID, ASIN, DOI).
    pub identifiers: Vec<String>,
    /// Publication or release date (ISO-8601 or free text).
    pub publication_date: Option<String>,
    /// Container relative path or URI to the cover image.
    pub cover_image_href: Option<String>,
}

/// Individual item declared in the EPUB OPF package manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EpubManifestItem {
    /// Item identifier attribute in OPF.
    pub id: String,
    /// Container relative path to the resource.
    pub href: String,
    /// MIME content type of the file.
    pub media_type: String,
    /// Space-delimited properties (e.g., `cover-image`, `nav`, `scripted`).
    pub properties: Option<String>,
}

/// High-throughput e-book parser microkernel.
pub struct TTZipEbookParser<'a> {
    raw_data: &'a [u8],
    format: EbookFormat,
    metadata: EbookMetadata,
    spine: Vec<SpineItem>,
    manifest: HashMap<String, EpubManifestItem>,
    toc: Vec<EbookTocNode>,
    opf_path: String,
    opf_dir: String,
    zip: Option<ZipArchive<'a>>,
    mobi: Option<EbookMobiDecoder<'a>>,
}

impl<'a> TTZipEbookParser<'a> {
    /// Automatically detects e-book format and parses metadata, spine, and navigation structure from a byte slice.
    pub fn open_from_bytes(data: &'a [u8]) -> EbookResult<Self> {
        // 1. Probe for MOBI / AZW3 / PalmDOC format
        if let Ok(mobi_decoder) = EbookMobiDecoder::parse(data) {
            let format = if mobi_decoder.is_azw3() {
                EbookFormat::Azw3
            } else {
                EbookFormat::Mobi
            };

            let title = mobi_decoder.title();
            let authors = mobi_decoder.authors();
            let publisher = mobi_decoder.publisher();
            let description = mobi_decoder.description();
            let publication_date = mobi_decoder.publication_date();
            let rights = mobi_decoder.rights();
            let mut identifiers = Vec::new();
            if let Some(id) = mobi_decoder.asin_or_isbn() {
                identifiers.push(id);
            }

            let cover_image_href = if mobi_decoder.extract_cover_image().is_some() {
                Some("cover.jpg".to_string())
            } else {
                None
            };

            let metadata = EbookMetadata {
                title,
                authors,
                publisher,
                language: None,
                description,
                rights,
                identifiers,
                publication_date,
                cover_image_href,
            };

            return Ok(Self {
                raw_data: data,
                format,
                metadata,
                spine: Vec::new(),
                manifest: HashMap::new(),
                toc: Vec::new(),
                opf_path: String::new(),
                opf_dir: String::new(),
                zip: None,
                mobi: Some(mobi_decoder),
            });
        }

        // 2. Probe for EPUB (ZIP archive container)
        if let Ok(zip) = ZipArchive::open_slice(data) {
            if is_epub_container(&zip) {
                return Self::parse_epub_container(data, zip);
            }
        }

        Err(EbookError::UnsupportedFormat(
            "Payload does not match EPUB, MOBI, or AZW3 signature".to_string(),
        ))
    }

    /// Returns the raw binary payload slice.
    #[inline]
    pub fn raw_data(&self) -> &'a [u8] {
        self.raw_data
    }

    /// Returns the detected e-book format.
    #[inline]
    pub fn format(&self) -> EbookFormat {
        self.format
    }

    /// Returns the publication metadata.
    #[inline]
    pub fn metadata(&self) -> &EbookMetadata {
        &self.metadata
    }

    /// Returns the sequential reading spine.
    #[inline]
    pub fn spine(&self) -> &[SpineItem] {
        &self.spine
    }

    /// Returns the hierarchical Table of Contents.
    #[inline]
    pub fn toc(&self) -> &[EbookTocNode] {
        &self.toc
    }

    /// Returns the OPF manifest item lookup table.
    #[inline]
    pub fn manifest(&self) -> &HashMap<String, EpubManifestItem> {
        &self.manifest
    }

    /// Returns the OPF package root file path inside the container.
    #[inline]
    pub fn opf_path(&self) -> &str {
        &self.opf_path
    }

    /// Returns the raw underlying e-book byte slice.
    #[inline]
    pub fn raw_bytes(&self) -> &'a [u8] {
        self.raw_data
    }

    /// Returns the OPF base directory path.
    #[inline]
    pub fn opf_dir(&self) -> &str {
        &self.opf_dir
    }

    /// Extracts a resource by container relative path.
    pub fn extract_resource(&self, path: &str) -> EbookResult<EbookResource> {
        if let Some(ref zip) = self.zip {
            EbookResourceExtractor::extract_resource(zip, path, None)
        } else if let Some(ref mobi) = self.mobi {
            if path == "cover.jpg" || path == "cover.jpeg" || path == "cover.png" {
                if let Some(img_data) = mobi.extract_cover_image() {
                    return Ok(EbookResource {
                        path: path.to_string(),
                        media_type: "image/jpeg".to_string(),
                        data: img_data,
                    });
                }
            }
            Err(EbookError::NotFound(format!(
                "Direct resource extraction not supported for MOBI/AZW3: {path}"
            )))
        } else {
            Err(EbookError::NotFound("No container opened".to_string()))
        }
    }

    /// Extracts the text/XHTML payload of a specific spine chapter index.
    pub fn extract_chapter_text(&self, spine_index: usize) -> EbookResult<String> {
        let item = self.spine.get(spine_index).ok_or_else(|| {
            EbookError::NotFound(format!("Spine index out of bounds: {spine_index}"))
        })?;

        if let Some(ref zip) = self.zip {
            EbookResourceExtractor::extract_text(zip, &item.href)
        } else {
            Err(EbookError::NotFound(
                "Spine chapter extraction only supported on EPUB".to_string(),
            ))
        }
    }

    /// Extracts the binary cover image, if available.
    pub fn extract_cover(&self) -> EbookResult<Option<EbookResource>> {
        if let Some(ref mobi) = self.mobi {
            if let Some(cover_bytes) = mobi.extract_cover_image() {
                return Ok(Some(EbookResource {
                    path: "cover.jpg".to_string(),
                    media_type: "image/jpeg".to_string(),
                    data: cover_bytes,
                }));
            }
            return Ok(None);
        }

        if let Some(ref zip) = self.zip {
            if let Some(ref cover_href) = self.metadata.cover_image_href {
                if let Ok(res) = EbookResourceExtractor::extract_resource(zip, cover_href, None) {
                    return Ok(Some(res));
                }
            }
        }

        Ok(None)
    }

    /// Extracts full text from the e-book (MOBI decompressed records or EPUB spine concatenation).
    pub fn extract_full_text(&self) -> EbookResult<String> {
        if let Some(ref mobi) = self.mobi {
            return mobi.extract_full_text();
        }

        if let Some(ref zip) = self.zip {
            let mut full_text = String::new();
            for item in &self.spine {
                if let Ok(chapter_raw) = EbookResourceExtractor::extract_text(zip, &item.href) {
                    let stripped = strip_html_tags(&chapter_raw);
                    if !stripped.trim().is_empty() {
                        if !full_text.is_empty() {
                            full_text.push_str("\n\n");
                        }
                        full_text.push_str(stripped.trim());
                    }
                }
            }
            return Ok(full_text);
        }

        Ok(String::new())
    }

    /// Internal helper to parse full EPUB container and OPF package.
    fn parse_epub_container(raw_data: &'a [u8], zip: ZipArchive<'a>) -> EbookResult<Self> {
        let container_xml = EbookResourceExtractor::extract_text(&zip, "META-INF/container.xml")
            .map_err(|_| EbookError::NotFound("Missing META-INF/container.xml in EPUB".to_string()))?;

        let opf_path = parse_container_rootfile(container_xml.as_bytes())?;
        let opf_dir = match opf_path.rfind('/') {
            Some(pos) => opf_path[..pos].to_string(),
            None => String::new(),
        };

        let opf_xml = EbookResourceExtractor::extract_text(&zip, &opf_path)
            .map_err(|_| EbookError::NotFound(format!("Missing OPF package file at {opf_path}")))?;

        let mut opf = parse_opf_xml(opf_xml.as_bytes(), &opf_dir)?;
        let spine = resolve_epub_spine(opf.spine_raw, &opf.manifest, &zip);
        resolve_cover_href(&mut opf.metadata, opf.cover_meta_id, &opf.manifest);
        let toc = extract_epub_toc(
            &zip,
            &opf_dir,
            &opf.manifest,
            &spine,
            opf.nav_xhtml_id,
            opf.spine_toc_id,
        );

        Ok(Self {
            raw_data,
            format: opf.format,
            metadata: opf.metadata,
            spine,
            manifest: opf.manifest,
            toc,
            opf_path,
            opf_dir,
            zip: Some(zip),
            mobi: None,
        })
    }
}

/// Strips HTML/XML tags from a markup string to extract plain text paragraphs.
fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
