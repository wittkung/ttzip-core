# Tasks: 033-full-matrix-leapfrog-zero-flat-closure

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `033-full-matrix-leapfrog-zero-flat-closure`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 校验环境与历史峰值矩阵

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 校验 `docs/benchmarks/peak_performance_matrix.json` 包含历史最高纪录

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 契约与数据模型定义校验

- [x] T003 [P] 校验数据模型定义 in `specs/033-full-matrix-leapfrog-zero-flat-closure/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约 in `specs/033-full-matrix-leapfrog-zero-flat-closure/contracts/all_green_closure.schema.json`

---

## Phase 3: User Story 1 - LZ4 / LZIP 进程内纯 C 流式通道 (Priority: P1) 🎯 MVP

**Goal**: 在 `ttzip_tar_native.c` 与 `TarArchiveEngineTemplate.swift` 中接入进程内纯 C 原生流式通道，消除 `fork()` 开销

**Independent Test**:
- 验证 LZ4 10MB 日志打包解压吞吐恢复至 $\ge 3,500\text{ MB/s}$

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 与 `Sources/TTZipCore/TemplateMethod/TarArchiveEngineTemplate.swift` 中接入进程内纯 C 原生流式通道

---

## Phase 4: User Story 2 - TAR.XZ liblzma 多线程快速内存流 (Priority: P1)

**Goal**: 在 `ttzip_tar_native.c` 中为 XZ 接入 `lzma_stream_encoder_mt` 与快速匹配器，消除子进程开销

**Independent Test**:
- 验证 TAR.XZ 10MB 日志打包解压吞吐恢复至 $\ge 800\text{ MB/s}$

### Implementation for User Story 2

- [x] T006 [US2] 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中接入 `liblzma` 快速多线程流式编码器

---

## Phase 5: User Story 3 - AES-256 加密解压直通 ARM NEON SIMD 引擎 (Priority: P1)

**Goal**: 在 `ArchiveExtractor+Dispatch.swift` 中将 DMG/7Z 加密归档统一直派至 `SevenZipEngine`

**Independent Test**:
- 验证 7Z / DMG 100MB AES 解压吞吐稳定在 $\ge 8,000\text{ MB/s}$

### Implementation for User Story 3

- [x] T007 [US3] 在 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` 中强化加密归档直派 `SevenZipEngine`

---

## Phase 6: Polish & Full-Matrix Verification

**Purpose**: 全量回归验证与全矩阵大幅超越审计

- [x] T008 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T009 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T010 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证倒退清零与大幅超越
