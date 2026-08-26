# Phase 1 Quickstart Validation Guide: Single-Core 12-Tier Deflate Calibration and Full Pareto Frontier Supremacy

**Feature Directory**: `specs/126-single-core-pareto-supremacy-and-12tier-calibration`  
**Date**: 2026-08-19  
**Status**: Ready

---

## Scenario 1: Verify 12-Tier Monotonicity on Mixed Workspace (100MB)

Proves that all 12 levels ($L_1 \sim L_{12}$) produce strictly decreasing compressed file sizes without any level inversion or plateau.

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests/testTTZipVsLibdeflate1v1Duel_Mixed_Compound100MB
```

### Expected Output
```text
[PERF] [1v1 Duel Benchmark] Starting Mixed Modality: 100MB Real-World Workspace pure compression shootout (No Store)...
  [  1/ 18] [PERF] [TTZip 1-Core] L1 (Fast)           (20.xx ms) | 4.8x GB/s | 37.66 MB
  [  2/ 18] [PERF] [TTZip 1-Core] L2 (Fast2)          (28.xx ms) | 3.5x GB/s | 37.62 MB
  [  3/ 18] [PERF] [TTZip 1-Core] L3 (Fast3)          (85.xx ms) | 1.1x GB/s | 37.45 MB
  [  4/ 18] [PERF] [TTZip 1-Core] L4 (Normal)         (120.xx ms)| 830 MB/s  | 37.30 MB
  ...
  [ 12/ 18] [PERF] [TTZip 1-Core] L12 (Extreme15)     (210.xx s) | 0.4 MB/s  | 34.90 MB
Test Case '-[TTZipTests.ZipSingleCoreParetoFrontierPkTests testTTZipVsLibdeflate1v1Duel_Mixed_Compound100MB]' passed
```

### Failure Diagnostic
- If any two adjacent levels produce identical sizes (e.g. $L_4 == L_5$ at 37.66 MB): Inspect `ttzip_deflate_engine.c` tier option mapping to ensure `max_chain_depth` and `nice_match_len` are properly routed to different matchfinder functions.
- If $L_3$ is larger than $L_2$: Check that `ttzip_deflate_4way_lazy_mf_t` includes the prefix+tail lookahead filter and properly re-bases its relative offsets.

---

## Scenario 2: Verify Level 4 Supremacy over libdeflate L6 on enwik8 (100MB)

Proves that TTZip Level 4 achieves $\ge 800	ext{ MB/s}$ throughput and $\le 3.20	ext{ MB}$ compressed size, outperforming `libdeflate L6` (721.8 MB/s, 3.21 MB).

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests/testZipSingleCoreParetoFrontier
```

### Expected Output
```text
[PERF] [Single-Core Benchmark] Starting 100MB enwik8 pure 1-thread PK...
  ...
  [  4/ 22] [PERF] [TTZip 1-Core] Tier 4 (Compact Lazy (4))   (118.xx ms) | 845.2 MB/s | 3.19 MB
  ...
  [ 10/ 22] [PERF] [libdeflate  ] Level 6                     (132.13 ms) | 721.8 MB/s | 3.21 MB
```

### Failure Diagnostic
- If TTZip Level 4 throughput is $< 800	ext{ MB/s}$: Check that `ttzip_deflate_4way_lazy_find_matches` uses 16-bit relative indices and eliminates per-chunk `memset`.
- If compressed size is $> 3.20	ext{ MB}$: Ensure lookahead evaluation threshold uses the bit-cost heuristic $\Delta 	ext{Cost} = 4\Delta	ext{len} + \Delta	ext{clz} > 2$.

---

## Scenario 3: Verify Level 1 JSON Throughput ($\ge 5.8	ext{ GB/s}$) and Ratio ($\le 0.90	ext{ MB}$)

Proves that the 128KB hybrid 3-byte/4-byte matchfinder outperforms `libdeflate L1` (5.64 GB/s, 0.92 MB).

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests/testTTZipVsLibdeflate1v1Duel_Structured_JSON
```

### Expected Output
```text
[PERF] [1v1 Duel Benchmark] Starting Structured Logs & JSON: 100MB pure compression shootout (No Store)...
  [  1/ 18] [PERF] [TTZip 1-Core] L1 (Fast)                   (16.xx ms) | 5.92 GB/s | 0.88 MB
  [ 13/ 18] [PERF] [libdeflate  ] Level 1                     (15.60 ms) | 5.64 GB/s | 0.92 MB
Test Case '-[TTZipTests.ZipSingleCoreParetoFrontierPkTests testTTZipVsLibdeflate1v1Duel_Structured_JSON]' passed
```

### Failure Diagnostic
- If throughput is $< 5.0	ext{ GB/s}$: Check that dual-literal batch emission is active in `ttzip_deflate_fast.c` and that table size does not exceed 128 KB.
- If compressed size is $> 1.0	ext{ MB}$: Check that 3-byte multiplicative hash `(u24 * 0x1E35A7BDU) >> 17` is actively matching length-3 tokens.

---

## Scenario 4: Full Hard Performance Gate Regression

Ensures zero regressions across the entire suite of 13 performance gates.

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
- If any gate fails: Review git diff in `Sources/CTTZipBridge/` to ensure no dynamic heap allocations were introduced in parallel compression loops.
