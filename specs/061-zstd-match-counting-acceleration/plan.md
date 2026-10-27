# Implementation Plan: Zstandard Match Counting SWAR SIMD Acceleration & Upstream Alignment

**Branch**: `061-zstd-match-counting-acceleration` | **Date**: 2026-08-17 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/spec.md)

**Input**: Feature specification from `specs/061-zstd-match-counting-acceleration/spec.md`

---

## 1. Summary

This feature executes a dual-track strategy:
- **Track 1 (Upstream PR Submission)**: Submitted Pull Request to `facebook/zstd`:
  - **PR #4736**: [facebook/zstd#4736](https://github.com/facebook/zstd/pull/4736) (`feat/arm64-neon-rle-detect`): `[AArch64] Optimize ZSTD_isRLE() with 64-byte NEON vector unrolling`. (PR 1 and PR 2 retired following upstream determinism & microarchitecture audits).
- **Track 2 (Internal Engine Absorption)**: Deeply absorb zstd's Double-Fast dual-hash architecture (4B short + 8B long match lookup) into TTZip's `ttzip_lzma_hc4_neon.c` and `CTTZipNEONMatchFinder.h` with zero heap allocation on hot paths.

---

## 2. Technical Context

- **Language/Version**: C11 / POSIX APIs, Swift 6.0 (`swift-tools-version: 6.0`), ARMv8-A / Apple Silicon NEON & CRC32 intrinsics.
- **Primary Dependencies**: `libzstd` (Vendor / In-process C static binding), `Vendor/worktrees/zstd/` isolated upstream worktrees.
- **Testing**: `swift test --filter Zstd`, `swift test --filter XCTestPerformanceMeasureTests`, `make -C Vendor/worktrees/zstd/*/lib`.
- **Target Platform**: macOS 14.0+ (Apple Silicon primary, x86_64 secondary), Windows (AVX2 compatibility).
- **Performance Goals**:
  - TAR.ZST Direct Pack: $\ge 15,000\text{ MB/s}$ (Debug) / $\ge 22,000\text{ MB/s}$ (Release).
  - Match counting micro-throughput: $\ge 4.5\text{ GB/s}$.
- **Constraints**: Zero dynamic heap allocations in inner compression loops; zero modification of frozen ZIP files.

---

## 3. Constitution Check

- [x] **Zero-Cost Abstraction on Hot Paths**: Double-Fast tables use single preallocated workspace buffer; zero per-block `malloc`/`free`.
- [x] **Subsystem Freeze Compliance**: Zero modifications to `ZipParallelExtractor.swift`, `CTTZipExtract.c`, or other frozen files.
- [x] **Upstream Remote Action Hard Gate**: All upstream git branches isolated in worktrees; zero push/PR creation without explicit user authorization.
- [x] **Logging Discipline**: Zero bare `printf`/`puts`; `TTLogger` used for Swift logging.

---

## 4. Phase 0: Research Items

- R001 [SUBAGENT:research] 《Double-Fast 双哈希查找表架构》：在 zstd 的 `zstd_double_fast.c` 中分析 4 字节短匹配与 8 字节长匹配的双表查找机制，并在 TTZip 中设计零分配工作区内存布局。
- R002 [SUBAGENT:research] 《ARM64 NEON 128 位向量化展开》：在 `ZSTD_count()` 中设计 Tier 0 64-bit GPR SWAR + Tier 1 128-bit NEON 向量循环结构，消除跨寄存器域延迟。
- R003 [SUBAGENT:research] 《ARMv8 硬件 CRC32 哈希加速》：在 `ZSTD_hash4` 与 `ZSTD_hash8` 中引入 `__crc32w` / `__crc32d` 单周期硬件指令与 Salt 折叠优化。

---

## 5. Phase 1: Design & Contracts Index

- **Data Model**: [`specs/061-zstd-match-counting-acceleration/data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/data-model.md)
- **Contracts**:
  - [`contracts/double_fast_table.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/contracts/double_fast_table.json)
  - [`contracts/match_candidate.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/contracts/match_candidate.json)
  - [`contracts/upstream_patch.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/contracts/upstream_patch.json)
- **Quickstart**: [`specs/061-zstd-match-counting-acceleration/quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/061-zstd-match-counting-acceleration/quickstart.md)

---

## 6. Component Change List

### Upstream Worktrees (Track 1)
- `Vendor/worktrees/zstd/pr1-arm64-neon-count/lib/compress/zstd_compress_internal.h`: Integrate Tier 0 GPR + Tier 1 NEON in `ZSTD_count()`.
- `Vendor/worktrees/zstd/pr2-arm64-crc32-hash/lib/compress/zstd_compress_internal.h`: Integrate `__crc32w`/`__crc32d` in `ZSTD_hash4`/`ZSTD_hash8`.
- `Vendor/worktrees/zstd/pr3-arm64-neon-rle-detect/lib/compress/zstd_compress.c`: Integrate 64-byte NEON vector unrolling in `ZSTD_isRLE()`.

### TTZip Internal Engine (Track 2)
- `Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`: Add Double-Fast table definitions & workspace initialization prototypes.
- `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`: Implement Double-Fast lookahead search & preallocated memory buffer support.
- `Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h`: Inline hybrid SWAR + NEON match length helpers.
