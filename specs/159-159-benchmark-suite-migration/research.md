# Research & Technical Decisions: Full Migration of Benchmark Suites to Native C11

**Feature**: `159-159-benchmark-suite-migration`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Monotonic Timing & Nanosecond Throughput Measurement

### Decision: Direct OS Hardware Monotonic Clock in C11
- On macOS: Use `clock_gettime_nsec_np(CLOCK_UPTIME_RAW)` or `clock_gettime(CLOCK_MONOTONIC)` with mach absolute time scaling.
- On Linux/POSIX: Use `clock_gettime(CLOCK_MONOTONIC_RAW, &ts)`.
- Formula:
  $$\text{Throughput (MB/s)} = \frac{\text{Bytes Processed} \times 1000000000.0}{\text{Elapsed Nanos} \times 1048576.0}$$
  $$\text{Compression Ratio (\%)} = \frac{\text{Compressed Bytes} \times 100.0}{\text{Uncompressed Bytes}}$$
  $$\text{MIPS Score} = \text{Throughput (MB/s)} \times \left(1.0 + \frac{100.0 - \text{Ratio}}{50.0}\right)$$
- **Rationale**: Eliminates Swift struct boxing and heap allocations during measurement loops.
- **Alternatives Considered**: Swift `ContinuousClock.measure` (rejected due to task executor queue jitter).
- **Source**: `tests/c/ttzip_test_harness.h`, POSIX `clock_gettime`.

---

## 2. In-Memory Synthetic Corpus Generators in C

### Decision: Multi-Modal Corpus Generator Function in `ttzip_benchmark_harness.h`
- Generates 4 distinct synthetic corpus types:
  1. `CORPUS_XML_TEXT`: Highly redundant structured markup (high compressibility, ratio ~10%).
  2. `CORPUS_BINARY_EXE`: Mixed ARM64/x86 code with relative jump offsets (medium compressibility, ratio ~40%).
  3. `CORPUS_COMPRESSED_RANDOM`: Pseudo-random noise with uniform distribution (entropy ~7.99, incompressible, ratio ~100%).
  4. `CORPUS_HOMOGENEOUS_ZERO`: All-zero buffer (stress test for run-length encoding & RLE).
- **Rationale**: Deterministic generation in C memory buffers ensures exact reproducibility across runs without disk I/O bottlenecks.
- **Source**: `Tests/TTZipTests/SyntheticXmlCorpusGeneratorTests.swift`, `HyperCompressCorpusGenerator.swift`.

---

## 3. Mathematical Pareto Frontier Curve Calculation in C

### Decision: Non-Dominated Sorting Algorithm in C11
- A codec configuration point $A = (\text{Ratio}_A, \text{Speed}_A)$ dominates point $B$ if:
  $$\text{Ratio}_A \le \text{Ratio}_B \quad \text{AND} \quad \text{Speed}_A \ge \text{Speed}_B$$
  (with at least one strict inequality).
- The C Pareto calculator sorts candidate codec runs and extracts the Pareto-optimal envelope in $\mathcal{O}(N \log N)$ time with zero heap allocations.
- **Rationale**: Replaces `ParetoFrontierCalculatorTests.swift` and `SoftwareParetoFrontierPkTests.swift` with ANSI C mathematical verification.
- **Source**: `Tests/TTZipTests/ParetoFrontierCalculatorTests.swift`.

---

## 4. Mapping of All 34 Swift Benchmark Files to C Architecture

| C Benchmark Suite | Migrated Swift Benchmark / Performance Test Files |
| :--- | :--- |
| `tests/c/bench_codecs.c` | 1. `ExtremeRatioBenchmarkSuiteTests.swift`<br>2. `TTZipCoreCodecBenchmarkTests.swift`<br>3. `EngineBenchmarkSuiteTests.swift`<br>4. `InMemoryBenchmarkSuiteTests.swift`<br>5. `Blosc2ComparativeMicroBenchmarkTests.swift`<br>6. `ZipExtremeBlockWriterTests.swift`<br>7. `TTZipContainerParallelBenchmarks.swift`<br>8. `SilesiaCorpusBenchmarkSuiteTests.swift`<br>9. `SilesiaCorpusIntegrityTests.swift`<br>10. `LibarchiveGoldenCorpusTests.swift`<br>11. `ArchiveGoldenCorpusTests.swift` |
| `tests/c/bench_checksums.c` | 12. `AlgorithmicOptimizationBenchmarkTests.swift`<br>13. `HyperCompressIntegrityAndEntropyTests.swift`<br>14. `ArchiveEncryptionCorpusTests.swift`<br>15. `MIPSBenchmarkEngineTests.swift` |
| `tests/c/bench_pareto.c` | 16. `ParetoFrontierCalculatorTests.swift`<br>17. `SoftwareParetoFrontierPkTests.swift`<br>18. `ComprehensiveCorpusBenchmarkPkTests.swift`<br>19. `TarZstParetoFrontierPkTests.swift`<br>20. `SVGParetoPlotterTests.swift`<br>21. `CompetitorMultiModalBenchmarkTests.swift`<br>22. `PerformanceRegressionGuardTests.swift`<br>23. `TestBenchmarkTier.swift` |
| `tests/c/bench_stress_vfs.c` | 24. `ExtremeStressTests.swift`<br>25. `GbScaleStressTests.swift`<br>26. `TwoGbScaleEncryptedSplitStressTests.swift`<br>27. `HyperCompressBatchGateTests.swift`<br>28. `DirectoryScanPerformanceTests.swift`<br>29. `PerformanceIoTests.swift`<br>30. `RealWorldPerformanceTests.swift`<br>31. `SyntheticXmlCorpusGeneratorTests.swift`<br>32. `HyperCompressCorpusGenerator.swift`<br>33. `AsyncBenchmarkRunner.swift`<br>34. `CorpusOrchestratorTests.swift` |
