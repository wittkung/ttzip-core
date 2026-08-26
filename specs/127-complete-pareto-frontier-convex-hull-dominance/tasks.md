# Implementation Tasks: Complete Pareto Frontier Convex Hull Dominance

**Branch**: `127-complete-pareto-frontier-convex-hull-dominance` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup & Pre-Flight

- [x] T001 [P] Verify L1D cache residency and struct memory alignment in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h

---

## Phase 2: Foundational (Engine Topology & Routing)

- [x] T002 [P] Update matchfinder struct definitions for 64KB direct 3-byte table in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h
- [x] T003 Update tier option routing for Levels 2..5 in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c

---

## Phase 3: User Story 1 - JSON & Structured Text Fast-Path Acceleration (Priority: P1) 🎯 MVP

**Goal**: Exceed $6.2\text{ GB/s}$ on Structured JSON 100MB while keeping compressed size $\le 0.72\text{ MB}$ (dominating `libdeflate L1`).

- [x] T004 [US1] Implement 64KB direct 3-byte multiplicative hash and dual-issue SWAR verification in Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c
- [x] T005 [US1] Verify JSON Level 1 throughput in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 4: User Story 2 - Binary Mach-O Word-Stride Vectorization (Priority: P1)

**Goal**: Exceed $7.5\text{ GB/s}$ on Binary Mach-O 100MB (dominating `libdeflate L1`).

- [x] T006 [US2] Implement 4-byte instruction word-stride matchfinder in Sources/CTTZipBridge/native_deflate/ttzip_deflate_fast.c
- [x] T007 [US2] Verify Binary Level 1 throughput in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 5: User Story 3 - Mixed Workspace Mid-Tier Dominance (Priority: P1)

**Goal**: Exceed $350\text{ MB/s}$ on Level 5 and beat `libdeflate L4..L9` on Mixed Workspace 100MB.

- [x] T008 [US3] Implement early lazy cutoff (cur_match >= 16) and probe break in Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c
- [x] T009 [US3] Wire Level 2..5 compact HT-4 matchfinder in Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c
- [x] T010 [US3] Verify Mixed Workspace 100MB 12-tier monotonicity and throughput in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 6: User Story 4 - Single-Core Full PK 12-Tier Alignment & Convex Hull Enclosure (Priority: P1)

**Goal**: Ensure TTZip forms an unbroken convex hull enclosing all competitors on enwik8 100MB.

- [x] T011 [US4] Re-align testZipSingleCoreParetoFrontier to sweep native 12 tiers in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift
- [x] T012 [US4] Execute full benchmark suite and export all 4 Retina PNG Pareto charts in Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift

---

## Phase 7: Polish & Regression Gates

- [x] T013 Verify 100% compliance with contracts/pareto-frontier-schema.json
- [x] T014 Run full regression test suite (1138+ tests) via swift test
- [x] T015 Run 13 hard performance gates via swift test --filter XCTestPerformanceMeasureTests

