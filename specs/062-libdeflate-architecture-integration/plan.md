# Implementation Plan: Libdeflate Architecture Integration & Performance Exploitation

**Branch**: `062-libdeflate-architecture-integration` | **Date**: 2026-08-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/062-libdeflate-architecture-integration/spec.md`

## Summary

Deeply integrate `libdeflate` across all DEFLATE/GZIP/ZLIB paths in TTZip, eliminating legacy fallback mechanisms in `CTTZipStreamCoder.c`, adding 7Z Method ID `0x040108` direct native decoding, standardizing hardware CRC-32 and CRC-64 acceleration, and verifying zero performance regression across the entire benchmark matrix.

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.
**Primary Dependencies**: `libdeflate` (v1.22 in `Vendor/libTTZipVendor.a`), `CTTZipBridge`.
**Storage**: Bounded in-memory chunk staging + direct APFS file descriptor writes.
**Testing**: `swift test`, `swift test --filter XCTestPerformanceMeasureTests`.
**Target Platform**: macOS 14.0+ (Apple Silicon NEON/PMULL prioritized, Intel x86_64 compatible).
**Project Type**: In-process native C/Swift high-performance compression engine.
**Performance Goals**: Decompression $\ge 7500\text{ MB/s}$ (multi-core), CRC-32 $\ge 15000\text{ MB/s}$, constant streaming resident RAM $\le 64\text{MB}$.
**Constraints**: Zero heap allocation in hot loops, 100% standard RFC 1950/1951/1952 compliance.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] Stream-First: Chunked streaming bounds resident memory to $\le 64\text{MB}$ via 256KB/1MB staging buffers.
- [x] Bounds-First: Magic validation on all internal coder states, zeroing before deallocation.
- [x] Invariant-First: Thread-local compressor/decompressor caching without locking in parallel loops.
- [x] Oracle-First: Verified against Python/zlib and standard system `/usr/bin/unzip` & `/usr/bin/gzip`.

## Phase 0: Research Items

- R001 [SUBAGENT:research] 《Replace Legacy `zlib.h` Fallback in `CTTZipStreamCoder.c` with Chunked `libdeflate` Pipeline》: Refactor `ttzip_deflate_stream_state_t` internal state to use bounded `libdeflate` staging blocks.
- R002 [SUBAGENT:research] 《7Z Native Decoder DEFLATE Method ID (0x040108) Direct Routing》: Route raw RFC 1951 bitstreams in 7z blocks directly to `ttzip_libdeflate_decompress`.
- R003 [SUBAGENT:research] 《Verification of Apple Silicon NEON / PMULL Hardware Acceleration Paths》: Assert zero scalar table loops on hot paths for CRC-32 and CRC-64.

## Phase 1: Artifacts & Contracts

- Data Models: [`data-model.md`](data-model.md)
- Contracts:
  - [`contracts/stream_chunk_coder.json`](contracts/stream_chunk_coder.json) [SUBAGENT:research]
  - [`contracts/sevenzip_deflate_bridge.json`](contracts/sevenzip_deflate_bridge.json) [SUBAGENT:research]
- Quickstart & Validation: [`quickstart.md`](quickstart.md)

## Component Change Manifest

### CTTZipBridge
- `Sources/CTTZipBridge/CTTZipStreamCoder.c`: Refactor stream state, eliminate `zlib.h` dependency, fix CRC32/Adler32 assignment.
- `Sources/CTTZipBridge/ttzip_7z_block_decoder.c`: Add direct `ttzip_libdeflate_decompress` routing for 7z method `0x040108`.

### TTZipCore
- `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift`: Verify and harden stream engine against refactored C bridge state.
- `Tests/TTZipTests/LibdeflateAcceleratorTests.swift`: Add comprehensive roundtrip and streaming chunk validation.
