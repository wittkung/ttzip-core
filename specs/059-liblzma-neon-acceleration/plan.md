# Implementation Plan: Liblzma (XZ Utils) ARM NEON Match Finder Acceleration & Upstream Baseline Integration

**Branch**: `059-liblzma-neon-acceleration` | **Date**: 2026-08-17 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/059-liblzma-neon-acceleration/spec.md)

**Input**: Feature specification from `specs/059-liblzma-neon-acceleration/spec.md`

## Summary

This feature integrates hardware-accelerated ARM NEON and ACLE CRC32 primitives into the LZMA / LZMA2 match finding pipelines. By adopting a two-tier hybrid architecture (Tier 0 64-bit GPR SWAR + Tier 1 128-bit NEON unrolling) and ARMv8 ACLE hardware CRC32 instruction (`__crc32w`), the system eliminates cross-domain register latency on short-match rejections and doubles throughput on extended dictionary matches. The change enhances both TTZip's native 7Z/LZMA2 fast paths and the foundational `Vendor/liblzma.a` static library, while preparing atomic, 0BSD-compliant patches for upstream submission to `tukaani-project/xz`.

---

## Technical Context

**Language/Version**: C11 / POSIX APIs + Swift 6.0 (`swift-tools-version: 6.0`).
**Primary Dependencies**: `Vendor/xz-upstream` (liblzma), Apple Clang with `<arm_neon.h>` and `<arm_acle.h>`, `Vendor/TTZipVendor.xcframework`.
**Storage**: In-memory ring/sliding dictionary buffers (`ttzip_hc4_t` / `lzma_mf`), zero-allocation hot paths.
**Testing**: `swift test`, `swift test --filter HybridMatchFinderMicroTests`, `swift test --filter XCTestPerformanceMeasureTests`.
**Target Platform**: macOS 14.0+ (Apple Silicon ARM64 prioritized, Intel x86_64 compatible).
**Project Type**: In-process native static C library + Swift Core Engine.
**Performance Goals**: Match length comparison >= 4.5 GB/s; 7Z Level 1 >= 3,200 MB/s; LZMA2 Level 5 >= 480 MB/s; zero performance regression across all 16 formats.
**Constraints**: 100% MAS sandbox compliance (`-DMAS_BUILD`); zero bare object allocations in hot loops; zero undefined behavior under ASan/UBSan.
**Scale/Scope**: Core match finding, static vendor library build pipeline, upstream patch generation.

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Invariant | Status | Verification & Compliance Strategy |
| :--- | :--- | :--- |
| **Hot-Path Zero Allocation** | ✅ PASS | `ttzip_hybrid_match_len_neon` and `ttzip_hc4_hash_calc` perform 0 heap allocations; all operations execute in CPU registers. |
| **Zero Shared Locks in Concurrency** | ✅ PASS | Match finding operates exclusively on thread-local block chunks without locks or semaphores. |
| **Fast-Path Bypass Preservation** | ✅ PASS | Level 1 2MB zero-chunk bypass and HC4 NEON fast paths are fully preserved and explicitly tested. |
| **Throughput Floors Enforcement** | ✅ PASS | Hard performance floors (7Z L1 >= 3200 MB/s, 7Z Extract >= 6600 MB/s) verified via `XCTestPerformanceMeasureTests`. |
| **Stream-First & Zero-Memory** | ✅ PASS | Memory buffers are bounded by block sizes; no unbounded whole-file allocations. |
| **Bounds-First & Magic Verification** | ✅ PASS | Strict array bounds clamping (`len + 16 <= limit` and `my_min`), safe unaligned memory loads (`memcpy` / `vld1q_u8`). |
| **Oracle-First Testing** | ✅ PASS | Decompressed outputs verified against reference SHA-256 / CRC32 golden oracles. |

---

## Phase 0: Research & Investigation

- R001 [SUBAGENT:research] 《liblzma memcmplen NEON 向量化比对适配》：深入调研 `Vendor/xz-upstream/src/liblzma/common/memcmplen.h` 与 `lz_encoder_mf.c`，确定如何在 aarch64 上以 C99/C11 兼容方式引入 128-bit NEON 向量展开（`vld1q_u8` + `veorq_u8`），并与现有 64-bit SWAR 逻辑无缝结合。
  - *Status*: Completed in `specs/059-liblzma-neon-acceleration/research.md`.
- R002 [SUBAGENT:research] 《ARMv8 ACLE 硬件 CRC32 哈希加速集成》：调研 `Vendor/xz-upstream/src/liblzma/lz/lz_encoder_hash.h` 中 `hash_4_calc()` 的哈希机制，确定如何使用 `__ARM_FEATURE_CRC32` 和 `__crc32w` 替代软件查表，并确保非 CRC32 架构与跨平台编译兼容性。
  - *Status*: Completed in `specs/059-liblzma-neon-acceleration/research.md`.
- R003 [SUBAGENT:research] 《Vendor/liblzma.a 编译系统与静态库重新打包》：调研 `Vendor/xz-upstream/` 的 CMakeLists.txt / Makefile.am 构建系统，确定如何将编译产物打包至 `Vendor/libTTZipVendor.a` 并保持 Mac App Store 沙盒与 Xcode 静态链接兼容。
  - *Status*: Completed in `specs/059-liblzma-neon-acceleration/research.md`.

---

## Phase 1: Design Artifacts & Contracts

- **Data Model**: `specs/059-liblzma-neon-acceleration/data-model.md`
- **Contracts**:
  - `contracts/lzma_encoder_match_finder_contract.json` [SUBAGENT:research]
  - `contracts/liblzma_vendor_build_contract.json` [SUBAGENT:research]
  - `contracts/lzma2_pipeline_benchmark_contract.json` [SUBAGENT:research]
- **Quickstart Guide**: `specs/059-liblzma-neon-acceleration/quickstart.md`

---

## Planned Code Changes by Component

### Component 1: Core C Bridge & Native Match Finder (`Sources/CTTZipBridge/`)
- [`ttzip_lzma_hc4_neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c): Refine and verify hybrid SWAR/NEON match length evaluation and hardware CRC32 hashing.
- [`include/ttzip_lzma_hc4_neon.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h): Ensure clean C symbol exports and function signatures.

### Component 2: Upstream Source Tree & Static Vendor Libraries (`Vendor/`)
- [`Vendor/xz-upstream/src/liblzma/common/memcmplen.h`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/xz-upstream/src/liblzma/common/memcmplen.h): Implement ARM NEON 128-bit match length comparison with preprocessor architecture guards.
- [`Vendor/xz-upstream/src/liblzma/lz/lz_encoder_hash.h`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/xz-upstream/src/liblzma/lz/lz_encoder_hash.h): Implement ARMv8 ACLE hardware CRC32 hash calculation.
- [`scripts/build_liblzma.sh`](file:///Users/kevintung/Documents/dev/TTZip/scripts/build_liblzma.sh): Automated CMake build script for Universal 2 `liblzma.a` and `libTTZipVendor.a` re-packaging.

### Component 3: Unit Tests & Micro-Benchmarks (`Tests/TTZipTests/`)
- [`Tests/TTZipTests/HybridMatchFinderMicroTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/HybridMatchFinderMicroTests.swift): Add comprehensive boundary, throughput, and parity verification test suites.
