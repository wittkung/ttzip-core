# Implementation Plan: Streaming Fast-Path Decompressor & Dual-Symbol LUT

**Feature**: `124-streaming-fastpath-decompressor-dual-symbol-lut`
**Created**: 2026-08-19
**Status**: Ready for Implementation

---

## Technical Context

PR 5 focuses on ultra-high-throughput single-core Deflate decompression:
1. Construct 10-bit primary direct lookup table (`1024` entries) supporting dual-literal emit in 1 cycle.
2. Implement ARM NEON 128-bit match copy and 64-bit SWAR pattern duplication for overlapping short offsets.
3. Integrate into `ttzip_inflate_engine.c` and expose via `ttzip_deflate_decompress`.

---

## Constitution Check

- [x] **Hot-Path Zero-Cost Abstraction**: 0 dynamic allocations during decompression.
- [x] **Bitstream Integrity**: 100% RFC 1951 compliant decompression.
- [x] **Zero Warnings**: `-Wall -Wextra -Werror` clean.

---

## Phase 0: Research Index

- - R001 [SUBAGENT:research] 《10-Bit Dual-Symbol Decode Table Design》：4KB 1024-entry LUT for dual-literal single-cycle emit.
- - R002 [SUBAGENT:research] 《NEON 128-Bit & SWAR Fast Match Replication》：16-byte unaligned copy and 64-bit pattern broadcast.

See [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/124-streaming-fastpath-decompressor-dual-symbol-lut/research.md).

---

## Phase 1: Data Model & Contracts Index

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/124-streaming-fastpath-decompressor-dual-symbol-lut/data-model.md)
- **Contracts**:
  - [`contracts/decompressor_request.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/124-streaming-fastpath-decompressor-dual-symbol-lut/contracts/decompressor_request.schema.json)
  - [`contracts/decompressor_response.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/124-streaming-fastpath-decompressor-dual-symbol-lut/contracts/decompressor_response.schema.json)
- **Quickstart**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/124-streaming-fastpath-decompressor-dual-symbol-lut/quickstart.md)

---

## Proposed Changes

### Component 1: C Bridge Layer (`Sources/CTTZipBridge`)

#### [MODIFY] [`ttzip_inflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_inflate_engine.c)
- Implement 10-bit dual-symbol LUT decoding loop.
- Implement NEON 128-bit match copy and SWAR short offset broadcast.

---

### Component 2: Swift Core & Test Suite (`Tests/TTZipTests`)

#### [NEW] [`StreamingDecompressorDualSymbolLutTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/StreamingDecompressorDualSymbolLutTests.swift)
- Unit tests asserting multi-compressor stream equivalence and $\ge 8,000\text{ MB/s}$ throughput floor.

---

## Verification Plan

### Automated Tests
```bash
swift test --filter StreamingDecompressorDualSymbolLutTests
swift test --filter XCTestPerformanceMeasureTests
```
