# Quickstart & Verification Guide: Unified SOTA Codec Architecture

**Feature**: `142-unified-sota-codec-architecture`  
**Date**: 2026-08-20  
**Phase**: Phase 1 Design

---

## 1. Overview & Verification Scenarios

### Scenario 1: Multi-Core Deflate Bitstream Compliance Verification
Verifies that parallel Deflate chunking strictly adheres to RFC 1951 BFINAL standards and extracts with 100% byte fidelity via `/usr/bin/unzip`.

- **Command**:
  ```bash
  swift test --filter AllFormatsAndAdvancedParametersMatrixTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'AllFormatsAndAdvancedParametersMatrixTests' passed at 2026-08-20.
  	 Executed 20 tests, with 1 test skipped and 0 failures in 1.072 seconds.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Extraction truncated or checksum mismatch on last chunk.
  - *Remediation*: Check `ttzip_bitstream_seq.c` to confirm BFINAL=0 on intermediate chunks and BFINAL=1 on the terminal chunk.

---

### Scenario 2: SOTA Single-Core Fast-LZMA2 Speedup Assertion
Validates that `fast-lzma2` compression outperforms standard scalar LZMA2 by $\ge 2.5\times$.

- **Command**:
  ```bash
  swift test --filter SevenZipBridgeTests
  ```
- **Expected Output**:
  ```text
  [Fast-LZMA2 Bench] Single-Core Throughput: 38.5 MB/s (vs Baseline 11.2 MB/s, Speedup: 3.43x)
  Test Case '-[TTZipTests.SevenZipBridgeTests testFastLZMA2RadixMatchFinder]' passed.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Speedup $< 2.0\times$.
  - *Remediation*: Verify Radix Match Finder table size in `fast-lzma2/radix_mf.c` is properly aligned to CPU L1/L2 cache boundaries.

---

### Scenario 3: Memory Envelope Invariant Assertion under 50GB Payload
Validates that resident memory during massive chunked streaming never exceeds 128MB.

- **Command**:
  ```bash
  swift test --filter BatchSmallFileMemoryTests
  ```
- **Expected Output**:
  ```text
  [Memory-Invariant] Peak Resident Set Size (RSS): 48.2 MB (Cap: 128.0 MB) -> PASSED
  Test Case '-[TTZipTests.BatchSmallFileMemoryTests testBoundedMemoryStreaming]' passed.
  ```
- **Failure Diagnostic**:
  - *Symptom*: Memory footprint exceeds 128MB or triggers kernel swapping.
  - *Remediation*: Check `MemoryPageFlyweightPool` recycling logic and ensure buffer allocations pair with `deallocateAlignedPageBuffer`.
