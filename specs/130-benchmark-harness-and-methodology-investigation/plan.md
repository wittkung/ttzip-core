# Implementation Plan: 130-benchmark-harness-and-methodology-investigation

## Technical Context
- **System Boundaries**:
  - `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/`
  - `Sources/TTZipCore/Benchmark/`
  - `scripts/` (Automated regression analysis and report generation)
- **Primary Tooling**: Google Benchmark C++ Framework, Apple Clang, Python 3 JSON analysis scripts.

---

## Constitution Check
- [x] **Zero-Cost Abstraction & Zero Allocation Invariance**: Timing loops execute with pre-allocated memory buffers, zero dynamic heap allocations during measurement.
- [x] **Platform Compatibility**: macOS 14.0+ Apple Silicon (M-series NEON hardware counters) + POSIX `CLOCK_MONOTONIC`.
- [x] **Logging Discipline**: Zero bare `printf`/`print` in production libraries; structured logging in benchmark harnesses.
- [x] **Verification Gates**: 100% test pass on CTest, GTest, and regression gates.

---

## Phase 0: Research Items
- - R001 [SUBAGENT:research] 《压缩基准测试的语料特征分类与内存隔离模型》: 调研 8 类数据类型与 RAM-to-RAM 隔离执行模型。 (Completed in `research.md`)
- - R002 [SUBAGENT:research] 《纳秒级微架构比对测试与缓存对齐方法学》: 调研 0..256B 长度扫描、64B 缓存对齐与 DoNotOptimize 内存屏障。 (Completed in `research.md`)
- - R003 [SUBAGENT:research] 《自动化基准数据对比、Pareto 前沿分析与 PR 报表生成规范》: 调研 Mann-Whitney U 检验、四级判定闸门与 Markdown 报表格式。 (Completed in `research.md`)

---

## Phase 1: Design & Specification Artifacts
- **Data Model**: `specs/130-benchmark-harness-and-methodology-investigation/data-model.md`
- **Contracts**:
  - `specs/130-benchmark-harness-and-methodology-investigation/contracts/benchmark_report.schema.json`
- **Validation Guide**: `specs/130-benchmark-harness-and-methodology-investigation/quickstart.md`

---

## Component Changes & File Modifications

### 1. Benchmark Harness & Corpora Support
- `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/benchmark_data_types.cc`
- `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/benchmark_deflate.cc`
- `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/benchmark_compare256.cc`

### 2. Analysis & Reporting Tooling
- `scratch/generate_reply.py`
- `scripts/audit_performance_regression.py`
