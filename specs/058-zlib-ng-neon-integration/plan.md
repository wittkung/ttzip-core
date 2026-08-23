# Implementation Plan: zlib-ng NEON LCP Acceleration & Dual-Platform Integration

**Branch**: `058-zlib-ng-neon-integration` | **Date**: 2026-08-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/058-zlib-ng-neon-integration/spec.md`

## Summary

This plan outlines the architecture and execution strategy for integrating `zlib-ng` into TTZip's dual-platform core, optimizing the ARM64 NEON/SWAR match-length finder to eliminate Apple Silicon cross-register domain latency, replacing legacy scalar zlib in streaming pipelines on macOS and Windows, and establishing the upstream patch for `arch/arm/compare256_neon.c`.

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.
**Primary Dependencies**: `libdeflate` (in-memory block engine), `zlib-ng` (`ZLIB_COMPAT=ON` streaming engine), `libarchive` (archive parser).
**Storage**: In-memory page buffers and streaming UNIX file descriptors / `FILE*`.
**Testing**: `swift test` (525+ tests), `XCTestPerformanceMeasureTests`, `AllFormatsPkSuiteTests`.
**Target Platform**: macOS 14.0+ (Apple Silicon NEON + Intel x86_64), Windows 10+ (x86_64 AVX2/AVX-512 + ARM64 NEON).
**Project Type**: Native High-Performance Compression Framework & Multi-Platform Desktop Engine.
**Performance Goals**:
- Deflate Streaming Level 1: >= 350 MB/s (Apple Silicon) / >= 380 MB/s (Windows x86_64 AVX2).
- Deflate In-Memory Block Level 1: Maintain >= 1,500 MB/s (libdeflate fast-path).
- Short Match (<8 bytes) Evaluation Latency: <= 3 CPU cycles (SWAR GPR bypass).
**Constraints**:
- Zero heap allocation inside inner compression/decompression loops.
- 100% RFC 1951 Deflate and RFC 1952 GZIP standard compatibility.
- Zero symbol conflicts with system libraries or `libdeflate`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant | Status | Verification & Compliance |
| :--- | :---: | :--- |
| **Stream-First** | ✅ PASS | Memory bounded <= 64MB; stream pipeline uses sliding window + unallocated page buffers. |
| **Invariant-First** | ✅ PASS | Deflate bitstream validation + unaligned memory bounds clamping. |
| **Bounds-First** | ✅ PASS | Context structures initialized with magic headers; zero-fill sanitization on destruction. |
| **Oracle-First** | ✅ PASS | Standard corpora (Silesia, Enwik) & dual-oracle differential verification against `/usr/bin/gzip`. |
| **Zero-Cost Abstraction**| ✅ PASS | Hot-path uses GPR SWAR + direct NEON intrinsics; zero dynamic object trees or locks. |
| **Subsystem Freeze** | ✅ PASS | No modification to frozen ZIP files; extensions routed via `CTTZipStreamCoder.c` & `Vendor/`. |

---

## Project Structure

### Documentation (this feature)

```text
specs/058-zlib-ng-neon-integration/
├── spec.md              # Feature specification
├── plan.md              # Implementation plan (this file)
├── research.md          # Phase 0 research results
├── data-model.md        # Phase 1 data model & state entities
├── quickstart.md        # Phase 1 validation guide
├── contracts/           # Phase 1 interface contracts & schemas
│   ├── deflate_stream_coder_contract.json
│   └── hybrid_match_finder_contract.json
└── tasks.md             # Phase 2 implementation task breakdown
```

### Source Code Impact Matrix

```text
Sources/
├── CTTZipBridge/
│   ├── include/
│   │   ├── CTTZipStreamCoder.h        # [MODIFY] Expose zlib-ng dual-tier streaming primitives
│   │   └── ttzip_lzma_hc4_neon.h      # [MODIFY] Export hybrid SWAR/NEON match finder interface
│   ├── CTTZipStreamCoder.c            # [MODIFY] Implement tier-2 zlib-ng stream dispatcher
│   └── ttzip_lzma_hc4_neon.c          # [MODIFY] Upgrade ttzip_match_len_neon with hybrid SWAR/NEON
Vendor/
├── zlib-ng/                           # [NEW] Upstream zlib-ng source submodule (ZLIB_COMPAT)
└── libTTZipVendor.a                   # [MODIFY] Include compiled zlib-ng objects for macOS Universal 2
Package.swift                          # [MODIFY] Switch from system libz to static zlib-ng
CMakeLists.txt                         # [MODIFY] Add zlib-ng with DYNAMIC_CPU_DISPATCH for Windows/MSVC
```

---

## Phase 0: Research Outline & Dispatched Investigations

- R001 [SUBAGENT:research] 《zlib-ng 与 libdeflate 架构边界与流式集成模式》：调研 Whole-Buffer 与 Streaming 模型吞吐差异，确定双轨分层架构方案。*(Completed in `research.md` §1)*
- R002 [SUBAGENT:research] 《ARM64 NEON 与 SWAR 混合匹配查找微架构开销与集成方案》：调研 Apple Silicon 跨域停顿，设计前 8 字节 64-bit SWAR + 128-bit NEON 展开混合查找器。*(Completed in `research.md` §2)*
- R003 [SUBAGENT:research] 《Windows x86_64/ARM64 跨平台 zlib-ng AVX-512/NEON 依赖替换与 CTTZipBridge 绑定》：调研 MSVC 动态 CPU 分发、AVX-512 指令集挂载与 `zlib1.dll` 彻底替换方案。*(Completed in `research.md` §3)*

## Phase 1: Design & Contracts

- Data Model: [`data-model.md`](data-model.md) defining streaming contexts, matcher configuration, and hardware capabilities.
- Contracts [SUBAGENT:research]:
  - [`contracts/deflate_stream_coder_contract.json`](contracts/deflate_stream_coder_contract.json): Dual-tier stream coder configuration and lifecycle schema.
  - [`contracts/hybrid_match_finder_contract.json`](contracts/hybrid_match_finder_contract.json): Hybrid SWAR/NEON match finder parameter and output schema.
- Quickstart Guide: [`quickstart.md`](quickstart.md) defining end-to-end verification, throughput benchmarking, and diagnostic procedures.
