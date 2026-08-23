# Implementation Plan: Blosc2 Advanced Architectures Comprehensive Integration

## Technical Context
- Language: Swift 6.0 + C11/POSIX
- Platform: macOS 14.0+ (Apple Silicon NEON + x86_64)
- Target Modules: `Sources/CTTZipBridge/`, `Sources/TTZipCore/`, `Tests/TTZipTests/`

---

## Constitution Check & Performance Invariants
- 128-byte cacheline memory alignment on Apple Silicon (`hw.cachelinesize = 128`).
- Zero dynamic heap allocation in filter and prefetch loops; 64KB stack double-buffering.
- Complete backward compatibility with standard PKWARE ZIP and POSIX TAR containers.

---

## Phase 0: Research ADRs
- `- R001 [SUBAGENT:research] 《NEON Float Precision Truncation》: IEEE-754 mantissa zeroing with half-bit rounding & Shuffle synergy`
- `- R002 [SUBAGENT:research] 《Double-Buffered Async Prefetch》: Slot-based ring buffer state machine with condition variables`
- `- R003 [SUBAGENT:research] 《VLMeta Trailer Engine》: Appendable self-compressed MessagePack metadata trailer`
- `- R004 [SUBAGENT:research] 《N-Dimensional Tensor Slicing》: Two-level hyper-cube coordinate translation & bounding box pruning`

---

## Phase 1: Contracts & Data Models
- Data Model: `specs/086-blosc2-advanced-architectures/data-model.md`
- Contracts:
  - `specs/086-blosc2-advanced-architectures/contracts/truncate-filter-contract.json`
  - `specs/086-blosc2-advanced-architectures/contracts/prefetch-pipeline-contract.json`
  - `specs/086-blosc2-advanced-architectures/contracts/vlmeta-trailer-contract.json`
  - `specs/086-blosc2-advanced-architectures/contracts/tensor-slicing-contract.json`

---

## Component Change List
1. `Sources/CTTZipBridge/include/CTTZipFilterPipeline.h` & `CTTZipFilterPipeline.c`: Float32/Float64 NEON truncation routines.
2. `Sources/CTTZipBridge/include/CTTZipPrefetchPipeline.h` & `CTTZipPrefetchPipeline.c`: Ring buffer state machine.
3. `Sources/CTTZipBridge/include/CTTZipVLMeta.h` & `CTTZipVLMeta.c`: Binary trailer serialization and EOF append engine.
4. `Sources/CTTZipBridge/include/CTTZipTensorSlicing.h` & `CTTZipTensorSlicing.c`: N-dimensional coordinate translation and slicing routines.
5. `Sources/CTTZipBridge/include/CTTZipCoreArchitecture.h`: Export all new subsystems.
6. `Tests/TTZipTests/Blosc2AdvancedArchitecturesTests.swift`: Full comprehensive test suite across all 4 architectures.
