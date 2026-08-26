# Quickstart & Verification Guide: enwik8 / enwik9 Extreme Compression Benchmark

**Feature**: `050-enwik-extreme-compression-benchmark`
**Created**: 2026-08-17
**Status**: Ready

---

## Scenario 1: Standard enwik8 Extreme Ratio & Memory Ceiling Gate

### Purpose
Executes the standard 100 MB enwik8 benchmark suite, validating compression ratios across LZMA2/ZSTD, asserting peak RSS memory ceiling ($\le 512$ MB), and verifying byte-exact decompression fidelity.

### Command
```bash
TTZIP_RUN_BENCHMARKS=1 swift test --filter ExtremeRatioBenchmarkSuiteTests
```

### Expected Output
```text
================================================================================
  📊 [TTZip Test Suite] enwik8 Extreme Compression Ratio & Memory Gate
================================================================================
  [▶ LZMA2 L9] 载荷: 100.00 MB | 压缩包: 26.85 MB (26.8%) | 编解码: 18.2 / 385.4 MB/s | Peak RSS: 142.5 MB -> PASS [PERF_OPTIMAL]
  [▶ ZSTD L19] 载荷: 100.00 MB | 压缩包: 32.10 MB (32.1%) | 编解码: 45.1 / 1620.5 MB/s | Peak RSS: 188.0 MB -> PASS [PERF_OPTIMAL]
  [▶ BZIP2 L9] 载荷: 100.00 MB | 压缩包: 28.90 MB (28.9%) | 编解码: 22.4 / 115.0 MB/s | Peak RSS: 38.2 MB -> PASS [PERF_ACCEPTABLE]
--------------------------------------------------------------------------------
  ✅ 测试套件 [ExtremeRatioBenchmarkSuiteTests] 完成: 运行 6 测试 | 通过 6 | 失败 0
```

### Failure Diagnostic
- **Failure Symptom 1: Decompression SHA-256 mismatch**
  - *Cause*: Multi-threaded chunk boundary corruption or match finder pointer overflow.
  - *Action*: Run with single thread (`threadCount = 1`) to isolate race condition vs. algorithm encoding defect.
- **Failure Symptom 2: Peak RSS exceeds 512 MB**
  - *Cause*: Worker threads allocating unbounded dictionary search trees simultaneously without worker pooling.
  - *Action*: Inspect `PlatformMemory.currentMemoryUsage()` and throttle worker pool concurrency.

---

## Scenario 2: High-Speed Synthetic XML Corpus Generator Verification

### Purpose
Validates the offline zero-network deterministic XML synthesizer, verifying generation throughput ($\ge 2000$ MB/s), long-distance pattern repetition, and zero runtime heap leakage.

### Command
```bash
swift test --filter SyntheticXmlCorpusGeneratorTests
```

### Expected Output
```text
Test Suite 'SyntheticXmlCorpusGeneratorTests' started at 2026-08-17 04:30:00.000.
Test Case '-[TTZipTests.SyntheticXmlCorpusGeneratorTests test100MbGenerationThroughput]' passed (0.028 seconds).
Test Case '-[TTZipTests.SyntheticXmlCorpusGeneratorTests testDeterministicSha256Parity]' passed (0.015 seconds).
Test Case '-[TTZipTests.SyntheticXmlCorpusGeneratorTests testLongDistancePatternMatch]' passed (0.032 seconds).
Test Suite 'SyntheticXmlCorpusGeneratorTests' passed at 2026-08-17 04:30:00.075.
	 Executed 3 tests, with 0 failures (0 unexpected) in 0.075 (0.075) seconds
```

### Failure Diagnostic
- **Failure Symptom: Throughput < 2000 MB/s**
  - *Cause*: Intermediate `String` allocations or UTF-8 formatting used in chunk generator loop.
  - *Action*: Ensure `PlatformMemory.allocateAlignedPageBuffer` with raw pointer `memcpy` is used without Swift `Data(count:)` zeroing.

---

## Scenario 3: POSIX Inter-Process File Lock & Cache Concurrency Test

### Purpose
Validates that multiple parallel SPM test worker processes attempting to access or download fixture assets coordinate cleanly without data races or deadlocks.

### Command
```bash
swift test --parallel --filter EnwikFixtureCacheManagerTests
```

### Expected Output
```text
Test Suite 'EnwikFixtureCacheManagerTests' started.
[EnwikFixtureCacheManager] Acquired lock on 'enwik8.xml.lock' by PID 14205.
[EnwikFixtureCacheManager] Cache hit confirmed by PID 14206 after lock wait.
Test Case '-[TTZipTests.EnwikFixtureCacheManagerTests testConcurrentLockAcquisition]' passed.
Test Suite 'EnwikFixtureCacheManagerTests' passed.
```

### Failure Diagnostic
- **Failure Symptom: Process hang / deadlock on `flock`**
  - *Cause*: Lock file descriptor leak across forks or missing `defer { flock(fd, LOCK_UN); close(fd) }`.
  - *Action*: Check open file descriptors with `lsof -p <PID>` and verify RAII wrapper cleanup.

---

## Scenario 4: Fast Unit Test Zero-Overhead Assertion

### Purpose
Asserts that standard `swift test` runs remain completely untouched by extreme compression workloads when benchmark environment flags are absent.

### Command
```bash
swift test
```

### Expected Output
```text
Executed 525+ tests, with 0 failures (0 unexpected) in < 3.0 seconds
```

### Failure Diagnostic
- **Failure Symptom: `swift test` takes > 10 seconds or downloads fixtures**
  - *Cause*: Benchmark suite executed unconditionally instead of checking `TTZIP_RUN_BENCHMARKS == "1"`.
  - *Action*: Ensure `try XCTSkipUnless(ProcessInfo.processInfo.environment["TTZIP_RUN_BENCHMARKS"] == "1")` is in test `setUp()`.
