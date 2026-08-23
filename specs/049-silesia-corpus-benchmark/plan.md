# Implementation Plan: Silesia Corpus Standard Benchmark Fixtures & Regression Gates

**Branch**: `049-silesia-corpus-benchmark` | **Date**: 2026-08-17 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/spec.md)

**Input**: Feature specification from `specs/049-silesia-corpus-benchmark/spec.md`

## Summary

Integrate the complete 12-file Silesia Compression Corpus (~211.95 MB) into `Tests/TTZipTests/Fixtures/Silesia/` as gold-standard benchmark fixtures. Build a zero-copy fixture loader (`SilesiaFixtureLoader`), cryptographic manifest verification (`silesia_manifest.json`), statistical multi-round benchmark runner (`SilesiaCorpusBenchmarkSuite`), and automated CI performance regression gate asserting a hard $\le 3.0\%$ throughput regression floor on Apple Silicon and Windows/MSVC platforms.

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs

**Primary Dependencies**: In-process `CTTZipBridge`, `Vendor/*.a` (libarchive, libdeflate, LZMA SDK, zstd), SPM `Bundle.module`

**Storage**: Local immutable static test fixtures under `Tests/TTZipTests/Fixtures/Silesia/`

**Testing**: XCTest with `TTZIP_RUN_BENCHMARKS=1` gate, `IsolatedTempSandbox` for zero side-effect scratch storage

**Target Platform**: macOS 14.0+ (Apple Silicon UMA prioritized, Intel compatible) & Windows (MSVC/NTFS compatibility)

**Project Type**: Compression & Archive Engine Test Harness & Performance Gate

**Performance Goals**: Zero-copy I/O throughput ($> 2,000 \text{ MB/s}$ ZIP L1, $> 3,900 \text{ MB/s}$ 7Z L1, $> 22,000 \text{ MB/s}$ TAR.ZST), $CV \le 2.5\%$, regression threshold $\le 3.0\%$

**Constraints**: Zero intermediate heap allocations on hot paths, zero network dependencies in CI, strictly non-destructive RAII test sandboxes

**Scale/Scope**: 12 standardized files, 211,945,550 bytes total uncompressed payload

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Zero-Cost Abstraction on Hot Paths**: Fixture loader passes direct POSIX paths for C `mmap` or uses Swift `.alwaysMapped` without intermediate copies.
- [x] **No Shared Locks on Parallel Paths**: Benchmark suite runs isolated per-iteration sandboxes without global mutexes.
- [x] **Fast-Path Bypass Preservation**: Direct streaming compression/decompression fast paths preserved across all tested formats.
- [x] **Zero Regression Floor Enforcement**: Any throughput drop $> 3.0\%$ compared to historical baseline triggers CI test failure.
- [x] **Stream-First & Bounds-First**: Files are streamed with page-aligned buffers; memory footprint remains $\le 64\text{MB}$.
- [x] **Oracle-First**: Real 12-file heterogeneous entropy corpus replaces synthetic Mock data for true golden benchmarks.

## Phase 0: Outline & Research

- R001 [SUBAGENT:research] 《Silesia 语料库目录与校验 Manifest》：12 种典型真实场景文件清单、大小、SHA-256 校验和与静态资源打包方式。详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/research.md#r001-silesia-dataset-catalog-exact-file-roster--checksum-manifest)
- R002 [SUBAGENT:research] 《基准测试执行架构与防抖动门禁》：1 次预热 + 3 次采样中位数过滤、变异系数 $CV \le 2.5\%$ 与 3.0% 硬回归门禁。详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/research.md#r002-benchmark-execution-architecture-warmup-protocols--zero-regression-gating)
- R003 [SUBAGENT:research] 《零拷贝资源加载与 Swift 6.0 并发安全》：`Bundle.module` / `#filePath` 三级回退加载器、C `mmap` 与 Swift `alwaysMapped` 内存安全模型。详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/research.md#r003-memory-safe-zero-copy-fixture-loading--spm-file-resolution)

## Phase 1: Design & Contracts

- Data Model: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/data-model.md)
- Schema Contracts [SUBAGENT:research]:
  - `contracts/silesia_manifest.schema.json`: [silesia_manifest.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/contracts/silesia_manifest.schema.json)
  - `contracts/benchmark_report.schema.json`: [benchmark_report.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/contracts/benchmark_report.schema.json)
- Quickstart Guide: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/049-silesia-corpus-benchmark/quickstart.md)

## Project Structure

### Documentation (this feature)

```text
specs/049-silesia-corpus-benchmark/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 JSON schemas
│   ├── silesia_manifest.schema.json
│   └── benchmark_report.schema.json
└── checklists/
    └── requirements.md  # Spec quality checklist
```

### Source Code Changes

```text
Tests/TTZipTests/
├── Fixtures/
│   └── Silesia/                                    # [NEW] 12 Silesia files & manifest
│       ├── silesia_manifest.json                   # [NEW] Checksum & metadata manifest
│       ├── dickens, mozilla, mr, nci, ooffice...   # [NEW] 12 physical benchmark files
├── SilesiaFixtureLoader.swift                      # [NEW] 3-tier zero-copy fixture loader
├── SilesiaCorpusIntegrityTests.swift               # [NEW] Cryptographic & byte-size assertions
└── SilesiaCorpusBenchmarkSuiteTests.swift          # [NEW] Multi-format zero-regression gate
```

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
| :--- | :--- | :--- |
| None | N/A | N/A |
