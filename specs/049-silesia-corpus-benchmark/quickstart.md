# Quickstart: Silesia Corpus Benchmark Fixtures & Regression Gates

## Validation Scenarios

### Scenario 1: Verify Silesia Fixture Manifest & Cryptographic Integrity

**Prerequisites**: Silesia corpus files and `silesia_manifest.json` present under `Tests/TTZipTests/Fixtures/Silesia/`.

**Command**:
```bash
swift test --filter SilesiaCorpusIntegrityTests
```

**Expected Output**:
```text
Test Suite 'SilesiaCorpusIntegrityTests' started
Test Case '-[TTZipTests.SilesiaCorpusIntegrityTests testCorpusManifestIntegrity]' passed (0.045 seconds).
Test Case '-[TTZipTests.SilesiaCorpusIntegrityTests testAll12FilesByteLengthAndSha256]' passed (0.182 seconds).
Executed 2 tests, with 0 failures (0 unexpected) in 0.227 seconds
```

**Failure Diagnostic**:
- If missing files: Confirm `Tests/TTZipTests/Fixtures/Silesia/` contains all 12 uncompressed files (`dickens`, `mozilla`, `mr`, `nci`, `ooffice`, `osdb`, `reymont`, `samba`, `sao`, `webster`, `xml`, `x-ray`).
- If hash mismatch: Recompute SHA-256 via `shasum -a 256 Tests/TTZipTests/Fixtures/Silesia/<file>` and check against `silesia_manifest.json`.

---

### Scenario 2: Execute Zero-Regression Silesia Benchmark Suite across Primary Formats

**Prerequisites**: Build with release optimizations to test full Apple Silicon UMA streaming performance.

**Command**:
```bash
TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter SilesiaCorpusBenchmarkSuiteTests
```

**Expected Output**:
```text
Test Suite 'SilesiaCorpusBenchmarkSuiteTests' started
[SilesiaBenchmark] Platform: macOS 14.x (arm64), Cores: 12, Total Corpus: 211,945,550 bytes
[SilesiaBenchmark] Format: ZIP | Level: 1 | Comp: 2154.2 MB/s | Decomp: 11420.8 MB/s | Floor: PASS (Δ +2.4%)
[SilesiaBenchmark] Format: 7Z  | Level: 1 | Comp: 3950.1 MB/s | Decomp: 7890.4 MB/s  | Floor: PASS (Δ +1.8%)
[SilesiaBenchmark] Format: TAR.ZST | L1   | Comp: 22800.0 MB/s| Decomp: 10450.0 MB/s | Floor: PASS (Δ +0.9%)
Test Case '-[TTZipTests.SilesiaCorpusBenchmarkSuiteTests testSilesiaAllFormatsZeroRegressionGate]' passed (18.420 seconds).
Executed 1 test, with 0 failures (0 unexpected) in 18.420 seconds
```

**Failure Diagnostic**:
- If regression error (`Throughput regressed by > 3.0%`): Review the printed table for the specific regressed file. Check if a recent commit added heap allocations (`Data(count:)` or `malloc`) or thread contention (`NSLock`) into hot paths.
- If variance exceeds 2.5%: Ensure the system is not under thermal throttling or heavy background I/O contention.

---

### Scenario 3: Granular Corpus Diagnostic Report Generation

**Prerequisites**: `ttzip-cli` built in debug/release mode.

**Command**:
```bash
swift run ttzip-cli bench --silesia --format zip --json
```

**Expected Output**:
```json
{
  "timestamp": "2026-08-17T04:25:00Z",
  "platform": "macOS 14.x",
  "architecture": "arm64",
  "cpuCores": 12,
  "totalCorpusThroughputMBps": 2154.2,
  "records": [
    {
      "fileName": "dickens",
      "format": "ZIP",
      "uncompressedBytes": 10192446,
      "compressedBytes": 3840112,
      "compressionRatioPercent": 37.68,
      "compressionDurationSeconds": 0.0048,
      "compressionThroughputMBps": 2123.4,
      "decompressionDurationSeconds": 0.0009,
      "decompressionThroughputMBps": 11324.9,
      "coefficientOfVariationPercent": 1.2,
      "checksumMatched": true,
      "passedRegressionFloor": true
    }
  ],
  "allPassed": true
}
```

**Failure Diagnostic**:
- If JSON parsing fails: Validate output schema against `contracts/benchmark_report.schema.json`.
