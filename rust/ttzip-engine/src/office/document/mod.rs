// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Pure Safe Rust Word DOCX document parsing and Markdown/plain text extraction microkernel.

pub mod parser;

pub use parser::{
    DocxAlignment, DocxBodyItem, DocxParagraph, DocxRun, DocxTable, DocxTableCell, DocxTableRow,
    TTZipDocxParser,
};
