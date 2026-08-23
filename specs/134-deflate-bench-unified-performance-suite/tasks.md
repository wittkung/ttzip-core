# Tasks: Deflate-Bench Unified Performance & Test Suite Modernization

**Feature Directory**: `specs/134-deflate-bench-unified-performance-suite`  
**Target Subject**: zlib-ng 8 大语料逆向、纯内存 50 点秒级基准引擎与测试体系瘦身重构  
**Status**: Completed  

---

## Phase 1: Setup & In-Memory Corpus Engine

- [x] T001 [P] Implement C corpus generator in `Sources/CTTZipBridge/CTTZipCorpusGen.c` and `Sources/CTTZipBridge/include/CTTZipCorpusGen.h` with 8 exact algorithms (`text`, `short_match`, `dna`, `random`, `literals`, `mixed`, `realistic_rgb`, `striped_rgb`)
- [x] T002 [P] Expose C corpus generator in `Sources/CTTZipBridge/include/CTTZipBridge.h`
- [x] T003 [P] Implement Swift wrapper and page buffer pool in `Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift`

---

## Phase 2: User Story 1 & 2 - Core In-Memory Codec Benchmark Suite (Priority: P1)

- [x] T004 [P] [US1] [US2] Implement 50-point parameterized in-memory benchmark engine in `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift`
- [x] T005 [P] [US1] [US2] Implement XCTest harness with formatted terminal summary table in `Tests/TTZipTests/TTZipCoreCodecBenchmarkTests.swift`
- [x] T006 [US1] [US2] Verify 50-point matrix executes in < 1.0 second on Apple Silicon with 100% roundtrip memcmp pass

---

## Phase 3: User Story 3 - Multi-Core Parallel Container Benchmark (Priority: P2)

- [x] T007 [P] [US3] Implement in-memory 8-workload block-parallel container benchmark in `Tests/TTZipTests/TTZipContainerParallelBenchmarks.swift`

---

## Phase 4: User Story 4 - Test Suite Purge & De-cluttering (Priority: P2)

- [x] T008 [P] [US4] Delete 11 redundant and slow PK benchmark files:
  - `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`
  - `Tests/TTZipTests/SingleCoreDeflatePkTests.swift`
  - `Tests/TTZipTests/SingleCoreDecompressPkTests.swift`
  - `Tests/TTZipTests/BenchmarkTests.swift`
  - `Tests/TTZipTests/ZipBenchPkTests.swift`
  - `Tests/TTZipTests/AllFormatsPkSuiteTests.swift`
  - `Tests/TTZipTests/CompoundMixedCorpusBenchmarkPkTests.swift`
  - `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift`
  - `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift`
  - `Tests/TTZipTests/GlobalCompressionEliteParetoPkTests.swift`
  - `Tests/TTZipTests/SevenZipParetoFrontierPkTests.swift`
- [x] T009 [P] [US4] Streamline `Tests/TTZipTests/RealWorldPerformanceTests.swift` to pure functional packaging assertions without disk benchmark loops
- [x] T010 [P] [US4] Optimize `Tests/TTZipTests/DirectoryScanPerformanceTests.swift` and `Tests/TTZipTests/SwarOptimizationBenchmarkTests.swift` for sub-millisecond execution
- [x] T011 [US4] Update `Package.swift` or test targets if needed to ensure 100% clean compilation

---

## Phase 5: Verification & Gating

- [x] T012 Run full test suite (`swift test`) and verify execution time drops to $\le 3.5$ seconds with 0 failures
- [x] T013 Update pre-push CI test list to run the modernized clean test suite
