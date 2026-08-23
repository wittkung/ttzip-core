# Tasks: 025-short-sample-stabilization-and-full-peak-clearing

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `025-short-sample-stabilization-and-full-peak-clearing`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 初始化与历史最优峰值矩阵汇总

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 校验 `docs/benchmarks/peak_performance_matrix.json` 包含历史最高纪录

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心数据模型与契约校验

- [x] T003 [P] 校验数据模型定义 in `specs/025-short-sample-stabilization-and-full-peak-clearing/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约 in `specs/025-short-sample-stabilization-and-full-peak-clearing/contracts/benchmark_sampling.schema.json`

---

## Phase 3: User Story 1 - 短时负载自适应多轮迭代采样与耗时下限修正 (Priority: P1) 🎯 MVP

**Goal**: 在 `CompetitorBenchmarkRunner.swift` 中为 $\le 10\text{MB}$ 负载实施 1 轮预热 + 3 轮采样取最佳耗时，将耗时安全下限调整为 `1e-6`

**Independent Test**:
- 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift` 中为 $\le 10\text{MB}$ 负载实施 1 轮预热 + 3 轮采样取最佳耗时，将 `max(0.001, ...)` 改为 `max(1e-6, ...)`
- [x] T006 [US1] 验证 10MB 日志及小文件基准采样的稳定性

---

## Phase 4: Polish & 0-Regression Verification

**Purpose**: 全量回归验证与倒退清零审计

- [x] T007 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T008 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T009 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证倒退清零
