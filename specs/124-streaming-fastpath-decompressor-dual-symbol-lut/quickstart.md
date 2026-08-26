# Quickstart Validation: Streaming Fast-Path Decompressor & Dual-Symbol LUT

**Feature**: `124-streaming-fastpath-decompressor-dual-symbol-lut`
**Created**: 2026-08-19

---

## Validation Scenarios

### Scenario 1: Decompressor Oracle Verification
Validates that `ttzip_deflate_decompress` decompresses arbitrary Deflate streams produced by multiple compressors (libdeflate, zlib, ttzip) with 100% bit-exact equivalence.

- **Command**:
  ```bash
  swift test --filter StreamingDecompressorDualSymbolLutTests/testDecompressorOracleEquivalence
  ```
- **Expected Output**:
  ```text
  Test Suite 'StreamingDecompressorDualSymbolLutTests' passed
  Executed 1 test, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - Check 10-bit LUT entry decoding logic for non-standard code length permutations.

---

### Scenario 2: Single-Core Decompression Throughput Floor
Validates that single-core decompression throughput exceeds $\ge 8,000\text{ MB/s}$.

- **Command**:
  ```bash
  swift test --filter StreamingDecompressorDualSymbolLutTests/testDecompressionThroughputFloor
  ```
- **Expected Output**:
  ```text
  [BENCH] Single-Core Deflate Decompression: >= 8000 MB/s
  ```
