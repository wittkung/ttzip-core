// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Service and Pipeline Implementations for Office Introspection,
//! Spreadsheet Sheet Extraction, Formula Evaluation, DOCX Hierarchy, and Markdown Conversion.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use super::docx::parse_docx_archive;
use super::types::{
    UniFFICell, UniFFICellValue, UniFFIDocxDocument, UniFFIOfficeError, UniFFIOfficeFormat,
    UniFFISheetData,
};
use super::xlsx::{
    evaluate_spreadsheet_formula, extract_xlsx_sheet_data, extract_xlsx_sheet_names,
};
use crate::zip::reader::ZipArchive;

// ============================================================================
// Exported Free Functions
// ============================================================================

/// Probes and identifies the Office document format directly from in-memory bytes.
#[uniffi::export]
pub fn uniffi_probe_office_bytes(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIOfficeFormat, UniFFIOfficeError> {
    probe_office_format_internal(&data, file_name.as_deref())
}

/// Extracts worksheet names from an in-memory XLSX spreadsheet archive.
#[uniffi::export]
pub fn uniffi_extract_sheet_names(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<Vec<String>, UniFFIOfficeError> {
    let fmt = probe_office_format_internal(&data, file_name.as_deref())?;
    match fmt {
        UniFFIOfficeFormat::Xlsx => extract_xlsx_sheet_names(&data),
        _ => Err(UniFFIOfficeError::UnsupportedFormat {
            format: format!("{fmt:?}"),
        }),
    }
}

/// Extracts parsed worksheet data with cell coordinates and values from an XLSX archive.
#[uniffi::export]
pub fn uniffi_extract_sheet_data(
    data: Vec<u8>,
    sheet_name_or_index: String,
    max_rows: Option<u32>,
    file_name: Option<String>,
) -> Result<UniFFISheetData, UniFFIOfficeError> {
    let fmt = probe_office_format_internal(&data, file_name.as_deref())?;
    match fmt {
        UniFFIOfficeFormat::Xlsx => {
            extract_xlsx_sheet_data(&data, &sheet_name_or_index, max_rows)
        }
        _ => Err(UniFFIOfficeError::UnsupportedFormat {
            format: format!("{fmt:?}"),
        }),
    }
}

/// Dynamically evaluates a spreadsheet formula (SUM, AVERAGE, MIN, MAX, COUNT, IF, CONCAT, arithmetic).
#[uniffi::export]
pub fn uniffi_evaluate_formula(
    formula: String,
    context_cells: Option<Vec<UniFFICell>>,
) -> Result<UniFFICellValue, UniFFIOfficeError> {
    evaluate_spreadsheet_formula(&formula, context_cells.as_deref())
}

/// Extracts structured paragraphs, tables, and outline metrics from a DOCX archive.
#[uniffi::export]
pub fn uniffi_extract_docx_document(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<UniFFIDocxDocument, UniFFIOfficeError> {
    let fmt = probe_office_format_internal(&data, file_name.as_deref())?;
    match fmt {
        UniFFIOfficeFormat::Docx => parse_docx_archive(&data),
        _ => Err(UniFFIOfficeError::UnsupportedFormat {
            format: format!("{fmt:?}"),
        }),
    }
}

/// Converts a DOCX document into standard GitHub-Flavored Markdown.
#[uniffi::export]
pub fn uniffi_convert_docx_to_markdown(
    data: Vec<u8>,
    file_name: Option<String>,
) -> Result<String, UniFFIOfficeError> {
    let doc = uniffi_extract_docx_document(data, file_name)?;
    Ok(doc.markdown_content)
}

// ============================================================================
// Stateful UniFFI Service Object
// ============================================================================

/// High-performance thread-safe Office Document Introspection Service.
#[derive(uniffi::Object, Default)]
pub struct UniFFIOfficeService {}

#[uniffi::export]
impl UniFFIOfficeService {
    /// Constructs a new thread-safe office service instance.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    /// Probes the Office format from an in-memory byte buffer.
    pub fn probe_bytes(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIOfficeFormat, UniFFIOfficeError> {
        uniffi_probe_office_bytes(data, file_name)
    }

    /// Probes the Office format from a file on disk.
    pub fn probe_file(&self, file_path: String) -> Result<UniFFIOfficeFormat, UniFFIOfficeError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_probe_office_bytes(bytes, name)
    }

    /// Extracts worksheet names from an in-memory XLSX byte buffer.
    pub fn extract_sheet_names(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<Vec<String>, UniFFIOfficeError> {
        uniffi_extract_sheet_names(data, file_name)
    }

    /// Extracts worksheet names from an XLSX file on disk.
    pub fn extract_sheet_names_from_file(
        &self,
        file_path: String,
    ) -> Result<Vec<String>, UniFFIOfficeError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_sheet_names(bytes, name)
    }

    /// Extracts worksheet data from an in-memory XLSX byte buffer.
    pub fn extract_sheet_data(
        &self,
        data: Vec<u8>,
        sheet_name_or_index: String,
        max_rows: Option<u32>,
        file_name: Option<String>,
    ) -> Result<UniFFISheetData, UniFFIOfficeError> {
        uniffi_extract_sheet_data(data, sheet_name_or_index, max_rows, file_name)
    }

    /// Extracts worksheet data from an XLSX file on disk.
    pub fn extract_sheet_data_from_file(
        &self,
        file_path: String,
        sheet_name_or_index: String,
        max_rows: Option<u32>,
    ) -> Result<UniFFISheetData, UniFFIOfficeError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_sheet_data(bytes, sheet_name_or_index, max_rows, name)
    }

    /// Dynamically evaluates a formula with optional context cells.
    pub fn evaluate_formula(
        &self,
        formula: String,
        context_cells: Option<Vec<UniFFICell>>,
    ) -> Result<UniFFICellValue, UniFFIOfficeError> {
        uniffi_evaluate_formula(formula, context_cells)
    }

    /// Extracts structured DOCX document model from an in-memory byte buffer.
    pub fn extract_docx_document(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<UniFFIDocxDocument, UniFFIOfficeError> {
        uniffi_extract_docx_document(data, file_name)
    }

    /// Extracts structured DOCX document model from a file on disk.
    pub fn extract_docx_document_from_file(
        &self,
        file_path: String,
    ) -> Result<UniFFIDocxDocument, UniFFIOfficeError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_extract_docx_document(bytes, name)
    }

    /// Converts a DOCX file from an in-memory byte buffer to Markdown.
    pub fn convert_docx_to_markdown(
        &self,
        data: Vec<u8>,
        file_name: Option<String>,
    ) -> Result<String, UniFFIOfficeError> {
        uniffi_convert_docx_to_markdown(data, file_name)
    }

    /// Converts a DOCX file on disk to Markdown.
    pub fn convert_docx_to_markdown_from_file(
        &self,
        file_path: String,
    ) -> Result<String, UniFFIOfficeError> {
        let bytes = read_file_bytes(&file_path)?;
        let name = Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        uniffi_convert_docx_to_markdown(bytes, name)
    }
}

// ============================================================================
// Internal Helper Functions
// ============================================================================

fn probe_office_format_internal(
    data: &[u8],
    file_name: Option<&str>,
) -> Result<UniFFIOfficeFormat, UniFFIOfficeError> {
    if data.len() < 4 {
        return Err(UniFFIOfficeError::corrupted("Payload too small to probe"));
    }

    // ZIP container inspection
    if data.starts_with(b"PK\x03\x04")
        || data.starts_with(b"PK\x05\x06")
        || data.starts_with(b"PK\x07\x08")
    {
        if let Ok(zip) = ZipArchive::open_slice(data) {
            let has_word = zip.entries().iter().any(|e| e.rel_path.starts_with("word/"));
            if has_word {
                return Ok(UniFFIOfficeFormat::Docx);
            }

            let has_xl = zip.entries().iter().any(|e| e.rel_path.starts_with("xl/"));
            if has_xl {
                return Ok(UniFFIOfficeFormat::Xlsx);
            }

            let has_ppt = zip.entries().iter().any(|e| e.rel_path.starts_with("ppt/"));
            if has_ppt {
                return Ok(UniFFIOfficeFormat::Pptx);
            }

            // Check OpenDocument mimetype entry
            for (idx, entry) in zip.entries().iter().enumerate() {
                if entry.rel_path == "mimetype" {
                    if let Ok(mime_bytes) = zip.extract_entry_bytes(idx, None) {
                        let mime_str = String::from_utf8_lossy(&mime_bytes).trim().to_string();
                        if mime_str == "application/vnd.oasis.opendocument.text" {
                            return Ok(UniFFIOfficeFormat::Odt);
                        } else if mime_str == "application/vnd.oasis.opendocument.spreadsheet" {
                            return Ok(UniFFIOfficeFormat::Ods);
                        } else if mime_str == "application/vnd.oasis.opendocument.presentation" {
                            return Ok(UniFFIOfficeFormat::Odp);
                        }
                    }
                }
            }
        }
    }

    if let Some(name) = file_name {
        let l = name.to_lowercase();
        if l.ends_with(".docx") || l.ends_with(".docm") {
            return Ok(UniFFIOfficeFormat::Docx);
        }
        if l.ends_with(".xlsx") || l.ends_with(".xlsm") {
            return Ok(UniFFIOfficeFormat::Xlsx);
        }
        if l.ends_with(".pptx") || l.ends_with(".pptm") {
            return Ok(UniFFIOfficeFormat::Pptx);
        }
        if l.ends_with(".odt") {
            return Ok(UniFFIOfficeFormat::Odt);
        }
        if l.ends_with(".ods") {
            return Ok(UniFFIOfficeFormat::Ods);
        }
        if l.ends_with(".odp") {
            return Ok(UniFFIOfficeFormat::Odp);
        }
    }

    Ok(UniFFIOfficeFormat::Unknown)
}

fn read_file_bytes(file_path: &str) -> Result<Vec<u8>, UniFFIOfficeError> {
    let mut file = File::open(file_path).map_err(|e| UniFFIOfficeError::io_err(e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| UniFFIOfficeError::io_err(e))?;
    Ok(buffer)
}
