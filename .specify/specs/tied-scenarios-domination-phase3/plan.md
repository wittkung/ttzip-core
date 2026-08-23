# Implementation Plan: Tied Scenarios Domination (Phase 3)

## 1. Technical Context & Constraints
- Focus areas:
  - `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`
- Architectural Invariants:
  - In-process C / Swift 6 zero-allocation hot paths.
  - Strict `TTLogger` usage.

## 2. Architecture & Design

### Phase 1: TAR.ZST 64MB Non-Stalling Buffer & Entropy Fast-Path
1. In `ttzip_tar_zstd_direct.c`:
   - Calculate total input payload size.
   - For total payload >= 64MB, allocate a 64MB aligned output buffer (`valloc` / `posix_memalign`).
   - If upfront Shannon entropy > 7.90:
     - Set `ZSTD_c_strategy = ZSTD_fast`.
     - Set `ZSTD_c_targetLength = 0`.
     - Set `ZSTD_c_windowLog = 10`.
     - Set `ZSTD_c_minMatch = 7`.

### Phase 2: 7Z Small-File 16KB Micro-Dictionary & Rapid Collection
1. In `ttzip_lzma2_fast_encoder.c`:
   - If `level == 1`, set `opts.dict_size = 16384`, `opts.nice_len = 6`, `opts.depth = 1`, `opts.mf = LZMA_MF_HC3`.
2. In `ttzip_lzma2_enc_native.c`:
   - For `num_files >= 50` on Level 1, configure chunk divisor to `p_cores * 4` (128KB~256KB chunks).

### Phase 3: Verification & Benchmarking
1. Run `XCTestPerformanceMeasureTests` (9 gates).
2. Run `AllFormatsPkSuiteTests` and verify all tied/trailing scenarios surpass competitor throughput.
3. Run full 561 unit tests.
