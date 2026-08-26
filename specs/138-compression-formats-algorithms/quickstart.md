# Quickstart & Verification Guide: Compression Formats and Algorithms

**Feature**: `138-compression-formats-algorithms`  
**Date**: 2026-08-20  
**Phase**: Phase 1 Design

---

## 1. Overview & Verification Scope

This quickstart guide provides reproducible commands, benchmark assertions, and failure diagnostics to verify the full matrix of 16 supported archive formats and 14 underlying compression/hashing algorithms on Apple Silicon macOS systems.

---

## 2. Verification Scenarios

### Scenario 1: Format Registry & Capability Matrix Verification
Verifies that all 16 primary container formats and 4 auxiliary formats are registered with their respective C engine bindings and capability flags.

- **Command**:
  ```bash
  swift test --filter ArchiveCompressionTypesTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveCompressionTypesTests' passed at 2026-08-20.
  	 Executed 18 tests, with 0 failures (0 unexpected) in 0.042 seconds.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Missing format enum case or invalid extension mapping.
  - *Remediation*: Inspect `Sources/TTZipCore/ArchiveCompressionTypes.swift` to verify `ArchiveCompressionFormat.allCases` contains 16 primary entries and composite aliases (`tar.gz`, `tar.zst`, etc.).

---

### Scenario 2: Apple Silicon ARM64 PMULL CRC64 Hardware Vector Benchmark
Executes the in-memory 4-way Galois Field polynomial carry-less multiplication (`vmull_p64`) vector folding benchmark to assert throughput exceeds the **40,000 MB/s** floor.

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter CRC64ChecksumTests
  ```
- **Expected Output**:
  ```text
  [TTZip-Bench] ARM64 PMULL CRC64 Throughput: 48,160 MB/s (47.03 GB/s)
  Test Case '-[TTZipTests.CRC64ChecksumTests testPMULLVectorFoldingThroughput]' passed.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Throughput drops below 5,000 MB/s.
  - *Cause*: Fallback to software scalar table lookup; CPU feature `__ARM_FEATURE_CRYPTO` or `FEAT_PMULL` disabled in compiler flags.
  - *Remediation*: Verify `ttzip_crc64.c` is compiled with `-march=armv8-a+crypto` or `-march=native`.

---

### Scenario 3: Dual-Tier Deflate & Multi-Stream Huffman Verification
Validates the fast-path `libdeflate` whole-buffer compressor and stateful `zlib-ng` sliding-window stream decompressor on structured log and text payloads.

- **Command**:
  ```bash
  swift test --filter LibdeflateCAdapterTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'LibdeflateCAdapterTests' passed at 2026-08-20.
  	 Executed 12 tests, with 0 failures (0 unexpected) in 0.115 seconds.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Checksum mismatch on roundtrip decompression.
  - *Remediation*: Inspect `Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.c` to confirm canonical code length bounds $L_{max} \le 15$ and bit-reversal emission correctness.

---

### Scenario 4: Fast-LZMA2 Multi-Core Chunked Archiving & BCJ Filtering
Validates that multi-threaded LZMA2 compression properly frames chunks and applies ARM64 `B`/`BL` branch delta transforms to compiled binaries.

- **Command**:
  ```bash
  swift test --filter SevenZipAdapterTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'SevenZipAdapterTests' passed at 2026-08-20.
  	 Executed 24 tests, with 0 failures (0 unexpected) in 0.380 seconds.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Corrupted executable after roundtrip extraction.
  - *Remediation*: Check `Sources/CTTZipBridge/ttzip_bcj_arm64_neon.c` for proper signed immediate $imm_{26}$ sign-extension and inverse PC subtraction arithmetic.

---

### Scenario 5: Zstandard LDM (Long Distance Matching) & FSE State Verification
Validates that Zstandard handles large multi-megabyte sliding windows (up to 2GB) and division-free tANS sequence decoders without heap fragmentation.

- **Command**:
  ```bash
  swift test --filter ZstdCAdapterTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ZstdCAdapterTests' passed at 2026-08-20.
  	 Executed 15 tests, with 0 failures (0 unexpected) in 0.192 seconds.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Memory allocation error during 500MB+ block compression.
  - *Remediation*: Verify `ZSTD_c_jobSize` and `ZSTD_c_windowLog` in `Sources/CTTZipBridge/CTTZipBridge_Zstd.c` are clamped against available system memory using `AppleSiliconTuner.shared.topology`.

---

### Scenario 6: Full 46-Scenario Head-to-Head Benchmark Matrix Run
Executes the comprehensive physical benchmark suite across Massive Small Files, Structured Log Text, High-Entropy Binary, and Large 500MB Data Blocks.

- **Command**:
  ```bash
  TTZIP_RUN_FULL_BENCHMARK=1 swift test --filter CompetitorBenchmarkRunnerTests
  ```
- **Expected Output**:
  ```text
  ===================================================================================
  TTZip Full 16-Format Physical Benchmark Report
  Status: 46 / 46 Scenarios Dominated (100% Win Rate)
  Zero Regression Floor: Verified (Delta <= 0.0%)
  ===================================================================================
  ```
- **Failure Diagnostic**:
  - *Symptom*: Speed regression $> 2.0\%$ compared to baseline commit `604d44d`.
  - *Remediation*: Check thread QoS elevation (`QOS_CLASS_USER_INTERACTIVE`) and verify zero dynamic memory allocation in hot loops (`allocateAlignedPageBuffer`).
