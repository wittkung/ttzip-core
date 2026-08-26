# Implementation Plan: Deflate-Bench Unified Performance & Test Suite Modernization

**Feature Directory**: `specs/134-deflate-bench-unified-performance-suite`  
**Target Subject**: zlib-ng 8 大语料逆向、纯内存 50 点秒级基准引擎与测试体系瘦身重构  
**Status**: Ready for Execution  

---

## 1. Technical Context & Scope

TTZip's test directory currently contains 170+ Swift files, with over 18 redundant, slow, disk-bound benchmark files causing standard test runs to exceed 20 seconds.

This plan operationalizes the modernization across 4 structural phases:
1. **Corpus Generation Engine**: C/Swift 零分配实现 8 大确定性物理语料 (`Sources/CTTZipBridge/CTTZipCorpusGen.c` & `Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift`).
2. **Core In-Memory Matrix Benchmark Suite**: 50 点全覆盖参数化压测 (`Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift` & `Tests/TTZipTests/TTZipCoreCodecBenchmarkTests.swift`).
3. **Multi-Core Container Parallel Matrix**: 8 语料驱动的块并行与虚拟批处理 (`Tests/TTZipTests/TTZipContainerParallelBenchmarks.swift`).
4. **Test Suite Purge & De-cluttering**: 物理删除 11 个重度冗余 PK 文件，精简 3 个慢速测试，使日常 `swift test` 执行时间缩短至 $\le 3.0$ 秒。

---

## 2. Constitution Check

- **Zero Memory Allocation on Hot Paths**: Corpus generators operate on caller-allocated pointers.
- **Pure RAM Testing**: No temporary disk files in codec benchmarks.
- **Hardware Timer Rigor**: `mach_absolute_time()` with $\sim 8 latency.

---

## 3. Phase 0 & Phase 1 Artifacts Index

- **Phase 0 Research**: `research.md` (R001: 8 Workload Algorithms, R002: 50-Point In-Memory Matrix, R003: Purge & De-cluttering Safety Matrix).
- **Phase 1 Contracts & Models**:
  - `data-model.md`: `BenchmarkCorpusConfig`, `CodecBenchmarkResult`, `BenchmarkSuiteSummary`.
  - `contracts/deflate_bench_matrix.json`: JSON Schema Draft-07 compliant.
  - `quickstart.md`: Validation scenarios for 50-point matrix and clean CI test run.

---

## 4. Component Changes Breakdown

### A. C & Bridge Layer
- `Sources/CTTZipBridge/include/CTTZipCorpusGen.h` [NEW]: 8 corpus enum and generator signatures.
- `Sources/CTTZipBridge/CTTZipCorpusGen.c` [NEW]: Exact LCG / XorShift implementations.

### B. Swift Core & Benchmark Layer
- `Sources/TTZipCore/Benchmark/BenchmarkCorpusGenerator.swift` [NEW]: Strong-typed Swift wrapper with page buffer pool.
- `Sources/TTZipCore/Benchmark/TTZipCoreCodecBenchmarks.swift` [NEW]: 50-point matrix driver.
- `Tests/TTZipTests/TTZipCoreCodecBenchmarkTests.swift` [NEW]: XCTest suite executing the 50-point matrix in < 1s.

### C. Test Suite Purge & De-cluttering
- `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift` [DELETE]
- `Tests/TTZipTests/SingleCoreDeflatePkTests.swift` [DELETE]
- `Tests/TTZipTests/SingleCoreDecompressPkTests.swift` [DELETE]
- `Tests/TTZipTests/BenchmarkTests.swift` [DELETE]
- `Tests/TTZipTests/ZipBenchPkTests.swift` [DELETE]
- `Tests/TTZipTests/AllFormatsPkSuiteTests.swift` [DELETE]
- `Tests/TTZipTests/CompoundMixedCorpusBenchmarkPkTests.swift` [DELETE]
- `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift` [DELETE]
- `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift` [DELETE]
- `Tests/TTZipTests/GlobalCompressionEliteParetoPkTests.swift` [DELETE]
- `Tests/TTZipTests/SevenZipParetoFrontierPkTests.swift` [DELETE]
- `Tests/TTZipTests/RealWorldPerformanceTests.swift` [MODIFY]: Streamline to functional tests only.
- `Tests/TTZipTests/DirectoryScanPerformanceTests.swift` [MODIFY]: Reduce file count to 20.
- `Tests/TTZipTests/SwarOptimizationBenchmarkTests.swift` [MODIFY]: Reduce loop count.
