// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Office 6-Layer Defense-in-Depth Security Subsystem.
//!
//! Enforces deterministic container insulation, formula depth fuses, spreadsheet dimension bounds,
//! Shared String Table quotas, macro sandboxing/DDE interception, memory budgets, and sensitive zeroization:
//! 1. **Formula Depth & Tarjan Cycle Guard** ([`FormulaDepthGuard`]):
//!    AST nesting depth <= 32 levels, token count <= 1,024, and Tarjan SCC circular dependency detection.
//! 2. **Sheet Dimensions & Sparse Matrix Guard** ([`SheetDimensionsGuard`]):
//!    Row <= 1,048,576, Column <= 16,384 ('XFD'), active viewport cells <= 100,000 to prevent sparse matrix OOM.
//! 3. **Shared String Table (SST) Quota Guard** ([`SstQuotaGuard`]):
//!    Unique strings <= 500,000, single string <= 32 KiB, cumulative memory <= 32 MiB, HashDOS resistant.
//! 4. **Office Macro & DDE Sandbox Guard** ([`OfficeMacroSandboxGuard`]):
//!    Physical stripping of `vbaProject.bin`/ActiveX, blocking `=cmd|`/DDE vectors, neutralizing UNC/remote templates.
//! 5. **Office Memory Budget Watchdog** ([`OfficeMemoryBudgetGuard`]):
//!    Systemic memory watchdog enforcing <= 64 MiB resident task budget and modular viewport limits.
//! 6. **Sensitive Office Memory Buffer** ([`SensitiveOfficeBuffer`]):
//!    Zero-allocation / zeroize-on-drop volatile memory wiping for decrypted Office documents.

mod formula_depth;
mod macro_sandbox;
mod memory_budget;
mod pipeline;
mod sensitive;
mod sheet_dims;
mod sst_quota;

#[cfg(test)]
mod tests;

pub use formula_depth::{
    col_index_to_str, CellCoord, FormulaDepthGuard, FormulaInspection,
};
pub use macro_sandbox::{MacroSanitizationReport, OfficeMacroSandboxGuard};
pub use memory_budget::{OfficeMemoryBudgetGuard, OfficeMemoryPermit};
pub use pipeline::{OfficeSecurityConfig, OfficeSecurityPipeline, OfficeSecurityReport};
pub use sensitive::SensitiveOfficeBuffer;
pub use sheet_dims::{col_str_to_index, SheetDimensionRange, SheetDimensionsGuard};
pub use sst_quota::{SstInspectionReport, SstQuotaGuard};

// ============================================================================
// Defense Constants & Limits
// ============================================================================

/// Default maximum allowable formula AST nesting depth (32 levels).
pub const MAX_FORMULA_DEPTH: usize = 32;

/// Default maximum allowable formula token count (1,024 tokens).
pub const MAX_FORMULA_TOKENS: usize = 1024;

/// Maximum allowable rows in an ECMA-376 / XLSX sheet (1,048,576 rows).
pub const MAX_SHEET_ROWS: u32 = 1_048_576;

/// Maximum allowable columns in an ECMA-376 / XLSX sheet (16,384 columns, column XFD).
pub const MAX_SHEET_COLS: u32 = 16_384;

/// Default maximum allowable active non-empty cells in a sheet viewport slice (100,000 cells).
pub const MAX_VIEWPORT_ACTIVE_CELLS: usize = 100_000;

/// Default maximum allowable unique strings in Shared String Table (500,000 entries).
pub const MAX_SST_UNIQUE_ENTRIES: usize = 500_000;

/// Default maximum allowable length for a single SST string entry (32 KiB).
pub const MAX_SST_ENTRY_BYTES: usize = 32 * 1024;

/// Default maximum allowable total cumulative memory for SST (32 MiB).
pub const MAX_SST_TOTAL_BYTES: usize = 32 * 1024 * 1024;

/// Default maximum allowable global resident memory for an Office task (64 MiB).
pub const DEFAULT_MAX_OFFICE_BUDGET: usize = 64 * 1024 * 1024;

/// Default maximum allowable memory budget for a single worksheet viewport (16 MiB).
pub const DEFAULT_MAX_SHEET_VIEWPORT_BUDGET: usize = 16 * 1024 * 1024;

/// Default maximum allowable memory budget for document body text & outline (32 MiB).
pub const DEFAULT_MAX_DOCUMENT_BODY_BUDGET: usize = 32 * 1024 * 1024;

// ============================================================================
// Defense Error Models
// ============================================================================

/// Errors emitted when Office security invariants, memory fuses, or format limits are breached.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OfficeDefenseError {
    /// Formula AST nesting depth exceeded the safety ceiling.
    #[error("Formula expression nesting depth {depth} exceeds security limit {limit}")]
    FormulaDepthExceeded { depth: usize, limit: usize },

    /// Formula token count exceeded the safety ceiling.
    #[error("Formula token count {tokens} exceeds security limit {limit}")]
    FormulaTokensExceeded { tokens: usize, limit: usize },

    /// Circular dependency loop detected in formula evaluation graph.
    #[error("Circular formula dependency cycle detected: '{cycle}'")]
    FormulaCycleDetected { cycle: String },

    /// Worksheet row index out of standard bounds.
    #[error("Worksheet row index {row} exceeds maximum allowable rows {max_rows}")]
    RowOutOfBounds { row: u32, max_rows: u32 },

    /// Worksheet column index out of standard bounds.
    #[error("Worksheet column index {col} exceeds maximum allowable columns {max_cols}")]
    ColumnOutOfBounds { col: u32, max_cols: u32 },

    /// Invalid cell reference string encountered.
    #[error("Invalid cell reference coordinate: '{0}'")]
    InvalidCellReference(String),

    /// Active non-empty cell count exceeded the viewport safety ceiling.
    #[error("Worksheet active cells count {count} exceeds viewport ceiling {limit}")]
    ActiveCellsLimitExceeded { count: usize, limit: usize },

    /// Unique entries count in Shared String Table exceeded limit.
    #[error("Shared String Table unique entries count {count} exceeds limit {limit}")]
    SstUniqueEntriesExceeded { count: usize, limit: usize },

    /// Single string entry in SST exceeded maximum byte length.
    #[error("SST string entry length {len} bytes exceeds maximum limit {limit} bytes")]
    SstEntryTooLarge { len: usize, limit: usize },

    /// Total cumulative byte size of SST exceeded memory ceiling.
    #[error("SST total memory usage {total} bytes exceeds security ceiling {limit} bytes")]
    SstTotalBytesExceeded { total: usize, limit: usize },

    /// Dangerous DDE / command execution formula intercepted.
    #[error("Blocked dangerous DDE or command execution formula: '{formula}'")]
    DdeCommandBlocked { formula: String },

    /// Malicious formula payload or dangerous protocol intercepted.
    #[error("Blocked malicious formula payload or protocol scheme: '{formula}'")]
    DangerousFormulaPayload { formula: String },

    /// External UNC path relationship neutralized to prevent NTLM hash leaking.
    #[error("Neutralized dangerous UNC path target: '{target}'")]
    UncPathNeutralized { target: String },

    /// Remote template injection relationship neutralized.
    #[error("Neutralized dangerous remote template injection target: '{target}'")]
    RemoteTemplateNeutralized { target: String },

    /// Dangerous external relationship target blocked.
    #[error("Blocked dangerous external relationship target '{target}' for type '{rel_type}'")]
    DangerousRelationshipTarget { target: String, rel_type: String },

    /// Global Office memory budget ceiling exceeded.
    #[error("Office memory budget exceeded: requested {requested} bytes + allocated {current_allocated} bytes exceeds ceiling {limit} bytes")]
    MemoryBudgetExceeded {
        requested: usize,
        current_allocated: usize,
        limit: usize,
    },

    /// Single worksheet uncompressed size exceeded viewport budget limit.
    #[error("Worksheet content size {size} bytes exceeds viewport budget limit {limit} bytes")]
    SheetExceedsViewportLimit { size: usize, limit: usize },

    /// Document body uncompressed size exceeded limit.
    #[error("Document content size {size} bytes exceeds budget limit {limit} bytes")]
    DocumentExceedsLimit { size: usize, limit: usize },

    /// Malformed XML or broken Office Open XML package structure.
    #[error("Malformed Office Open XML syntax: {0}")]
    MalformedXml(String),

    /// Underlying parser or I/O error occurred.
    #[error("Underlying I/O or parser error: {0}")]
    ParserError(String),
}
