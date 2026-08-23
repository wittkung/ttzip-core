# Specification: Tied Scenarios Domination (Phase 3)

## 1. Background & Target Scenarios
In our latest PK matrix, 4 scenarios remain tied or slightly trailing:
1. `[7Z] [海量小文件 (10MB/100文件)] | Level 1 | 无加密`: TTZip 664.3 ~ 888.2 MB/s vs 7-Zip `7zz` 791.5 MB/s.
2. `[7Z] [海量小文件 (10MB/100文件)] | Level 1 | AES-256`: TTZip 749.7 ~ 824.5 MB/s vs 7-Zip `7zz` 761.5 ~ 843.5 MB/s.
3. `[TAR.ZST] [高熵物理Payload (100MB)] | Level 1 | 无加密`: TTZip 3,885.8 MB/s vs `zstd -T0` 5,244.8 MB/s.
4. `[TAR.ZST] [高熵物理Payload (100MB)] | Level 6 | 无加密`: TTZip 4,801.0 MB/s vs `zstd -T0` 6,230.0 MB/s.

## 2. Requirements & Acceptance Criteria
- **REQ-1 (TAR.ZST High-Entropy Non-Blocking Pipeline)**:
  - Expand Zstandard output buffer to 64MB dynamically for files >= 64MB to eliminate thread stalling during `ZSTD_compressStream2`.
  - For high-entropy payloads (entropy > 7.90), configure `ZSTD_c_strategy = ZSTD_fast`, `ZSTD_c_targetLength = 0`, and `ZSTD_c_windowLog = 10`.
  - Target throughput on 100MB high-entropy: **>= 7,000 MB/s** (exceeding `zstd -T0` 5,244 ~ 6,230 MB/s).
- **REQ-2 (7Z Small-File Micro-Dictionary & Metadata Turbo)**:
  - In `ttzip_lzma2_enc_native.c` and `ttzip_lzma2_fast_encoder.c`, for `level == 1` with multi-files (`num_files >= 50`), use 16KB ultra-compact dictionary (`opts.dict_size = 16384`, `opts.nice_len = 6`, `opts.depth = 1`).
  - Pre-allocate directory entry table with exponential growth to eliminate heap reallocation in directory traversal.
  - Target throughput on 100 small files: **>= 950 MB/s** (exceeding 7-Zip `7zz` 761 ~ 843 MB/s).
- **REQ-3 (Stability & Zero Regressions)**:
  - 100% pass across all 561 unit tests.
  - All 9 performance gates remain green.
