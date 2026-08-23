# Implementation Plan: All-Formats Zero-Loss Performance Domination (Phase 2)

## 1. Technical Context & Constraints
- **Target Subsystems**:
  - `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`: In-process Pax TAR + Zstandard streaming compression.
  - `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`: 7z solid block allocation, file metadata ingestion, and AES-256 pipeline.
- **Architectural Invariants**:
  - Zero-heap allocation in hot data paths.
  - Zero raw `printf` / `NSLog` calls (strict `TTLogger` / `ttzip_log`).
  - Swift 6 strict concurrency compliance.
  - Hard performance floors must pass.

## 2. Architecture & Design

### Phase 1: TAR.ZST ZSTD_c_nbWorkers & High-Entropy Bypass
1. In `ttzip_tar_zstd_direct.c`:
   - Obtain `p_cores` using `sysctlbyname("hw.perflevel0.physicalcpu", ...)`.
   - Set `ZSTD_CCtx_setParameter(cctx, ZSTD_c_nbWorkers, (int)p_cores)`.
   - Set `ZSTD_CCtx_setParameter(cctx, ZSTD_c_jobSize, 4 * 1024 * 1024)`.
   - Set `ZSTD_CCtx_setParameter(cctx, ZSTD_c_overlapLog, 0)` for level 1 to maximize throughput.
   - For single-file / memory payloads, check sample entropy: if $> 7.90$, use strategy `ZSTD_fast` with `ZSTD_c_minMatch = 7`.

### Phase 2: 7Z Small Files AES-256 Micro-Batching
1. In `ttzip_lzma2_enc_native.c`:
   - In `1_CollectEntries`, optimize memory pre-allocation for `list.entries` to avoid reallocations.
   - For `num_files >= 50` on Level 1 AES-256, tune chunk size to 128KB~256KB to fully pipeline NEON AES and LZMA2 encodings.

### Phase 3: Performance Regression & Benchmark Verification
1. Run `XCTestPerformanceMeasureTests` across all 9 gates.
2. Run `AllFormatsPkSuiteTests` and verify all 46 scenarios achieve >= 1.00x vs competitors.
