# Quickstart & Verification Guide: zlib-ng NEON LCP Acceleration & Dual-Platform Integration

**Feature**: `058-zlib-ng-neon-integration`
**Created**: 2026-08-17
**Status**: Draft

---

## 1. Scenario 1: Dual-Tier Streaming Deflate Roundtrip Verification

### Command
```bash
swift test --filter DeflateStreamCoderTests
```

### Expected Output
```text
Test Suite 'DeflateStreamCoderTests' passed at ...
	 Executed 12 tests, with 0 failures (0 unexpected) in 0.420 seconds
```

### Failure Diagnostic
- If test fails on `Z_DATA_ERROR` or checksum mismatch:
  1. Verify whether `window_bits` was configured with `31` for GZIP or `15` for standard zlib wrapper.
  2. Check if `DeflateStreamState.magic` was corrupted or uninitialized during pipeline construction.
  3. Ensure `flush_mode` correctly transitions to `FINISH` on the final chunk.

---

## 2. Scenario 2: Hybrid SWAR/NEON Match Finder Micro-Benchmark Verification

### Command
```bash
swift test --filter HybridMatchFinderMicroTests
```

### Expected Output
```text
Test Suite 'HybridMatchFinderMicroTests' passed at ...
	 Executed 8 tests, with 0 failures (0 unexpected) in 0.180 seconds
```

### Failure Diagnostic
- If short match length returns incorrect index:
  1. Inspect whether big-endian / little-endian byte ordering in `__builtin_ctzll` vs `__builtin_clzll` is properly branched with `#if defined(WORDS_BIGENDIAN)`.
  2. Verify that `len + 8 <= max_len` boundary condition prevents reads past allocated buffer limits.
  3. Confirm that pointer alignment does not trigger bus errors on strict architectures.

---

## 3. Scenario 3: Full-Matrix Performance Floor & Zero Regression Gate

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
[PERF GATE] ZIP Level 1 Compression Throughput: 1650.4 MB/s (Floor: >= 1500.0 MB/s) -> PASS
[PERF GATE] ZIP Decompression Throughput: 7850.2 MB/s (Floor: >= 7500.0 MB/s) -> PASS
[PERF GATE] Streaming Deflate Throughput: 395.8 MB/s (Floor: >= 350.0 MB/s) -> PASS
Test Suite 'XCTestPerformanceMeasureTests' passed.
```

### Failure Diagnostic
- If streaming Deflate throughput drops below 350 MB/s:
  1. Verify that `zlib-ng` was compiled with `-DZLIB_COMPAT=ON -DWITH_NATIVE_INSTRUCTIONS=ON` and linked statically.
  2. Check whether CPU dynamic dispatch correctly resolved `compare256_neon` instead of scalar fallback.
  3. Ensure no lock acquisitions (`NSLock`/`DispatchSemaphore`) were introduced in the chunk processing loop.
