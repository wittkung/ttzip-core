# Tasks: 7Z Final Two 500MB Conquest

**Feature**: 7Z Final Two 500MB Conquest
**Directory**: `specs/007-7z-final-two-500mb-conquest`
**Spec Path**: `specs/007-7z-final-two-500mb-conquest/spec.md`
**Plan Path**: `specs/007-7z-final-two-500mb-conquest/plan.md`

## Phase 1: Setup & Foundational

- [x] T001 [P] 校验 Apple Silicon M 系列架构下 500MB 块切分与统一内存直接读取环境
- [x] T002 校验 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 与 `ttzip_lzma2_fast_encoder.c` 的并发安全性

---

## Phase 2: User Story 1 - 500MB 大文件 Level 1 无加密压缩超越 7zz (Priority: P1) 🎯 MVP

- [x] T003 [P] [US1] 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中针对 500MB 启用动态 24 块（`p_cores * 2`，20.8MB）对齐并行压缩
- [x] T004 [US1] 运行 500MB 7Z Level 1 无加密压缩测试，验证吞吐达到 $\ge 5,600\text{ MB/s}$

---

## Phase 3: User Story 2 - 500MB 大文件 Level 1 AES-256 加密压缩超越 7zz (Priority: P2)

- [x] T005 [P] [US2] 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中实现 AES-256 In-Place 单流水线加密
- [x] T006 [US2] 运行 500MB 7Z Level 1 AES-256 压缩测试，验证吞吐达到 $\ge 5,600\text{ MB/s}$

---

## Phase 4: User Story 3 - 32/32 全胜统治与零倒退审计 (Priority: P3)

- [x] T007 [US3] 执行 `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests`
- [x] T008 [US3] 运行 7z 战况排行榜分析，验证 7Z 32 战 32 胜（100% 全胜）
- [x] T009 [US3] 运行 `python3 scripts/audit_performance_regression.py` 进行零倒退审计
- [x] T010 [US3] 运行 `swift test --filter XCTestPerformanceMeasureTests` 验证 11 大门禁全部绿灯
- [x] T011 [US3] 运行 `./scripts/run_all_tests.sh` 确保全量 560+ 单测 100% 绿灯
