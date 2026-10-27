# Tasks: LZ4 Engine Analysis and Architecture Integration

**Feature**: `063-lz4-engine-analysis`
**Created**: 2026-08-17
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/063-lz4-engine-analysis/spec.md) | **Plan**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/063-lz4-engine-analysis/plan.md)

---

## Phase 1: Setup & Environment Validation

- [x] T001 [P] [US1] Verify native `Vendor/include/lz4.h`, `lz4frame.h` and `Vendor/lib/liblz4.a` symbols in `Vendor/`
- [x] T002 [P] [US1] Inspect current `Sources/CTTZipBridge/CTTZipStreamCoder.c` for `compression.h` dependencies

---

## Phase 2: User Story 1 - 深度技术剖析与架构全景对比报告 (Priority: P1) 🎯 MVP

**Goal**: 完成官方 `lz4/lz4` 开源库原理剖析，对比 TTZip 现有链路，形成权威架构对比与演进决策文档。

- [x] T003 [P] [US1] Synthesize LZ4 kernel mechanics (Token, Wild Copy, L1 Hash, Zero-Entropy) in `specs/063-lz4-engine-analysis/research.md`
- [x] T004 [P] [US1] Document TTZip C-Bridge vs Apple `compression.h` architectural gap in `specs/063-lz4-engine-analysis/research.md`

---

## Phase 3: User Story 2 - 原生 C 桥接引擎强化与 Fast-Path 对齐 (Priority: P1)

**Goal**: 在 `CTTZipStreamCoder.c` 中废弃 Apple `compression.h`，实现基于原生 `liblz4` 的高吞吐编解码与 `acceleration` 加速因子控制。

- [x] T005 [P] [US2] Update `Sources/CTTZipBridge/CTTZipStreamCoder.c` to use native `lz4.h` (`LZ4_compress_fast`, `LZ4_decompress_safe`) across all platforms
- [x] T006 [US2] Verify `LZ4LzoEngine` acceleration passthrough in `Sources/TTZipCore/ProfessionalAlgorithmsSuite.swift`

---

## Phase 4: User Story 3 - 大体积 TAR.LZ4 极速穿透与 VFS 临时解压缓存池利用方案 (Priority: P2)

**Goal**: 确立基于 `TarSeekTable` 与两级（RAM-LZ4 + Disk-LZ4）VFS 临时解压缓存池的高性能利用架构。

- [x] T007 [P] [US3] Model TAR.LZ4 rapid streaming traversal and VFS temp caching architecture in `specs/063-lz4-engine-analysis/data-model.md`
- [x] T008 [P] [US3] Define strongly-typed contract schema in `specs/063-lz4-engine-analysis/contracts/lz4_engine_contract.json`

---

## Phase 5: User Story 4 - 性能基准门禁与零倒退回归 (Priority: P2)

**Goal**: 执行单元测试与性能门禁回归，确保零崩溃与零性能倒退。

- [x] T009 [P] [US4] Execute data roundtrip regression tests in `Tests/TTZipTests/Phase123FeatureCoverageTests.swift`
- [x] T010 [P] [US4] Execute LZ4 hard throughput floor benchmark in `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
- [x] T011 [US4] Run full format compatibility suite in `Tests/TTZipTests/AllFormatsAndAdvancedParametersMatrixTests.swift`

---

## Dependencies & Execution Order

- **Phase 1** ➔ **Phase 2 (US1)** ➔ **Phase 3 (US2)** ➔ **Phase 4 (US3)** ➔ **Phase 5 (US4)**
- T005 完成后执行 T006，随后并行执行 T009/T010/T011。
