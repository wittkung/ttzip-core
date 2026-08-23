# Tasks: Complete Optimization Wiring & Configuration Creep Audit (全量优化端到端装配与反配置膨胀深度审计)

**Input**: Design documents from `/specs/093-complete-optimization-wiring-and-configuration-creep-audit/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

## Format: `- [ ] [TaskID] [P?] [Story?] Description with file path`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., [US1], [US2], [US3], [US4])

---

## Phase 1: Zero-Allocation Hot Path Implementation (Priority: P1)

**Purpose**: Eliminate all per-file heap allocations (`malloc`/`free`, `asprintf`, `strdup`) in TAR native bridges

- [x] T001 [US1] Refactor `write_reg_file_data` in `Sources/CTTZipBridge/ttzip_tar_native.c` to replace `malloc(1MB)` with 64KB stack buffer loop and `mmap` zero-copy
- [x] T002 [US1] Replace `asprintf`/`strdup` in directory recursion in `Sources/CTTZipBridge/ttzip_tar_native.c` with stack `snprintf`
- [x] T003 [P] [US1] In `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`, replace scalar 512B zero loop with `ttzip_swar_is_zero_512b` and upgrade 4KB buffers to 64KB stack buffers

---

## Phase 2: User Story 2 - Configuration Creep Elimination & Transparent Defaults (Priority: P1)

**Purpose**: Ensure all advanced options models provide SoC-aware optimal physical defaults

- [x] T004 [US2] Audit `Sources/TTZipCore/ArchiveCompressionTypes.swift` and verify transparent defaults across `ArchiveAdvancedOptions`, `ZipFormatOptions`, `ZstdFormatOptions`, and `SevenZipFormatOptions`
- [x] T005 [P] [US2] Verify `Sources/TTZipCore/Adapters/ZstdCAdapter.swift` dynamic `windowLog` clamping against source file size

---

## Phase 3: User Story 3 - 16-Format Full-Stack Exhaustive Wiring & Verification Matrix (Priority: P1) 🎯 MVP

**Purpose**: Implement exhaustive automated audit test verifying all 16 formats execute via in-process C engines with zero CLI subprocesses

- [x] T006 [US3] Create `Tests/TTZipTests/ExhaustiveOptimizationAuditTests.swift` with 5-stage invariant pipeline across all 16 formats
- [x] T007 [P] [US3] Verify round-trip bitwise SHA-256 differential oracle for all 16 formats (ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, WIM, DMG, ISO, LZ4, LZIP, LRZIP, AAR, BROTLI, SNAPPY)

---

## Phase 4: Full Regression & Performance Floor Gatekeeper (Priority: P1)

**Purpose**: Execute full test suite regression and verify zero throughput regression against 13 hard performance floors

- [x] T008 Run full unit test suite `swift test` (1037 tests) and verify 0 failures
- [x] T009 Run performance floor gate tests `swift test --filter XCTestPerformanceMeasureTests` and assert all 13 throughput floors pass
- [x] T010 Run frontend performance gate tests `swift test --filter FrontendPerformanceGateTests` and assert UI latency thresholds pass
