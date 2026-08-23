# Implementation Plan: 7Z Final Four 500MB Level 1 Losses Conquest

## 1. Technical Context & Root Causes
- Research findings from 7-Zip LZMA SDK 24.x and Apple Silicon M-series profiling:
  1. **Compression (4,827 vs 5,374 MB/s)**:
     - Capping block size at 1MB created 500 tiny blocks, triggering GCD scheduling thrashing and repeated `lzma_raw_encoder` heap allocations.
     - High-speed zero chunk fast path was bypassed in favor of generic liblzma filter wrappers.
  2. **Decompression (4,745 vs 5,145 MB/s)**:
     - 2.5 GB redundant memory traffic: `posix_memalign(500MB)` -> unpack -> `CRC32(500MB)` pass -> `memcpy(500MB)` into mmap -> `munmap` flush.
  3. **AES-256 (4,402 vs 5,157 MB/s & 4,631 vs 4,869 MB/s)**:
     - Intermediate concatenation buffers and CommonCrypto overhead.

---

## 2. Architecture & File Modifications

### Component 1: `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` & `ttzip_lzma2_fast_encoder.c`
- **Block Sizing**:
  - For single large streams (>= 64MB) on Level 1, size blocks to $16\text{MB} \sim 32\text{MB}$ (16~32 blocks for 500MB) to align with 12~16 P-core execution pipelines and eliminate GCD task storms.
- **Fast Encoder Micro-Tuning**:
  - In `ttzip_lzma2_compress_block_tuned`:
    - Level 1 parameters: `opts.dict_size = 65536` (64KB), `opts.mode = LZMA_MODE_FAST`, `opts.mf = LZMA_MF_HC3`, `opts.nice_len = 8`, `opts.depth = 1`.
    - Zero block detection: Direct zero-chunk formatting without liblzma filter allocation overhead.

### Component 2: `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c` & `ttzip_lzma2_dec_native.c`
- **Zero-Copy Direct-to-mmap Decompression**:
  - Directly `ftruncate` the output file and `mmap` destination disk buffer.
  - Multi-threaded block decoder writes directly to `dst_map + block_offset`, eliminating 500MB `unpack_buf` allocation and 500MB `memcpy` (saving 1.5 GB memory bandwidth).
- **Fused NEON CRC32**:
  - Compute block CRC32 using ARMv8 `__crc32d` instructions in the decoding pass.
- **NEON RLE Broadcast**:
  - In `ttzip_lzma2_dec_native.c`, REP0 `dist == 1` matches use 128-bit NEON `vdupq_n_u8` with 64-byte unrolled stores.

### Component 3: `Sources/CTTZipBridge/ttzip_7z_crypto_neon.c`
- In-place ARMv8 AES-256 CBC encryption and decryption directly within page buffers.

---

## 3. Verification Plan
- **Verification 1**: `swift test --filter XCTestPerformanceMeasureTests` (9 upgraded gates pass).
- **Verification 2**: `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` (verify all 4 remaining 500MB scenarios achieve victory).
- **Verification 3**: `python3 scripts/audit_performance_regression.py` (verify 0 regressions across all 46 benchmarks).
- **Verification 4**: Full regression `swift test` (561 tests pass 100%).
