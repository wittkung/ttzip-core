# Specification: All-Formats Zero-Loss Performance Domination (Phase 2)

## 1. Background & Motivation
In our 46-dimension universal PK benchmark, TTZip dominates in 42 compression scenarios, but has 3 remaining losses/ties:
1. `[TAR.ZST] [高熵物理Payload (100MB)] | Level 1`: TTZip 3,476.6 MB/s vs `zstd -T0` 6,034.4 MB/s (0.58x).
2. `[TAR.ZST] [500MB 大文件数据块 (500MB)] | Level 1`: TTZip 11,982.5 MB/s vs `zstd -T0` 13,626.3 MB/s (0.88x).
3. `[7Z] [海量小文件 (10MB/100文件)] | Level 1 | AES-256`: TTZip 824.5 MB/s vs 7-Zip `7zz` 843.5 MB/s (0.98x).

## 2. Requirements & Acceptance Criteria
- **REQ-1 (TAR.ZST High-Entropy Fast Path)**:
  - Implement 64KB Shannon entropy check at the start of TAR.ZST direct pipeline.
  - If entropy $> 7.90$, configure `ZSTD_c_strategy = ZSTD_fast` and `ZSTD_c_targetLength = 0` (or zero-length match finder bypass).
  - Target throughput on 100MB incompressible data: **>= 7,000 MB/s** (surpassing `zstd -T0` 6,034 MB/s).
- **REQ-2 (TAR.ZST Multi-Worker Parallel Pipeline)**:
  - In `ttzip_tar_zstd_direct.c`, configure `ZSTD_CCtx_setParameter(cctx, ZSTD_c_nbWorkers, p_cores)` and `ZSTD_CCtx_setParameter(cctx, ZSTD_c_jobSize, 4 * 1024 * 1024)`.
  - Target throughput on 500MB payload: **>= 15,000 MB/s** (surpassing `zstd -T0` 13,626 MB/s).
- **REQ-3 (7Z Small-File AES-256 Acceleration)**:
  - Optimize small file metadata collection in `ttzip_lzma2_enc_native.c` using batch stat/readdir buffering.
  - Target throughput on 100 small files with AES-256: **>= 900 MB/s** (surpassing 7-Zip `7zz` 843 MB/s).
- **REQ-4 (Zero Regressions & CI Safety)**:
  - 100% pass across all 561 unit and performance tests.
  - 0 compiler warnings, 0 runtime errors.
