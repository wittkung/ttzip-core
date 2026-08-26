# Tasks: Blosc2 Exhaustive Architectural Conquest (全景架构穷尽式吸收与集成)

**Input**: Design documents from `/specs/091-blosc2-exhaustive-architectural-conquest/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

## Format: `- [ ] [TaskID] [P?] [Story?] Description with file path`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., [US1], [US2], [US3], [US4])

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Header prototypes, plugin ID mappings, and build exports

- [x] T001 [P] Create `Sources/CTTZipBridge/include/CTTZipPluginRegistry.h` with user plugin ID range (160..255) and callback typedefs
- [x] T002 [P] Create `Sources/CTTZipBridge/include/CTTZipBitGroom.h` with Bit-Grooming and mantissa precision quantization prototypes
- [x] T003 [P] Update `Sources/CTTZipBridge/include/CTTZipSuperChunk.h` to declare `ttzip_schunk_get_slice_buffer` prototype
- [x] T004 Update `Sources/CTTZipBridge/include/CTTZipBridge.h` master bridge header to include new headers

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Lock-free static jump tables and atomic synchronization primitives

- [x] T005 [P] Implement `Sources/CTTZipBridge/CTTZipPluginRegistry.c` with static BSS arrays, acquire-release atomic reads, and mutex write synchronization
- [x] T006 [P] Implement NEON floating-point mantissa bitmask generator (`ttzip_bitgroom_calc_mask`) in `Sources/CTTZipBridge/CTTZipBitGroom.c`

**Checkpoint**: Foundational structures compiled and ready for user story pipelines.

---

## Phase 3: User Story 1 - Dynamic Filter & Codec Plugin Registry (Priority: P1) 🎯 MVP

**Goal**: Support runtime registration of domain-specific filters/codecs with zero lock contention and inline built-in fast paths

**Independent Test**: `swift test --filter Blosc2PluginRegistryTests` verifies registration in range [160, 255] and execution.

- [x] T007 [P] [US1] Implement `ttzip_plugin_register_filter` and `ttzip_plugin_register_codec` with ID bounds check in `Sources/CTTZipBridge/CTTZipPluginRegistry.c`
- [x] T008 [US1] Implement `ttzip_plugin_dispatch_filter_forward` and `backward` with inline fast-path for IDs 0..15 in `Sources/CTTZipBridge/CTTZipPluginRegistry.c`
- [x] T009 [P] [US1] Create unit tests in `Tests/TTZipTests/Blosc2PluginRegistryTests.swift` validating custom plugin registration and execution

**Checkpoint**: User Story 1 complete and independently verified (MVP).

---

## Phase 4: User Story 2 - Block-Level Lazy Slicing & Sub-Chunk Zero-Copy Extraction (Priority: P1)

**Goal**: Implement `ttzip_schunk_get_slice_buffer` to decompress only intersecting 128KB micro-blocks and bypass non-intersecting blocks

**Independent Test**: `swift test --filter Blosc2LazySlicingTests` confirms $> 90\%$ decompression bypass and bit-exact extraction.

- [x] T010 [P] [US2] Implement micro-block range calculation math (`first_block`, `last_block`) in `Sources/CTTZipBridge/CTTZipSuperChunk.c`
- [x] T011 [US2] Implement `ttzip_schunk_get_slice_buffer` with true zero-copy direct write for interior blocks and scratchpad for boundary blocks in `Sources/CTTZipBridge/CTTZipSuperChunk.c`
- [x] T012 [P] [US2] Create unit and bypass throughput tests in `Tests/TTZipTests/Blosc2LazySlicingTests.swift`

**Checkpoint**: User Stories 1 and 2 independently functional.

---

## Phase 5: User Story 3 - Floating-Point Precision Quantization & Bit-Grooming (Priority: P2)

**Goal**: Implement Bit-Grooming and BitRound filters on Float32/Float64 to boost BitShuffle compression ratio

**Independent Test**: `swift test --filter Blosc2BitGroomingTests` verifies bounded relative error and $> 500\%$ compression ratio boost.

- [x] T013 [P] [US3] Implement `ttzip_filter_bitgroom_float32_neon` and `ttzip_filter_bitgroom_float64_neon` in `Sources/CTTZipBridge/CTTZipBitGroom.c`
- [x] T014 [P] [US3] Implement `ttzip_filter_bitround_float32_neon` nearest-even rounding kernel in `Sources/CTTZipBridge/CTTZipBitGroom.c`
- [x] T015 [US3] Connect BitGroom and BitRound to Swift `Blosc2FilterBridge.swift`
- [x] T016 [P] [US3] Create precision verification and compression synergy tests in `Tests/TTZipTests/Blosc2BitGroomingTests.swift`

**Checkpoint**: User Stories 1, 2, and 3 functional.

---

## Phase 6: Polish & Cross-Cutting Quality Gates

**Purpose**: System integration, regression verification, and performance gates

- [x] T017 Run full regression suite `swift test` ensuring 100% pass across all 535+ tests
- [x] T018 Run performance gate `swift test --filter XCTestPerformanceMeasureTests` ensuring all 13 performance floors green
- [x] T019 Complete Spec Kit delivery and verification
