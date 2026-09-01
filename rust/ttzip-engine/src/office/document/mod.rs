// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure Safe Rust Word DOCX document parsing and Markdown/plain text extraction microkernel.

pub mod parser;

pub use parser::{
    DocxAlignment, DocxBodyItem, DocxParagraph, DocxRun, DocxTable, DocxTableCell, DocxTableRow,
    TTZipDocxParser,
};
