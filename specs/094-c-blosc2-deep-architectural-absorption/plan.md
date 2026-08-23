# Implementation Plan: C-Blosc2 Exhaustive Architectural Absorption (Feature 094)

## Technical Context

- **Target OS**: macOS 14.0+ Sonoma (Apple Silicon ARM64 / Intel x86_64)
- **Language**: Swift 6.0 + C11/POSIX
- **In-Process C Binding**: 100% In-Process static C compilation in `Sources/CTTZipBridge/`, 0 external subprocesses.
- **Hardware SIMD Vectorization**: ARM NEON 64-bit/128-bit unaligned `wild_copy`, 64-byte cacheline alignment, 16KB physical page direct buffers.

---

## Constitution Check

- **Zero-Cost Abstraction on Hot Paths**: 0 `malloc`/`free` during inner block compression loops; all memory pre-allocated via thread-local context memory pools.
- **Fast-Path Retention**: BloscLZ + BitShuffle direct in-process C pathway without intermediate object allocations.
- **Hard Performance Floors**: 13 historical performance thresholds protected against regression.

---

## Phase 0: Research Findings Index

- R001 [SUBAGENT:research] 《BloscLZ Byte-Oriented LZ77 Engine & Branchless Token Packing》: Native 3-byte short matching (`len >= 3`), 13-bit compact offset packing, $O(1)$ L1 data cache table residency (`HASH_LOG=12..14`), and 64-bit unaligned `wild_copy` on Apple Silicon.
- R002 [SUBAGENT:research] 《N-Dimensional Tensor & Hyper-Cube Slicing (`b2nd`)》: 2-level hyper-cubic partitioning (Pineapple partitioning: Chunks at L3/SLC level $\to$ Blocks at L2 level) with `bstarts` block directory table for $O(1)$ random-access block seeking and orthogonal axis sub-array extraction.
- R003 [SUBAGENT:research] 《Thread-Local Context Memory Pooling & 64-Byte Cacheline Alignment》: Lockless working buffer reuse with 64-byte SIMD alignment and 16KB Direct I/O hardware page alignment.

---

## Phase 1: Design Artifacts & Contracts

- `data-model.md`: Data structures for BloscLZ config, NDim tensor layouts, and thread context memory pool.
- `contracts/blosclz-engine-contract.json`: JSON Schema for BloscLZ codec operations.
- `contracts/ndim-tensor-slicing-contract.json`: JSON Schema for N-Dimensional tensor chunking and sub-array slicing.
- `contracts/context-memory-pool-contract.json`: JSON Schema for context memory pool allocation and zero-allocation assertions.
- `quickstart.md`: 3 validation scenarios for BloscLZ, NDim tensor slicing, and context pool testing.

---

## Implementation Breakdown by Component

### 1. `Sources/CTTZipBridge/` (C Native Layer)
- [NEW] `ttzip_blosclz.h` & `ttzip_blosclz.c`: Clean, optimized C implementation of BloscLZ codec with 3-byte matching, `HASH_LOG 12..14` L1 cache residency, and 64-bit unaligned ARM64 `wild_copy`.
- [NEW] `ttzip_context_pool.h` & `ttzip_context_pool.c`: Thread-local context memory pool allocating 64-byte cacheline and 16KB page-aligned working scratchpads with zero mutex contention.

### 2. `Sources/TTZipCore/` (Swift Domain Engine)
- [NEW] `NDim/NDimTensorLayout.swift`: Multi-dimensional tensor geometry, hypercube partition calculator, and `bstarts` block intersection solver.
- [NEW] `Memory/ThreadLocalContextPoolAdapter.swift`: Swift adapter bridging C thread-local context pools with task-isolated workers.
- [MODIFY] `CTTZipFilterPipeline.c`: Wire BloscLZ as codec ID 4 in the dynamic filter pipeline.

### 3. `Tests/TTZipTests/` (Test Suite)
- [NEW] `BloscLZNativeEngineTests.swift`: Unit tests for BloscLZ roundtrip parity, boundary tests, and throughput benchmarking.
- [NEW] `NDimTensorHypercubeSlicingTests.swift`: Multi-dimensional 2D/3D tensor slicing and orthogonal axis extraction tests.
- [NEW] `ContextMemoryPoolTests.swift`: Multi-threaded zero-heap-allocation stress test under 16 concurrent workers.
