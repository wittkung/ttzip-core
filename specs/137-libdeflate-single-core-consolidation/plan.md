# Implementation Plan: Consolidate Single-Core Deflate Engine on libdeflate and Modernize Architecture

**Feature Branch**: `137-libdeflate-single-core-consolidation`

**Date**: 2026-08-20

## Technical Context

- **Target Platforms**: macOS 14.0+ (Apple Silicon ARM64 & Intel x86_64).
- **Core Codec**: `libdeflate` 1.20+ (Layer 0 pristine upstream, static C link via `CTTZipBridge`).
- **Streaming Pipeline**: `zlib-ng` 2.2.0+ (Dynamic SIMD dispatch for stateful sliding windows).
- **Swift Layer**: Swift 6 Strict Concurrency (`Sendable`, `@MainActor`, zero ARC overhead in pointer fast paths).
- **Memory Safety Invariant**: Page-aligned flyweights for $\le 64\text{ KB}$ buffers (`posix_memalign`), `bytesNoCopy` with custom deallocators for $> 64\text{ KB}$ payloads, zero `Data(count:)` zero-fill faults.

## Constitution Check

- **Zero-Cost Abstraction on Hot Paths**: Verified. `ttzip_libdeflate_compress` / `decompress` execute with thread-local compressor reuse and direct raw pointer pass-through.
- **Prohibited Anti-Patterns**: Zero shared locks, zero mutexes in parallel compression loops, zero dynamic object tree allocations in inner loops.
- **Four Systemic Invariants**:
  - *Stream-First*: Max resident memory per task $\le 64\text{MB} \sim 128\text{MB}$.
  - *Invariant-First*: Hardware-accelerated overflow bounds checking (`__builtin_add_overflow`).
  - *Bounds-First*: Zero UAF, memory ownership strictly managed through Swift `Data` deallocators.
  - *Oracle-First*: Differential oracle testing against `/usr/bin/unzip` and standard `zlib`.

## Phase 0: Research Items

- R001 [SUBAGENT:research] 《Thread-Local Pooling Lifecycle》: C11 `_Thread_local` compressor caching across GCD worker threads.
- R002 [SUBAGENT:research] 《Swift 6 Zero-Copy Adapter Performance》: `LibdeflateCAdapter` flyweight and `bytesNoCopy` memory strategy.
- R003 [SUBAGENT:research] 《Multi-Tier Deflate Architecture & Boundaries》: Tier 1 (libdeflate whole-buffer/chunk) vs Tier 2 (zlib-ng streaming).

Detailed findings are recorded in [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/137-libdeflate-single-core-consolidation/research.md).

## Phase 1: Design Artifacts

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/137-libdeflate-single-core-consolidation/data-model.md) defines codec request/response payloads, compression level mappings, buffer memory lifecycles, and flyweight structures.
- **Contracts**:
  - [`contracts/libdeflate_codec_api.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/137-libdeflate-single-core-consolidation/contracts/libdeflate_codec_api.json)
  - [`contracts/deflate_stream_api.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/137-libdeflate-single-core-consolidation/contracts/deflate_stream_api.json)
- **Quickstart & Validation**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/137-libdeflate-single-core-consolidation/quickstart.md) provides executable benchmark and regression verification scripts with diagnostic guidelines.

## Component Changes Breakdown

### 1. C Bridge Layer (`Sources/CTTZipBridge/`)
- [`CTTZipStreamCoder.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipStreamCoder.c): Ensure `ttzip_libdeflate_compress` and `ttzip_libdeflate_decompress` maintain strict thread-local pooling and clamping for levels 0–12.
- [`include/CTTZipStreamCoder.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipStreamCoder.h): Header declarations for C bridge ABI.

### 2. Core Engine Layer (`Sources/TTZipCore/`)
- [`Adapters/LibdeflateCAdapter.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift): Canonical adapter providing thread-safe, memory-flyweight-backed Swift Deflate APIs.
- [`Zip/ZipBlockParallelCompressor.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipBlockParallelCompressor.swift): Enforce direct routing to `ttzip_libdeflate_compress` across 512KB chunk slices.
- [`Zip/ZipBlockParallelDecompressor.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipBlockParallelDecompressor.swift): Enforce direct routing to `ttzip_libdeflate_decompress`.
- [`Pipeline/DeflateStreamEngine.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift): Maintain Tier 2 streaming engine for stateful step-by-step compression.

### 3. Architecture Documentation
- [`ARCHITECTURE.md`](file:///Users/kevintung/Documents/dev/TTZip/ARCHITECTURE.md): Update Section 2.5 to explicitly document the dual-tier strategy, libdeflate single-core supremacy, and the consolidation rationale.
