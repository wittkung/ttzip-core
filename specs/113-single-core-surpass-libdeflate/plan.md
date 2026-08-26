# Implementation Plan: Single-Core DEFLATE Engine Surpassing libdeflate

**Branch**: `113-single-core-surpass-libdeflate` | **Date**: 2026-08-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/113-single-core-surpass-libdeflate/spec.md`

## Summary

This plan establishes the architecture and execution blueprint for TTZip's ultra-high-throughput single-core DEFLATE compression and decompression engines, surpassing libdeflate single-core performance on Apple Silicon (and modern CPUs). The design incorporates:
1. **4-Way NEON Parallel Hash Probing + 2-Tier SWAR Match Length Resolution** for compression match finding.
2. **Dual-Token & Quad-Token Tree-Parallel Bitstream Packing** combined with an **8-Archetype Pre-Compiled Codebook Cluster** for zero-latency dynamic Huffman header emission.
3. **12-bit Dual-Symbol Direct Huffman Decoding LUT** and **ARM NEON In-Register Small-Distance Replication ($D < 16$)** for decompression acceleration.
4. **Zero-Heap Hot-Path Execution & Ecosystem Oracle Round-Trip Verification**.

---

## Technical Context

- **Language/Version**: C11 / POSIX APIs for core compute pipelines, Swift 6.0 (`swift-tools-version: 6.0`) for high-level wrappers and benchmarks.
- **Primary Dependencies**: In-process `CTTZipBridge` static bindings, Apple Silicon ARM NEON SIMD intrinsics (`arm_neon.h`), hardware CRC32/PMULL extensions.
- **Storage**: In-memory streaming and bounded block buffers.
- **Testing**: `swift test`, XCTest microbenchmarks, differential PK test suites (`SingleCoreDeflatePkTests`, `SingleCoreDecompressPkTests`, `SingleCoreDeflateOracleTests`).
- **Target Platform**: macOS 14.0+ (Apple Silicon ARMv8.2-A+ NEON prioritized, Intel x86_64 compatible).
- **Project Type**: Native High-Performance Compression & Archive Engine.
- **Performance Goals**:
  - Single-Core Level 1 Compression: $\ge 2,400\text{ MB/s}$ ($\ge +10.0\%$ over libdeflate L1).
  - Single-Core Level 6 Compression: $\ge 1,400\text{ MB/s}$ ($\ge +5.0\%$ over libdeflate L6).
  - Single-Core Decompression: $\ge 11,000\text{ MB/s}$ ($\ge +10.0\%$ over libdeflate).
- **Constraints**: Zero intermediate heap allocations (`malloc`/`Data(count:)`) on hot paths, 100% RFC 1951 compatibility.
- **Scale/Scope**: Payloads from micro-chunks (< 4 KB) to multi-gigabyte continuous streams.

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Zero-Cost Abstraction on Hot Paths**: No dynamic object trees, no intermediate allocations in compression/decompression inner loops.
- [x] **No Shared Locks on Fast Paths**: Zero `NSLock`/`pthread_mutex` on compute threads.
- [x] **Zero Kernel Zeroing**: Use raw pointers or pre-allocated scratch memory; no `Data(count:)`.
- [x] **Stream-First Micro-Buffering**: Memory resident set strictly bounded.
- [x] **Oracle-First Differential Verification**: 100% bit-exact validation against `/usr/bin/unzip` and `/usr/bin/gzip`.
- [x] **Logging Discipline**: All diagnostics routed through structured logging; zero bare `printf` in hot paths.

---

## Project Structure

### Documentation & Design Artifacts (this feature)

```text
specs/113-single-core-surpass-libdeflate/
├── spec.md              # Feature specification
├── plan.md              # Implementation plan (this document)
├── research.md          # Phase 0 research findings
├── data-model.md        # Phase 1 data entities and structures
├── contracts/           # Phase 1 interface contracts
│   └── single-core-deflate-engine-contract.json
├── quickstart.md        # Phase 1 verification and testing guide
└── checklists/
    └── requirements.md  # Requirements quality matrix
```

### Source Code Targets

```text
Sources/CTTZipBridge/
├── include/
│   ├── CTTZipNEONMatchFinder.h      # 4-way NEON parallel hash & 2-tier SWAR match length
│   └── CTTZipDeflateEngine.h        # Unified C API for single-core deflate/inflate
├── native_deflate/
│   ├── ttzip_deflate_fast.c         # Level 1 fast greedy / 4-way NEON match finder
│   ├── ttzip_deflate_bitstream.h    # Dual/Quad-token tree-parallel bitstream packer
│   ├── ttzip_deflate_huffman.c      # 8-Archetype pre-compiled codebook cluster
│   └── ttzip_deflate_engine.c       # Main compression dispatcher
└── native_inflate/
    ├── ttzip_inflate_dual_lut.h     # 12-bit dual-symbol direct Huffman LUT
    ├── ttzip_inflate_neon_replicate.h # NEON in-register D<16 match replicator
    └── ttzip_inflate_engine.c       # Main decompression dispatcher

Tests/TTZipTests/
├── SingleCoreDeflatePkTests.swift   # Differential compression throughput PK vs libdeflate
├── SingleCoreDecompressPkTests.swift # Differential decompression throughput PK vs libdeflate
└── SingleCoreDeflateOracleTests.swift # Round-trip oracle & standard tool cross-verification
```

---

## Phase 0: Research Items

- [x] `- R001 [SUBAGENT:research] 《ARM64 NEON 4路并行哈希探测与SWAR匹配长度比较算法》`：Resolved in `research.md`.
- [x] `- R002 [SUBAGENT:research] 《双符号并行直接霍夫曼查表与NEON全距离向量化解压展开》`：Resolved in `research.md`.
- [x] `- R003 [SUBAGENT:research] 《双 Token 并行位流打包器与零时延自适应动态霍夫曼树生成》`：Resolved in `research.md`.

---

## Phase 1: Design & Contracts

- [x] `data-model.md`: Complete entity schemas for `CompressionContext`, `DecompressionContext`, `CompressionRequest/Result`, `DecompressionRequest/Result`, and `BenchmarkMetricRecord`.
- [x] `contracts/single-core-deflate-engine-contract.json`: [SUBAGENT:research] Complete JSON Schema with discriminated union for Requests, Responses, and Events.
- [x] `quickstart.md`: Runnable validation scenarios covering compression PK, decompression PK, and oracle verification.

---

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
| :--- | :--- | :--- |
| None | N/A (Fully compliant with constitution) | N/A |
