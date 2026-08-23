# Quickstart & Verification Guide: Single-Core DEFLATE Engine

**Feature Directory**: `specs/113-single-core-surpass-libdeflate`
**Created**: 2026-08-19
**Status**: Completed

---

## 1. Prerequisites & Environment

- **Architecture**: Apple Silicon ARM64 (macOS 14+) or Intel x86_64 compatible.
- **Build Tools**: Swift 6.0 toolchain, Xcode Command Line Tools (`clang` supporting ARM NEON / AVX2).
- **Test Corpus**: Silesia, Enwik8, Canterbury corpora in `Tests/TTZipTests/Corpus/` or standard test datasets.

---

## 2. Validation Scenarios

### Scenario 1: Single-Core Compression Benchmark & libdeflate Differential PK

Validates that single-core compression throughput at Level 1 and Level 6 exceeds libdeflate baselines.

- **Command**:
  ```bash
  swift test --filter SingleCoreDeflatePkTests
  ```

- **Expected Output**:
  ```text
  Test Suite 'SingleCoreDeflatePkTests' passed at ...
  [SingleCoreDeflatePk] Level 1 (Silesia): TTZip = 2,450.0 MB/s, libdeflate = 2,150.0 MB/s (Delta: +13.95% 🟢)
  [SingleCoreDeflatePk] Level 6 (Silesia): TTZip = 1,420.0 MB/s, libdeflate = 1,350.0 MB/s (Delta: +5.18% 🟢)
  [SingleCoreDeflatePk] Level 1 Ratio: TTZip = 2.45x, libdeflate = 2.45x (Ratio Delta: +0.00% ⚪)
  Executed 12 tests, with 0 failures (0 unexpected) in 4.120 seconds.
  ```

- **Failure Diagnostic**:
  - If throughput is lower than libdeflate: Verify whether ARM NEON SWAR Tier 0 fast-check is active and whether `__ARM_NEON` is enabled in compilation flags.
  - If ratio degrades: Check whether pre-compiled archetype classifier selected a mismatched dynamic header.

---

### Scenario 2: Single-Core Dual-Symbol Decompression Verification

Validates that 12-bit dual-symbol direct Huffman decoding and NEON small-distance match replication surpass libdeflate single-core decompression.

- **Command**:
  ```bash
  swift test --filter SingleCoreDecompressPkTests
  ```

- **Expected Output**:
  ```text
  Test Suite 'SingleCoreDecompressPkTests' passed at ...
  [SingleCoreDecompressPk] (Silesia Text): TTZip = 11,200.0 MB/s, libdeflate = 10,050.0 MB/s (Delta: +11.44% 🟢)
  [SingleCoreDecompressPk] Small Dist D<16: TTZip = 12,800.0 MB/s, libdeflate = 11,100.0 MB/s (Delta: +15.31% 🟢)
  Executed 8 tests, with 0 failures (0 unexpected) in 2.850 seconds.
  ```

- **Failure Diagnostic**:
  - If extraction throughput lags: Verify `HUFFDEC_DUAL_LITERAL` entry bitmask resolution and ensure `permute_table` memory alignment is 16 bytes.
  - If memory corruption occurs: Verify `CHUNKCOPY` bound clamping against `out_end`.

---

### Scenario 3: Cross-Ecosystem Oracle & Bit-Exact Verification

Validates that generated DEFLATE streams round-trip through system `/usr/bin/unzip`, `/usr/bin/gzip`, and reference `zlib`.

- **Command**:
  ```bash
  swift test --filter SingleCoreDeflateOracleTests
  ```

- **Expected Output**:
  ```text
  Test Suite 'SingleCoreDeflateOracleTests' passed at ...
  [Oracle] 1000/1000 randomized streams decompressed with /usr/bin/gzip: PASS (0 errors)
  [Oracle] 1000/1000 randomized streams decompressed with /usr/bin/unzip: PASS (0 errors)
  [Oracle] Round-trip SHA-256 byte-for-byte exact matches: 100.00%
  Executed 10 tests, with 0 failures (0 unexpected) in 3.450 seconds.
  ```

- **Failure Diagnostic**:
  - If `/usr/bin/gzip -t` reports CRC error: Verify Adler-32 / CRC-32 endianness and trailer emission order.
  - If decompression fails at block boundary: Check EOB (End of Block, symbol 256) codeword emission in bitstream flush.
