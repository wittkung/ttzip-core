# Implementation Plan: 全覆盖测试与基准遥测零回退体系 (Feature 162)

**Feature ID**: `162-full-coverage-testing-and-benchmark-framework`  
**Created**: 2026-08-20  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **Target Architecture**: Apple Silicon ARM64 (macOS Sonoma / Sequoia), Apple Clang `-O3`, Pure C11 Native Microkernel + SwiftPM Benchmark Tools.
- **Core Dependencies**:
  - Codecs: `libdeflate`, `zstd`, `fast-lzma2`, `lzfse`, `snappy`, `brotli` (macOS Compression.framework), `libbzip2`, `blosclz`.
  - Checksums: ACLE PMULL CRC32 12-way, NEON DotProd Adler-32 4-way, PMULL CRC64-XZ.
  - Formats: `CTTZipBridge_Archive.c`, `ttzip_tar_native.c`, `ttzip_tar_zstd_direct.c`, `CTTZipBridge_ZipWrite.c`, `CTTZipBridge_UnRAR.c`.

### 1.2 Constitution Check
- [x] **Zero Cloud Quota / 100% Local**: All benchmark and test runners execute 100% locally with zero cloud network calls.
- [x] **Strict Native Library Dominance**: Direct C bridge bindings to native libraries, zero black-box custom codec replacements.
- [x] **Zero Bare Objects & Schema Strictness**: JSON telemetry contract (`contracts/benchmark-telemetry-schema.json`) enforces strict draft-07 types, no bare `type: "object"`.
- [x] **Zero-Regression Floor**: Mandatory 5-gate pipeline ensuring zero performance and compression density regressions.

---

## 2. Phase 0 & Phase 1 Artifacts Index

- [x] **Phase 0 Research**: [`research.md`](research.md)
  - `- R001 [SUBAGENT:research] 《zlib-ng 微观切片与标准语料在 C11 中的扩展集成》`
  - `- R002 [SUBAGENT:research] 《libarchive 全格式正交解压与端到端 I/O 压测模型》`
  - `- R003 [SUBAGENT:research] 《5 重物理闸门与结构化 JSON 遥测契约规范》`
- [x] **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- [x] **Phase 1 Contract**: [`contracts/benchmark-telemetry-schema.json`](contracts/benchmark-telemetry-schema.json)
- [x] **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)

---

## 3. Component Breakdown & Planned Changes

### Component 1: C11 Native Benchmark Runner Expansion (`tests/c/`)
- [MODIFY] `tests/c/bench_codecs.c`: Expand to include LZ4 (L1/9), Brotli (Q6/9), Bzip2 (L1/9), and BloscLZ (L1/9), supporting all 8 standard corpora with dual-directional CPB calculation.
- [NEW] `tests/c/bench_formats.c`: Create container format packaging and extraction benchmarks for ZIP, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, 7Z, and UnRAR, capturing Peak RSS.
- [MODIFY] `tests/c/bench_main.c`: Add `--formats` and `--all` CLI flags to run the complete suite.
- [MODIFY] `CMakeLists.txt`: Register `bench_formats.c` under `ttzip_benchmark_runner`.

### Component 2: 5-Gate Zero-Regression Automation Script (`scripts/`)
- [NEW] `scripts/run_optimization_gate.sh`: Implement the 5-stage automated gate runner with `--bail`, `--stage`, and `--json` support.

---

## 4. Verification Plan

1. **Native C Suite**:
   - `cmake --build build && ./build/ttzip_c_test_runner all`
   - `./build/ttzip_benchmark_runner --all`
2. **5-Gate Execution**:
   - `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
3. **Contract Validation**:
   - Validate `build/gate_report.json` against `contracts/benchmark-telemetry-schema.json`.
