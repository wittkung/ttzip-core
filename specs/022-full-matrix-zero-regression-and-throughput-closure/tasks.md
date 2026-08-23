# Tasks: 022-full-matrix-zero-regression-and-throughput-closure

**Input**: User stories from `spec.md`, architecture from `plan.md`, data model from `data-model.md`, schemas from `contracts/`  
**Feature Branch**: `022-full-matrix-zero-regression-and-throughput-closure`  

---

## Phase 1: Setup & Grounded Baseline

**Purpose**: 初始化与基线数据加载

- [x] T001 校验环境与 Feature 分支上下文 in `Package.swift`
- [x] T002 加载 `604d44d` 历史最优基线跑分报告 in `docs/benchmarks/benchmark_report_2026-08-15_071939.json`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 核心数据模型与契约校验

- [x] T003 [P] 校验数据模型实体定义 in `specs/022-full-matrix-zero-regression-and-throughput-closure/data-model.md`
- [x] T004 [P] 校验 JSON Schema 契约完备性 in `specs/022-full-matrix-zero-regression-and-throughput-closure/contracts/regression_closure_audit.schema.json`

---

## Phase 3: User Story 1 - ZIP 大文件与高熵物理写盘解压性能恢复 (Priority: P1) 🎯 MVP

**Goal**: 恢复 ZIP 500MB 大文件与 100MB 高熵 Payload 在纯物理写盘解压下的历史最高吞吐（$\ge 9,500\text{ MB/s}$）

**Independent Test**:
- 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`
- 验证 ZIP 解压 8 项严重倒退清零

### Implementation for User Story 1

- [x] T005 [US1] 在 `Sources/CTTZipBridge/CTTZipExtract.c` 中对 $\ge 4\text{MB}$ 大文件解压引入 `fcntl(out_fd, F_PREALLOCATE, &fst)` + `ftruncate` 连续物理 Extent 预分配
- [x] T006 [P] [US1] 在 `Sources/CTTZipBridge/CTTZipExtract.c` 中对大文件解压目标开启 `fcntl(out_fd, F_NOCACHE, 1)` Direct I/O 并采用 16KB `posix_memalign` 页对齐解压缓冲
- [x] T007 [P] [US1] 在 `Sources/CTTZipBridge/CTTZipExtract.c` 中实现 64MB 分块批量 `pwrite` 替代带全局文件锁的 `write_all`
- [x] T008 [US1] 运行 `swift test -c release --filter XCTestPerformanceMeasureTests/testZipDecompression_ThroughputFloor` 验证 ZIP 解压门禁稳超 9,500+ MB/s

---

## Phase 4: User Story 2 - 7Z 100MB 高熵 Payload 解压吞吐恢复 (Priority: P1)

**Goal**: 7Z 100MB 高熵 Payload 解压通过 256KB L2 Cache 对齐与 NEON 向量直通恢复至 $\ge 7,500\text{ MB/s}$

**Independent Test**:
- 运行 7Z 100MB 解压跑分测试并比对历史最优

### Implementation for User Story 2

- [x] T009 [US2] 在 `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c` 中实现 256KB（4 × 64KB 子块）批量未压缩流式扫描与 `ttzip_neon_copy_match` 向量展开拷贝
- [x] T010 [P] [US2] 在 `Sources/CTTZipBridge/ttzip_7z_block_decoder.c` 中强化 `primary_method_id == 0x00` Direct Store Fast-Path 与 256KB 边界对齐
- [x] T011 [US2] 运行 `swift test -c release --filter XCTestPerformanceMeasureTests/testSevenZipDecompression_ThroughputFloor` 验证 7Z 解压吞吐 $\ge 7,500\text{ MB/s}$

---

## Phase 5: User Story 3 - DMG 格式管道流式写入与解压零冗余拷贝 (Priority: P1)

**Goal**: 消除 DMG 镜像写入中间临时文件，提升拟真日志与海量小文件 DMG 压缩解压吞吐 30%+

**Independent Test**:
- 运行 DMG 格式 4 项场景测试并比对历史基准

### Implementation for User Story 3

- [x] T012 [US3] 在 `Sources/TTZipCore/ArchiveWriter+Dispatch.swift` 中优化 DMG 打包流式管道，消除中间磁盘临时文件拷贝
- [x] T013 [P] [US3] 在 `Sources/CTTZipBridge/CTTZipDiagnostics.c` 与 DMG 引擎中集成零冗余内存映射解析
- [x] T014 [US3] 运行 DMG 格式基准验证 4 项严重倒退清零

---

## Phase 6: User Story 4 - TAR.ZST 50MB Direct 突破 19,000 MB/s 与 Tar 变体收敛 (Priority: P1)

**Goal**: TAR.ZST 50MB Direct 打包稳跨 19,000 MB/s 门禁，消除 TAR/TAR.GZ/TAR.ZST/LZ4 12 项倒退

**Independent Test**:
- 运行 `swift test -c release --filter XCTestPerformanceMeasureTests/testTarZstdDirect_50MB_ThroughputFloor`

### Implementation for User Story 4

- [x] T015 [US4] 在 `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c` 中实现 `static _Thread_local ZSTD_CCtx* s_tar_zstd_cctx` 静态线程池保活与 `ZSTD_CCtx_reset` 快速复位
- [x] T016 [P] [US4] 在 `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c` 中修复 Level 1 Job Size 退化，启用 2MB~4MB 自适应分块与 16KB 页对齐缓冲
- [x] T017 [US4] 运行 `swift test -c release --filter XCTestPerformanceMeasureTests/testTarZstdDirect_50MB_ThroughputFloor` 验证吞吐突破 $\ge 19,000\text{ MB/s}$

---

## Phase 7: Polish & 0-Regression Audit Final Verification

**Purpose**: 全量回归验证与 28 项倒退清零审计

- [x] T018 [P] 运行全量 593+ 单元测试 `./scripts/run_all_tests.sh` 确保 100% 绿灯
- [x] T019 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 生成最新基准测试报告
- [x] T020 运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json` 验证全部 28 项严重倒退清零（0-Regression Verified）

---

## Dependencies & Execution Order

### Phase Dependencies
- **Setup (Phase 1)**: 无依赖，立即执行
- **Foundational (Phase 2)**: 依赖 Phase 1 完成
- **User Story 1 (Phase 3)**: 依赖 Phase 2 完成，与 US2、US3、US4 文件解耦，可并行开发
- **User Story 2 (Phase 4)**: 依赖 Phase 2 完成
- **User Story 3 (Phase 5)**: 依赖 Phase 2 完成
- **User Story 4 (Phase 6)**: 依赖 Phase 2 完成
- **Polish (Phase 7)**: 依赖所有 User Stories 完成

### Parallel Execution Opportunities
- **Foundational**: T003 与 T004 可并发校验
- **User Story 1**: T006 与 T007 可并发实现
- **User Story 2**: T010 可与 T009 并发实现
- **User Story 4**: T016 可与 T015 并发实现
