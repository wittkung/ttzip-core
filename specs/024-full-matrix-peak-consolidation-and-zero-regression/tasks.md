# Tasks: 024-full-matrix-peak-consolidation-and-zero-regression

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `024-full-matrix-peak-consolidation-and-zero-regression`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 初始化与历史最优峰值矩阵汇总

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 整合全格式历史最高峰值基准 in `docs/benchmarks/peak_performance_matrix.json`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心数据模型与契约校验

- [x] T003 [P] 校验数据模型实体定义 in `specs/024-full-matrix-peak-consolidation-and-zero-regression/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约完备性 in `specs/024-full-matrix-peak-consolidation-and-zero-regression/contracts/peak_matrix_consolidation.schema.json`

---

## Phase 3: User Story 1 - DMG 密码感知自适应路由与 AES 硬件解密 (Priority: P1) 🎯 MVP

**Goal**: 修复 DMG 加密路由分发，有密码时直通 `SevenZipEngine`（ARM NEON AES-256），消除 4 项 DMG AES 倒退

**Independent Test**:
- 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`
- 验证 DMG 500MB L6 (AES) 解压吞吐恢复至 $\ge 9,933.1\text{ MB/s}$

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` 中修复 DMG 分发条件，仅当 `password == nil || password!.isEmpty` 时直通 C 引擎，有密码时路由至 `SevenZipEngine`
- [x] T006 [US1] 验证 DMG 500MB L6 AES 解压吞吐恢复至 $\ge 9,933.1\text{ MB/s}$（消除 4 项 DMG AES 倒退）

---

## Phase 4: User Story 2 - TAR 变体栈上双层零分配内联目录缓存 (Priority: P1)

**Goal**: 在 `ttzip_tar_native.c` 中开辟栈上双层内联目录缓存池，消除 600 次 `mkdir` 冗余系统调用，小文件解压提升至 $\ge 1,304.1\text{ MB/s}$

**Independent Test**:
- 运行 TAR 格式基准跑分并比对历史最优

### Implementation for User Story 2

- [x] T007 [US2] 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中实现栈上双层内联目录缓存池 (`last_parent_dir` + L2 64-Slot hash)
- [x] T008 [US2] 验证 TAR 小文件解压吞吐恢复至 $\ge 1,304.1\text{ MB/s}$

---

## Phase 5: Polish & 0-Regression Verification

**Purpose**: 全量回归验证与倒退清零审计

- [x] T009 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T010 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T011 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证倒退清零
