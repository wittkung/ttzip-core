# Implementation Plan: 7Z Final Six Losses Conquest & Universal Dominance

## 1. Technical Context & Constraints
- Target Focus: Conquering the 6 remaining battle items in 7Z format to achieve a 32/32 full sweep (100% win rate) against official 7-Zip CLI (`7zz`).
- Files to modify:
  - `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`
  - `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`
- Architectural Invariants:
  - Zero heap allocation in parallel loops (pre-allocated thread-local dictionary buffers).
  - Apple Silicon ARM64 NEON SIMD vectorization (hardware AES, SHA-256, CRC32, and 128-bit match copy).
  - 100% pass across all 561 unit tests and 9 hard performance gates.

---

## 2. Phase-by-Phase Architecture & Implementation Design

### Phase 1: Uncompressed Sub-chunk Bypass & Pre-allocated Thread Context for 7Z Decompression
1. In `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c`:
   - Inspect LZMA2 sub-chunk control byte (`0x01` / `0x02`).
   - When control byte is `0x01` (reset dict) or `0x02` (no reset), bypass the `liblzma` / Range Coder state machine and execute 128-bit NEON uncompressed stream copy (`ttzip_neon_copy_match`).
   - Adapt dictionary sizing: dynamically use 64KB dictionary for Level 1 archives rather than allocating 64MB per block.
2. In `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`:
   - Use `ftruncate` + 16KB-aligned `mmap` with `MADV_SEQUENTIAL` + `MADV_WILLNEED` for files >= 4MB.
   - Run ARM64 hardware `CRC32X` (`ttzip_compute_buffer_crc32_neon`) during writeback to overlap memory I/O and verification.
   - Targets: 500MB L1 Decompress ($3,422 \to 6,000+\text{ MB/s}$), 100MB High-Entropy L1 Decompress ($3,530 \to 5,100+\text{ MB/s}$), 500MB L1 AES Decompress ($5,194 \to 5,600+\text{ MB/s}$).

### Phase 2: Level 1 500MB Large Stream Compressor Multi-Core Tuning & Match Finder Optimization
1. In `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`:
   - For continuous large streams (>= 50MB) on Level 1, configure `opts.dict_size = 65536` (64KB), `opts.nice_len = 16`, `opts.depth = 1` with HC3/HC4 match finder.
   - Ensure dictionary and hash tables stay completely within the 128KB L1 Data Cache of Apple Silicon P-cores.
2. In `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`:
   - Optimize chunk partitioning for >= 500MB payloads to 4MB~8MB worker chunks to balance GCD thread scheduling overhead and vectorization throughput.
   - Targets: 500MB L1 No-Enc Compress ($5,018 \to 5,800+\text{ MB/s}$), 500MB L1 AES Compress ($4,997 \to 5,800+\text{ MB/s}$).

### Phase 3: Small-File AES-256 KDF Overlap & In-Place Vectorized Cipher Stream
1. In `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` and `ttzip_lzma2_enc_native.c`:
   - Asynchronously pre-derive the AES-256 session key on a background P-core during the multi-threaded `pread` / metadata collection phase of 100 small files.
   - Perform in-place NEON AES-256-CBC multi-block encryption on the solid output buffer.
   - Target: 100 Small Files L1 AES Compress ($839.0 \to 950+\text{ MB/s}$).

### Phase 4: Verification, Hard Gates & 1v1 Full Sweep Benchmark
1. Run `swift test --filter XCTestPerformanceMeasureTests` to verify all 9 hard performance gates.
2. Run `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests` and verify all 6 previously trailing scenarios flip to dominant wins.
3. Run full 561-test regression suite with `swift test`.
