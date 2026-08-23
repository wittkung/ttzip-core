# Specification: 7Z Final Six Losses Conquest & Universal Dominance

## 1. Background & Target Scenarios
In our latest 1v1 PK benchmark across all 16 scenarios (32 operation items) against official 7-Zip CLI (`7zz`), TTZip achieves 26 wins (up to 35.6x). However, 6 specific battle items remain below 1.00x:

1. **[7Z] [500MB 大文件数据块 (500MB)] | Level 1 | 无加密 | 解压**:
   - TTZip: 3,422.0 MB/s vs 7-Zip `7zz`: 5,629.2 MB/s (0.61x, -39%)
2. **[7Z] [高熵物理Payload (100MB)] | Level 1 | 无加密 | 解压**:
   - TTZip: 3,530.5 MB/s vs 7-Zip `7zz`: 4,391.2 MB/s (0.80x, -20%)
3. **[7Z] [500MB 大文件数据块 (500MB)] | Level 1 | AES-256 | 压缩**:
   - TTZip: 4,997.9 MB/s vs 7-Zip `7zz`: 5,499.9 MB/s (0.91x, -9%)
4. **[7Z] [500MB 大文件数据块 (500MB)] | Level 1 | 无加密 | 压缩**:
   - TTZip: 5,018.8 MB/s vs 7-Zip `7zz`: 5,478.4 MB/s (0.92x, -8%)
5. **[7Z] [海量小文件 (10MB/100文件)] | Level 1 | AES-256 | 压缩**:
   - TTZip: 839.0 MB/s vs 7-Zip `7zz`: 874.8 MB/s (0.96x, -4%, 仅差 35 MB/s)
6. **[7Z] [500MB 大文件数据块 (500MB)] | Level 1 | AES-256 | 解压**:
   - TTZip: 5,194.5 MB/s vs 7-Zip `7zz`: 5,370.9 MB/s (0.97x, -3%, 仅差 176 MB/s)

---

## 2. Technical Requirements & Acceptance Criteria

### REQ-1: 7Z Native Direct In-Process Zero-Copy Extraction Pipeline (Uncompressed & High-Entropy Direct Bypass)
- **Target Items**: Item 1 (500MB L1 Decompress: 3,422 -> >= 6,000 MB/s), Item 2 (100MB High-Entropy L1 Decompress: 3,530 -> >= 5,500 MB/s), Item 6 (500MB L1 AES Decompress: 5,194 -> >= 5,800 MB/s).
- **Core Design**:
  - In `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c` and `ttzip_native_archive.c`:
    - For non-encrypted 7Z extraction, implement a Direct Fast-Path bypassing `archive_read_data_block` chunk looping when decoding large solid streams.
    - Leverage ARM64 NEON CRC32 (`ttzip_compute_buffer_crc32_neon`) for both encrypted and non-encrypted streams.
    - For file size >= 4MB, use `ftruncate` + `mmap` with `MADV_SEQUENTIAL` + `MADV_WILLNEED` direct write buffer.
    - Add Uncompressed Chunk Direct Copy Fast-Path: when LZMA2 indicates an uncompressed chunk (`0x01` / `0x02`), bypass Range Coder and execute AVX/NEON 128-bit aligned `memcpy`.

### REQ-2: 7Z 500MB Level 1 High-Throughput Compressor Pipeline (Fast Match Finder & Lock-Free Ring Buffer)
- **Target Items**: Item 3 (500MB L1 AES Compress: 4,997 -> >= 5,800 MB/s), Item 4 (500MB L1 No-Enc Compress: 5,018 -> >= 5,800 MB/s).
- **Core Design**:
  - In `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` and `ttzip_lzma2_fast_encoder.c`:
    - For single large streams (files >= 50MB, `level == 1`), configure `dict_size = 65536` (64KB), `nice_len = 16`, `depth = 1` with 8MB worker chunk partitions.
    - Pipeline overlapping: worker threads perform LZMA2 block encoding while main writer asynchronously writes 7Z block headers and stream metadata.

### REQ-3: 7Z Small-File AES-256 Solid Stream Zero-Lock Pipeline
- **Target Item**: Item 5 (100 Small Files L1 AES Compress: 839.0 -> >= 950 MB/s).
- **Core Design**:
  - In `ttzip_7z_kdf_arm64.c` and `ttzip_lzma2_enc_native.c`:
    - Session KDF key derivation is pre-warmed once per solid archive.
    - AES-256-CBC multi-block hardware pipeline is initialized per solid stream rather than per entry, eliminating redundant crypto context initialization across 100 small files.

### REQ-4: Regression & Stability Gates
- **Gate 1**: 100% pass on all 561 unit tests (`swift test`).
- **Gate 2**: All 9 performance floors in `XCTestPerformanceMeasureTests` remain strictly green.
- **Gate 3**: Zero performance regressions across all other 26 winning 7Z scenarios and all ZIP/TAR formats.
