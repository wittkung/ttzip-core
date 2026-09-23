// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Pure Safe Rust spreadsheet parsing, formula evaluation, and XLSX serialization microkernel.

pub mod formula;
pub mod parser;
pub mod writer;

pub use formula::{FormulaExpr, TTZipFormulaEngine};
pub use parser::TTZipSpreadsheetParser;
pub use writer::{CellFormat, TTZipSpreadsheetWriter};
