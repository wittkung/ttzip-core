# Specification: Universal Zero-Loss Grand Slam Across All Formats

## 1. Background & Target Scenarios
TTZip has achieved 100% win rates in ZIP (32/32) and TAR.GZ (16/16), and 81.2% win rate in 7Z (26/32). This feature targets achieving 100% victory across all formats by conquering the remaining bottlenecks in 7Z and TAR.ZST:

1. **[7Z] [500MB 大文件 (500MB)] | Level 1 | 压缩 (无加密 & AES-256)**:
   - Current: 4,664.6 ~ 4,967.1 MB/s vs 7-Zip `7zz` 5,475.6 ~ 5,486.6 MB/s (0.85x ~ 0.91x).
   - Target: **>= 5,800 MB/s** (> 1.05x).
2. **[7Z] [500MB 大文件 (500MB)] | Level 1 | 解压 (无加密 & AES-256)**:
   - Current: 3,552.4 ~ 4,143.1 MB/s vs 7-Zip `7zz` 4,600.9 ~ 5,274.1 MB/s (0.67x ~ 0.90x).
   - Target: **>= 5,500 MB/s** (> 1.05x).
3. **[7Z] [高熵物理Payload (100MB)] | Level 1 | 无加密 | 解压**:
   - Current: 3,188.7 MB/s vs 7-Zip `7zz` 3,762.4 MB/s (0.85x).
   - Target: **>= 4,200 MB/s** (> 1.05x).
4. **[TAR.ZST] [高熵物理Payload (100MB)] | Level 1 | 压缩 & 解压**:
   - Current: Compress 3,709.4 MB/s vs `zstd -T0` 5,258.3 MB/s (0.71x); Decompress 3,868.4 MB/s vs `zstd -T0` 6,380.8 MB/s (0.61x).
   - Target: Compress **>= 6,000 MB/s**, Decompress **>= 6,500 MB/s** (> 1.05x).
5. **[TAR.ZST] [500MB 大文件 (500MB)] | Level 1 & Level 6 | 解压**:
   - Current: 4,971.2 ~ 5,549.0 MB/s vs `zstd -T0` 5,741.3 ~ 6,354.4 MB/s (0.87x).
   - Target: **>= 6,800 MB/s** (> 1.05x).

---

## 2. Requirements & Acceptance Criteria

### REQ-1: 7Z 500MB Level 1 Multi-Block Parallel LZMA2 Streaming & RLE Optimization
- In `ttzip_lzma2_enc_native.c` and `ttzip_lzma2_fast_encoder.c`:
  - For single large streams (>= 50MB) on Level 1, partition into `p_cores * 2` independent parallel LZMA2 blocks (e.g. 20MB~25MB per block) so that all 12 P-cores compress concurrently at maximum memory bandwidth.
  - In `ttzip_lzma2_compress_block_tuned`, configure `opts.dict_size = 65536`, `opts.nice_len = 8`, `opts.mode = LZMA_MODE_FAST`, `opts.mf = LZMA_MF_HC3` for rapid match finding.

### REQ-2: 7Z In-Process Parallel Decoder Direct Streaming & Uncompressed Bypass
- In `ttzip_lzma2_dec_native.c` and `CTTZipBridge_7zNativeDecoder.c`:
  - On decompression of 7Z solid streams, decode independent LZMA2 blocks in parallel using GCD `dispatch_apply` directly into the `mmap` destination memory.
  - Bypass intermediate heap buffers: write decoded bytes directly into the target file mapped pages with `MADV_SEQUENTIAL`.

### REQ-3: TAR.ZST Direct In-Process High-Entropy Multi-Threaded Streaming
- In `ttzip_tar_zstd_direct.c`:
  - For 100MB high-entropy payloads (entropy > 7.75) on Level 1, use `ZSTD_c_strategy = ZSTD_fast`, `ZSTD_c_windowLog = 10`, `ZSTD_c_nbWorkers = p_cores`, and allocate 64MB streaming buffers to prevent any worker stalling.
  - In `ttzip_extract_tar_zstd_direct_c`, implement Direct In-Process Zstd block streaming with 8MB read chunking and direct USTAR header extraction, eliminating libarchive filter overhead.

### REQ-4: Hard Stability & Zero Regressions
- 100% pass on all 561 unit tests (`swift test`).
- All 9 performance gates in `XCTestPerformanceMeasureTests` remain strictly green.
- Zero regressions across ZIP (100%), TAR.GZ (100%), and existing winning scenarios.
