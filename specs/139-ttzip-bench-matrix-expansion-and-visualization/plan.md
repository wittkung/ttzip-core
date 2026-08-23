# Implementation Plan: ttzip-bench Multi-Format Matrix Expansion & Interactive Visualizations

**Feature Directory**: `specs/139-ttzip-bench-matrix-expansion-and-visualization`  
**Status**: Ready  

---

## 1. Technical Context & Constitution Check

### Technical Context
- **Target Goal**: Expand `ttzip-bench` from 3 codecs (50 points) to full multi-codec coverage (74 points across libdeflate, zstd, lz4, lzfse, snappy, brotli, bzip2), add zero-dependency interactive SVG & HTML5 Zen UI dashboard generation (`ttzip-bench plot`), and provide regression diff comparison tooling (`ttzip-bench diff`).
- **Performance Budget**: Full 74-point matrix must execute in $\le 2.0\text{ s}$ RAM-to-RAM.
- **Dependency Isolation**: 100% self-contained HTML/SVG generation with 0 external CDN requests.

### Constitution Check
- [x] Principle 1: Pure in-memory RAM-to-RAM benchmarking without disk I/O noise.
- [x] Principle 2: Zero heap allocations during inner benchmark timing loop.
- [x] Principle 3: Pre-push 6-stage CI gate passing cleanly with automated regression detection.

---

## 2. Phase 0 & Phase 1 Artifacts

- **Phase 0 Research**: `specs/139-ttzip-bench-matrix-expansion-and-visualization/research.md` (R001: Multi-Engine APIs, R002: Zen UI HTML5/SVG Architecture, R003: Diff Regression Gating).
- **Phase 1 Data Models**: `specs/139-ttzip-bench-matrix-expansion-and-visualization/data-model.md`.
- **Phase 1 Contracts**:
  - `specs/139-ttzip-bench-matrix-expansion-and-visualization/contracts/matrix-telemetry.schema.json`
  - `specs/139-ttzip-bench-matrix-expansion-and-visualization/contracts/matrix-diff-report.schema.json`
- **Phase 1 Quickstart**: `specs/139-ttzip-bench-matrix-expansion-and-visualization/quickstart.md`.

---

## 3. Component Breakdown & Modification Plan

### Component 1: C Bridge Codec Adapters (`Sources/CTTZipBridge/`)
- [MODIFY] `Sources/CTTZipBridge/include/CTTZipStreamCoder.h`: Expose C prototypes for `ttzip_brotli_compress`, `ttzip_bzip2_compress`.
- [MODIFY] `Sources/CTTZipBridge/CTTZipStreamCoder.c`: Implement zero-allocation wrappers around `compression_encode_buffer` / `BZ2_bzBuffToBuffCompress`.

### Component 2: Unified In-Memory Matrix Expansion (`Sources/TTZipCore/Benchmark/`)
- [MODIFY] `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift`: Expand `run50PointMatrix()` into `runUnifiedMatrix()` with 74 points spanning all 7 engines with backward-compatible alias.

### Component 3: Visualization Engines (`Sources/TTZipBench/Plotters/`)
- [MODIFY] `Sources/TTZipBench/Plotters/SVGParetoPlotter.swift`: Enhance interactive SVG with CSS hover tooltips and dynamic color maps.
- [NEW] `Sources/TTZipBench/Plotters/HTMLParetoDashboardGenerator.swift`: Generate standalone zero-dependency Zen UI HTML dashboard.

### Component 4: CLI Router & Subcommands (`Sources/TTZipBench/`)
- [MODIFY] `Sources/TTZipBench/main.swift`: Add support for `plot --html-out <path>`, `plot --svg-out <path>`, and `diff <base.json> <cand.json>`.

### Component 5: Tests & Verification (`Tests/TTZipTests/`)
- [MODIFY] `Tests/TTZipTests/TTZipCoreCodecBenchmarkTests.swift`: Verify 74-point matrix execution and JSON schema conformity.
