# Tasks: 026-zero-regression-final-mile

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `026-zero-regression-final-mile`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 初始化与历史最优峰值矩阵汇总

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 校验 `docs/benchmarks/peak_performance_matrix.json` 包含历史最高纪录

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心数据模型与契约校验

- [x] T003 [P] 校验数据模型定义 in `specs/026-zero-regression-final-mile/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约 in `specs/026-zero-regression-final-mile/contracts/final_mile_audit.schema.json`

---

## Phase 3: User Story 1 - TAR / TAR.ZST 单文件与根目录条目快速路径旁路 (Priority: P1) 🎯 MVP

**Goal**: 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中为单文件/根目录条目实施 Fast-Path 旁路，跳过 `snprintf`、`strrchr`、FNV-1a 哈希与 `mkdir_cache`

**Independent Test**:
- 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`
- 验证 TAR 10MB 日志解压吞吐恢复至 $\ge 8,654.9\text{ MB/s}$

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中为单文件/根目录条目实施 Fast-Path 旁路，跳过 `snprintf`、`strrchr`、FNV-1a 哈希与 `mkdir_cache`
- [x] T006 [US1] 验证 TAR 10MB 日志解压吞吐恢复至 $\ge 8,654.9\text{ MB/s}$

---

## Phase 4: Polish & 0-Regression Verification

**Purpose**: 全量回归验证与倒退清零审计

- [x] T007 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T008 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T009 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证倒退清零
