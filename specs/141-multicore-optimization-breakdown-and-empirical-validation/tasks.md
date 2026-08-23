# Tasks: Multi-Core Optimization Breakdown & Point-by-Point Empirical Validation

**Feature Branch**: `141-multicore-optimization-breakdown-and-empirical-validation`  
**Date**: 2026-08-20  

---

## Phase 1: Data Model & Contract Foundation

- [x] T001 [P] [US1] Define MultiCoreOptimizationPoint models in Sources/TTZipCore/Benchmark/MultiCoreOptimizationModels.swift
- [x] T002 [P] [US1] Validate JSON Schema contract compliance in specs/141-multicore-optimization-breakdown-and-empirical-validation/contracts/multicore_optimization_api.json

---

## Phase 2: Isolated Single-Point Empirical Test Harnesses

- [x] T003 [P] [US2] Implement OP-1 (TLS Zero-Lock vs Mutex) & OP-2 (512KB Block Parallel vs Single-Thread) in Tests/TTZipTests/MultiCoreOptimizationBreakdownTests.swift
- [x] T004 [P] [US2] Implement OP-3 (Multi-Tile Decompress) & OP-4 (Container Multi-File Pack) in Tests/TTZipTests/MultiCoreOptimizationBreakdownTests.swift
- [x] T005 [P] [US2] Implement OP-5 (Container Multi-File Extract) & OP-6 (ARMv8 PMULL Checksum) in Tests/TTZipTests/MultiCoreOptimizationBreakdownTests.swift
- [x] T006 [P] [US2] Implement OP-7 (APFS fstore_t Preallocation) & OP-8 (Topology QoS Scheduling) in Tests/TTZipTests/MultiCoreOptimizationBreakdownTests.swift
- [x] T007 [P] [US3] Verify positive speedup assertions (speedup > 1.0x) and bit-exact SHA-256 integrity in Tests/TTZipTests/MultiCoreOptimizationBreakdownTests.swift

---

## Phase 3: Diagnostic Engine & Performance Documentation

- [x] T008 [P] [US4] Implement MultiCoreBreakdownRunner diagnostic engine in Sources/TTZipCore/Benchmark/MultiCoreBreakdownRunner.swift
- [x] T009 [P] [US4] Update Section 8 of docs/PERFORMANCE.md with the 8-point empirical speedup table in docs/PERFORMANCE.md

---

## Phase 4: Full Suite Convergence & Non-Regression Execution

- [x] T010 [US3] Execute swift test to verify all 8 isolated multi-core tests and full test suite pass with 0 warnings
