# Tasks: Liblzma (XZ Utils) ARM NEON Match Finder Acceleration & Upstream Baseline Integration

**Input**: Design artifacts from `specs/059-liblzma-neon-acceleration/`
**Status**: Completed

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Build infrastructure initialization and environment verification

- [x] T001 [P] Verify Apple Clang ARM NEON and ACLE CRC32 compiler support via `Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`
- [x] T002 [P] Inspect `Vendor/xz-upstream/` git tree state and build configuration in `Vendor/xz-upstream/CMakeLists.txt`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core match finding and hardware detection primitives required by all user stories

**⚠️ CRITICAL**: Must complete before user story implementation begins

- [x] T003 Verify ARMv8 CRC32 hardware instruction detection and fallback macros in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T004 [P] Verify 128-bit vector and 64-bit SWAR hybrid match length evaluation in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T005 [P] Create comprehensive micro-benchmark test harness in `Tests/TTZipTests/HybridMatchFinderMicroTests.swift`

**Checkpoint**: Foundation ready - user story implementation can proceed

---

## Phase 3: User Story 1 - High-Throughput LZMA2 & XZ Compression on Apple Silicon (Priority: P1) 🎯 MVP

**Goal**: Deliver verified >= 4.5 GB/s match finding throughput and accelerated LZMA2 compression on Apple Silicon without regression

**Independent Test**: Run `swift test --filter HybridMatchFinderMicroTests` and verify all tests pass with 100% bit-exact parity

### Implementation for User Story 1

- [x] T006 [P] [US1] Implement unit test suite for exhaustive length boundary checks (0..273 bytes) in `Tests/TTZipTests/HybridMatchFinderMicroTests.swift`
- [x] T007 [P] [US1] Implement unaligned pointer memory safety tests in `Tests/TTZipTests/HybridMatchFinderMicroTests.swift`
- [x] T008 [US1] Harden Tier 0 GPR fast-check and Tier 1 128-bit NEON unrolling in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T009 [US1] Verify multi-tier direct hash tables (hash2, hash3, hash4) and chain traversal in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T010 [US1] Validate integration between HC4 NEON match finder and fast LZMA2 block encoder in `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`

**Checkpoint**: User Story 1 complete and independently verifiable

---

## Phase 4: User Story 2 - Zero-Regression Integration with Vendor Library Foundation (Priority: P2)

**Goal**: Update upstream source tree `Vendor/xz-upstream/` with ARM NEON and ACLE CRC32 acceleration, and rebuild `Vendor/libTTZipVendor.a`

**Independent Test**: Run `swift test --filter XCTestPerformanceMeasureTests` and `swift test` ensuring all 525+ tests pass with zero performance regression

### Implementation for User Story 2

- [x] T011 [P] [US2] Implement ARM NEON 128-bit match length comparison in `Vendor/xz-upstream/src/liblzma/common/memcmplen.h`
- [x] T012 [P] [US2] Implement ARMv8 ACLE hardware CRC32 acceleration in `Vendor/xz-upstream/src/liblzma/lz/lz_encoder_hash.h`
- [x] T013 [US2] Create automated build and packaging script `scripts/build_liblzma.sh` to compile `liblzma.a` and merge `Vendor/libTTZipVendor.a`
- [x] T014 [US2] Execute `scripts/build_liblzma.sh` to update `Vendor/lib/liblzma.a`, `Vendor/libTTZipVendor.a`, and `Vendor/TTZipVendor.xcframework`
- [x] T015 [US2] Run `swift test --filter XCTestPerformanceMeasureTests` to verify performance floor compliance across all formats

**Checkpoint**: User Story 2 complete and static library updated

---

## Phase 5: User Story 3 - Upstream Contribution & Open Source Reference Alignment (Priority: P3)

**Goal**: Prepare isolated, portable patch files and documentation compliant with `tukaani-project/xz` upstream contribution guidelines

**Independent Test**: Validate patch formatting and compile upstream `Vendor/xz-upstream` test suite without warnings

### Implementation for User Story 3

- [x] T016 [P] [US3] Generate atomic patch file `patches/0001-liblzma-arm-neon-memcmplen.patch` for `memcmplen.h`
- [x] T017 [P] [US3] Generate atomic patch file `patches/0002-liblzma-arm64-acle-crc32-hash.patch` for `lz_encoder_hash.h`
- [x] T018 [US3] Document upstream integration guide and benchmark metrics in `docs/upstream/tukaani_xz_neon_acceleration.md`

**Checkpoint**: All user stories complete

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final verification, code review, and full regression sweep

- [x] T019 [P] Execute full unit test suite `swift test` across all 525+ test cases
- [x] T020 Run `quickstart.md` validation scenarios and record execution logs
- [x] T021 Final code review and static analysis verification

---

## Dependencies & Execution Order

### Phase Dependencies
- **Setup (Phase 1)**: Completed.
- **Foundational (Phase 2)**: Completed.
- **User Stories (Phase 3+)**: Completed.
- **Polish (Phase 6)**: Completed.

### Parallel Opportunities
- T001, T002 completed.
- T004, T005 completed.
- T006, T007 completed.
- T011, T012 completed.
- T016, T017 completed.

---

## Implementation Strategy

### MVP First (User Story 1 Only)
1. Complete Setup (T001-T002) + Foundational (T003-T005).
2. Complete User Story 1 (T006-T010).
3. Validate via `HybridMatchFinderMicroTests` (MVP ready).

### Incremental Delivery
1. User Story 1 → Fast in-process native match finder verified.
2. User Story 2 → Upstream vendor static library rebuilt and benchmarked.
3. User Story 3 → Atomic upstream patches generated.
4. Polish → Full 525+ test regression passed.
