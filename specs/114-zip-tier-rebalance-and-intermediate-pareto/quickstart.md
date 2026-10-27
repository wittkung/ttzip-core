# Quickstart Guide: ZIP 8-Tier Rebalancing & Intermediate Pareto Verification

**Feature**: [`specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md)  
**Date**: 2026-08-19  
**Status**: Ready  

---

## Scenario 1: Multi-Core 18-Thread Pareto Benchmark Execution

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipMultiCoreParetoFrontierPkTests
```

### Expected Output
```text
[PERF] [Multi-Core 18-Thread Benchmark] Starting 100MB enwik8 parallel PK...
  [  1/ 23] [PERF] [TTZip 18-Cor] Tier 0 (Store (0))          (15.15 ms) | 6.15 GB/s | 95.37 MB
  [  2/ 23] [PERF] [TTZip 18-Cor] Tier 1 (Fast (1))           (16.33 ms) | 5.70 GB/s | 3.96 MB
  [  3/ 23] [PERF] [TTZip 18-Cor] Tier 2 (Normal (2))         (17.37 ms) | 5.36 GB/s | 3.38 MB
  [  4/ 23] [PERF] [TTZip 18-Cor] Tier 3 (Maximum (3))        (21.76 ms) | 4.28 GB/s | 3.23 MB
  [  5/ 23] [PERF] [TTZip 18-Cor] Tier 4 (High (4))           (510.0 ms) | 195.0 MB/s | 3.06 MB
  [  6/ 23] [PERF] [TTZip 18-Cor] Tier 5 (Graph Fast (5))     (4.667 s)  | 20.4 MB/s  | 2.87 MB
  [  7/ 23] [PERF] [TTZip 18-Cor] Tier 6 (Ultra Zopfli (6))   (17.34 s)  | 5.5 MB/s   | 2.86 MB
  [  8/ 23] [PERF] [TTZip 18-Cor] Tier 7 (Extreme Peak (7))   (51.55 s)  | 1.9 MB/s   | 2.82 MB
```

### Failure Diagnostic
- If Tier 4 throughput falls below 150 MB/s, inspect `ttzip_zopfli_engine.c` to confirm `target_level == 4` triggers the `ttzip_libdeflate_compress` Fast-Path bypass instead of falling into full Zopfli iteration loops.

---

## Scenario 2: Single-Core 1-Thread Pareto Benchmark Execution

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests
```

### Expected Output
```text
[PERF] [Single-Core Benchmark] Starting 100MB enwik8 pure 1-thread PK...
  [  1/ 22] [PERF] [TTZip 1-Core] Tier 0 (Store (0))          (7.54 ms)  | 12.34 GB/s | 95.37 MB
  [  2/ 22] [PERF] [TTZip 1-Core] Tier 1 (Fast (1))           (71.98 ms) | 1.29 GB/s  | 3.97 MB
  [  3/ 22] [PERF] [TTZip 1-Core] Tier 2 (Normal (2))         (104.2 ms) | 914.9 MB/s | 3.38 MB
  [  4/ 22] [PERF] [TTZip 1-Core] Tier 3 (Maximum (3))        (136.8 ms) | 696.8 MB/s | 3.21 MB
  [  5/ 22] [PERF] [TTZip 1-Core] Tier 4 (High (4))           (7.77 s)   | 12.3 MB/s  | 3.02 MB
  [  6/ 22] [PERF] [TTZip 1-Core] Tier 5 (Graph Fast (5))     (63.42 s)  | 1.5 MB/s   | 2.86 MB
```

### Failure Diagnostic
- If single-core tiers fail to compile, verify `ZipCompressionProfile.allProfiles` enum case references and `ArchiveCompressionLevel` match.

---

## Scenario 3: Performance Floor & Regression Verification

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Suite 'XCTestPerformanceMeasureTests' passed
Executed 13 tests, with 0 failures (0 unexpected)
```

### Failure Diagnostic
- If any performance gate fails, check `GEMINI.md` Section IV.3 throughput minimums.
