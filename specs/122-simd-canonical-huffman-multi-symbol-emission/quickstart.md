# Quickstart Validation: SIMD Canonical Huffman Coding & Multi-Symbol Emission

**Feature**: `122-simd-canonical-huffman-multi-symbol-emission`
**Created**: 2026-08-19

---

## Validation Scenarios

### Scenario 1: Multi-Symbol Bitstream Serializer Oracle Test
Validates that `ttzip_bs_write_bits64` generates identical RFC 1951 bit sequences to the reference bitstream writer across mixed token sequences.

- **Command**:
  ```bash
  swift test --filter HuffmanBitstreamOptimizationTests/testMultiSymbolBitstreamEmissionOracle
  ```
- **Expected Output**:
  ```text
  Test Suite 'HuffmanBitstreamOptimizationTests' passed
  Executed 1 test, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - If output bits mismatch, verify that `packed_bits` mask calculation preserves LSB-first ordering.

---

### Scenario 2: Single-Core Mixed Workspace Benchmark Validation
Validates that 250MB compound mixed workspace single-core Level 1 compression throughput exceeds $800\text{ MB/s}$.

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter CompoundMixedCorpusBenchmarkPkTests/testCompoundMixedCorpusSingleCoreVsLibdeflate1v1
  ```
- **Expected Output**:
  ```text
  Level 1 (Fast) | >= 800.0 MB/s
  ```
- **Failure Diagnostic**:
  - If throughput is $< 800\text{ MB/s}$, verify that static Huffman fast-path is active on small files ($< 4\text{KB}$).
