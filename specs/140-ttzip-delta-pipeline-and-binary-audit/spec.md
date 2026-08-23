# Feature Specification: TTZip Delta Pipeline & Automated Binary/Compression Audit

**Feature Branch**: `140-ttzip-delta-pipeline-and-binary-audit`  
**Created**: 2026-08-20  
**Status**: Specified  
**Input**: User directive: "我们也非常需要 (zlib-ng /delta 自动化差异审计与二进制/压缩率比对体系)"  

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Binary Footprint & Mach-O Section Delta Audit (Priority: P1)

As a systems engineer or release maintainer evaluating pull requests or kernel optimizations, I want an automated `ttzip-bench delta` (or `scripts/run_delta_audit.sh`) tool to compute binary size diffs, Mach-O section allocations (`__TEXT.__text`, `__TEXT.__stubs`, `__DATA.__data`, `__DATA.__bss`), and stripped file sizes between Git base (or baseline binary) and head, so that I can instantly detect code bloat or runaway template/inlining expansion.

**Why this priority**:
Ensures strict binary footprint governance across Apple Silicon (macOS) and Linux, preventing microarchitectural cache pollution caused by unintentional instruction bloat.

**Independent Test**:
Can be verified by running `ttzip-bench delta --base <baseline_dir_or_ref> --head <head_dir_or_ref>` and verifying that section breakdown tables (`__TEXT`, `__DATA`, `__BSS`) and stripped binary size deltas ($\\Delta\\%$) are accurately reported.

**Acceptance Scenarios**:
1. **Given** two binary builds (base and head), **When** running binary section delta analysis, **Then** output displays `.text`, `.data`, `.bss`, total decimal bytes, and $\\Delta\\%$ change.
2. **Given** macOS Darwin environment, **When** analyzing Mach-O binaries, **Then** `otool -l` / `size -m` or `llvm-size` is automatically resolved and executed without hardcoded paths.

---

### User Story 2 - Exported Dynamic Symbol Integrity & ABI Guard (Priority: P1)

As a framework architect or library developer, I want the delta tool to extract and diff exported public symbols (`nm -gU`), flagging any added symbols (`+`) or removed symbols (`-`), so that public C/Swift APIs remain strictly encapsulated and accidental global symbol leakage is blocked.

**Why this priority**:
Guarantees zero symbol namespace pollution in `libTTZipVendor.a`, `CTTZipBridge`, and CLI binaries.

**Independent Test**:
Can be tested by comparing symbol dumps between base and head and verifying that newly introduced non-static C symbols or removed public entry points are flagged in the report.

**Acceptance Scenarios**:
1. **Given** a PR that introduces or renames exported symbols, **When** running symbol diff, **Then** an exact list of added (`+`) and removed (`-`) dynamic symbols is presented.
2. **Given** zero symbol changes, **When** running symbol diff, **Then** report states "Exported symbols: 0 added, 0 removed".

---

### User Story 3 - Multi-Level Compression Ratio & Byte-Level Delta Matrix (Priority: P1)

As a compression algorithm researcher or performance engineer, I want the delta pipeline to compress standard corpora (Silesia text/corpus, synthetic RGB, DNA, mixed) across all supported compression levels (L1 to L12 for Deflate/libdeflate, L1 to L19 for Zstandard), and output a level-by-level byte delta table comparing Base vs. Head compressed sizes, so that any compression density regression or entropy coder discrepancy is surfaced immediately.

**Why this priority**:
Provides the exact equivalent of zlib-ng's Silesia L1..L9 compression table, catching subtle match-finder heuristic regressions that degrade output density.

**Independent Test**:
Can be verified by executing multi-level compression across base and head engines and confirming byte-accurate delta reporting with $\\Delta\\%$ calculations.

**Acceptance Scenarios**:
1. **Given** baseline and candidate codec engines, **When** evaluating multi-level compression, **Then** a Markdown table with columns `Level`, `Base (Bytes)`, `Head (Bytes)`, `Delta (Bytes)`, `Percent (%)` is produced.
2. **Given** a non-zero compression density loss ($> 0.10\\%$ byte expansion on deterministic corpus), **When** threshold check is active, **Then** the regression is highlighted with warning indicator.

---

### User Story 4 - GitHub Markdown PR-Ready Report Generation (Priority: P2)

As a developer opening a PR or a CI pipeline bot posting automated review comments, I want `ttzip-bench delta` to output a GitHub Flavored Markdown report featuring collapsible `<details>` tags for Total File Size, Compression Sizes, Exported Symbols, and Section breakdowns, so that it can be directly posted to GitHub PR discussions.

**Why this priority**:
Brings world-class open-source maintainer experience directly to TTZip's CI and developer workflows.

---

## Technical Invariants & Execution Bounds

- **Execution Speed**: Full local delta evaluation executes in $\\le 3.0\\text{ s}$.
- **Zero External Tool Dependency**: Native Swift + Darwin `size`/`nm` + in-process C bridges.
- **Cross-Platform Readiness**: Works on macOS Mach-O and Linux ELF without code modifications.
