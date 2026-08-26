# Tasks: Blosc2 Deep Architectural Study and Meta-Compression Pipeline Integration

**Input**: Design documents from `/specs/088-blosc2-deep-architectural-study-and-integration/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

## Format: `- [ ] [TaskID] [P?] [Story?] Description with file path`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., [US1], [US2], [US3], [US4])

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Pipeline headers, constant definitions, and build configuration

- [x] T001 [P] Update filter types and function signatures in `Sources/CTTZipBridge/include/CTTZipFilterPipeline.h`
- [x] T002 [P] Define special-value codes and branchless scanner prototypes in `Sources/CTTZipBridge/include/CTTZipQuantumPipeline.h`
- [x] T003 [P] Create SuperChunk container definitions and dictionary structs in `Sources/CTTZipBridge/include/CTTZipSuperChunk.h`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core SIMD arithmetic primitives and hardware cache topology arbiter

**⚠️ CRITICAL**: Must be completed before user story implementation

- [x] T004 [P] Implement ARM NEON 64-bit 3-stage delta-swap bit-matrix transpose vector helper in `Sources/CTTZipBridge/CTTZipSIMD.c`
- [x] T005 [P] Implement branchless 64-byte SWAR and NEON uniform-block reduction detector in `Sources/CTTZipBridge/CTTZipSIMD.c`
- [x] T006 [P] Implement hardware L1D cache topology alignment helper in `Sources/CTTZipBridge/CTTZipUtils.c`

**Checkpoint**: Core vector primitives ready — user story implementation can begin in parallel

---

## Phase 3: User Story 1 - SIMD Multi-Filter Chaining (BitShuffle & ByteDelta) (Priority: P1) 🎯 MVP

**Goal**: Implement ARM NEON SIMD BitShuffle and 128-byte unrolled Kogge-Stone ByteDelta vector differencing/reconstruction

**Independent Test**: `swift test --filter Blosc2AdvancedArchitecturesTests` passes with $\ge 4,000\text{ MB/s}$ BitShuffle and $\ge 8,000\text{ MB/s}$ ByteDelta.

- [x] T007 [P] [US1] Implement forward and backward NEON BitShuffle (`ttzip_filter_bitshuffle_forward_neon`, `backward`) with tail-cascade in `Sources/CTTZipBridge/CTTZipFilterPipeline.c`
- [x] T008 [P] [US1] Implement 128-byte unrolled ARM NEON ByteDelta forward differencing and Kogge-Stone prefix-sum reconstruction in `Sources/CTTZipBridge/CTTZipFilterPipeline.c`
- [x] T009 [US1] Connect BitShuffle and ByteDelta to `ttzip_filter_pipeline_apply_forward` and `backward` in `Sources/CTTZipBridge/CTTZipFilterPipeline.c`
- [x] T010 [P] [US1] Create Swift bridge wrapper for filter pipeline in `Sources/TTZipCore/Platform/Blosc2FilterBridge.swift`
- [x] T011 [P] [US1] Add BitShuffle and ByteDelta lossless roundtrip and throughput tests in `Tests/TTZipTests/Blosc2AdvancedArchitecturesTests.swift`

**Checkpoint**: User Story 1 complete and independently verified (MVP).

---

## Phase 4: User Story 2 - Special-Value Uniform Block Bypass (Priority: P2)

**Goal**: Implement branchless SIMD detection for `SPECIAL_ZERO` / `SPECIAL_VALUE` with memory-bus saturated `dc zva` / `memset_pattern8` fill

**Independent Test**: `swift test --filter Blosc2SpecialValueTests` confirms $\ge 25,000\text{ MB/s}$ fill throughput with 0 payload bytes stored.

- [x] T012 [P] [US2] Implement branchless `ttzip_detect_uniform_block` with NEON zero-count and 64-bit word XOR in `Sources/CTTZipBridge/CTTZipQuantumPipeline.c`
- [x] T013 [P] [US2] Implement Darwin `dc zva` and `memset_pattern8` memory-bus line-rate fill in `Sources/CTTZipBridge/CTTZipQuantumPipeline.c`
- [x] T014 [US2] Integrate special-value bypass into quantum chunk compressor and decompressor in `Sources/CTTZipBridge/CTTZipQuantumPipeline.c`
- [x] T015 [P] [US2] Create unit and throughput benchmark tests for special-value bypass in `Tests/TTZipTests/Blosc2SpecialValueTests.swift`

**Checkpoint**: User Stories 1 and 2 independently functional.

---

## Phase 5: User Story 3 - Two-Tier Cache-Aware Partitioning & Shared Dictionary (Priority: P3)

**Goal**: Implement Super-Chunk container geometry (128KB L1D blocks), 64-bit sparse index, and frame-level shared Zstd dictionary training

**Independent Test**: `swift test --filter Blosc2SuperChunkTests` passes with $\ge 2.5\times$ compression ratio on structured records.

- [x] T016 [P] [US3] Implement Super-Chunk container lifecycle (`ttzip_schunk_create`, `append`, `free`) in `Sources/CTTZipBridge/CTTZipSuperChunk.c`
- [x] T017 [P] [US3] Implement 64-bit sparse `coffsets` index table with MSB special-value tag encoding in `Sources/CTTZipBridge/CTTZipSuperChunk.c`
- [x] T018 [US3] Implement frame-level shared `ZSTD_CDict` and `ZSTD_DDict` dictionary training and ingestion in `Sources/CTTZipBridge/CTTZipSuperChunk.c`
- [x] T019 [P] [US3] Create comprehensive Super-Chunk and dictionary tests in `Tests/TTZipTests/Blosc2SuperChunkTests.swift`

**Checkpoint**: User Stories 1, 2, and 3 independently functional.

---

## Phase 6: User Story 4 - Small-Block Heuristic Auto-Tuning (Priority: P4)

**Goal**: Implement 16KB micro-sampling with Shannon entropy and stride autocorrelation for Pareto-optimal filter/codec selection

**Independent Test**: `swift test --filter Blosc2HeuristicTunerTests` verifies $\le 0.15\%$ probe overhead and accurate early-exit on incompressible streams.

- [x] T020 [P] [US4] Implement 16KB micro-sampling Shannon entropy calculator and NEON zero-run probe in `Sources/CTTZipBridge/CTTZipHeuristicTuner.c`
- [x] T021 [P] [US4] Implement multi-stride autocorrelation estimator for `typesize` detection (2/4/8 bytes) in `Sources/CTTZipBridge/CTTZipHeuristicTuner.c`
- [x] T022 [US4] Implement Pareto-optimal objective scoring and filter configuration dispatch in `Sources/CTTZipBridge/CTTZipHeuristicTuner.c`
- [x] T023 [P] [US4] Create validation tests for heuristic tuner probing in `Tests/TTZipTests/Blosc2HeuristicTunerTests.swift`

**Checkpoint**: All user stories functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: System integration, benchmark PK suites, and zero-regression audit

- [x] T024 [P] Run full regression suite `swift test` (530+ tests) ensuring 100% pass
- [x] T025 Run performance gate `swift test --filter XCTestPerformanceMeasureTests` ensuring all 13 performance floors green
- [x] T026 Verify contracts compliance across `contracts/*.json`
- [x] T027 Complete end-to-end Spec Kit delivery and verification
