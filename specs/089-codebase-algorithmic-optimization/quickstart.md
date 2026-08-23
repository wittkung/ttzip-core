# Quickstart & Validation Guide: Codebase Algorithmic Optimization and Algebraic Kernels

**Feature**: `089-codebase-algorithmic-optimization`
**Created**: 2026-08-18
**Status**: Ready for Verification

---

## Scenario 1: Adler-32 Scalar and NEON Mathematical Correctness & Performance

### 1. Command
```bash
swift test --filter HardwareChecksumTests
```

### 2. Expected Output
```text
Test Suite 'HardwareChecksumTests' passed at ...
	 Executed 7 tests, with 0 failures (0 unexpected) in ... seconds
```
All test vectors (empty data, single byte, ASCII phrases, 64KB multi-chunk data, arbitrary unaligned offsets) pass with 100% bit-exact equivalence.

### 3. Failure Diagnostic
- If `HardwareChecksumTests` fails:
  - Check whether `TTZIP_ADLER32_SCALAR_CHUNK` in `Sources/CTTZipBridge/CTTZipAdler32Neon.c` accumulated more than $N_{\max} = 5552$ bytes before modulo reduction.
  - Verify that `(s1) %= TTZIP_ADLER32_DIVISOR` and `(s2) %= TTZIP_ADLER32_DIVISOR` execute at the boundary of each 5552-byte slice.
  - Verify alignment offset masking `(uintptr_t)p & 15`.

---

## Scenario 2: 7Z Variable-Length Integer Branchless Decoding Verification

### 1. Command
```bash
swift test --filter SevenZipHeaderParserTests
```

### 2. Expected Output
```text
Test Suite 'SevenZipHeaderParserTests' passed at ...
	 Executed ... tests, with 0 failures (0 unexpected)
```
7Z archive metadata parsing completes with 0 failures across 1-byte, 2-byte, 3-byte, 4-byte, and 9-byte 64-bit varints without undefined behavior or integer truncation.

### 3. Failure Diagnostic
- If `SevenZipHeaderParserTests` fails:
  - Check `ttzip_7z_read_varint_fast` in `Sources/CTTZipBridge/ttzip_7z_header_parser.c`.
  - Confirm `__builtin_clz((~(uint32_t)first << 24) | 0x00800000)` properly extracts $k \in [0, 8]$.
  - Confirm shift count is clamped via `(k & 7) * 8` to avoid shift by 64 UB.

---

## Scenario 3: TAR SWAR Octal Parsing and 512-Byte Header Checksum Validation

### 1. Command
```bash
swift test --filter TarNativeArchiveTests
```

### 2. Expected Output
```text
Test Suite 'TarNativeArchiveTests' passed at ...
	 Executed ... tests, with 0 failures (0 unexpected)
```
TAR header parsing succeeds on standard ustar archives, GNU tar binary extensions, and zero-filled End-of-Archive markers.

### 3. Failure Diagnostic
- If `TarNativeArchiveTests` fails:
  - Check `ttzip_octal_parse8_swar` bit masks in `Sources/CTTZipBridge/ttzip_tar_native.c`.
  - Check `ttzip_tar_checksum_512_neon` and linear adjustment formula:
    `*out_unsigned_sum = raw_unsigned - field_u + 256;`
  - Ensure End-of-Archive zero-block detection returns immediately on dual 512-byte zero blocks.

---

## Scenario 4: Full Test Suite and Performance Gate Verification

### 1. Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### 2. Expected Output
```text
Test Suite 'XCTestPerformanceMeasureTests' passed at ...
	 Executed 13 tests, with 0 failures (0 unexpected) in ... seconds
```
All 13 performance floor assertions (ZIP, 7Z, TAR.ZST, LZ4, TAR.XZ, Small Files) pass comfortably above constitutional thresholds.

### 3. Failure Diagnostic
- If any performance test reports degradation:
  - Check if intermediate heap allocations or lock synchronization were introduced into hot loops.
  - Verify that Apple Silicon NEON fast-paths were not bypassed by scalar fallbacks.
