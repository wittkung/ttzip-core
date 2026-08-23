# Tasks: 7Z Grand Slam Supremacy (32/32 All Conquest)

**Feature**: 7Z Grand Slam Supremacy  
**Directory**: `specs/008-7z-grand-slam`  
**Spec Path**: `specs/008-7z-grand-slam/spec.md`  
**Plan Path**: `specs/008-7z-grand-slam/plan.md`  

---

## Phase 1: Setup & Foundational

- [x] T001 [P] 校验 Apple Silicon 统一内存下 500MB 大流分块对齐与 GCD 并发安全性
- [x] T002 校验 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 与 `ttzip_lzma2_fast_encoder.c` 宏定义与线程局部状态

---

## Phase 2: User Story 1 - 500MB 大文件 Level 1 无加密压缩冲刺 6,000+ MB/s (Priority: P1) 🎯 MVP

- [x] T003 [P] [US1] 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中优化 500MB 流分块，锁定在 24~32 块黄金吞吐区间
- [x] T004 [US1] 在 `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 中调优极简 HC3 状态机
- [x] T005 [US1] 运行 500MB 7Z Level 1 压缩测试，验证吞吐达到 $\ge 5,800\text{ MB/s}$

---

## Phase 3: User Story 2 - 500MB 大文件 Level 1 AES-256 加密压缩稳胜 (Priority: P2)

- [x] T006 [P] [US2] 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中确保 AES-256 In-Place 单流水线零开销
- [x] T007 [US2] 运行 500MB 7Z Level 1 AES-256 压缩测试，验证吞吐达到 $\ge 5,600\text{ MB/s}$

---

## Phase 4: User Story 3 - 32/32 全胜统治与零倒退审计 (Priority: P3)

- [x] T008 [US3] 执行 `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests`
- [x] T009 [US3] 运行 7z 战况排行榜分析，验证 7Z 达成 32 战 32 胜（100% 全胜统治）
- [x] T010 [US3] 运行 `python3 scripts/audit_performance_regression.py` 进行零倒退审计
- [x] T011 [US3] 运行 `swift test --filter XCTestPerformanceMeasureTests` 验证 11 大门禁全部绿灯
- [x] T012 [US3] 运行 `./scripts/run_all_tests.sh` 确保全量 560+ 单测 100% 绿灯
