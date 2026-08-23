# Tasks: 080-test-suite-acceleration-and-optimization

## Phase 1: Setup & Shared Infrastructure
- [x] T001 [P] [US1] Create test environment scale helper in Tests/TTZipTests/TestBenchmarkTier.swift
- [x] T002 [P] [US1] Validate schema contracts against data model in specs/080-test-suite-acceleration-and-optimization/contracts/

## Phase 2: User Story 1 (P1) - ArchiveMutationFuzzTests Concurrent & In-Memory Optimization
- [x] T003 [P] [US1] Refactor testComprehensiveDeterministicFuzzMatrix with withThrowingTaskGroup and in-memory buffer probing in Tests/TTZipTests/ArchiveMutationFuzzTests.swift
- [x] T004 [P] [US1] Refactor testTruncateStreamMutationStability with withThrowingTaskGroup and in-memory streams in Tests/TTZipTests/ArchiveMutationFuzzTests.swift
- [x] T005 [P] [US1] Refactor testOversizeHeaderIntegerOverflowHardening with concurrent task groups in Tests/TTZipTests/ArchiveMutationFuzzTests.swift
- [x] T006 [P] [US1] Refactor testCorruptCRCMutationStability and testInvalidDictSizeDecoderRejection with concurrent task groups in Tests/TTZipTests/ArchiveMutationFuzzTests.swift
- [x] T007 [US1] Refactor testCorruptMagicMutationStability and testInjectZipSlipPathSecurityDefense with TTZIP_DEEP_FUZZ adaptive scales in Tests/TTZipTests/ArchiveMutationFuzzTests.swift
- [x] T008 [US1] Verify ArchiveMutationFuzzTests execution runtime is <= 8.0s (target < 1.0s)

## Phase 3: User Story 2 (P2) - Concurrency Sync & Benchmark Adaptive Sampling Optimization
- [x] T009 [P] [US2] Refactor testRound3MultiCoreBruteForce100PlusTasksGroupCancellationSafety to remove Task.sleep and implement 3-phase deterministic cancellation in Tests/TTZipTests/StrategyPatternTests.swift
- [x] T010 [P] [US2] Refactor testHighConcurrency100ThreadsPasswordRepositoryReadWrite with isolated test vault instance in Tests/TTZipTests/RepositoryPatternTests.swift
- [x] T011 [P] [US2] Add adaptive iteration scaling to testComparativeSpeedupBenchmark in Tests/TTZipTests/CRC64HardwareTests.swift
- [x] T012 [P] [US2] Optimize exhaustive permutation matrix in Tests/TTZipTests/ExhaustiveCompressionCombinationsTests.swift
- [x] T013 [P] [US2] Add adaptive iteration scaling to testBenchmark_* in Tests/TTZipTests/LZ4DeepIntegrationAndVFSTests.swift
- [x] T014 [US2] Verify StrategyPatternTests, RepositoryPatternTests, CRC64HardwareTests, and LZ4DeepIntegrationAndVFSTests runtimes

## Phase 4: User Story 3 (P3) - Full Suite Speedup & Verification
- [x] T015 [US3] Run full swift test suite and verify total runtime is <= 20.0s
- [x] T016 [US3] Run XCTestPerformanceMeasureTests and verify 100% hard performance floor compliance
- [x] T017 [US3] Run speckit-analyze consistency validation
