# Implementation Plan: TTZip Delta Pipeline & Automated Binary/Compression Audit

**Feature Directory**: `specs/140-ttzip-delta-pipeline-and-binary-audit`  
**Status**: Ready  

---

## 1. Technical Context & Constitution Check

### Technical Context
- **Target Goal**: Build an automated `/delta` style audit engine for TTZip (via `ttzip-bench delta` & `scripts/run_delta_audit.sh`) that evaluates:
  1. Mach-O / ELF binary size, `.text`/`.data`/`.bss` sections, stripped footprint (`strip -x`), and exported symbol differential (`nm -gU`).
  2. Multi-level compression size delta matrix across 4 deterministic corpora on Deflate (L1..L12), Zstd (L1..L19), and Bzip2 (L1..L9) — 160 total verification points.
  3. GitHub PR-ready Markdown generation with collapsible `<details>` tags matching upstream zlib-ng standards.
- **Performance Ceiling**: Full delta run must complete in $\le 3.0\text{ s}$.

### Constitution Check
- [x] Principle 1: 100% in-memory deterministic corpus generation and RAM-to-RAM compression.
- [x] Principle 2: Safe, robust process invocation of system toolchain binaries (`size`, `nm`, `strip`, `git`) with fallback resolution.
- [x] Principle 3: Pre-push local CI/CD gate compatibility.

---

## 2. Phase 0 & Phase 1 Artifacts

- **Phase 0 Research**: `specs/140-ttzip-delta-pipeline-and-binary-audit/research.md` (R001: Mach-O/ELF Inspection, R002: Multi-Level Ratio Matrix, R003: Git & GFM Report Format).
- **Phase 1 Data Models**: `specs/140-ttzip-delta-pipeline-and-binary-audit/data-model.md`.
- **Phase 1 Contracts**: `specs/140-ttzip-delta-pipeline-and-binary-audit/contracts/delta-audit-schema.json`.
- **Phase 1 Quickstart**: `specs/140-ttzip-delta-pipeline-and-binary-audit/quickstart.md`.

---

## 3. Component Breakdown & Modification Plan

### Component 1: Binary Section & Symbol Inspector (`Sources/TTZipBench/Audit/`)
- [NEW] `Sources/TTZipBench/Audit/BinaryInspector.swift`: Executes `size -m` / `otool -l`, `nm -gU`, and `strip -x` to extract section breakdown and exported symbol set.

### Component 2: Multi-Level Compression Delta Engine (`Sources/TTZipBench/Audit/`)
- [NEW] `Sources/TTZipBench/Audit/CompressionDeltaEngine.swift`: Runs in-memory 160-point multi-level sweep across Deflate, Zstd, Bzip2, recording exact byte sizes and calculating deltas.

### Component 3: Markdown & GitHub PR Card Generator (`Sources/TTZipBench/Audit/`)
- [NEW] `Sources/TTZipBench/Audit/DeltaReportFormatter.swift`: Generates zlib-ng-style GitHub PR Markdown cards with `<details open>` and `<details>` collapsible sections.

### Component 4: CLI Subcommand Integration (`Sources/TTZipBench/`)
- [MODIFY] `Sources/TTZipBench/main.swift`: Add `delta` subcommand with `--markdown-out`, `--json-out`, `--fail-pct` options.

### Component 5: Shell Script Wrapper & Tests (`scripts/`, `Tests/`)
- [NEW] `scripts/run_delta_audit.sh`: Automated pre-flight check script.
- [NEW] `Tests/TTZipTests/DeltaAuditEngineTests.swift`: Comprehensive unit tests verifying binary snapshot and compression delta calculations.
