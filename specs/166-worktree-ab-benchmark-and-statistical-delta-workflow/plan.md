# Implementation Plan: 工业级 Git Worktree A/B 基准对标与统计显著性自动化流水线 (Feature 166)

**Feature ID**: `166-worktree-ab-benchmark-and-statistical-delta-workflow`  
**Created**: 2026-08-21  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **Toolchain**: Bash/POSIX Shell (`set -euo pipefail`), Python 3 Standard Library (`math`, `statistics`, `json`, `sys`, `argparse`), CMake 3.20+, Apple Clang / GCC.
- **Core Principles**:
  - Zero repository contamination via `git worktree add --detach` and strict signal trap cleanup.
  - Interleaved sampling ($B \to C \to B \to C$) to eliminate thermal throttling and CPU frequency scaling bias.
  - Zero-dependency Welch's t-test with Lentz continued fraction for exact p-values.
  - Dual ANSI terminal + GitHub Markdown + Draft-07 JSON Schema reporting.

### 1.2 Constitution Check
- [x] **Zero Cloud Quota / 100% Local**: All worktrees, builds, and statistics run purely on local hardware.
- [x] **Zero External Python Dependency**: Strictly pure Python standard library (`math`, `statistics`, `json`).
- [x] **Zero Bare Objects & Schema Strictness**: JSON telemetry contract (`contracts/ab-bench-report-schema.json`) enforces strict draft-07 types.
- [x] **Robust Error Handling**: Traps ensure 0 orphan worktrees or build caches remain on disk.

---

## 2. Phase 0 & Phase 1 Artifacts Index

- [x] **Phase 0 Research**: [`research.md`](research.md)
  - `- R001 [SUBAGENT:research] 《Git Worktree 生命周期与 POSIX Shell 信号捕获隔离机制》`
  - `- R002 [SUBAGENT:research] 《零外部依赖纯 Python Welch t 检验与 p-value 连分数计算引擎》`
  - `- R003 [SUBAGENT:research] 《双模态终端彩色表格与 Markdown/JSON 契约报告规范》`
- [x] **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- [x] **Phase 1 Contract**: [`contracts/ab-bench-report-schema.json`](contracts/ab-bench-report-schema.json)
- [x] **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)

---

## 3. Component Breakdown & Planned Changes

### Component 1: Statistical Delta Calculation Engine (`scripts/statistical_delta.py`)
- [NEW] `scripts/statistical_delta.py`: Pure Python 3 statistical calculator implementing Welch's t-test, degrees of freedom, Lentz continued fraction incomplete beta function, ANSI coloring, and Markdown/JSON generation.

### Component 2: Master Worktree A/B Orchestration Script (`scripts/benchmark_ab.sh`)
- [NEW] `scripts/benchmark_ab.sh`: Shell script orchestrating worktree creation, build flags, warm-up run, interleaved sampling loop, and calling `statistical_delta.py`.

### Component 3: JSON Telemetry Output in Benchmark Runner (`tests/c/bench_main.c`)
- [MODIFY] `tests/c/bench_main.c`: Add `--json <output_path>` flag support to write structured telemetry arrays for automated ingestion by `statistical_delta.py`.

---

## 4. Verification Plan

1. **A/B Benchmark Trial**:
   - `./scripts/benchmark_ab.sh HEAD~1 HEAD --runs 3`
   - Verify worktree is cleanly removed and report is generated in `reports/`.
2. **5-Gate Pipeline Compliance**:
   - `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
