# Task Breakdown: Zstandard Match Counting Acceleration & Dual-Track Execution

**Feature**: `061-zstd-match-counting-acceleration`
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/spec.md) | **Plan**: [plan.md](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/plan.md)

---

## Phase 1: Upstream PR 1 — ARM64 NEON Vectorization in `ZSTD_count()` (Track 1)

- [x] T001 [P] [US1] Implement Tier 0 64-bit GPR SWAR + Tier 1 128-bit NEON unrolling in `ZSTD_count()` in `Vendor/worktrees/zstd/pr1-arm64-neon-count/lib/compress/zstd_compress_internal.h`
- [x] T002 [US1] Compile and verify clean build of `libzstd.a` in `Vendor/worktrees/zstd/pr1-arm64-neon-count/`

---

## Phase 2: Upstream PR 2 — ARMv8 CRC32 Hardware Hashing in ZSTD (Track 1)

- [x] T003 [P] [US2] Implement `__crc32w` and `__crc32d` hardware acceleration in `ZSTD_hash4` and `ZSTD_hash8` in `Vendor/worktrees/zstd/pr2-arm64-crc32-hash/lib/compress/zstd_compress_internal.h`
- [x] T004 [US2] Compile and verify clean build of `libzstd.a` in `Vendor/worktrees/zstd/pr2-arm64-crc32-hash/`

---

## Phase 3: TTZip Internal Double-Fast Match Finder Absorption (Track 2)

- [x] T005 [P] [US3] Add Double-Fast dual-hash struct definitions (`ttzip_double_fast_t`) and workspace prototypes in `Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`
- [x] T006 [P] [US3] Implement Double-Fast lookahead search and contiguous zero-allocation workspace support in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T007 [US3] Integrate and benchmark hybrid SWAR + NEON in `Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h`

---

## Phase 4: Full Regression, Performance Floor & Upstream Artifact Review

- [x] T008 Run full unit test suite `swift test --filter Zstd` and `swift test --filter XCTestPerformanceMeasureTests`
- [x] T009 Prepare atomic commit structures, diffs, and PR submission templates for user authorization
