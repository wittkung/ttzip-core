# Tasks: 7Z Comprehensive Conquest (全面超越 7-Zip 官方引擎)

**Feature**: 7Z Comprehensive Conquest
**Directory**: `specs/006-7z-comprehensive-conquest`
**Spec Path**: `specs/006-7z-comprehensive-conquest/spec.md`
**Plan Path**: `specs/006-7z-comprehensive-conquest/plan.md`

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 准备 7Z 优化专用的测试载荷与基准验证环境

- [x] T001 [P] 验证 7-Zip 官方 ARM64 7zz CLI 工具与多线程基准运行环境
- [x] T002 [P] 校验 Apple Silicon ARMv8 NEON 向量指令与 AES/SHA 硬件扩展支持

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 7Z 核心 C 编解码内核与多核切分基础设施

- [x] T003 校验 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中的多核分块调度接口与错误处理
- [x] T004 [P] 校验 `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 中的 HC3 匹配器与 Range Coder 状态机

**Checkpoint**: 基础设施就绪，开始 User Story 逐项攻坚

---

## Phase 3: User Story 1 - 500MB+ 大文件 7Z 极速压缩超越 7-Zip 7zz (Priority: P1) 🎯 MVP

**Goal**: 将 500MB 单流 Level 1 压缩（无加密与 AES-256）吞吐拉升至 $\ge 5,600\text{ MB/s}$，全面超越 7zz（5,498 MB/s 与 5,382 MB/s）

**Independent Test**: 对 500MB 大文件执行 7Z Level 1 压缩，验证吞吐 $\ge 5,600\text{ MB/s}$ 且生成的 `.7z` 归档能被 7-Zip 官方 CLI 正常校验解压

- [x] T005 [P] [US1] 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中优化大文件 Level 1 的分块切分粒度（`p_cores * 2`，20MB 对齐分块）与 HC3 极简哈希搜索参数
- [x] T006 [P] [US1] 在 `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 中优化极速 LZMA2 Range Coder 的主循环分支消除与无锁任务派发
- [x] T007 [US1] 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中实现 AES-256 原地流式加密流水线（In-Place NEON AES），消除临时缓冲区二次内存往返
- [x] T008 [US1] 运行 500MB 7Z Level 1 压缩验证，确保吞吐突破 5,600+ MB/s

**Checkpoint**: User Story 1 完成，500MB 场景全面战胜 7zz 官方 CLI

---

## Phase 4: User Story 2 - 海量小文件 7Z 极速打包全面胜出 (Priority: P2)

**Goal**: 将 100 个小文件 Level 1 压缩吞吐提升至 $\ge 950\text{ MB/s}$，消除 28 MB/s 差距并超越 7zz（883 MB/s）

**Independent Test**: 对 100 个小文件目录执行 7Z Level 1 压缩，测算吞吐并验证归档完整性

- [x] T009 [P] [US2] 在 `Sources/TTZipCore/Engines/SevenZip/` 中优化小文件固实流的连续内存预分配与高效目录遍历
- [x] T010 [US2] 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中针对小文件聚合流优化初始字典与 Range Coder 初始化开销
- [x] T011 [US2] 运行 100 小文件 7Z Level 1 压缩验证，确保吞吐突破 950+ MB/s

**Checkpoint**: User Story 2 完成，小文件场景全面领先 7zz

---

## Phase 5: User Story 3 - 7Z 全维度 32/32 项 100% 胜率对决验证与基准审计 (Priority: P3)

**Goal**: 运行全矩阵 32 项 7Z 竞品 1v1 极限压测，实现 32 战 32 胜（100% 全胜统治）

**Independent Test**: 运行 `AllFormatsPkSuiteTests` 与 `audit_performance_regression.py` 验证 7Z 32/32 全胜且 0 项倒退

- [x] T012 [US3] 执行 `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` 测算 7Z 全维度 32 项吞吐
- [x] T013 [US3] 执行 `python3 scripts/audit_performance_regression.py` 进行零倒退审计，确保 0 项倒退且 7Z 保持 32 战全胜
- [x] T014 [US3] 执行 `swift test --filter XCTestPerformanceMeasureTests` 验证 11 大性能硬门禁全部通过

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T015 [P] 更新 `docs/competitor_benchmark_report.md` 与基准测试文档
- [x] T016 执行 `./scripts/run_all_tests.sh` 确保全量 560+ 单测 100% 绿灯

---

## Dependencies & Execution Order

1. **Setup & Foundational (T001 ~ T004)**: 无依赖，立即执行
2. **User Story 1 (T005 ~ T008)**: 依赖 T003/T004，聚焦 500MB 大文件与 In-Place AES
3. **User Story 2 (T009 ~ T011)**: 依赖 T003/T004，聚焦小文件 Solid 预分配
4. **User Story 3 (T012 ~ T014)**: 依赖 US1 & US2，执行全矩阵对决与审计
5. **Polish (T015 ~ T016)**: 依赖全矩阵验证通过，回归全量单测
