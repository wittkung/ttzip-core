# Quickstart: ZIP Compression Architecture & Micro-Optimization Validation (112-zip-architecture-and-micro-optimization)

## Overview

This guide provides end-to-end executable validation procedures to verify ZIP compression throughput floors, zero-allocation micro-optimizations, and standards compliance.

---

## Validation Scenarios

### Scenario 1: Multi-Core Saturated ZIP Level 1 Throughput Verification

**Command**:
```bash
swift test --filter XCTestPerformanceMeasureTests/testZipCompression_ThroughputFloor
```

**Expected Output**:
```text
Test Suite 'XCTestPerformanceMeasureTests' passed
  Executed 1 test, with 0 failures (0 unexpected) in 0.18s
```
*Verification Assertions*:
- Throughput $\ge 1500\text{ MB/s}$ (Debug build) / $\ge 5000\text{ MB/s}$ (Release build).
- Output file successfully extracts via `/usr/bin/unzip -t`.

**Failure Diagnostic**:
- If throughput falls below 1500 MB/s, inspect thread pool saturation (`DispatchQueue.concurrentPerform`) and verify that intermediate `Data(count:)` zeroing was not reintroduced in `ZipBlockParallelCompressor.swift`.

---

### Scenario 2: Single-Core Deflate Algorithmic Pareto Benchmark

**Command**:
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ZipSingleCoreParetoFrontierPkTests
```

**Expected Output**:
```text
Test Case '-[TTZipTests.ZipSingleCoreParetoFrontierPkTests testZipSingleCoreParetoFrontier]' passed (26.301 seconds).
```
*Verification Assertions*:
- Single-core Level 1 throughput $\ge 1400\text{ MB/s}$.
- Single-core Level 6 throughput $\ge 800\text{ MB/s}$.
- Generated plot `docs/benchmarks/pareto_pk_zip_singlecore.png` strictly dominates Apple `ditto` and `zip -6`.

**Failure Diagnostic**:
- If single-core Level 6 falls below 800 MB/s, check for excessive hash chain lookups or absence of NEON 128-bit match finding in `ttzip_deflate_lazy.c`.

---

### Scenario 3: 100,000+ Small File Batch Compression & APFS Alignment

**Command**:
```bash
swift test --filter BatchSmallFileMemoryTests
```

**Expected Output**:
```text
Test Suite 'BatchSmallFileMemoryTests' passed
  Executed 3 tests, with 0 failures (0 unexpected)
```
*Verification Assertions*:
- Peak memory usage during 500+ small-file batch $\le 32\text{ MB}$.
- Output archive passes byte-level CRC-32 integrity audit.

**Failure Diagnostic**:
- If memory exceeds 64 MB, check whether `ttzip_c_item_t` was used instead of compact 48-byte `ttzip_compact_item_t` with string arena memory pool.
