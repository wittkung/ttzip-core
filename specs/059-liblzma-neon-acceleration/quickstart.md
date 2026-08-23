# Quickstart & Verification Guide: Liblzma (XZ Utils) ARM NEON Match Finder Acceleration

**Feature Branch**: `059-liblzma-neon-acceleration`
**Date**: 2026-08-17
**Status**: Completed

---

## 1. Scenario 1: Hybrid Match Finder Micro-Benchmark Verification

Verifies that the hybrid 64-bit SWAR & 128-bit ARM NEON match length comparison achieves target throughput (>= 4.5 GB/s) and maintains 100% bit-exact parity with scalar comparison across all length boundaries.

### Command
```bash
swift test --filter HybridMatchFinderMicroTests
```

### Expected Output
```text
Test Suite 'HybridMatchFinderMicroTests' passed at ...
Executed 5 tests, with 0 failures (0 unexpected) in 0.082 seconds
```

### Failure Diagnostic
- If test fails on boundary comparisons (`testHybridMatchLenBoundaries`): Inspect `ttzip_lzma_hc4_neon.c:14-127` to ensure `my_min(len, limit)` and `< 8` scalar convergence paths correctly clamp to `max_len`.
- If memory sanitizer reports out-of-bounds reads: Verify that `vld1q_u8` is strictly guarded by `len + 16 <= max_len` or that buffer has `LZMA_MEMCMPLEN_EXTRA` padding.

---

## 2. Scenario 2: 7Z & LZMA2 Comprehensive Performance Gate Verification

Executes the automated performance floor gate to assert zero performance regression across all 7Z, TAR.XZ, and LZMA2 pipelines.

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Case '-[TTZipTests.XCTestPerformanceMeasureTests testZipAnd7zPerformanceFloor]' passed.
Test Suite 'XCTestPerformanceMeasureTests' passed with 0 failures.
```

### Failure Diagnostic
- If 7Z Level 1 throughput drops below `3200 MB/s (Debug)` / `3900 MB/s (Release)`: Check if zero-chunk fast path in `ttzip_lzma2_fast_encoder.c` is being bypassed or if compiler failed to vectorize inner loops.
- If 7Z LZMA2 Level 5 drops below `480 MB/s (Debug)` / `620 MB/s (Release)`: Check `ttzip_fl2_bridge.c` thread count and dictionary configuration.

---

## 3. Scenario 3: Full-Matrix All Formats Regression Suite

Runs the complete 525+ test suite to guarantee that all archive formats (ZIP, 7Z, TAR, TAR.XZ, WIM, DMG, etc.) pass without functional regressions.

### Command
```bash
swift test
```

### Expected Output
```text
Executed 525+ tests, with 0 failures (0 unexpected)
```

### Failure Diagnostic
- If any golden corpus or decompression test fails: Check SHA-256 and CRC32 outputs against known reference fixtures in `Tests/TTZipTests/GoldenCorpusTests.swift`.
