# Quickstart: HyperCompressBench Benchmark Suite

**Feature Branch**: `051-hypercompress-bench-corpus`  
**Created**: 2026-08-17  
**Status**: Draft  
**Source Spec**: [spec.md](./spec.md)

---

## Validation Scenarios

### Scenario 1: Standard Small-File Batch Compression Floor Gate (500 Files)

Runs the fast CI gate verifying that TTZip achieves $\ge 50$ MB/s (Debug) or $\ge 70$ MB/s (Release) on 500+ micro-files across ZIP, TAR.ZST, and 7Z formats.

- **Command**:
  ```bash
  swift test --filter HyperCompressBatchGateTests
  ```

- **Expected Output**:
  ```text
  Test Suite 'HyperCompressBatchGateTests' started at 2026-08-17 04:39:00.000.
  Test Case '-[TTZipTests.HyperCompressBatchGateTests testZipBatchMicroFilesFloorGate]' passed (0.182 seconds).
  [HYPERCOMPRESS] ZIP Level 1: 500 files (3.42 MB uncompressed) -> 1.12 MB (32.7%) @ 118.4 MB/s [PASS >= 70.0 MB/s]
  Test Case '-[TTZipTests.HyperCompressBatchGateTests testTarZstBatchMicroFilesFloorGate]' passed (0.095 seconds).
  [HYPERCOMPRESS] TAR.ZST Level 3: 500 files (3.42 MB uncompressed) -> 0.98 MB (28.6%) @ 184.2 MB/s [PASS >= 70.0 MB/s]
  Test Case '-[TTZipTests.HyperCompressBatchGateTests test7zBatchMicroFilesFloorGate]' passed (0.210 seconds).
  [HYPERCOMPRESS] 7Z LZMA2: 500 files (3.42 MB uncompressed) -> 0.85 MB (24.8%) @ 82.6 MB/s [PASS >= 70.0 MB/s]
  Test Suite 'HyperCompressBatchGateTests' passed at 2026-08-17 04:39:00.487.
  	 Executed 3 tests, with 0 failures (0 unexpected) in 0.487 seconds
  ```

- **Failure Diagnostic**:
  - If throughput falls below 50 MB/s (Debug) or 70 MB/s (Release):
    1. Inspect if per-file heap allocation or `malloc`/`free` was added in `ZipParallelCompressor.swift` or `CTTZipBridge`.
    2. Check whether thread-local context caching (`libdeflate_compressor`) was bypassed or invalidated.
    3. Ensure `allocateAlignedPageBuffer` is used rather than `Data(count:)` page-clearing in the inner batch loop.

---

### Scenario 2: Deep Directory Traversal & VFS Metadata Scanner Benchmark (50,000 Nodes)

Evaluates APFS / NTFS directory tree construction speed on a 50,000-node synthetic hierarchy.

- **Command**:
  ```bash
  TTZIP_RUN_STRESS_BENCHMARKS=1 swift test --filter DirectoryScanPerformanceTests
  ```

- **Expected Output**:
  ```text
  Test Suite 'DirectoryScanPerformanceTests' started at 2026-08-17 04:39:01.000.
  Test Case '-[TTZipTests.DirectoryScanPerformanceTests test50kNodesDirectoryScanPerformance]' passed (0.198 seconds).
  [DIRECTORY_SCAN] Scanned 50,000 nodes (4,218 directories, 45,782 files) in 0.185s -> 270,270 nodes/sec [PASS <= 250.0 ms]
  [DIRECTORY_SCAN] Peak Open FDs: 48 (quota <= 128) [PASS]
  Test Suite 'DirectoryScanPerformanceTests' passed at 2026-08-17 04:39:01.198.
  	 Executed 1 test, with 0 failures (0 unexpected) in 0.198 seconds
  ```

- **Failure Diagnostic**:
  - If scan duration exceeds 250 ms:
    1. Verify `ZipDirectoryScanner` flags include `FTS_NOCHDIR` and `FTS_NOSTAT` to avoid redundant syscalls.
    2. Confirm that tree nodes are collected into a flat linear array rather than recursively building heavy `ArchiveComponentTree` objects.
    3. If `EMFILE` (Too many open files) error occurs, verify directory worker handles are closed promptly.

---

### Scenario 3: Mixed-Entropy Early-Exit & Round-Trip Hash Integrity

Tests the 20% high-entropy slice alongside JSON and log fragments to assert early-exit efficiency and 100% byte fidelity.

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter HyperCompressIntegrityAndEntropyTests
  ```

- **Expected Output**:
  ```text
  Test Suite 'HyperCompressIntegrityAndEntropyTests' started at 2026-08-17 04:39:02.000.
  Test Case '-[TTZipTests.HyperCompressIntegrityAndEntropyTests testMixedEntropyEarlyExitEfficiency]' passed (0.142 seconds).
  [ENTROPY] High-entropy blobs early-exit skipped in 0.021s (Speedup: 3.4x vs full match-search) [PASS]
  Test Case '-[TTZipTests.HyperCompressIntegrityAndEntropyTests testRoundTripByteLevelIntegrity]' passed (0.312 seconds).
  [INTEGRITY] 2,000 micro-files extracted and verified. CRC32 / SHA-256 matches: 2,000/2,000 (100.0%) [PASS]
  Test Suite 'HyperCompressIntegrityAndEntropyTests' passed at 2026-08-17 04:39:02.454.
  	 Executed 2 tests, with 0 failures (0 unexpected) in 0.454 seconds
  ```

- **Failure Diagnostic**:
  - If payload expands > 100.5% or match-search does not early-exit:
    1. Inspect match-finder early-exit threshold in `ttzip_lzma2_*.c` or zstd configuration.
    2. If checksum mismatch occurs, inspect local file header extra field alignment or CRC32 computation in `ZipStoreStreamWriter`.
