# Implementation Plan: SIMD Canonical Huffman Coding & Multi-Symbol Emission

**Feature**: `122-simd-canonical-huffman-multi-symbol-emission`
**Created**: 2026-08-19
**Status**: Ready for Implementation

---

## Technical Context

In single-core Deflate compression, serialization of LZ77 tokens into RFC 1951 Huffman bitstreams represents $> 40\%$ of CPU cycles on small files. Each match token previously required 4 separate function calls to emit length and distance fields. By:
1. Packing all 4 match token components ($< 48$ bits) into a single 64-bit word and emitting with a single branchless write.
2. Pairing adjacent literal tokens into 64-bit dual-literal writes.
3. Adding a precomputed static Huffman fast-path for small files $< 4\text{KB}$.

Single-core Deflate serialization throughput increases by $> 3\text{x}$, boosting mixed-workspace small-file throughput from 459 MB/s to $\ge 800\text{ MB/s}$.

---

## Constitution Check

- [x] **Hot-Path Zero-Cost Abstraction**: 0 heap allocations inside the Huffman bitstream loop.
- [x] **RFC 1951 Compliance**: LSB-first bit ordering and 15-bit codeword length limits preserved 100%.
- [x] **Zero Warnings**: Compiles under `-Wall -Wextra -Werror`.

---

## Phase 0: Research Index

- - R001 [SUBAGENT:research] 《64-Bit Multi-Symbol Bitstream Word Packing Architecture》：Packed match token serialization into single `uint64_t` ($< 48$ bits).
- - R002 [SUBAGENT:research] 《Small-File Static Huffman Threshold & Fast Dynamic Tree Bypass》：Bypass dynamic tree generation on files $< 4\text{KB}$ in favor of precomputed static tables.

See [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/122-simd-canonical-huffman-multi-symbol-emission/research.md).

---

## Phase 1: Data Model & Contracts Index

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/122-simd-canonical-huffman-multi-symbol-emission/data-model.md)
- **Contracts**:
  - [`contracts/huffman_encoder_request.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/122-simd-canonical-huffman-multi-symbol-emission/contracts/huffman_encoder_request.schema.json)
  - [`contracts/huffman_encoder_response.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/122-simd-canonical-huffman-multi-symbol-emission/contracts/huffman_encoder_response.schema.json)
- **Quickstart**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/122-simd-canonical-huffman-multi-symbol-emission/quickstart.md)

---

## Proposed Changes

### Component 1: C Bridge Layer (`Sources/CTTZipBridge`)

#### [MODIFY] [`ttzip_deflate_bitstream.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_bitstream.h)
- Add `ttzip_bs_write_bits64` supporting up to 56 bits in a single store.

#### [MODIFY] [`ttzip_deflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c)
- Implement packed 4-field match token serialization.
- Implement dual-literal pairing loop.
- Implement sub-4KB static Huffman bypass.

---

### Component 2: Swift Core & Test Suite (`Tests/TTZipTests`)

#### [NEW] [`HuffmanBitstreamOptimizationTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/HuffmanBitstreamOptimizationTests.swift)
- Exhaustive multi-symbol bitstream encoding oracle test.
- Small-file static vs dynamic throughput benchmark.

---

## Verification Plan

### Automated Tests
```bash
swift test --filter HuffmanBitstreamOptimizationTests
TTZIP_RUN_BENCHMARKS=1 swift test --filter CompoundMixedCorpusBenchmarkPkTests/testCompoundMixedCorpusSingleCoreVsLibdeflate1v1
swift test --filter XCTestPerformanceMeasureTests
```
