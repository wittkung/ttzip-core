# Tasks Breakdown: Zero Performance Regression Governance & Hard Floor Enforcement (Feature 017)

**Feature**: Zero Performance Regression Governance & Hard Floor Invariant Enforcement  
**Directory**: `specs/017-zero-performance-regression-and-floor-enforcement/`  
**Status**: Ready for Implementation

---

## Phase 1: Foundational Setup & Governance Tooling

- [x] T001 [P] [US1] Upgrade scripts/audit_performance_regression.py with double-tier thresholds (3% warning, 10% hard block with exit code 1) in scripts/audit_performance_regression.py
- [x] T002 [US1] Verify schema compliance of audit output against contracts/zero_regression_report.schema.json

---

## Phase 2: User Story 1 - 彻底根除全格式 > 10% 性能倒退 (Priority: P1) 🎯 MVP

**Goal**: 修复全部导致 > 10% 性能倒退的根因，消除所有严重退化项。

### 1. Lzip 极速模式级别参数修复
- [x] T003 [P] [US1] Fix Lzip compression level mapping to use compression-level 1 for fast mode in Sources/CTTZipBridge/ttzip_tar_native.c

### 2. WIM 与多层目录解包父目录末次缓存与乐观 open
- [x] T004 [P] [US1] Implement stack-based last_parent_dir caching and optimistic open in ttzip_extract_tar_native_c in Sources/CTTZipBridge/ttzip_tar_native.c
- [x] T005 [P] [US1] Implement stack-based last_parent_dir caching and optimistic open in ttzip_extract_tar_from_memory in Sources/CTTZipBridge/ttzip_tar_native.c

### 3. DMG / ISO / WIM 解包性能回归验证
- [x] T006 [US1] Verify WIM and DMG/ISO directory unpacking speed via SingleTestDiagnosticRunner in Tests/TTZipTests/AllFormatDiagnosticSuiteTests.swift

---

## Phase 3: User Story 2 - 核心热路径性能倒退率严格收敛至 < 3.0% (Priority: P2)

**Goal**: 运行全量基准测试与零倒退审计，确保 0 项严重倒退且核心路径收敛至 < 3.0%。

- [x] T007 [US2] Run AllFormatsPkSuiteTests full benchmark suite in Tests/TTZipTests/AllFormatsPkSuiteTests.swift
- [x] T008 [US2] Run upgraded audit_performance_regression.py and verify 0 critical regressions (>10%) in docs/benchmarks/latest_regression_audit.md
- [x] T009 [US2] Run XCTestPerformanceMeasureTests performance gates in Tests/TTZipTests/XCTestPerformanceMeasureTests.swift

---

## Phase 4: User Story 3 - MAS 沙盒与全量单测合规验证 (Priority: P3)

**Goal**: 保证 591+ 单测 100% 绿灯，零裸日志。

- [x] T010 [US3] Run full test regression suite via swift test
- [x] T011 [US3] Verify strict TTLogger compliance across modified C bridge and Swift files

---

## Phase 5: Polish, Consistency Scan & Finalization

- [x] T012 [P] Update benchmark reports and peak performance matrix in docs/benchmarks/
- [x] T013 Run speckit-analyze consistency scan and finalize feature 017
