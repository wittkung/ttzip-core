# Specification: 7Z Final Four 500MB Level 1 Losses Conquest

## 1. Background & Target Scenarios
In the comprehensive 1v1 benchmark against 7-Zip (`7zz`), TTZip has secured 27 wins and 1 tie out of 32 matches (84.4% win rate). All 4 remaining losses are exclusively in the **500MB Giant Payload · Level 1 Fast Mode**:

| Item | Scenario | Operation | Current TTZip | 7-Zip `7zz` | Current Gap | Target |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **1** | 500MB 大文件 · L1 · 无加密 | **压缩** | 4,827.2 MB/s (0.90x) | 5,374.8 MB/s | -547.6 MB/s | **>= 5,600 MB/s** (> 1.04x) |
| **2** | 500MB 大文件 · L1 · 无加密 | **解压** | 4,745.5 MB/s (0.92x) | 5,145.6 MB/s | -400.1 MB/s | **>= 5,500 MB/s** (> 1.05x) |
| **3** | 500MB 大文件 · L1 · AES-256 | **压缩** | 4,402.6 MB/s (0.85x) | 5,157.6 MB/s | -755.0 MB/s | **>= 5,400 MB/s** (> 1.05x) |
| **4** | 500MB 大文件 · L1 · AES-256 | **解压** | 4,631.9 MB/s (0.95x) | 4,869.3 MB/s | -237.4 MB/s | **>= 5,200 MB/s** (> 1.05x) |

---

## 2. Requirements & Acceptance Criteria

### REQ-1: Level 1 LZMA2 Fast Encoder Micro-Dictionary & Direct Pipeline
- In `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` and `ttzip_lzma2_enc_native.c`:
  - For single large files (>= 64MB) on Level 1, configure `opts.dict_size = 65536` (64KB), `opts.nice_len = 8`, `opts.mode = LZMA_MODE_FAST`, `opts.mf = LZMA_MF_HC3`.
  - Partition 500MB into `p_cores * 4` (48~64 blocks of ~8MB~10MB) to maximize CPU multi-pipe utilization while retaining high L2 cache locality.

### REQ-2: Parallel Block Decompression with NEON RLE Broadcast
- In `Sources/CTTZipBridge/ttzip_7z_block_decoder.c` and `ttzip_lzma2_dec_native.c`:
  - Identify all parallel blocks in 500MB payload and execute GCD `dispatch_apply` with worker thread pool.
  - Optimize REP0 (`dist == 1`) copy with ARM64 NEON `vdupq_n_u8` broadcast and 64-byte unrolled stores to avoid RAW hazard.

### REQ-3: In-Place NEON AES-256 Streaming for Single Large Streams
- In `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` and `CTTZipBridge_7zNativeDecoder.c`:
  - Execute ARM64 NEON AES-256 CBC in-place within the page buffer directly before compression / after decompression without intermediate DRAM roundtrips.

### REQ-4: Zero Regression Floor
- All 561 unit tests (`swift test`) pass 100%.
- All 9 upgraded performance gates in `XCTestPerformanceMeasureTests` pass.
- No regressions on existing 27 wins in 7Z, ZIP, TAR.GZ, or TAR.ZST.
