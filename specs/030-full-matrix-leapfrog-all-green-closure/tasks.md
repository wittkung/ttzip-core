# Tasks: 030-full-matrix-leapfrog-all-green-closure

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `030-full-matrix-leapfrog-all-green-closure`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 初始化与历史最优峰值矩阵汇总

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 校验 `docs/benchmarks/peak_performance_matrix.json` 包含历史最高纪录

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心数据模型与契约校验

- [x] T003 [P] 校验数据模型定义 in `specs/030-full-matrix-leapfrog-all-green-closure/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约 in `specs/030-full-matrix-leapfrog-all-green-closure/contracts/all_green_closure.schema.json`

---

## Phase 3: User Story 1 - DMG / ISO 解压前 P-Core 提频调度 (Priority: P1) 🎯 MVP

**Goal**: 在 `CompetitorBenchmarkRunner.swift` 与 `SevenZipEngine.swift` 中注入解压前显式 `AppleSiliconTuner.shared.boostCurrentThreadPriority()`，锁定最高频 P-Core

**Independent Test**:
- 验证 DMG 100MB L6 解压吞吐恢复至 $\ge 9,556.6\text{ MB/s}$

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 与 `Sources/TTZipCore/SevenZip/SevenZipEngine.swift` 中注入解压前显式 `AppleSiliconTuner.shared.boostCurrentThreadPriority()`

---

## Phase 4: User Story 2 - WIM 纯 C 8MB 零拷贝极速通道 (Priority: P1)

**Goal**: 在 `ttzip_native_archive.c` 中保持 WIM 识别与直通特化 C 引擎

**Independent Test**:
- 验证 WIM 解压吞吐全线稳定在 $\ge 11,000\text{ MB/s}$

### Implementation for User Story 2

- [x] T006 [US2] 在 `Sources/CTTZipBridge/ttzip_native_archive.c` 中保持 WIM 识别与直通特化 C 引擎

---

## Phase 5: Polish & Full-Matrix Verification

**Purpose**: 全量回归验证与全矩阵大幅超越审计

- [x] T007 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T008 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T009 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证倒退清零与大幅超越
