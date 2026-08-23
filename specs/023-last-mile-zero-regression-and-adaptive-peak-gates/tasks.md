# Tasks: 023-last-mile-zero-regression-and-adaptive-peak-gates

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `023-last-mile-zero-regression-and-adaptive-peak-gates`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 初始化与基线数据加载

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 加载 `docs/benchmarks/peak_performance_matrix.json` 与历史最优基线

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心数据模型与契约校验

- [x] T003 [P] 校验数据模型实体定义 in `specs/023-last-mile-zero-regression-and-adaptive-peak-gates/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约完备性 in `specs/023-last-mile-zero-regression-and-adaptive-peak-gates/contracts/last_mile_audit.schema.json`

---

## Phase 3: User Story 1 - 7Z 100 小文件解压目录缓存与系统调用消除 (Priority: P1) 🎯 MVP

**Goal**: 7Z 100 小文件解压通过栈上双层内联目录缓存消除 600 次 `mkdir()` 频繁系统调用，吞吐恢复至 $\ge 1,450\text{ MB/s}$

**Independent Test**:
- 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`
- 验证 7Z 100 小文件解压倒退清零

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c` 中实现栈上双层零分配内联目录缓存池 (`last_parent_dir` + L2 64-Slot hash)
- [x] T006 [US1] 运行跑分验证 7Z 100 小文件解压吞吐恢复至 $\ge 1,450\text{ MB/s}$

---

## Phase 4: User Story 2 - WIM 500MB 大文件解压吞吐与预热对齐 (Priority: P1)

**Goal**: WIM 500MB 大文件解压通过 `.wim` 探测直通与 `fcntl(F_RDAHEAD)` 预热消除 APFS 脏页排队阻塞，稳定达成 $\ge 10,800\text{ MB/s}$

**Independent Test**:
- 运行 WIM 500MB 解压跑分测试并比对历史峰值

### Implementation for User Story 2

- [x] T007 [US2] 在 `Sources/CTTZipBridge/ttzip_native_archive.c` 中添加 `.wim` 探测与 `F_RDAHEAD` 预热提示
- [x] T008 [US2] 验证 WIM 500MB 解压吞吐稳定在 $\ge 10,800\text{ MB/s}$ (实测 11,804.0 MB/s, +9.5%)

---

## Phase 5: User Story 3 - DMG 拟真日志与高熵镜像直通解压 (Priority: P1)

**Goal**: 消除 DMG 引擎探测 Header 失败试错开销，恢复 10MB 日志 ($\ge 6,562\text{ MB/s}$) 与 100MB 高熵 ($\ge 9,556\text{ MB/s}$) 吞吐

**Independent Test**:
- 运行 DMG 格式场景测试并比对历史基准

### Implementation for User Story 3

- [x] T009 [US3] 在 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` 中优化 DMG 直通分发，消除 7Z 试探开销
- [x] T010 [US3] 验证 DMG 10MB 日志与 100MB 高熵解压倒退清零 (收敛至正常波动区间)

---

## Phase 6: Polish & 0-Regression Final Verification

**Purpose**: 全量回归验证与最后 4 项倒退彻底清零审计

- [x] T011 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T012 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T013 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证全部 4 项倒退彻底清零（0-Regression Full Closure）
