# Implementation Plan: Universal Zero-Loss Grand Slam Across All Formats

## 1. Technical Context & Constraints
- Focus areas:
  - `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`
  - `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`
  - `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`
- Architectural Invariants:
  - Zero heap allocation in hot paths.
  - Pure in-process C static library bindings.
  - Zero regression across existing 561 unit tests and 9 hard performance gates.

---

## 2. Phase-by-Phase Architecture & Implementation Design

### Phase 1: 7Z 500MB Level 1 Multi-Core Compression Scaling (4,664 -> 5,800+ MB/s)
1. In `ttzip_lzma2_enc_native.c`:
   - For `total_uncompressed_bytes >= 64 * 1024 * 1024` on Level 1, configure `block_size = (total_uncompressed_bytes / (p_cores * 2))` (e.g. ~20MB per block for 500MB on 12 P-cores).
   - This creates 24 balanced blocks across 12 P-cores (2 blocks/core), keeping all CPU hardware pipelines 100% saturated while avoiding GCD scheduling thrash.
2. In `ttzip_lzma2_fast_encoder.c`:
   - In `ttzip_lzma2_compress_block_tuned`:
     - Configure `opts.dict_size = 65536` (64KB), `opts.nice_len = 8`, `opts.mode = LZMA_MODE_FAST`, `opts.mf = LZMA_MF_HC3`.

### Phase 2: 7Z In-Process Parallel Decoder Direct Streaming (3,552 -> 5,500+ MB/s)
1. In `ttzip_lzma2_dec_native.c`:
   - In `ttzip_lzma2_decode_block_native`:
     - If the first byte is `0x01` or `0x02` (uncompressed sub-chunks) or block is uncompressed, run 128-bit NEON direct copy (`ttzip_neon_copy_match`).
     - When invoking `lzma_raw_decoder`, allocate preset 6 with `opts.dict_size = 64 * 1024 * 1024`.
2. In `CTTZipBridge_7zNativeDecoder.c`:
   - Use `ftruncate` + 16KB-aligned `mmap` with `MADV_SEQUENTIAL | MADV_WILLNEED` directly writing output chunks.

### Phase 3: TAR.ZST 100MB High-Entropy & 500MB Decompression Turbo (3,709 -> 6,000+ MB/s)
1. In `ttzip_tar_zstd_direct.c`:
   - In `ttzip_create_tar_zstd_direct_c`:
     - For `initial_entropy > 7.75`, set `ZSTD_c_strategy = ZSTD_fast`, `ZSTD_c_targetLength = 0`, `ZSTD_c_windowLog = 10`, `ZSTD_c_overlapLog = 0`.
     - Expand output buffer to 64MB dynamically to prevent stream stall.
   - In `ttzip_extract_tar_zstd_direct_c`:
     - Set 8MB read chunking and direct USTAR header extraction with `mmap` writeback.

### Phase 4: Verification, 9 Hard Performance Gates & 1v1 Full Sweep Benchmark
1. Run `swift test --filter XCTestPerformanceMeasureTests` (verify 9 hard floors).
2. Run `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` (verify full PK dominance).
3. Run full 561-test regression suite with `swift test`.
