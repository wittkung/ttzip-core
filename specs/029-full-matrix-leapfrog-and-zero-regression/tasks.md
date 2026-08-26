# Tasks: 029-full-matrix-leapfrog-and-zero-regression

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `029-full-matrix-leapfrog-and-zero-regression`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 初始化与历史最优峰值矩阵汇总

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 校验 `docs/benchmarks/peak_performance_matrix.json` 包含历史最高纪录

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心数据模型与契约校验

- [x] T003 [P] 校验数据模型定义 in `specs/029-full-matrix-leapfrog-and-zero-regression/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约 in `specs/029-full-matrix-leapfrog-and-zero-regression/contracts/leapfrog_audit.schema.json`

---

## Phase 3: User Story 1 - APFS 场景级延迟集中清理 (Priority: P1) 🎯 MVP

**Goal**: 在 `CompetitorBenchmarkRunner.swift` 中实施场景级延迟集中清理，各 Pass 路径完全正交独立，运行期间严禁 `removeItem`，彻底消除 APFS 锁争用

**Independent Test**:
- 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 中实施场景级延迟集中清理，各 Pass 路径完全正交独立，运行期间严禁 `removeItem`

---

## Phase 4: User Story 2 - WIM 纯 C 原生极速直通 (Priority: P1)

**Goal**: 在 `ttzip_native_archive.c` 中优化 WIM 识别与流式解压

**Independent Test**:
- 验证 WIM 解压吞吐全线越过 $\ge 10,000\text{ MB/s}$

### Implementation for User Story 2

- [x] T006 [US2] 在 `Sources/CTTZipBridge/ttzip_native_archive.c` 中优化 WIM 识别与流式解压

---

## Phase 5: Polish & 0-Regression Verification

**Purpose**: 全量回归验证与倒退清零审计

- [x] T007 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T008 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T009 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证倒退彻底清零
