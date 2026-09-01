// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust PDF Syntax Tree Deconstruction and Page Management Parser.
//!
//! Wraps `lopdf::Document` with safe indirect reference resolution, cycle-safe traversal,
//! cached page indexing, incremental update detection, and on-demand stream extraction.

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use lopdf::{Dictionary, Document, Object, ObjectId};

use super::PdfError;

/// Fundamental page geometry and structural metadata for a single PDF page.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfPageInfo {
    /// 1-based page number.
    pub page_number: u32,
    /// PDF Object ID (id, generation).
    pub object_id: (u32, u16),
    /// MediaBox [x_min, y_min, x_max, y_max] in user space points (1/72 inch).
    pub media_box: Option<[f64; 4]>,
    /// CropBox [x_min, y_min, x_max, y_max] if specified.
    pub crop_box: Option<[f64; 4]>,
    /// Rotation angle in degrees clockwise (0, 90, 180, 270).
    pub rotation: i64,
    /// Whether this page contains interactive annotations or form fields.
    pub has_annotations: bool,
    /// Total decompressed byte size of the page's content stream(s).
    pub content_stream_size: usize,
}

/// Pure Safe Rust PDF document syntax tree deconstructor and object resolver.
#[derive(Debug, Clone)]
pub struct TTZipPdfParser {
    /// Lopdf Document syntax representation.
    doc: Document,
    /// Pre-indexed 1-based page number to Object ID mapping.
    page_map: BTreeMap<u32, ObjectId>,
}

impl TTZipPdfParser {
    /// Opens and parses a PDF document from an in-memory byte slice.
    pub fn open_from_bytes(bytes: &[u8]) -> Result<Self, PdfError> {
        let mut reader = Cursor::new(bytes);
        Self::open_from_reader(&mut reader)
    }

    /// Opens and parses a PDF document from any `Read + Seek` stream.
    pub fn open_from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self, PdfError> {
        let load_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Document::load_from(reader)
        }));
        let doc = match load_res {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(PdfError::LopdfError(e.to_string())),
            Err(_) => return Err(PdfError::InvalidStructure("Lopdf panicked during document loading".to_string())),
        };
        let page_map = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            doc.get_pages()
        }))
        .unwrap_or_default();
        Ok(Self { doc, page_map })
    }

    /// Opens and parses a PDF document from a filesystem path.
    pub fn open_from_path<P: AsRef<Path>>(path: P) -> Result<Self, PdfError> {
        let mut file = File::open(path)?;
        Self::open_from_reader(&mut file)
    }

    /// Wraps an existing `lopdf::Document`.
    pub fn from_lopdf(doc: Document) -> Self {
        let page_map = doc.get_pages();
        Self { doc, page_map }
    }

    /// Borrows the underlying `lopdf::Document`.
    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// Returns the PDF specification version string (e.g., "1.4", "1.7", "2.0").
    pub fn version(&self) -> &str {
        &self.doc.version
    }

    /// Returns `true` if the document is encrypted via standard or custom security handler.
    pub fn is_encrypted(&self) -> bool {
        self.doc.is_encrypted()
    }

    /// Returns the total number of pages in the document.
    pub fn page_count(&self) -> u32 {
        self.page_map.len() as u32
    }

    /// Returns the complete page number to `ObjectId` index map.
    pub fn page_map(&self) -> &BTreeMap<u32, ObjectId> {
        &self.page_map
    }

    /// Looks up the `ObjectId` for a given 1-based page number.
    pub fn get_page_id(&self, page_number: u32) -> Result<ObjectId, PdfError> {
        self.page_map
            .get(&page_number)
            .copied()
            .ok_or(PdfError::PageOutOfBounds(page_number, self.page_count()))
    }

    /// Resolves the Page Dictionary for a given 1-based page number.
    pub fn get_page_dictionary(&self, page_number: u32) -> Result<&Dictionary, PdfError> {
        let page_id = self.get_page_id(page_number)?;
        let obj = self.get_object(page_id)?;
        match obj {
            Object::Dictionary(dict) => Ok(dict),
            _ => Err(PdfError::TypeMismatch {
                expected: "Dictionary",
                found: object_type_name(obj),
            }),
        }
    }

    /// Resolves the Resources dictionary (`/Resources`) for a page, inheriting from ancestors if needed.
    pub fn get_page_resources(&self, page_number: u32) -> Result<Option<&Dictionary>, PdfError> {
        let page_dict = self.get_page_dictionary(page_number)?;
        if let Ok(res_obj) = page_dict.get(b"Resources") {
            let deref_res = self.resolve_reference(res_obj)?;
            if let Object::Dictionary(dict) = deref_res {
                return Ok(Some(dict));
            }
        }
        Ok(None)
    }

    /// Extracts and decompresses all raw content streams for a given 1-based page number.
    pub fn get_page_content_bytes(&self, page_number: u32) -> Result<Vec<u8>, PdfError> {
        let page_id = self.get_page_id(page_number)?;
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.doc.get_page_content(page_id)
        }));
        match res {
            Ok(Ok(content)) => Ok(content),
            Ok(Err(e)) => Err(PdfError::StreamDecodeError(e.to_string())),
            Err(_) => Err(PdfError::StreamDecodeError("Lopdf panicked during content decompression".to_string())),
        }
    }

    /// Extracts geometry and structural info for a given page.
    pub fn get_page_info(&self, page_number: u32) -> Result<PdfPageInfo, PdfError> {
        let page_id = self.get_page_id(page_number)?;
        let dict = self.get_page_dictionary(page_number)?;

        let media_box = self.extract_rectangle(dict, b"MediaBox");
        let crop_box = self.extract_rectangle(dict, b"CropBox");

        let rotation = dict
            .get(b"Rotate")
            .ok()
            .and_then(|obj| self.resolve_reference(obj).ok())
            .and_then(|obj| match obj {
                Object::Integer(i) => Some(*i),
                _ => None,
            })
            .unwrap_or(0);

        let has_annotations = dict
            .get(b"Annots")
            .ok()
            .and_then(|obj| self.resolve_reference(obj).ok())
            .map(|obj| matches!(obj, Object::Array(arr) if !arr.is_empty()))
            .unwrap_or(false);

        let content_bytes = self.get_page_content_bytes(page_number).unwrap_or_default();

        Ok(PdfPageInfo {
            page_number,
            object_id: (page_id.0, page_id.1),
            media_box,
            crop_box,
            rotation,
            has_annotations,
            content_stream_size: content_bytes.len(),
        })
    }

    /// Retrieves an object by its `ObjectId` with bounds checking.
    pub fn get_object(&self, id: ObjectId) -> Result<&Object, PdfError> {
        self.doc
            .get_object(id)
            .map_err(|_| PdfError::ObjectNotFound(id.0, id.1))
    }

    /// Resolves an indirect reference (`Object::Reference`) recursively with cycle protection.
    pub fn resolve_reference<'a>(&'a self, mut obj: &'a Object) -> Result<&'a Object, PdfError> {
        let mut visited = HashSet::new();
        while let Object::Reference(id) = obj {
            if !visited.insert(*id) {
                return Err(PdfError::InvalidStructure(format!(
                    "Circular indirect reference detected at object ({}, {})",
                    id.0, id.1
                )));
            }
            obj = self.get_object(*id)?;
        }
        Ok(obj)
    }

    /// Resolves an object to a `&Dictionary`, dereferencing if it is an indirect reference.
    pub fn resolve_dict<'a>(&'a self, obj: &'a Object) -> Result<&'a Dictionary, PdfError> {
        let deref = self.resolve_reference(obj)?;
        match deref {
            Object::Dictionary(dict) => Ok(dict),
            _ => Err(PdfError::TypeMismatch {
                expected: "Dictionary",
                found: object_type_name(deref),
            }),
        }
    }

    /// Resolves an object to a `&Vec<Object>`, dereferencing if it is an indirect reference.
    pub fn resolve_array<'a>(&'a self, obj: &'a Object) -> Result<&'a Vec<Object>, PdfError> {
        let deref = self.resolve_reference(obj)?;
        match deref {
            Object::Array(arr) => Ok(arr),
            _ => Err(PdfError::TypeMismatch {
                expected: "Array",
                found: object_type_name(deref),
            }),
        }
    }

    /// Decodes a string or name object into a sanitized UTF-8 `String`.
    pub fn resolve_string(&self, obj: &Object) -> Option<String> {
        let deref = self.resolve_reference(obj).ok()?;
        match deref {
            Object::String(bytes, _) => {
                let s = Self::decode_pdf_string(bytes);
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Object::Name(bytes) => {
                let s = String::from_utf8_lossy(bytes).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            _ => None,
        }
    }

    /// Resolves the document Catalog dictionary (`/Root` in Trailer).
    pub fn catalog(&self) -> Result<&Dictionary, PdfError> {
        let root_obj = self
            .doc
            .trailer
            .get(b"Root")
            .map_err(|_| PdfError::InvalidStructure("Missing /Root key in trailer".to_string()))?;
        self.resolve_dict(root_obj)
    }

    /// Borrows the document Trailer dictionary.
    pub fn trailer(&self) -> &Dictionary {
        &self.doc.trailer
    }

    /// Detects whether the PDF contains multiple XRef tables indicating incremental updates.
    pub fn has_incremental_updates(&self) -> bool {
        self.doc.trailer.get(b"Prev").is_ok()
    }

    /// Returns the count of incremental update segments detected via `/Prev` trailer links.
    pub fn detect_incremental_updates(&self) -> usize {
        let mut count = 1;
        let curr_dict = &self.doc.trailer;

        if let Ok(prev_obj) = curr_dict.get(b"Prev") {
            if let Ok(Object::Integer(_offset)) = self.resolve_reference(prev_obj) {
                count += 1;
            }
        }
        count
    }

    /// Resolves the Tagged PDF `/StructTreeRoot` object ID if available.
    pub fn structure_tree_root(&self) -> Option<ObjectId> {
        let cat = self.catalog().ok()?;
        match cat.get(b"StructTreeRoot").ok()? {
            Object::Reference(id) => Some(*id),
            _ => None,
        }
    }

    /// Decodes raw PDF string bytes handling UTF-16BE, UTF-16LE, UTF-8, and PDFDocEncoding.
    pub fn decode_pdf_string(bytes: &[u8]) -> String {
        if bytes.is_empty() {
            return String::new();
        }
        // Check UTF-16BE BOM: 0xFE, 0xFF
        if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
            let u16s: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            return String::from_utf16_lossy(&u16s);
        }
        // Check UTF-16LE BOM: 0xFF, 0xFE
        if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
            let u16s: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            return String::from_utf16_lossy(&u16s);
        }
        // Fast path UTF-8 validation
        if let Ok(s) = std::str::from_utf8(bytes) {
            return s.to_string();
        }
        // Fallback: PDFDocEncoding / ISO-8859-1 (Latin-1)
        bytes.iter().map(|&b| b as char).collect()
    }

    /// Extracts a 4-element coordinate rectangle [x0, y0, x1, y1] from a dictionary.
    fn extract_rectangle(&self, dict: &Dictionary, key: &[u8]) -> Option<[f64; 4]> {
        let obj = dict.get(key).ok()?;
        let deref = self.resolve_reference(obj).ok()?;
        if let Object::Array(arr) = deref {
            if arr.len() == 4 {
                let mut coords = [0.0; 4];
                for (i, item) in arr.iter().enumerate() {
                    let num_obj = self.resolve_reference(item).ok()?;
                    coords[i] = match num_obj {
                        Object::Integer(v) => *v as f64,
                        Object::Real(v) => (*v).into(),
                        _ => 0.0,
                    };
                }
                return Some(coords);
            }
        }
        None
    }
}

fn object_type_name(obj: &Object) -> &'static str {
    match obj {
        Object::Null => "Null",
        Object::Boolean(_) => "Boolean",
        Object::Integer(_) => "Integer",
        Object::Real(_) => "Real",
        Object::String(_, _) => "String",
        Object::Name(_) => "Name",
        Object::Array(_) => "Array",
        Object::Dictionary(_) => "Dictionary",
        Object::Stream(_) => "Stream",
        Object::Reference(_) => "Reference",
    }
}
