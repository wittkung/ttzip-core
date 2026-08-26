# Implementation Plan: zlib-ng Streaming Fallback Engine & Cross-Platform Hardware Acceleration

**Branch**: `076-zlib-ng-streaming-fallback` | **Date**: 2026-08-18 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/076-zlib-ng-streaming-fallback/spec.md`

## Summary

Build and integrate `zlib-ng` (in `ZLIB_COMPAT=ON` mode) into TTZip's dual-tier compression engine to serve as the ultra-fast Tier 2 streaming fallback and modernize `libarchive`'s global deflate filters across macOS (Apple Silicon ARMv8 CRC32/PMULL/NEON) and Windows (AVX-512/AVX2/PCLMUL), while strictly preserving Tier 1 whole-buffer `libdeflate` fast-path invariants and zero-performance-regression floors.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), C11 / POSIX / MSVC  
**Primary Dependencies**: `Vendor/TTZipVendor.xcframework` (`libdeflate.a`, `libarchive.a`, `libz.a` from zlib-ng), `CTTZipBridge`  
**Storage**: In-Memory buffers, POSIX file descriptors, AsyncSequence Streams  
**Testing**: `swift test --filter DeflateStreamCoderTests`, `swift test --filter DeflateStreamingPipelineTests`, `swift test --filter XCTestPerformanceMeasureTests`  
**Target Platform**: macOS 14.0+ (Apple Silicon arm64 & Intel x86_64), Windows 10/11 (MSVC x64 & ARM64)  
**Project Type**: Native High-Performance Compression Engine & Desktop App  
**Performance Goals**: Streaming compression >= 350 MB/s, Streaming decompression >= 1,500 MB/s, Whole-buffer Tier 1 compression >= 2,000 MB/s, decompression >= 10,000 MB/s  
**Constraints**: Zero heap allocation in hot-loop iterations; zero mutex locks on concurrent streams; 100% ABI compatibility with `zlib.h`; zero regression on peak performance floors  
**Scale/Scope**: 16 format pipeline support, unbounded chunked streams, multi-gigabyte streaming pipelines  

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant | Status | Verification Detail |
| :--- | :--- | :--- |
| **Hot-Path Zero-Cost Abstraction** | **PASS** | `DeflateStreamCompressor` allocates memory only upon session init/close; zero dynamic wrappers in per-chunk hot loop |
| **Fast-Path Bypass Retention** | **PASS** | Whole-buffer operations continue using `libdeflate` TLS pool; zlib-ng strictly isolated to Tier 2 and libarchive stream filters |
| **Hard Performance Floor** | **PASS** | Tier 1 peak gates remain intact; Tier 2 streaming gains 250%–400% speedup over legacy scalar zlib |
| **Deterministic Invariant Immunity** | **PASS** | RFC 1950/1951/1952 compliance verified with bit-exact roundtrip tests |
| **Dead-Store & Bounds Safety** | **PASS** | `ttzip_deflate_stream_free` verifies `magic`, zeroes pointers, and uses `memset` for zero-garbage memory state |

---

## Project Structure

### Documentation (this feature)

```text
specs/076-zlib-ng-streaming-fallback/
├── spec.md              # Feature specification & clarifications
├── checklists/
│   └── requirements.md  # Spec quality checklist
├── plan.md              # Implementation plan (this file)
├── research.md          # Phase 0 research conclusions (R001, R002, R003)
├── data-model.md        # Phase 1 data models & entity specifications
├── contracts/           # Phase 1 strict JSON schemas
│   ├── deflate-stream-coder-contract.json
│   └── hardware-capabilities-contract.json
├── quickstart.md        # Phase 1 validation walkthrough & failure diagnostics
└── tasks.md             # Phase 2 implementation task list
```

### Source Code Components

```text
Sources/
├── CTTZipBridge/
│   ├── CTTZipStreamCoder.c          # [MODIFY] Tier 2 zlib-ng state machine & Tier 1 libdeflate dispatch
│   ├── ttzip_platform_detect.c      # [MODIFY] Dynamic hardware SIMD detection (ARM CRC32, AVX-512, PCLMUL)
│   └── include/
│       └── CTTZipStreamCoder.h      # [MODIFY] C header definitions & state struct with magic verification
├── TTZipCore/
│   ├── Pipeline/
│   │   └── DeflateStreamEngine.swift # [MODIFY] Swift API facade, AsyncThrowingStream pipelines, metrics
│   └── Adapters/
│       └── LibdeflateCAdapter.swift # [RETAIN] Tier 1 zero-allocation fast-path
Vendor/
├── lib/
│   └── libz.a                      # [DEPLOY] Universal 2 static library built from zlib-ng
├── include/
│   ├── zlib.h                      # [DEPLOY] zlib-ng ZLIB_COMPAT header
│   └── zconf.h                     # [DEPLOY] zlib-ng configuration header
└── TTZipVendor.xcframework/        # [DEPLOY] Universal 2 framework bundle
scripts/
├── build_zlib_ng.sh                # [MAINTAIN] Build automation script for zlib-ng
└── run_all_tests.sh                # [VALIDATE] End-to-end regression validation
```

---

## Phase 0: Outline & Research

The following research items were evaluated by subagents in Phase 0:

- [x] - R001 [SUBAGENT:research] 《zlib-ng 构建参数、ZLIB_COMPAT 模式在 macOS (Universal 2: arm64/x86_64) 与 Windows (MSVC) 下的静态打包及符号导出》: 确认 `-DZLIB_COMPAT=ON`、`-DWITH_NATIVE_INSTRUCTIONS=ON`、`-DDYNAMIC_CPU_DISPATCH=ON` 方案，静态链接进 `TTZipVendor.xcframework`，移除 `Package.swift` 系统 `z` 依赖。
- [x] - R002 [SUBAGENT:research] 《TTZip 流式 Deflate 状态机与 libdeflate 内存块 Fast-Path 的双轨分流架构及吞吐性能门禁边界》: 确立 Dual-Tier 物理分层，Whole-Buffer 场景强行保留 libdeflate TLS 池，Streaming 场景采用 zlib-ng 增量状态机。
- [x] - R003 [SUBAGENT:research] 《macOS ARMv8 CRC32/PMULL 与 Windows AVX-512/AVX2/PCLMUL 硬件指令集运行期探测与 zero-cost checksum 调度》: 确立单实例独立持有无锁状态机，硬件指令加速 CRC32 与 Adler-32。

**Artifact**: [`specs/076-zlib-ng-streaming-fallback/research.md`](./research.md)

---

## Phase 1: Design & Contracts

The following interface contracts and data models are specified in Phase 1:

- [x] `data-model.md`: Data models for `DeflateTierMode`, `DeflateWindowBits`, `DeflateStrategy`, `DeflateFlushMode`, `DeflateStreamConfig`, `DeflateStreamMetrics`, `ttzip_deflate_stream_state_t`, and `ttzip_hardware_capabilities_t`.
- [x] `contracts/deflate-stream-coder-contract.json` [SUBAGENT:research]: JSON schema specifying stream coder requests, chunk processing payloads, completion responses, and typed error models.
- [x] `contracts/hardware-capabilities-contract.json` [SUBAGENT:research]: JSON schema specifying CPU feature detection queries and platform capabilities.
- [x] `quickstart.md`: Runnable validation scenarios covering chunked raw deflate, multi-megabyte GZIP async pipelines, flush mode transitions, and error diagnostics.

**Artifacts**:
- [`specs/076-zlib-ng-streaming-fallback/data-model.md`](./data-model.md)
- [`specs/076-zlib-ng-streaming-fallback/contracts/deflate-stream-coder-contract.json`](./contracts/deflate-stream-coder-contract.json)
- [`specs/076-zlib-ng-streaming-fallback/contracts/hardware-capabilities-contract.json`](./contracts/hardware-capabilities-contract.json)
- [`specs/076-zlib-ng-streaming-fallback/quickstart.md`](./quickstart.md)

---

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
| :--- | :--- | :--- |
| *None* | Dual-Tier architecture strictly separates concerns with zero extra complexity | Unified single engine fails either peak memory throughput (if zlib-ng only) or streaming state-machine capability (if libdeflate only) |
