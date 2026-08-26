# Quickstart Validation: Zstandard Match Counting Acceleration & Dual-Track Verification

**Feature**: `061-zstd-match-counting-acceleration`
**Date**: 2026-08-17

---

## 1. Track 1: Upstream PR 1 Worktree Compilation & Verification

### Command
```bash
make -C Vendor/worktrees/zstd/pr1-arm64-neon-count/lib libzstd.a
```

### Expected Output
```text
Compiling libzstd.a with zero errors and zero warnings
```

### Failure Diagnostic
- If compilation fails due to NEON intrinsics, verify `<arm_neon.h>` inclusion and `#if defined(__ARM_NEON)` header guard definitions.

---

## 2. Track 1: Upstream PR 2 Worktree Compilation & Verification

### Command
```bash
make -C Vendor/worktrees/zstd/pr2-arm64-crc32-hash/lib libzstd.a
```

### Expected Output
```text
Compiling libzstd.a with zero errors and zero warnings
```

### Failure Diagnostic
- If compilation fails due to `__crc32w` / `__crc32d`, check for `#if defined(__ARM_FEATURE_CRC32)` and `<arm_acle.h>`.

---

## 3. Track 2: TTZip Internal Unit & Benchmark Regression

### Command
```bash
swift test --filter Zstd
```

### Expected Output
```text
Test Suite 'Selected tests' passed
Executed 9 tests, with 0 failures (0 unexpected)
```

### Failure Diagnostic
- If test fails, verify Double-Fast table boundaries and zero-allocation workspace offsets in `ttzip_lzma_hc4_neon.c`.

---

## 4. Track 2: Performance Gate & Zero-Regression Floor Verification

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests/testTarZstdDirect_50MB_ThroughputFloor
```

### Expected Output
```text
Test Case '-[TTZipTests.XCTestPerformanceMeasureTests testTarZstdDirect_50MB_ThroughputFloor]' passed
[▶ TAR.ZST Direct 50MB] 编解码: >= 15000 MB/s (Debug) -> PASS
```

### Failure Diagnostic
- If throughput drops below 15,000 MB/s, inspect memory layout for heap allocation regressions or lock contention on hot paths.
