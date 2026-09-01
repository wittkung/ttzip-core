// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 2: Spreadsheet Dimension & Sparse Matrix OOM Safety Guard.
//!
//! Enforces hardware and format boundary constraints:
//! 1. Maximum row index <= 1,048,576 (2^20).
//! 2. Maximum column index <= 16,384 (2^14, column 'XFD').
//! 3. Viewport active non-empty cell limit <= 100,000.
//! 4. Anti-Sparse-Matrix OOM protection against malicious bounding box declarations (e.g. A1:XFD1048576).

use super::{OfficeDefenseError, MAX_SHEET_COLS, MAX_SHEET_ROWS, MAX_VIEWPORT_ACTIVE_CELLS};

/// Parsed sheet dimension bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetDimensionRange {
    pub start_col: u32,
    pub start_row: u32,
    pub end_col: u32,
    pub end_row: u32,
}

impl SheetDimensionRange {
    /// Returns the total potential cell count in the rectangular bounding span.
    pub fn theoretical_cell_count(&self) -> u64 {
        let cols = (self.end_col.saturating_sub(self.start_col) + 1) as u64;
        let rows = (self.end_row.saturating_sub(self.start_row) + 1) as u64;
        cols.saturating_mul(rows)
    }
}

/// Guard enforcing spreadsheet dimension bounds and active cell limits.
#[derive(Debug, Clone)]
pub struct SheetDimensionsGuard {
    max_rows: u32,
    max_cols: u32,
    max_active_cells: usize,
    current_active_cells: usize,
}

impl Default for SheetDimensionsGuard {
    fn default() -> Self {
        Self::new(MAX_SHEET_ROWS, MAX_SHEET_COLS, MAX_VIEWPORT_ACTIVE_CELLS)
    }
}

impl SheetDimensionsGuard {
    /// Creates a new guard with specified row, column, and active cell limits.
    pub const fn new(max_rows: u32, max_cols: u32, max_active_cells: usize) -> Self {
        Self {
            max_rows,
            max_cols,
            max_active_cells,
            current_active_cells: 0,
        }
    }

    /// Parses and validates an Excel dimension reference string (e.g. "A1:Z100" or "A1").
    pub fn parse_and_validate_dimension(
        &self,
        dim_ref: &str,
    ) -> Result<SheetDimensionRange, OfficeDefenseError> {
        let trimmed = dim_ref.trim();
        if trimmed.is_empty() {
            return Ok(SheetDimensionRange {
                start_col: 1,
                start_row: 1,
                end_col: 1,
                end_row: 1,
            });
        }

        let range = if let Some((start_str, end_str)) = trimmed.split_once(':') {
            let (sc, sr) = self.parse_single_cell_coord(start_str)?;
            let (ec, er) = self.parse_single_cell_coord(end_str)?;
            SheetDimensionRange {
                start_col: sc.min(ec),
                start_row: sr.min(er),
                end_col: sc.max(ec),
                end_row: sr.max(er),
            }
        } else {
            let (c, r) = self.parse_single_cell_coord(trimmed)?;
            SheetDimensionRange {
                start_col: 1,
                start_row: 1,
                end_col: c,
                end_row: r,
            }
        };

        self.validate_coordinates(range.end_col, range.end_row)?;
        Ok(range)
    }

    /// Validates raw row and column coordinates against sheet boundaries.
    pub fn validate_coordinates(&self, col: u32, row: u32) -> Result<(), OfficeDefenseError> {
        if col == 0 || col > self.max_cols {
            return Err(OfficeDefenseError::ColumnOutOfBounds {
                col,
                max_cols: self.max_cols,
            });
        }
        if row == 0 || row > self.max_rows {
            return Err(OfficeDefenseError::RowOutOfBounds {
                row,
                max_rows: self.max_rows,
            });
        }
        Ok(())
    }

    /// Registers one or more active non-empty cells in the stream, enforcing the active cell quota.
    pub fn register_active_cells(&mut self, count: usize) -> Result<(), OfficeDefenseError> {
        self.current_active_cells = self.current_active_cells.saturating_add(count);
        if self.current_active_cells > self.max_active_cells {
            return Err(OfficeDefenseError::ActiveCellsLimitExceeded {
                count: self.current_active_cells,
                limit: self.max_active_cells,
            });
        }
        Ok(())
    }

    /// Returns the currently registered active cell count.
    #[inline]
    pub fn active_cells(&self) -> usize {
        self.current_active_cells
    }

    /// Resets the active cell counter for a new sheet.
    pub fn reset(&mut self) {
        self.current_active_cells = 0;
    }

    /// Parses a single A1-format cell reference (e.g. "A1", "$C$45", "XFD1048576").
    fn parse_single_cell_coord(&self, cell_ref: &str) -> Result<(u32, u32), OfficeDefenseError> {
        let clean = cell_ref.replace('$', "");
        let mut col_str = String::new();
        let mut row_str = String::new();

        for ch in clean.chars() {
            if ch.is_ascii_alphabetic() {
                if !row_str.is_empty() {
                    return Err(OfficeDefenseError::InvalidCellReference(cell_ref.to_string()));
                }
                col_str.push(ch.to_ascii_uppercase());
            } else if ch.is_ascii_digit() {
                row_str.push(ch);
            } else {
                return Err(OfficeDefenseError::InvalidCellReference(cell_ref.to_string()));
            }
        }

        if col_str.is_empty() || row_str.is_empty() {
            return Err(OfficeDefenseError::InvalidCellReference(cell_ref.to_string()));
        }

        let col = col_str_to_index(&col_str)
            .ok_or_else(|| OfficeDefenseError::InvalidCellReference(cell_ref.to_string()))?;
        let row: u32 = row_str
            .parse()
            .map_err(|_| OfficeDefenseError::InvalidCellReference(cell_ref.to_string()))?;

        self.validate_coordinates(col, row)?;
        Ok((col, row))
    }
}

/// Converts an Excel column string to a 1-based column index (e.g. "A" -> 1, "Z" -> 26, "AA" -> 27, "XFD" -> 16384).
pub fn col_str_to_index(col_str: &str) -> Option<u32> {
    let mut index: u32 = 0;
    for &b in col_str.as_bytes() {
        if !b.is_ascii_alphabetic() {
            return None;
        }
        let val = (b.to_ascii_uppercase() - b'A' + 1) as u32;
        index = index.checked_mul(26)?.checked_add(val)?;
    }
    if index == 0 {
        None
    } else {
        Some(index)
    }
}
