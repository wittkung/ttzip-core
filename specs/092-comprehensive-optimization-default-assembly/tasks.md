# Tasks: Comprehensive Optimization Default Assembly (全量优化技术默认装配与透明解耦中枢)

**Input**: Design documents from `/specs/092-comprehensive-optimization-default-assembly/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

## Format: `- [ ] [TaskID] [P?] [Story?] Description with file path`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., [US1], [US2], [US3], [US4])

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: C header declarations for scientific float detector

- [x] T001 [P] Declare `ttzip_detect_scientific_float_neon` in `Sources/CTTZipBridge/include/CTTZipHeuristicTuner.h`
- [x] T002 Implement `ttzip_detect_scientific_float_neon` in `Sources/CTTZipBridge/CTTZipHeuristicTuner.c` with NEON exponent variance and stride autocorrelation

---

## Phase 2: Foundational (Core Adaptive Orchestrator)

**Purpose**: High-performance Swift adaptive pipeline orchestrator with zero heap allocations

- [x] T003 [P] Implement `Sources/TTZipCore/Adaptive/AdaptivePipelineOrchestrator.swift` evaluating 16KB micro-samples and directing Store downgrade / Special-Value bypass / BitGrooming

**Checkpoint**: Core orchestrator ready for engine and template method injection.

---

## Phase 3: User Story 1 - Transparent Adaptive Heuristic Probing in Generic ArchiveWriter (Priority: P1) 🎯 MVP

**Goal**: Automatically downgrade incompressible files ($H > 7.65$) to Store and bypass compression loops with $< 5\,\mu\text{s}$ probe overhead

**Independent Test**: `swift test --filter TransparentAdaptivePipelineTests/testHighEntropyStoreAutoDowngrade` confirms 0 volume expansion and $> 5,000\,\text{MB/s}$ throughput.

- [x] T004 [US1] Inject `AdaptivePipelineOrchestrator` probe into `Sources/TTZipCore/TemplateMethod/BaseArchiveEngineTemplate.swift`
- [x] T005 [US1] Update `Sources/TTZipCore/ArchiveWriter+Dispatch.swift` to transparently route high-entropy and special-value files to Store Direct I/O
- [x] T006 [P] [US1] Create unit tests in `Tests/TTZipTests/TransparentAdaptivePipelineTests.swift` validating automatic Store downgrade and special-value bypass

**Checkpoint**: User Story 1 complete and independently verified (MVP).

---

## Phase 4: User Story 2 - Scientific Float Detection & Transparent Bit-Grooming (Priority: P1)

**Goal**: Automatically detect Float32/Float64 data streams and apply Bit-Grooming + BitShuffle

**Independent Test**: `swift test --filter TransparentAdaptivePipelineTests/testScientificFloatAutoDetectionAndBitGrooming` confirms $> 2.5\times$ compression boost.

- [x] T007 [US2] Connect float detector to `AdaptivePipelineOrchestrator.swift` to inject Bit-Grooming filter chain for scientific data
- [x] T008 [P] [US2] Add unit tests in `Tests/TTZipTests/TransparentAdaptivePipelineTests.swift` testing float detection and BitGrooming synergy

**Checkpoint**: User Stories 1 and 2 independently functional.

---

## Phase 5: User Story 3 - Multi-Modal Dataset Generator & Competitor Benchmark Matrix Wiring (Priority: P1)

**Goal**: Add 4 deterministic dataset archetypes (Float32, High-Entropy, Sparse, JSON) and wire into `CompetitorBenchmarkRunner`

**Independent Test**: `swift test --filter CompetitorMultiModalBenchmarkTests` validates all formats against multi-modal workloads.

- [x] T009 [P] [US3] Implement `Sources/TTZipCore/Benchmark/MultiModalDatasetGenerator.swift` with zero-heap streaming POSIX generation
- [x] T010 [US3] Wire multi-modal datasets into `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Setup.swift`
- [x] T011 [P] [US3] Create benchmark test in `Tests/TTZipTests/CompetitorMultiModalBenchmarkTests.swift` validating end-to-end PK leaderboards

**Checkpoint**: User Stories 1, 2, and 3 fully operational.

---

## Phase 6: Polish & Cross-Cutting Quality Gates

**Purpose**: System integration, regression verification, and performance gates

- [x] T012 Run full regression suite `swift test` ensuring 100% pass across all 540+ tests
- [x] T013 Run performance gate `swift test --filter XCTestPerformanceMeasureTests` ensuring all 13 performance floors green
- [x] T014 Complete Spec Kit delivery and verification
