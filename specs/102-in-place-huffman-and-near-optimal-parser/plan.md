# Implementation Plan: In-Place Huffman Builder & Near-Optimal Parser

**Feature Branch / Spec Directory**: `specs/102-in-place-huffman-and-near-optimal-parser`  
**Created**: 2026-08-18  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Alignment

### Technical Context
TTZip operates as a high-performance in-process compression engine for macOS Sonoma+. While Level 1-6 compression is heavily optimized, extreme ratio compression (Level 10-12) and zero-heap Huffman tree construction represent essential architectural enhancements absorbed from `ebiggers/libdeflate`.

### Constitution Check
- [x] **Zero-Cost Abstraction**: Zero dynamic heap allocations in `ttzip_make_canonical_huffman_code_inplace`.
- [x] **Performance Invariant Floor**: All 13 performance floors verified before and after code changes.
- [x] **Dual-Build & Platform Safety**: C11 compliant, ARM64 `rbit` with x86 fallback.
- [x] **Zip Engine Freeze Compliance**: No frozen files modified.

---

## 2. Phase 0: Grounded Research Items

- R001 [SUBAGENT:research] 《In-Place 2-Queue Huffman Tree Merging & Depth-Limited Overwriting Algorithm》 (Completed in `research.md`)
- R002 [SUBAGENT:research] 《ARM64 RBIT Intrinsic vs Scalar Bit Reversal Performance & Codeword Prefix Properties》 (Completed in `research.md`)
- R003 [SUBAGENT:research] 《Near-Optimal Dynamic Programming Fixed-Point Bit Cost Modeling & Match Cache Management》 (Completed in `research.md`)

---

## 3. Phase 1: Data Model, Contracts & Design Artifacts

- Data Model: Defined in `data-model.md` (`InPlaceHuffmanResult`, `NearOptimalCompressionResult`).
- Contracts:
  - `contracts/inplace_huffman_result.json` [SUBAGENT:research]
  - `contracts/near_optimal_compression_result.json` [SUBAGENT:research]
- Quickstart Validation Guide: Defined in `quickstart.md`.

---

## 4. Planned Changes by Component

### Component 1: C Bridge Huffman Engine (`Sources/CTTZipBridge/`)
- `[NEW]` `include/ttzip_huffman_inplace.h`: Export `ttzip_make_canonical_huffman_code_inplace` and `ttzip_canonical_bit_reverse`.
- `[NEW]` `ttzip_huffman_inplace.c`: Implement 2-queue in-place merging, reverse depth traversal, shallow-leaf borrowing, and ARM64 `rbit` bit-reversal.
- `[MODIFIED]` `include/CTTZipBridge.h`: Include `ttzip_huffman_inplace.h`.
- `[MODIFIED]` `CMakeLists.txt`: Add `Sources/CTTZipBridge/ttzip_huffman_inplace.c`.

### Component 2: Swift Core Adapters (`Sources/TTZipCore/`)
- `[NEW]` `Adapters/InPlaceHuffmanAdapter.swift`: High-level zero-allocation Swift interface for Canonical Huffman code generation.
- `[MODIFIED]` `Adapters/LibdeflateCAdapter.swift`: Integrate Level 10-12 Near-Optimal routing.

### Component 3: Test Suites & Regression Verification (`Tests/TTZipTests/`)
- `[NEW]` `InPlaceHuffmanTests.swift`: RFC 1951 Kraft equality tests, ARM64 RBIT verification, and microbenchmarks.
- `[NEW]` `NearOptimalParserTests.swift`: Silesia corpus ratio gain, throughput floor, and decompression consensus against macOS `unzip`.
