# Implementation Plan: Blosc2 Exhaustive Architectural Conquest

**Feature Directory**: `specs/091-blosc2-exhaustive-architectural-conquest/`  
**Status**: APPROVED FOR IMPLEMENTATION  
**Priority**: P1  
**Created**: 2026-08-18  

---

## 1. Technical Context & Constitution Check

### Technical Context
- **Target Subsystem**: `CTTZipBridge` (C11 Core Bridge) + `TTZipCore` (Swift 6.0 Engine)
- **Target Architectures**: Apple Silicon (arm64, NEON SIMD) + Intel x86_64 fallback
- **Performance Floors**: Hot-path zero heap allocations, zero lock contention, lock-free plugin reads, hardware 128KB L1D cache alignment.

### Constitution Check
- [x] **Zero-Cost Abstraction**: Plugin registry and lazy slicing allocate 0 heap bytes on read paths.
- [x] **Fast-Path Preservation**: Built-in SIMD filters ($0\text{--}15$) bypass the plugin table entirely via direct inline branches.
- [x] **No Frozen File Modifications**: All additions are cleanly isolated in dedicated C bridge headers and implementation units (`CTTZipPluginRegistry`, `CTTZipLazySlice`, `CTTZipBitGroom`).

---

## 2. Phase 0: Grounded Research Index

- `R001` [SUBAGENT:research]: Dynamic Filter & Codec Plugin Registry (`blosc2_register_filter`, `blosc2_register_codec`, IDs $160\text{--}255$).
- `R002` [SUBAGENT:research]: Block-Level Lazy Chunk Decompression & Sub-Chunk Zero-Copy Slicing (`blosc2_schunk_get_slice_buffer`).
- `R003` [SUBAGENT:research]: Lossy Floating-Point Precision Quantization & Bit-Grooming Filters.
- `R004` [SUBAGENT:research]: Blosc2 Frame v2 Standard Serialization & Variable-Length Metalayers (`b2fr`).

---

## 3. Phase 1: Design & Contract Index

- `Data Model`: [`data-model.md`](data-model.md)
- `Contracts`:
  - [`contracts/plugin-registry-contract.json`](contracts/plugin-registry-contract.json)
  - [`contracts/lazy-slice-contract.json`](contracts/lazy-slice-contract.json)
  - [`contracts/bitgroom-filter-contract.json`](contracts/bitgroom-filter-contract.json)
  - [`contracts/frame-format-contract.json`](contracts/frame-format-contract.json)
- `Quickstart Validation Guide`: [`quickstart.md`](quickstart.md)

---

## 4. Component Change Manifest

### C Native Bridge Layer (`Sources/CTTZipBridge/`)
- `[NEW]` `include/CTTZipPluginRegistry.h`: C declarations for dynamic filter/codec registration.
- `[NEW]` `CTTZipPluginRegistry.c`: Atomic lock-free jump table and inline dispatch logic.
- `[NEW]` `include/CTTZipBitGroom.h`: Bit-Grooming and floating-point mantissa quantization declarations.
- `[NEW]` `CTTZipBitGroom.c`: NEON-accelerated Bit-Grooming and BitRound kernels.
- `[MODIFY]` `include/CTTZipSuperChunk.h`: Add `ttzip_schunk_get_slice_buffer` prototype and sub-chunk slicing structures.
- `[MODIFY]` `CTTZipSuperChunk.c`: Implement block-level range slicing with special-value bypass.
- `[MODIFY]` `include/CTTZipBridge.h`: Master include exports.

### Swift Platform Layer (`Sources/TTZipCore/`)
- `[MODIFY]` `Sources/TTZipCore/Platform/Blosc2FilterBridge.swift`: Expose Bit-Grooming and plugin registration to Swift.

### Test Suite (`Tests/TTZipTests/`)
- `[NEW]` `Tests/TTZipTests/Blosc2PluginRegistryTests.swift`
- `[NEW]` `Tests/TTZipTests/Blosc2LazySlicingTests.swift`
- `[NEW]` `Tests/TTZipTests/Blosc2BitGroomingTests.swift`
