# Quickstart Validation: Single-Core LZ77 Vector Match Finder

**Feature**: `121-single-core-lz77-vector-match-finder`
**Created**: 2026-08-19

---

## Validation Scenarios

### Scenario 1: Match Length Vector Comparison Oracle Validation
Validates that `ttzip_fast_match_len_arm64` computes the exact match length across 0..258 bytes for all 16 memory misalignment combinations (0..15 bytes).

- **Command**:
  ```bash
  swift test --filter LZ77VectorMatchFinderTests/testMatchLengthVectorOracle
  ```
- **Expected Output**:
  ```text
  Test Suite 'LZ77VectorMatchFinderTests' passed
  Executed 1 test, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - If mismatches occur on lengths $< 8$, verify the 64-bit SWAR trailing zero count (`__builtin_ctzll`).
  - If mismatches occur on lengths $\ge 16$, inspect the 16-byte `vceqq_u8` unrolled vector loop.

---

### Scenario 2: Single-Core Tier 1 Match Finder Throughput ($\ge 2,200\text{ MB/s}$)
Validates that single-core greedy match finding reaches $\ge 2,200\text{ MB/s}$ on standard text and binary corpora.

- **Command**:
  ```bash
  swift test --filter LZ77VectorMatchFinderTests/testTier1MatchFinderThroughputFloor
  ```
- **Expected Output**:
  ```text
  [PASS] Single-Core Tier 1 Match Finder Throughput: >= 2200.0 MB/s
  Test Suite 'LZ77VectorMatchFinderTests' passed
  ```
- **Failure Diagnostic**:
  - If throughput is $< 2,200\text{ MB/s}$, verify that the hash table structure size is $\le 64\text{KB}$ and resides in L1 D-Cache.
