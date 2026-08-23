# Feature Specification: ttzip-bench Multi-Format Matrix Expansion & Interactive Visualizations

**Feature Branch**: `139-ttzip-bench-matrix-expansion-and-visualization`  
**Created**: 2026-08-20  
**Status**: Specified  
**Input**: User directive: "先方案三（ttzip-bench 评测矩阵多格式扩展与可视化增强），其他我有安排"  

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Multi-Engine Unified In-Memory Matrix Expansion (Priority: P1)

As a compression engineer, performance architect, or systems researcher running `ttzip-bench`, I want to execute a comprehensive in-memory benchmark matrix spanning all primary compression engines (libdeflate, Zstandard, LZ4, Brotli, LZFSE, Snappy, Bzip2) across 8 deterministic corpora and 2 buffer scales (128KB and 1MB), so that I can evaluate throughput, compression ratio, and engine performance ceilings under 100% RAM-to-RAM isolation in $< 2.5\text{ s}$.

**Why this priority**:
Extends the baseline 50-point matrix into a full-spectrum compression taxonomy matrix, giving immediate visibility into how modern algorithms (tANS, LZ77, FSE, BWT, LZ4, Brotli) perform across distinct entropy regimes.

**Independent Test**:
Can be verified by executing `swift run ttzip-bench matrix` and verifying that all configured engines (libdeflate, zstd, lz4, brotli, lzfse, snappy, bzip2) execute cleanly with 100% integrity validation across 128KB and 1MB buffer tiers without disk I/O.

**Acceptance Scenarios**:
1. **Given** `ttzip-bench matrix`, **When** executed on macOS / Apple Silicon, **Then** all configured engine points run in RAM-to-RAM memory isolation with nanosecond timestamps, reporting Compression MB/s, Decompression MB/s, Compression Ratio %, and CV stability %.
2. **Given** `--filter-engine <names>` or `--filter-corpus <types>`, **When** specifying subsets (e.g. `ttzip-bench matrix --filter-engine zstd,brotli`), **Then** only matching points are executed.
3. **Given** `--json-out <path>`, **When** exporting results, **Then** a strictly formatted JSON report conforming to `matrix-telemetry.schema.json` is generated.

---

### User Story 2 - Interactive Vector SVG & Standalone HTML Pareto Visualizations (Priority: P1)

As a technical lead or documentation author, I want `ttzip-bench plot` to generate high-resolution interactive SVG vector graphics and standalone zero-dependency HTML dashboard visualizations, so that I can easily embed interactive Pareto frontier charts into project documentation, reports, and web presentations.

**Why this priority**:
Visual Pareto frontier plots are essential for communicating compression vs. speed trade-offs without requiring external Python/matplotlib dependencies.

**Independent Test**:
Can be verified by executing `swift run ttzip-bench plot --svg-out docs/benchmarks/pareto.svg` and `swift run ttzip-bench plot --html-out docs/benchmarks/dashboard.html` and validating rendering in browser/SVG viewers.

**Acceptance Scenarios**:
1. **Given** `ttzip-bench plot --svg-out <path>`, **When** generating SVG, **Then** an interactive SVG with CSS hover tooltips, color-coded engine series, logarithmic speed axes, and convex hull Pareto frontier lines is generated.
2. **Given** `ttzip-bench plot --html-out <path>`, **When** generating HTML, **Then** a single-file, zero-cloud-dependency HTML dashboard is produced featuring dark mode Zen styling, searchable data tables, and dynamic chart tooltips.
3. **Given** `ttzip-bench plot --terminal`, **When** executing in a terminal, **Then** a high-resolution Unicode Braille 2D scatter plot is rendered directly to stdout.

---

### User Story 3 - Regression Diffing & Automated Gate Differential (Priority: P2)

As a CI engineer or reviewer evaluating optimization pull requests, I want `ttzip-bench diff <baseline.json> <candidate.json>` (or `ttzip-bench matrix --diff <baseline.json>`) to compute point-by-point speed differentials ($\\Delta\\%$) and flag statistically significant regressions ($> 2.00\\%$ slowdown), so that performance regressions are caught before merging.

**Why this priority**:
Prevents microarchitectural performance degradation by providing automated, objective delta comparisons with configurable tolerance thresholds.

**Independent Test**:
Can be verified by running `ttzip-bench diff` on two JSON telemetry reports and verifying correct calculation of $\\Delta\\%$, regression flags, and non-zero exit codes when thresholds are breached.

**Acceptance Scenarios**:
1. **Given** baseline and candidate JSON reports, **When** running `ttzip-bench diff base.json cand.json`, **Then** a formatted table showing speed deltas, ratio deltas, and regression status is output.
2. **Given** a regression exceeding threshold (e.g. $> 2.0\\%$ drop on critical points), **When** `--gate` is active, **Then** the process exits with non-zero error code (`EX_SOFTWARE = 70`).

---

## Technical Invariants & Performance Bounds

- **100% In-Memory Isolation**: Zero disk reads/writes during benchmark point measurement loop.
- **Microsecond Monotonic Timing**: Hardware frequency-calibrated `PlatformMonotonicTimer`.
- **CV Stability Guarantee**: Median variation coefficient $CV \\le 1.50\\%$ across warm-up iterations.
- **Execution Speed**: Full extended matrix executes in $< 2.5\\text{ s}$ total execution time.
- **Zero Cloud / External JS Dependency**: HTML dashboard and SVG plotters must be completely self-contained (no CDN dependencies).
