# Implementation Plan: Adaptive Block Splitting & Fast Container Engine

**Feature Branch / Spec Directory**: `specs/103-adaptive-block-splitting-and-container-engine`  
**Created**: 2026-08-19  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Alignment

### Technical Context
TTZip features high-performance stream and block compression. Integrating `libdeflate`'s adaptive 300KB/5KB block splitting and zero-overhead GZIP/ZLIB container framing eliminates memory copying overhead and prevents micro-block tree bloat across streaming operations.

### Constitution Check
- [x] **Zero-Cost Abstraction**: Direct pointer writes with zero intermediate buffer copies.
- [x] **Performance Invariant Floor**: All 13 performance floors verified before and after code changes.
- [x] **Zip Engine Freeze Compliance**: No frozen files modified.

---

## 2. Phase 0: Grounded Research Items

- R001 [SUBAGENT:research] 《libdeflate 300KB/5KB Block Splitting Heuristic & 3-Way Block Type Optimization》 (Completed in `research.md`)
- R002 [SUBAGENT:research] 《Zero-Overhead GZIP & ZLIB Container Serialization with Single-Pass SIMD Checksum Fusion》 (Completed in `research.md`)

---

## 3. Phase 1: Data Model, Contracts & Design Artifacts

- Data Model: Defined in `data-model.md` (`AdaptiveBlockSplitStats`, `ContainerFramingResult`).
- Contracts:
  - `contracts/adaptive_block_split_decision.json` [SUBAGENT:research]
  - `contracts/container_framing_result.json` [SUBAGENT:research]
- Quickstart Validation Guide: Defined in `quickstart.md`.

---

## 4. Planned Changes by Component

### Component 1: C Bridge Block Splitter & Container Engine (`Sources/CTTZipBridge/`)
- `[NEW]` `include/ttzip_adaptive_block_split.h`: Export observation structures and block splitting decision functions.
- `[NEW]` `ttzip_adaptive_block_split.c`: Implement 10-class observation tracking, L1 drift detection, and 3-way bit cost evaluation.
- `[NEW]` `include/ttzip_container_fast.h`: Export `ttzip_gzip_compress_fast`, `ttzip_gzip_decompress_fast`, `ttzip_zlib_compress_fast`, `ttzip_zlib_decompress_fast`.
- `[NEW]` `ttzip_container_fast.c`: Implement zero-overhead GZIP/ZLIB container framing and fused SIMD checksums.
- `[MODIFIED]` `include/CTTZipBridge.h`: Include new headers.
- `[MODIFIED]` `CMakeLists.txt`: Register new source files.

### Component 2: Swift Core Adapters (`Sources/TTZipCore/`)
- `[NEW]` `Adapters/FastContainerEngine.swift`: Swift API for zero-copy GZIP and ZLIB packaging.
- `[NEW]` `Adapters/AdaptiveBlockSplitAdapter.swift`: Swift interface for entropy-aware block splitting.

### Component 3: Test Suites & Regression Verification (`Tests/TTZipTests/`)
- `[NEW]` `AdaptiveBlockSplitTests.swift`: Unit tests for observation drift, tail absorption, and 3-way arbitration.
- `[NEW]` `FastContainerStreamTests.swift`: Roundtrip verification and Apple `libcompression` consensus for GZIP/ZLIB.
