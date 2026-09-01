// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified office document types, cell representations, coordinates, and error definitions.
//!
//! Provides strong algebraic data types for spreadsheet cell values, A1-notation cell coordinates,
//! rectangular cell ranges, office format identifiers, and domain-specific result/error types.

use std::fmt;
use thiserror::Error;

/// Recognized office document and spreadsheet formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OfficeFormat {
    /// Microsoft Excel OpenXML Spreadsheet (.xlsx).
    Xlsx,
    /// Microsoft Excel 97-2004 Binary Format (.xls).
    Xls,
    /// Microsoft Excel Binary Spreadsheet (.xlsb).
    Xlsb,
    /// OpenDocument Spreadsheet (.ods).
    Ods,
    /// Microsoft Word OpenXML Document (.docx).
    Docx,
    /// Unrecognized or unsupported office format.
    #[default]
    Unknown,
}

impl OfficeFormat {
    /// Detects office format from a file path or extension.
    pub fn from_path_or_extension(path: &str) -> Self {
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "xlsx" | "xlsm" | "xltx" | "xltm" => Self::Xlsx,
            "xls" | "xlt" => Self::Xls,
            "xlsb" => Self::Xlsb,
            "ods" | "fods" => Self::Ods,
            "docx" | "docm" | "dotx" | "dotm" => Self::Docx,
            _ => Self::Unknown,
        }
    }

    /// Returns true if the format is a spreadsheet format.
    #[inline]
    pub fn is_spreadsheet(&self) -> bool {
        matches!(self, Self::Xlsx | Self::Xls | Self::Xlsb | Self::Ods)
    }

    /// Returns true if the format is a word processing document format.
    #[inline]
    pub fn is_document(&self) -> bool {
        matches!(self, Self::Docx)
    }
}

/// Generic cell value across all spreadsheet implementations.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum OfficeCellValue {
    /// Empty / blank cell.
    #[default]
    Empty,
    /// Textual string value.
    String(String),
    /// 64-bit signed integer value.
    Int(i64),
    /// 64-bit IEEE-754 floating-point numeric value.
    Float(f64),
    /// Boolean flag value.
    Bool(bool),
    /// ISO-8601 formatted date or time string.
    DateTime(String),
    /// Spreadsheet error value (e.g. `#VALUE!`, `#REF!`, `#DIV/0!`, `#CYCLE!`).
    Error(String),
}

impl OfficeCellValue {
    /// Converts cell value to a user-presentable display string.
    pub fn as_string(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::String(s) => s.clone(),
            Self::Int(n) => n.to_string(),
            Self::Float(f) => {
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    format!("{:.0}", f)
                } else {
                    f.to_string()
                }
            }
            Self::Bool(b) => if *b { "TRUE".to_string() } else { "FALSE".to_string() },
            Self::DateTime(dt) => dt.clone(),
            Self::Error(e) => e.clone(),
        }
    }

    /// Attempts to extract or coerce numeric floating-point value.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Empty => Some(0.0),
            Self::Int(n) => Some(*n as f64),
            Self::Float(f) => Some(*f),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::String(s) => s.trim().parse::<f64>().ok(),
            Self::DateTime(_) | Self::Error(_) => None,
        }
    }

    /// Attempts to extract or coerce integer value.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Empty => Some(0),
            Self::Int(n) => Some(*n),
            Self::Float(f) => Some(*f as i64),
            Self::Bool(b) => Some(if *b { 1 } else { 0 }),
            Self::String(s) => s.trim().parse::<i64>().ok(),
            Self::DateTime(_) | Self::Error(_) => None,
        }
    }

    /// Attempts to extract or coerce boolean value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Empty => Some(false),
            Self::Bool(b) => Some(*b),
            Self::Int(n) => Some(*n != 0),
            Self::Float(f) => Some(*f != 0.0),
            Self::String(s) => {
                let lower = s.trim().to_ascii_lowercase();
                if lower == "true" || lower == "1" {
                    Some(true)
                } else if lower == "false" || lower == "0" {
                    Some(false)
                } else {
                    None
                }
            }
            Self::DateTime(_) | Self::Error(_) => None,
        }
    }

    /// Returns true if the cell is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns true if the cell contains an error.
    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

impl fmt::Display for OfficeCellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

/// 0-indexed cell coordinate with A1 notation representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OfficeCellAddress {
    /// 0-based row index (0 = Row 1).
    pub row: u32,
    /// 0-based column index (0 = Column A).
    pub col: u32,
    /// Canonical A1 notation (e.g. "A1", "Z25", "AA100").
    pub a1: String,
}

impl OfficeCellAddress {
    /// Creates a cell address from 0-based row and column indices.
    pub fn from_row_col(row: u32, col: u32) -> Self {
        let a1 = format!("{}{}", col_to_a1(col), row + 1);
        Self { row, col, a1 }
    }

    /// Parses a cell address from A1 notation (e.g. "B5", "$C$10").
    pub fn from_a1(a1_str: &str) -> Result<Self, OfficeError> {
        let trimmed = a1_str.trim().replace('$', "");
        if trimmed.is_empty() {
            return Err(OfficeError::CellParseError("Empty cell coordinate".to_string()));
        }

        let mut col_str = String::new();
        let mut row_str = String::new();

        for ch in trimmed.chars() {
            if ch.is_ascii_alphabetic() {
                if !row_str.is_empty() {
                    return Err(OfficeError::CellParseError(format!("Invalid cell coordinate: {a1_str}")));
                }
                col_str.push(ch.to_ascii_uppercase());
            } else if ch.is_ascii_digit() {
                row_str.push(ch);
            } else {
                return Err(OfficeError::CellParseError(format!("Invalid character in coordinate: {ch}")));
            }
        }

        if col_str.is_empty() || row_str.is_empty() {
            return Err(OfficeError::CellParseError(format!("Incomplete coordinate: {a1_str}")));
        }

        let col = a1_to_col(&col_str)?;
        let row_1based: u32 = row_str
            .parse()
            .map_err(|_| OfficeError::CellParseError(format!("Invalid row number: {row_str}")))?;

        if row_1based == 0 {
            return Err(OfficeError::CellParseError("Row index cannot be 0 in A1 notation".to_string()));
        }

        let row = row_1based - 1;
        let canonical_a1 = format!("{}{}", col_to_a1(col), row + 1);
        Ok(Self {
            row,
            col,
            a1: canonical_a1,
        })
    }
}

/// Converts a 0-based column index to an A1 column letters (0 -> "A", 25 -> "Z", 26 -> "AA").
pub fn col_to_a1(mut col: u32) -> String {
    let mut result = Vec::new();
    loop {
        let rem = (col % 26) as u8;
        result.push(b'A' + rem);
        if col < 26 {
            break;
        }
        col = (col / 26) - 1;
    }
    result.reverse();
    String::from_utf8(result).unwrap_or_else(|_| "A".to_string())
}

/// Converts A1 column letters to a 0-based column index ("A" -> 0, "Z" -> 25, "AA" -> 26).
pub fn a1_to_col(col_str: &str) -> Result<u32, OfficeError> {
    let mut col: u32 = 0;
    for ch in col_str.chars() {
        if !ch.is_ascii_alphabetic() {
            return Err(OfficeError::CellParseError(format!("Invalid column letter: {ch}")));
        }
        let val = (ch.to_ascii_uppercase() as u32) - ('A' as u32) + 1;
        col = col
            .checked_mul(26)
            .and_then(|c| c.checked_add(val))
            .ok_or_else(|| OfficeError::CellParseError("Column overflow".to_string()))?;
    }
    if col == 0 {
        return Err(OfficeError::CellParseError("Empty column string".to_string()));
    }
    Ok(col - 1)
}

/// 2D rectangular cell range within a specific worksheet.
#[derive(Debug, Clone, PartialEq)]
pub struct OfficeRange {
    /// Worksheet name.
    pub sheet_name: String,
    /// Top-left starting cell address.
    pub start: OfficeCellAddress,
    /// Bottom-right ending cell address.
    pub end: OfficeCellAddress,
    /// 2D grid of cell values indexed by [row_offset][col_offset].
    pub values: Vec<Vec<OfficeCellValue>>,
}

impl OfficeRange {
    /// Creates an empty range for a given worksheet.
    pub fn empty(sheet_name: &str) -> Self {
        Self {
            sheet_name: sheet_name.to_string(),
            start: OfficeCellAddress::from_row_col(0, 0),
            end: OfficeCellAddress::from_row_col(0, 0),
            values: Vec::new(),
        }
    }

    /// Returns the number of rows in the range.
    #[inline]
    pub fn row_count(&self) -> usize {
        self.values.len()
    }

    /// Returns the maximum column count across all rows.
    pub fn col_count(&self) -> usize {
        self.values.iter().map(|r| r.len()).max().unwrap_or(0)
    }

    /// Retrieves a cell value by absolute 0-based coordinates.
    pub fn get_cell(&self, row: u32, col: u32) -> Option<&OfficeCellValue> {
        if row < self.start.row || row > self.end.row || col < self.start.col || col > self.end.col {
            return None;
        }
        let r_offset = (row - self.start.row) as usize;
        let c_offset = (col - self.start.col) as usize;
        self.values.get(r_offset).and_then(|r| r.get(c_offset))
    }
}

/// Domain errors encountered during office document and spreadsheet operations.
#[derive(Debug, Error)]
pub enum OfficeError {
    /// Failure from underlying ZIP archive container.
    #[error("ZIP container error: {0:?}")]
    Zip(#[from] crate::types::TTZipStatus),

    /// XML tokenization or parsing error.
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// Standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Unsupported or unrecognized office document format.
    #[error("Unsupported office format: {0}")]
    UnsupportedFormat(String),

    /// Syntax or semantic error in spreadsheet formula.
    #[error("Formula syntax error: {0}")]
    InvalidFormula(String),

    /// Target worksheet was not found.
    #[error("Worksheet '{0}' not found")]
    SheetNotFound(String),

    /// Malformed cell coordinate or range.
    #[error("Cell coordinate parsing error: {0}")]
    CellParseError(String),

    /// Formula runtime evaluation error.
    #[error("Formula evaluation error: {0}")]
    EvaluationError(String),

    /// Circular dependency cycle detected in formula DAG.
    #[error("Circular dependency cycle detected: {0}")]
    CycleDetected(String),

    /// Corrupted document payload or invalid relationship structure.
    #[error("Corrupt office package: {0}")]
    Corrupt(String),

    /// UTF-8 encoding violation.
    #[error("UTF-8 decoding error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// Result type alias for office engine operations.
pub type OfficeResult<T> = Result<T, OfficeError>;
