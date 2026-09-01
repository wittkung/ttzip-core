// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust spreadsheet parsing, formula evaluation, and XLSX serialization microkernel.

pub mod formula;
pub mod parser;
pub mod writer;

pub use formula::{FormulaExpr, TTZipFormulaEngine};
pub use parser::TTZipSpreadsheetParser;
pub use writer::{CellFormat, TTZipSpreadsheetWriter};
