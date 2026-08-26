# Phase 1 Quickstart & Validation Guide

**Feature Branch / Spec Directory**: `specs/102-in-place-huffman-and-near-optimal-parser`  
**Created**: 2026-08-18  
**Status**: Completed  

---

## 1. Test Scenarios & Execution Matrix

### Scenario 1: In-Place Huffman Tree Builder Unit & Microbenchmark
- **Command**:
  ```bash
  swift test --filter InPlaceHuffmanTests
  ```
- **Expected Output**:
  - `testInPlaceHuffman_StandardAlphabet_CodewordLengths`: Passed (Codewords satisfy $\le 15$ bits and Kraft equality).
  - `testInPlaceHuffman_ARM64RBIT_BitReversal_Correctness`: Passed (Bit-reversed codewords match RFC 1951).
  - `testInPlaceHuffman_MicrobenchmarkLatency`: Passed ($\le 1.0 \mu s$ on Release, $\le 5.0 \mu s$ on Debug).
- **Failure Diagnostic**:
  - If latency exceeds $5.0 \mu s$, verify that insertion sort or 2-queue loops are properly inlined without dynamic memory allocations.

### Scenario 2: Near-Optimal Level 10-12 Dynamic Programming Compression
- **Command**:
  ```bash
  swift test --filter NearOptimalParserTests
  ```
- **Expected Output**:
  - `testNearOptimal_SilesiaCorpus_CompressionGain`: Passed ($\ge 3.0\%$ size reduction vs Level 6).
  - `testNearOptimal_RFC1951_DecompressionConsensus`: Passed (Bit-for-bit decompressed match with Apple Archive / `unzip`).
  - `testNearOptimal_ThroughputFloor`: Passed ($\ge 8.0$ MB/s Debug, $\ge 18.0$ MB/s Release).
- **Failure Diagnostic**:
  - If decompression fails, verify whether BFINAL and dynamic block header precode information is properly synced.

### Scenario 3: Closed-Loop Zero-Regression Floor Enforcement
- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  - All 13 performance test cases pass with zero failures and 0% regression against pre-optimization baseline.
- **Failure Diagnostic**:
  - Check thread-local compressor level mappings in `CTTZipStreamCoder.c`.
