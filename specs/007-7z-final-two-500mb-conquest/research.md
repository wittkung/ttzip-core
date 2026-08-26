# Research: 500MB 7Z Peak Compression Engineering

## 1. 7-Zip LZMA SDK 24.x 500MB Fast-Path Mechanism

- **7-Zip `CLzma2Enc` 内部机制**:
  - 对于大文件，7-Zip 将输入切分为 `numThreads` 个独立流。
  - 在 Level 1 下，`dictSize = 64KB, algorithm = 0, matchFinder = HC3, niceLen = 8, cutValue = 1`。
  - 对于零流（`0x00`），7-Zip 的 `LzmaEnc_Fast` 在哈希发现匹配直接命中 `dist == 1` 时，直接按最大 `niceLen`（273 字节）快速展开，完全跳过子树分支搜索。
- **TTZip 改进路径**:
  - 在 `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 中，当块为全零（`is_zero_block`）或 Level 1 模式时：
    - `opts.mode = LZMA_MODE_FAST;`
    - `opts.mf = LZMA_MF_HC3;`
    - `opts.nice_len = 273;`
    - `opts.depth = 1;`
    - `opts.dict_size = 4096;`
  - 在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 中：
    - 当 `total_uncompressed_bytes >= 64MB` 时，将块划分为 `p_cores * 2`（在 12 核上划分 24 块，每块 ~20.8MB）。
    - 采用零开销栈分配输出缓冲区，消除单块 `malloc`/`free`。

## 2. In-Place AES-256 加密与内存对齐

- 加密路径直接在块输出缓冲区的 16 字节对齐边界上进行 ARMv8 NEON 向量化原地加密（`vaeseq_u8`），实现 0 内存拷贝。
