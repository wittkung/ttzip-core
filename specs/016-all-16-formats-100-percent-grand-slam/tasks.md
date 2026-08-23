# Tasks Breakdown: 100% Grand Slam Win Rate Across All 16 Formats (Feature 016)

**Feature**: 100% Grand Slam Win Rate Across All 16 Archive Formats  
**Directory**: `specs/016-all-16-formats-100-percent-grand-slam/`  
**Status**: Ready for Implementation

---

## Phase 1: Foundational Setup & Diagnosis

- [x] T001 [P] [US1] Create NativeBrotliEngine test baseline in Tests/TTZipTests/NativeBrotliEngineTests.swift
- [x] T002 [P] [US1] Configure AllFormatDiagnosticSuiteTests for Brotli native creation in Tests/TTZipTests/AllFormatDiagnosticSuiteTests.swift

---

## Phase 2: User Story 1 - 全 16 格式 280 项竞品对决 100% 胜率通关 (Priority: P1) 🎯 MVP

**Goal**: 针对 39 处负场实现 7 大模块的架构优化，达成 100% 全胜率。

### 1. Brotli 100% 原生 In-Process 引擎与 TAR 管道集成
- [x] T003 [P] [US1] Implement NativeBrotliEngine with Apple Compression.framework in Sources/TTZipCore/Brotli/NativeBrotliEngine.swift
- [x] T004 [US1] Integrate Brotli stream routing in Sources/TTZipCore/TemplateMethod/TarArchiveEngineTemplate.swift
- [x] T005 [US1] Integrate Brotli dispatch in Sources/TTZipCore/ArchiveWriter+Dispatch.swift and ArchiveExtractor+Dispatch.swift

### 2. TAR.XZ 多核并发 LZMA2 / XZ 解压管道升级
- [x] T006 [P] [US1] Enhance TarArchiveEngineTemplate with multi-core LZMA2 MT decompression for .tar.xz in Sources/TTZipCore/TemplateMethod/TarArchiveEngineTemplate.swift
- [x] T007 [US1] Add multi-core fallback in ArchiveExtractor for .txz / .tar.xz in Sources/TTZipCore/ArchiveExtractor.swift

### 3. 纯 TAR 500MB 大文件与海量小文件 Direct I/O Fast-Path
- [x] T008 [P] [US1] Implement Direct I/O streaming writer for uncompressed TAR in Sources/CTTZipBridge/ttzip_tar_native.c
- [x] T009 [US1] Export C symbols in Sources/CTTZipBridge/include/CTTZipBridge.h and bridge in Sources/CTTZipBridge/CTTZipBridge_Archive.c
- [x] T010 [US1] Wire TarFastWriter fast-path in Sources/TTZipCore/ArchiveWriter+Dispatch.swift

### 4. TAR.ZST 32MB 窗口与高熵解压优化
- [x] T011 [P] [US1] Optimize ZSTD decompression context buffer size to 32MB and tune worker threads in Sources/CTTZipBridge/ttzip_tar_zstd.c
- [x] T012 [US1] Wire ZSTD streaming buffer improvements in Sources/TTZipCore/NativeZstdEngine.swift

### 5. LZIP / LRZIP / LZ4 并发参数调优
- [x] T013 [P] [US1] Optimize LZIP multithread filter options and LZ4 block size in Sources/CTTZipBridge/ttzip_tar_native.c

---

## Phase 3: User Story 2 - 零性能倒退质量保障与验证 (Priority: P2)

**Goal**: 运行全量基准测试与零倒退审计，确保胜率达到 100% 且倒退 < 3.0%。

- [x] T014 [US2] Run AllFormatsPkSuiteTests full benchmark in Tests/TTZipTests/AllFormatsPkSuiteTests.swift
- [x] T015 [US2] Run audit_performance_regression.py and verify zero regression (<3.0%) in docs/benchmarks/latest_regression_audit.md
- [x] T016 [US2] Run XCTestPerformanceMeasureTests performance gates in Tests/TTZipTests/XCTestPerformanceMeasureTests.swift

---

## Phase 4: User Story 3 - MAS 沙盒与全量单测合规验证 (Priority: P3)

**Goal**: 验证 100% In-Process、无外部进程调用、560+ 单测全绿。

- [x] T017 [US3] Run full regression test suite via scripts/run_all_tests.sh
- [x] T018 [US3] Verify zero bare prints and strict TTLogger compliance across modified files

---

## Phase 5: Polish & Convergence

- [x] T019 [P] Update benchmark documentation in docs/benchmarks/
- [x] T020 Run speckit-analyze consistency scan and finalize feature 016
