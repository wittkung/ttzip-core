// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Pure Safe Rust Office spreadsheet (XLSX/XLS/XLSB/ODS) and Word DOCX microkernel engine.
//!
//! Provides zero-allocation streaming reading, Shared String Table (SST) parsing,
//! Pratt expression parsing, Tarjan SCC cycle detection, formula recalculation,
//! OpenXML XLSX generation, and Word DOCX structured DOM/Markdown extraction.

pub mod document;
pub mod spreadsheet;
pub mod types;

#[cfg(test)]
mod tests;

pub use document::{
    DocxAlignment, DocxBodyItem, DocxParagraph, DocxRun, DocxTable, DocxTableCell, DocxTableRow,
    TTZipDocxParser,
};
pub use spreadsheet::{
    CellFormat, FormulaExpr, TTZipFormulaEngine, TTZipSpreadsheetParser, TTZipSpreadsheetWriter,
};
pub use types::*;
