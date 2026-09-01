// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI 0.28 Records, Enums, and Errors for Office Document Metadata,
//! Spreadsheet Worksheets, Cell Grids, Formula Evaluation, and DOCX Structures.

/// Supported Office Open XML and OpenDocument container formats.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, uniffi::Enum)]
pub enum UniFFIOfficeFormat {
    #[default]
    Unknown,
    Docx,
    Xlsx,
    Pptx,
    Odt,
    Ods,
    Odp,
}

/// Strongly-typed Office operation error enum mapped directly to Swift `throws UniFFIOfficeError`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum UniFFIOfficeError {
    /// Failure due to malformed or corrupted archive structure.
    #[error("Corrupted office archive: {message}")]
    CorruptedFormat { message: String },

    /// Requested worksheet was not found in workbook.
    #[error("Worksheet '{name}' not found in workbook")]
    SheetNotFound { name: String },

    /// Requested document part or resource was not found.
    #[error("Office entry or resource not found: {path}")]
    EntryNotFound { path: String },

    /// Failure during XML / OpenXML schema parsing.
    #[error("XML parsing error in office container: {message}")]
    XmlParseError { message: String },

    /// Dynamic formula calculation or parsing failure.
    #[error("Formula evaluation error for '{formula}': {reason}")]
    FormulaEvaluationError { formula: String, reason: String },

    /// Format is not supported or recognized.
    #[error("Unsupported office format: {format}")]
    UnsupportedFormat { format: String },

    /// File system or stream I/O failure.
    #[error("I/O error during office operation: {message}")]
    IoError { message: String },

    /// Operation was cancelled by caller.
    #[error("Office operation cancelled")]
    Cancelled,
}

impl UniFFIOfficeError {
    pub fn corrupted(msg: impl std::fmt::Display) -> Self {
        Self::CorruptedFormat {
            message: msg.to_string(),
        }
    }

    pub fn sheet_not_found(name: impl std::fmt::Display) -> Self {
        Self::SheetNotFound {
            name: name.to_string(),
        }
    }

    pub fn entry_not_found(path: impl std::fmt::Display) -> Self {
        Self::EntryNotFound {
            path: path.to_string(),
        }
    }

    pub fn xml_err(msg: impl std::fmt::Display) -> Self {
        Self::XmlParseError {
            message: msg.to_string(),
        }
    }

    pub fn formula_err(formula: impl std::fmt::Display, reason: impl std::fmt::Display) -> Self {
        Self::FormulaEvaluationError {
            formula: formula.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn io_err(msg: impl std::fmt::Display) -> Self {
        Self::IoError {
            message: msg.to_string(),
        }
    }
}

/// Strongly-typed spreadsheet cell value descriptor.
#[derive(Clone, Debug, PartialEq, uniffi::Enum)]
pub enum UniFFICellValue {
    Empty,
    Text {
        value: String,
    },
    Number {
        value: f64,
    },
    Boolean {
        value: bool,
    },
    Formula {
        expression: String,
        cached_value: Option<String>,
    },
    Error {
        message: String,
    },
}

impl UniFFICellValue {
    /// Formats the cell value to a user-facing plain string representation.
    pub fn to_display_string(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Text { value } => value.clone(),
            Self::Number { value } => {
                if value.fract() == 0.0 && !value.is_infinite() && !value.is_nan() {
                    format!("{:.0}", value)
                } else {
                    format!("{}", value)
                }
            }
            Self::Boolean { value } => {
                if *value {
                    "TRUE".to_string()
                } else {
                    "FALSE".to_string()
                }
            }
            Self::Formula {
                expression,
                cached_value,
            } => {
                if let Some(cached) = cached_value {
                    cached.clone()
                } else {
                    format!("={expression}")
                }
            }
            Self::Error { message } => format!("#{message}"),
        }
    }

    /// Extracts numeric value if cell is numeric or numeric-convertible string.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number { value } => Some(*value),
            Self::Text { value } => value.trim().parse::<f64>().ok(),
            Self::Boolean { value } => Some(if *value { 1.0 } else { 0.0 }),
            Self::Formula { cached_value, .. } => {
                cached_value.as_ref().and_then(|v| v.trim().parse::<f64>().ok())
            }
            _ => None,
        }
    }

    /// Extracts boolean value if cell is boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean { value } => Some(*value),
            Self::Number { value } => Some(*value != 0.0),
            Self::Text { value } => {
                let lower = value.trim().to_lowercase();
                if lower == "true" || lower == "1" {
                    Some(true)
                } else if lower == "false" || lower == "0" {
                    Some(false)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Strongly-typed single spreadsheet cell record with coordinates.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFICell {
    /// 1-based row index.
    pub row: u32,
    /// 1-based column index (1 = 'A', 2 = 'B', 27 = 'AA').
    pub col: u32,
    /// Standard alphanumeric cell reference (e.g. "A1", "C15").
    pub coordinate: String,
    /// Resolved typed cell value.
    pub value: UniFFICellValue,
    /// Raw formula string if cell contains a formula.
    pub formula: Option<String>,
}

/// Sequential row in a spreadsheet worksheet grid.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFISheetRow {
    /// 1-based row number.
    pub row_number: u32,
    /// Non-empty cells contained in this row.
    pub cells: Vec<UniFFICell>,
}

/// Extracted worksheet grid data structure with metrics.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct UniFFISheetData {
    /// Worksheet name (e.g. "Sheet1", "Financial_Summary").
    pub sheet_name: String,
    /// 1-based sequential sheet index in workbook.
    pub sheet_index: u32,
    /// Total row count populated in the sheet.
    pub total_rows: u32,
    /// Total column count populated in the sheet.
    pub total_cols: u32,
    /// Declared dimension reference if present (e.g. "A1:G100").
    pub dimension_ref: Option<String>,
    /// Extracted row list.
    pub rows: Vec<UniFFISheetRow>,
    /// Total shared strings referenced.
    pub shared_strings_count: u32,
}

/// A structured paragraph inside a DOCX document.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIDocxParagraph {
    /// Style name (e.g. "Normal", "Heading 1", "List Paragraph").
    pub style: String,
    /// Plain text content of the paragraph.
    pub text: String,
    /// Heading level if detected (0 for Title, 1 for Heading 1, 2 for Heading 2, etc.).
    pub heading_level: Option<u32>,
    /// Whether this paragraph is part of a bulleted or numbered list.
    pub is_list_item: bool,
    /// 0-based indentation level for nested lists.
    pub list_level: Option<u32>,
}

/// A single row within a DOCX table.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIDocxTableRow {
    /// Cell string contents from left to right.
    pub cells: Vec<String>,
}

/// A structured table extracted from a DOCX document.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIDocxTable {
    /// Total row count.
    pub total_rows: u32,
    /// Total column count.
    pub total_cols: u32,
    /// Header row labels if detected.
    pub headers: Vec<String>,
    /// Table body rows.
    pub rows: Vec<UniFFIDocxTableRow>,
}

/// Comprehensive DOCX structured document representation.
#[derive(Clone, Debug, PartialEq, Eq, Default, uniffi::Record)]
pub struct UniFFIDocxDocument {
    /// Document title if identified.
    pub title: Option<String>,
    /// Ordered paragraph sequence.
    pub paragraphs: Vec<UniFFIDocxParagraph>,
    /// Ordered table sequence.
    pub tables: Vec<UniFFIDocxTable>,
    /// Total computed word count.
    pub total_words: u32,
    /// Total computed character count.
    pub total_characters: u32,
    /// Rendered standard GitHub-Flavored Markdown representation.
    pub markdown_content: String,
}
