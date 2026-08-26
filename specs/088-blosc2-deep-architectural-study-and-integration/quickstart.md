# Phase 1 Quickstart Validation Guide: Blosc2 Meta-Compression Architecture

**Feature**: `088-blosc2-deep-architectural-study-and-integration`
**Date**: 2026-08-18
**Status**: Ready

---

## 1. Prerequisites & Environment

- **Operating System**: macOS 14.0+ (Sonoma, Sequoia) on Apple Silicon (M1/M2/M3/M4) or Intel x86_64.
- **Toolchain**: Swift 6.0 (`swift-tools-version: 6.0`), Apple Clang (C11 / ARM NEON SIMD).
- **Build Configuration**: Standard Debug and Release builds.

```bash
# Verify Swift toolchain and hardware architecture
swift --version
uname -m
```

---

## 2. Validation Scenarios

### Scenario 1: SIMD BitShuffle & ByteDelta Roundtrip & Throughput Verification

Validates that ARM NEON vector bit-matrix transposition (64-bit delta-swap on `uint64x2_t`) and 128-byte unrolled ByteDelta differencing achieve lossless roundtrip and meet hardware throughput floors.

- **Command**:
  ```bash
  swift test --filter Blosc2AdvancedArchitecturesTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'Blosc2AdvancedArchitecturesTests' passed at ...
  Executed 6 tests, with 0 failures (0 unexpected) in 0.214 seconds
  ```
- **Failure Diagnostic**:
  - If tests fail with checksum mismatch on odd sizes: inspect `bshuf_trans_bit_byte_neon` tail-cascade handling for buffers where `len % 64 != 0`.
  - If throughput drops below 4,000 MB/s: check compiler optimization flags (`-O3`) and ensure loop variables are not spilled to stack.

---

### Scenario 2: Special-Value Uniform Block Bypass & Memory-Bus Saturated Fill

Validates branchless SIMD scanning of uniform blocks (`SPECIAL_ZERO`, `SPECIAL_NAN`, `SPECIAL_VALUE`) and verifies that decompression bypass achieves $> 25,000\text{ MB/s}$ on Apple Silicon via `dc zva` and `memset_pattern8`.

- **Command**:
  ```bash
  swift test --filter Blosc2SpecialValueTests
  ```
- **Expected Output**:
  ```text
  Test Case '-[TTZipTests.Blosc2SpecialValueTests testSpecialZeroBypassThroughput]' passed (measured: >= 35,000 MB/s).
  Test Suite 'Blosc2SpecialValueTests' passed with 0 failures.
  ```
- **Failure Diagnostic**:
  - If throughput is $< 20,000\text{ MB/s}$: verify that libc `memset` is executing the `dc zva` hardware block zero instruction instead of a byte-by-byte loop.

---

### Scenario 3: Two-Tier Cache-Aware Partitioning & Shared Dictionary Compression

Validates Super-Chunk $\rightarrow$ Chunk $\rightarrow$ Block hierarchy (128KB L1D alignment) and pre-trained frame dictionary sharing across small structured records.

- **Command**:
  ```bash
  swift test --filter Blosc2SuperChunkTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'Blosc2SuperChunkTests' passed with 0 failures.
  Compression ratio with shared dictionary: >= 2.8x vs 1.4x independent.
  ```
- **Failure Diagnostic**:
  - If dictionary training fails or yields low ratios: check that sample buffer slice count is $\ge 100$ items and dictionary size is clamped to $\le 112\text{ KB}$.

---

### Scenario 4: Small-Sample Heuristic Auto-Tuning (BTune Micro-Probe)

Validates that 16KB micro-sampling correctly identifies incompressible files (bypassing filters in $< 3.5\,\mu\text{s}$) and engages BitShuffle + ByteDelta on floating-point arrays.

- **Command**:
  ```bash
  swift test --filter Blosc2HeuristicTunerTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'Blosc2HeuristicTunerTests' passed with 0 failures.
  Micro-probe overhead: <= 0.15% on 10MB test streams.
  ```
- **Failure Diagnostic**:
  - If micro-probe causes false positives on encrypted streams: inspect the Shannon entropy threshold ($H > 7.65\text{ bits/byte}$) in `ttzip_heuristic_tuner_probe`.

---

### Scenario 5: Full Regression & Performance Floor Gate

Ensures zero degradation across TTZip's 525+ existing test suite and 13 performance floors.

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  ```text
  Executed 13 tests, with 0 failures (0 unexpected).
  All 13 performance floors green.
  ```
- **Failure Diagnostic**:
  - If any performance floor regresses: locate the offending commit, inspect memory allocations with Instruments, and ensure zero heap allocation on hot paths.
