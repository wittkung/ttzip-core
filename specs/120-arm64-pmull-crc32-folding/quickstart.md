# Quickstart Validation: ARM64 PMULL / CRC32 Multi-Way Folding & Cache Fusion

**Feature**: `120-arm64-pmull-crc32-folding`
**Created**: 2026-08-19

---

## Validation Scenarios

### Scenario 1: Differential CRC-32 Oracle Correctness Matrix
Validates that the 12-way PMULL polynomial folding kernel produces bit-exact IEEE 802.3 CRC-32 values across 16,384 combinations of buffer lengths (0..1024 bytes) and memory alignments (0..15 bytes).

- **Command**:
  ```bash
  swift test --filter CRC32PmullDifferentialTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'CRC32PmullDifferentialTests' passed
  Executed 1 test, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - If mismatches occur on lengths $len < 16$, inspect the alignment prologue in `ttzip_core_crc32_neon_single()`.
  - If mismatches occur on lengths $64 \le len < 576$, inspect the 4-way folding constants (`CRC32_X543_MODG` / `CRC32_X479_MODG`).
  - If mismatches occur on lengths $len \ge 576$, inspect the 12-way folding reduction tree in `CTTZipCRC32Neon.c`.

---

### Scenario 2: In-Cache Microbenchmark Throughput Floor ($\ge 35\text{ GB/s}$)
Validates that single-core CRC-32 calculation on 32KB~64KB cache-hot buffers achieves $\ge 35\text{ GB/s}$ throughput on Apple Silicon performance cores.

- **Command**:
  ```bash
  swift test --filter CRC32PmullPerformanceGateTests
  ```
- **Expected Output**:
  ```text
  [PASS] Single-Core In-Cache CRC32 Throughput: >= 35000.0 MB/s (Actual: >= 60000.0 MB/s)
  Test Suite 'CRC32PmullPerformanceGateTests' passed
  ```
- **Failure Diagnostic**:
  - If throughput is $< 35\text{ GB/s}$, verify that Clang target attribute `__attribute__((target("aes,crc,sha3")))` is active and that `PMULL2` / `EOR3` instructions are not being emitted as scalar emulation fallbacks.
  - Verify that the test buffer is warm in L1/L2 cache before measuring duration.

---

### Scenario 3: Large Memory Buffer Throughput ($\ge 15\text{ GB/s}$)
Validates that single-core CRC-32 calculation on a 50MB memory buffer sustains memory bus read saturation ($\ge 15\text{ GB/s}$).

- **Command**:
  ```bash
  swift test --filter CRC32PmullLargeBufferTests
  ```
- **Expected Output**:
  ```text
  [PASS] Single-Core Large-Buffer CRC32 Throughput: >= 15000.0 MB/s
  Test Suite 'CRC32PmullLargeBufferTests' passed
  ```
- **Failure Diagnostic**:
  - If throughput falls below $15\text{ GB/s}$, check whether memory is paging out or whether background system activity is throttling memory bus bandwidth.
