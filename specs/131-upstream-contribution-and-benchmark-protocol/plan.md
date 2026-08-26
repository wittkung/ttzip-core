# Implementation Plan: Feature 131 - Upstream Contribution & Benchmark Verification Protocol

## Technical Context
- **Project**: TTZip & Upstream Contribution Pipeline
- **Target Subsystems**:
  1. `scripts/upstream_crossover_bench.py` - Automated dual cross-over runner with CMake flag parity verification.
  2. `scripts/upstream_report_gen.py` - Zero-hallucination markdown report generator binding directly to JSON AST.
  3. `.agents/rules/upstream-contribution.md` - Hard governance rule enforcing pair-programming sovereignty and remote write gates.

## Constitution Check
- Strict compliance with `.specify/memory/constitution.md`.
- Zero bare object schema declarations.
- Direct single source of truth data flow.

## Phase 0: Research & Architecture Decisions
- Completed in `research.md` (R001: Pre-flight flag parity, R002: Dual cross-over execution, R003: Zero-hallucination reporting, R004: Remote permission gate).

## Phase 1: Data Model & Contracts
- Completed in `data-model.md`, `contracts/benchmark-result.schema.json`, and `contracts/audit-gate.schema.json`.

## Phase 2: Implementation Breakdown
- **Component 1**: Dual Cross-Over Benchmark Runner (`scripts/upstream_crossover_bench.py`)
- **Component 2**: Zero-Hallucination Dynamic Report Generator (`scripts/upstream_report_gen.py`)
- **Component 3**: Upstream Contribution Rule & Permission Gate Update (`.agents/rules/upstream-contribution.md`)
