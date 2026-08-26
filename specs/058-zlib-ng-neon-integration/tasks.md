# Tasks: zlib-ng NEON LCP Acceleration & Dual-Platform Integration

**Input**: Design documents from `specs/058-zlib-ng-neon-integration/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, independent modules)
- **[Story]**: User story identifier ([US1], [US2], [US3])
- Exact file paths included in all descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project build configuration and upstream module integration

- [x] T001 [P] Configure `Vendor/zlib-ng` submodule setup and universal static build flags (`-DZLIB_COMPAT=ON -DWITH_NATIVE_INSTRUCTIONS=ON`) in `scripts/build_zlib_ng.sh`
- [x] T002 [P] Update `Package.swift` to link static `TTZipVendor` and remove macOS system library `.linkedLibrary("z")`
- [x] T003 [P] Update `CMakeLists.txt` for Windows MSVC with `DYNAMIC_CPU_DISPATCH=ON` and `WITH_AVX512=ON`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: C Bridge data structures and hardware capability probing

**⚠️ CRITICAL**: Must complete before implementing specific user stories

- [x] T004 Define `DeflateStreamConfig`, `DeflateStreamState`, and `HardwareAccelerationCapabilities` in `Sources/CTTZipBridge/include/CTTZipStreamCoder.h`
- [x] T005 [P] Implement runtime CPU feature detection (`ttzip_detect_cpu_features`) in `Sources/CTTZipBridge/ttzip_platform_detect.c`
- [x] T006 [P] Add magic structural validation and lifecycle zeroing helpers in `Sources/CTTZipBridge/CTTZipStreamCoder.c`

**Checkpoint**: Foundation ready - User Story tasks can now proceed

---

## Phase 3: User Story 1 - Dual-Platform High-Performance Streaming Deflate Pipeline (Priority: P1) 🎯 MVP

**Goal**: Implement dual-tier Deflate streaming engine keeping `libdeflate` for whole-buffer fast-path and `zlib-ng` for streaming/libarchive pipelines.

**Independent Test**: Execute `swift test --filter DeflateStreamCoderTests` and assert streaming throughput >= 350 MB/s on Apple Silicon with 100% RFC 1951 fidelity.

### Tests for User Story 1
- [x] T007 [P] [US1] Create unit and roundtrip tests for streaming Deflate in `Tests/TTZipTests/DeflateStreamCoderTests.swift`
- [x] T008 [P] [US1] Create streaming large-file pipeline tests in `Tests/TTZipTests/DeflateStreamingPipelineTests.swift`

### Implementation for User Story 1
- [x] T009 [US1] Implement dual-tier stream initialization (`ttzip_zng_deflate_init` / `ttzip_zng_inflate_init`) in `Sources/CTTZipBridge/CTTZipStreamCoder.c`
- [x] T010 [US1] Implement chunk processing loop with sliding window in `Sources/CTTZipBridge/CTTZipStreamCoder.c`
- [x] T011 [US1] Bridge `DeflateStreamState` to Swift stream reader/writer in `Sources/TTZipCore/Pipeline/DeflateStreamEngine.swift`
- [x] T012 [US1] Verify and benchmark streaming throughput floor (>= 350 MB/s) in `Tests/TTZipTests/DeflateStreamCoderTests.swift`

**Checkpoint**: User Story 1 MVP fully functional and verified independently

---

## Phase 4: User Story 2 - Micro-Architecture Match-Length Comparison Optimization (Priority: P2)

**Goal**: Implement hybrid SWAR/NEON match finder eliminating Apple Silicon vector-to-GPR cross-domain latency on short matches (< 8 bytes).

**Independent Test**: Execute `swift test --filter HybridMatchFinderMicroTests` and verify short-match cycle latency <= 3 cycles and correct matching up to 258/273 bytes.

### Tests for User Story 2
- [x] T013 [P] [US2] Create micro-benchmark and correctness tests in `Tests/TTZipTests/HybridMatchFinderMicroTests.swift`

### Implementation for User Story 2
- [x] T014 [P] [US2] Declare `ttzip_hybrid_match_len_neon` prototype and inline attributes in `Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`
- [x] T015 [US2] Implement Tier 0 (64-bit SWAR GPR fast-fail) + Tier 1 (128-bit NEON unrolling) in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T016 [US2] Integrate hybrid match finder into LZMA2 / HC4 candidate search loops in `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
- [x] T017 [US2] Validate memory bounds and endianness safety against `contracts/hybrid_match_finder_contract.json`

**Checkpoint**: User Story 2 verified with monotonic latency and throughput gains

---

## Phase 5: User Story 3 - Windows x86_64 Legacy zlib Replacement & Upstream Contribution (Priority: P3)

**Goal**: Validate Windows AVX-512/AVX2 dynamic CPU dispatch build and prepare upstream optimization patch for `zlib-ng`.

**Independent Test**: Verify CMake Windows build configuration and generate `upstream-patches/0001-arm64-hybrid-swar-neon-match-len.patch`.

### Implementation for User Story 3
- [x] T018 [P] [US3] Verify CMake export targets and symbol visibility for Windows in `CMakeLists.txt`
- [x] T019 [US3] Generate upstream contribution patch for `zlib-ng/arch/arm/compare256_neon.c` in `docs/patches/zlib-ng-arm64-hybrid-match.patch`
- [x] T020 [US3] Document upstream PR justification and benchmark evidence in `docs/research/zlib_ng_upstream_proposal.md`

**Checkpoint**: User Story 3 deliverables completed and ready for cross-platform packaging

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Full regression suite, performance gate verification, and documentation synchronization

- [x] T021 [P] Run full performance gate verification via `swift test --filter XCTestPerformanceMeasureTests`
- [x] T022 [P] Run full regression test suite via `swift test` (525+ tests)
- [x] T023 Run performance regression audit script via `python3 scripts/audit_performance_regression.py`
- [x] T024 Synchronize architectural updates in `ARCHITECTURE.md` and `docs/research/compression_acceleration_ecosystem.md`

---

## Dependencies & Execution Order

### Phase Dependencies
- **Phase 1 (Setup)**: Can start immediately.
- **Phase 2 (Foundational)**: Depends on Phase 1 completion - blocks all User Stories.
- **Phase 3 (US1 - Streaming Pipeline)**: Depends on Phase 2 completion.
- **Phase 4 (US2 - Hybrid Match Finder)**: Depends on Phase 2 completion (can run in parallel with US1).
- **Phase 5 (US3 - Windows & Upstream)**: Depends on Phase 3 and Phase 4.
- **Phase 6 (Polish & Audit)**: Depends on all prior phases being complete.

### Parallel Opportunities
- Setup tasks `T001`, `T002`, `T003` can execute in parallel.
- Test creation `T007`, `T008`, `T013` can execute in parallel before implementations.
- US1 (`T009-T012`) and US2 (`T014-T017`) touch separate subsystem files and can run concurrently.
