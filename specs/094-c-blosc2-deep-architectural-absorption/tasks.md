## Phase 1: User Story 1 - Native BloscLZ Codec Implementation (Priority: P1) 🎯 MVP

**Purpose**: Deliver clean, zero-allocation C implementation of BloscLZ byte-oriented LZ77 codec with 3-byte matching and ARM64 64-bit wild copy

- [x] T001 [US1] Implement `Sources/CTTZipBridge/include/ttzip_blosclz.h` defining BloscLZ C prototypes and configuration structures
- [x] T002 [US1] Implement `Sources/CTTZipBridge/ttzip_blosclz.c` with 3-byte match finder, L1 D-Cache hash table (`HASH_LOG 12..14`), 13-bit offset encoding, and branchless 64-bit `wild_copy`
- [x] T003 [P] [US1] Integrate BloscLZ into `Sources/CTTZipBridge/CTTZipFilterPipeline.c` as standard Codec ID 4 (`BLOSC_BLOSCLZ`)

---

## Phase 2: User Story 2 - N-Dimensional Tensor Hypercube Slicing (`b2nd`) (Priority: P1)

**Purpose**: Implement multi-dimensional array geometry, hypercube partition calculator, and `bstarts` block intersection solver

- [x] T004 [US2] Create `Sources/TTZipCore/NDim/NDimTensorLayout.swift` with multi-dimensional shape definitions, strides, and 2-level hypercube partition logic
- [x] T005 [P] [US2] Implement orthogonal axis slicing and sub-array bounding box intersection solver in `Sources/TTZipCore/NDim/NDimTensorLayout.swift`

---

## Phase 3: User Story 3 - Thread-Local Context Memory Pool (Priority: P1)

**Purpose**: Implement lockless context memory pool with 64-byte SIMD alignment and zero heap allocations

- [x] T006 [US3] Implement `Sources/CTTZipBridge/include/ttzip_context_pool.h` and `Sources/CTTZipBridge/ttzip_context_pool.c` with 64-byte aligned scratchpads
- [x] T007 [P] [US3] Implement `Sources/TTZipCore/Memory/ThreadLocalContextPoolAdapter.swift` bridging C context scratchpads into Swift async tasks

---

## Phase 4: Verification & Quality Gates (Priority: P1)

**Purpose**: Comprehensive automated testing, roundtrip parity, and performance floor regression

- [x] T008 [P] [US1] Create `Tests/TTZipTests/BloscLZNativeEngineTests.swift` testing roundtrip parity, 3-byte matches, and throughput
- [x] T009 [P] [US2] Create `Tests/TTZipTests/NDimTensorHypercubeSlicingTests.swift` testing 2D/3D tensor slicing and orthogonal axis extraction
- [x] T010 [P] [US3] Create `Tests/TTZipTests/ContextMemoryPoolTests.swift` validating zero dynamic heap allocations in hot loops
- [ ] T011 Run full regression suite `swift test` ensuring 100% pass across all 1037+ tests
- [ ] T012 Run performance gate `swift test --filter XCTestPerformanceMeasureTests` ensuring all 13 performance floors green
