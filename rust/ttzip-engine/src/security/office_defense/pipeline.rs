// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Unified 6-Layer Office Defense-in-Depth Pipeline.
//!
//! Orchestrates formula depth/cycle fuses, spreadsheet dimensions/sparse OOM protection,
//! Shared String Table quotas, macro sandboxing/DDE interception, memory budgets, and sensitive zeroization.

use std::collections::HashMap;
use std::io::BufRead;

use super::{
    FormulaDepthGuard, FormulaInspection, MacroSanitizationReport, OfficeDefenseError,
    OfficeMacroSandboxGuard, OfficeMemoryBudgetGuard, OfficeMemoryPermit, SensitiveOfficeBuffer,
    SheetDimensionRange, SheetDimensionsGuard, SstInspectionReport, SstQuotaGuard,
    DEFAULT_MAX_DOCUMENT_BODY_BUDGET, DEFAULT_MAX_OFFICE_BUDGET,
    DEFAULT_MAX_SHEET_VIEWPORT_BUDGET, MAX_FORMULA_DEPTH, MAX_FORMULA_TOKENS, MAX_SHEET_COLS,
    MAX_SHEET_ROWS, MAX_SST_ENTRY_BYTES, MAX_SST_TOTAL_BYTES, MAX_SST_UNIQUE_ENTRIES,
    MAX_VIEWPORT_ACTIVE_CELLS,
};

/// Configuration options for the 6-layer Office security pipeline.
#[derive(Debug, Clone)]
pub struct OfficeSecurityConfig {
    pub max_formula_depth: usize,
    pub max_formula_tokens: usize,
    pub max_sheet_rows: u32,
    pub max_sheet_cols: u32,
    pub max_active_cells: usize,
    pub max_sst_unique_entries: usize,
    pub max_sst_entry_bytes: usize,
    pub max_sst_total_bytes: usize,
    pub max_global_budget: usize,
    pub max_sheet_viewport_budget: usize,
    pub max_document_body_budget: usize,
    pub strip_macros: bool,
    pub block_dde_formulas: bool,
    pub neutralize_remote_rels: bool,
}

impl Default for OfficeSecurityConfig {
    fn default() -> Self {
        Self {
            max_formula_depth: MAX_FORMULA_DEPTH,
            max_formula_tokens: MAX_FORMULA_TOKENS,
            max_sheet_rows: MAX_SHEET_ROWS,
            max_sheet_cols: MAX_SHEET_COLS,
            max_active_cells: MAX_VIEWPORT_ACTIVE_CELLS,
            max_sst_unique_entries: MAX_SST_UNIQUE_ENTRIES,
            max_sst_entry_bytes: MAX_SST_ENTRY_BYTES,
            max_sst_total_bytes: MAX_SST_TOTAL_BYTES,
            max_global_budget: DEFAULT_MAX_OFFICE_BUDGET,
            max_sheet_viewport_budget: DEFAULT_MAX_SHEET_VIEWPORT_BUDGET,
            max_document_body_budget: DEFAULT_MAX_DOCUMENT_BODY_BUDGET,
            strip_macros: true,
            block_dde_formulas: true,
            neutralize_remote_rels: true,
        }
    }
}

/// Comprehensive report emitted by the Office security pipeline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OfficeSecurityReport {
    pub stripped_entries: Vec<String>,
    pub inspected_formulas: usize,
    pub verified_dags: usize,
    pub sst_report: Option<SstInspectionReport>,
    pub macro_report: MacroSanitizationReport,
}

/// Unified 6-Layer Office Security Pipeline orchestrator.
#[derive(Debug)]
pub struct OfficeSecurityPipeline {
    pub formula_guard: FormulaDepthGuard,
    pub sheet_guard: SheetDimensionsGuard,
    pub sst_guard: SstQuotaGuard,
    pub macro_guard: OfficeMacroSandboxGuard,
    pub memory_guard: OfficeMemoryBudgetGuard,
    inspected_formulas_count: usize,
    verified_dags_count: usize,
    stripped_entries: Vec<String>,
}

impl Default for OfficeSecurityPipeline {
    fn default() -> Self {
        Self::new(OfficeSecurityConfig::default())
    }
}

impl OfficeSecurityPipeline {
    /// Creates a new Office security pipeline from configuration.
    pub fn new(config: OfficeSecurityConfig) -> Self {
        let formula_guard =
            FormulaDepthGuard::new(config.max_formula_depth, config.max_formula_tokens);
        let sheet_guard = SheetDimensionsGuard::new(
            config.max_sheet_rows,
            config.max_sheet_cols,
            config.max_active_cells,
        );
        let sst_guard = SstQuotaGuard::new(
            config.max_sst_unique_entries,
            config.max_sst_entry_bytes,
            config.max_sst_total_bytes,
        );
        let macro_guard = OfficeMacroSandboxGuard::new()
            .with_strip_macros(config.strip_macros)
            .with_block_dde(config.block_dde_formulas)
            .with_neutralize_remote_rels(config.neutralize_remote_rels);
        let memory_guard = OfficeMemoryBudgetGuard::new(
            config.max_global_budget,
            config.max_sheet_viewport_budget,
            config.max_document_body_budget,
        );

        Self {
            formula_guard,
            sheet_guard,
            sst_guard,
            macro_guard,
            memory_guard,
            inspected_formulas_count: 0,
            verified_dags_count: 0,
            stripped_entries: Vec::new(),
        }
    }

    /// Inspects an archive entry path; returns `true` if the entry is safe, or `false` if stripped.
    pub fn filter_archive_entry(&mut self, path: &str) -> bool {
        if self.macro_guard.should_strip_entry(path) {
            self.stripped_entries.push(path.to_string());
            false
        } else {
            true
        }
    }

    /// Validates a formula string for AST depth, token quota, and malicious DDE execution.
    pub fn validate_formula(&mut self, formula: &str) -> Result<FormulaInspection, OfficeDefenseError> {
        self.macro_guard.inspect_formula_security(formula)?;
        let inspection = self.formula_guard.inspect_formula(formula)?;
        self.inspected_formulas_count = self.inspected_formulas_count.saturating_add(1);
        Ok(inspection)
    }

    /// Verifies that a cell dependency graph is an acyclic DAG.
    pub fn verify_dependency_graph(
        &mut self,
        graph: &HashMap<String, Vec<String>>,
    ) -> Result<(), OfficeDefenseError> {
        self.formula_guard.verify_dependency_dag(graph)?;
        self.verified_dags_count = self.verified_dags_count.saturating_add(1);
        Ok(())
    }

    /// Validates sheet dimension bounds (e.g. "A1:XFD1048576").
    pub fn validate_sheet_dimension(
        &self,
        dim_ref: &str,
    ) -> Result<SheetDimensionRange, OfficeDefenseError> {
        self.sheet_guard.parse_and_validate_dimension(dim_ref)
    }

    /// Parses and verifies an SST XML stream.
    pub fn parse_sst_stream<R: BufRead>(
        &mut self,
        reader: R,
    ) -> Result<SstInspectionReport, OfficeDefenseError> {
        self.sst_guard.parse_sst_stream(reader)
    }

    /// Neutralizes / validates an OpenXML relationship target.
    pub fn sanitize_relationship_target(
        &self,
        target: &str,
        target_mode: Option<&str>,
        rel_type: &str,
    ) -> Result<String, OfficeDefenseError> {
        self.macro_guard
            .sanitize_relationship_target(target, target_mode, rel_type)
    }

    /// Allocates memory from the global budget watchdog.
    pub fn allocate_memory(&self, size: usize) -> Result<OfficeMemoryPermit<'_>, OfficeDefenseError> {
        self.memory_guard.allocate(size)
    }

    /// Wraps plain bytes into a secure, volatile zeroizing buffer.
    pub fn create_sensitive_buffer(&self, bytes: &[u8]) -> SensitiveOfficeBuffer {
        SensitiveOfficeBuffer::from_slice(bytes)
    }

    /// Compiles a summary report of pipeline actions taken.
    pub fn generate_report(&self) -> OfficeSecurityReport {
        OfficeSecurityReport {
            stripped_entries: self.stripped_entries.clone(),
            inspected_formulas: self.inspected_formulas_count,
            verified_dags: self.verified_dags_count,
            sst_report: Some(SstInspectionReport {
                unique_entries: self.sst_guard.unique_count(),
                total_references: 0,
                cumulative_bytes: self.sst_guard.cumulative_bytes(),
            }),
            macro_report: MacroSanitizationReport {
                stripped_macro_files: self.stripped_entries.clone(),
                stripped_activex_files: Vec::new(),
                blocked_dde_formulas: 0,
                neutralized_external_rels: 0,
            },
        }
    }
}
