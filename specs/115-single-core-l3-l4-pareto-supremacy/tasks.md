# Tasks: Single-Core L3/L4 Intermediate Pareto Dominance

**Feature Branch**: `115-single-core-l3-l4-pareto-supremacy`
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Update C engine structures and define decoupled matchfinder interfaces.

- [ ] T001 [P] Declare compact 16-bit relative index structures and Tier 3 / Tier 4 matchfinder prototypes in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h`
- [ ] T002 [P] Update engine options struct with `lookahead_steps` and `skip_intermediate_hashes` in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core matchfinder algorithms and cache-resident chunking state.

- [ ] T003 [P] Implement Tier 3 Fast-Lazy matchfinder (`ttzip_deflate_fast_lazy_find_matches`) with 128KB L1D-resident table and tail-only skip in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c`
- [ ] T004 [P] Implement Dual-Anchor 64-bit GPR SWAR prefix mismatch filter (`ttzip_lazy_swar_dual_anchor`) in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c`
- [ ] T005 [P] Implement Tier 4 Deep-Lazy parser with 2-step lookahead (`lazy2`) and logarithmic distance entropy weighting in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c`

---

## Phase 3: User Story 1 - Differentiated Intermediate Compression Profiles (Priority: P1) 🎯 MVP

**Goal**: Monotonic differentiation between Tier 1, Tier 2, Tier 3, and Tier 4 with zero point collapse.

**Independent Test**: `swift test -c release --filter SingleCoreDeflatePkTests` validates separation.

### Implementation for User Story 1

- [ ] T006 [US1] Implement 64KB/128KB cache-resident block chunking loop with 256KB TLS token buffer in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T007 [US1] Implement distinct profile parameter dispatcher for Tier 3 (`tier_level=3`) and Tier 4 (`tier_level=4`) in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T008 [US1] Add single-core intermediate tier level tests in `Tests/TTZipTests/SingleCoreDeflatePkTests.swift`

---

## Phase 4: User Story 2 - Comprehensive Pareto Supremacy Over libdeflate (Priority: P2)

**Goal**: TTZip Tier 3 >= 1.20 GB/s (> libdeflate L3 ~1.07 GB/s) and Tier 4 >= 850 MB/s (> libdeflate L6 ~749 MB/s).

**Independent Test**: `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests` verifies Pareto envelope.

### Implementation for User Story 2

- [ ] T009 [P] [US2] Optimize Tier 3 hash chain traversal with nice match length early break in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c`
- [ ] T010 [P] [US2] Optimize Tier 4 2-step lookahead parser with reduced search depth at $pos+1$ and $pos+2$ in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c`
- [ ] T011 [US2] Validate full single-core Pareto frontier in `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift` and export updated chart

---

## Phase 5: User Story 3 - Deterministic Bit-Stream Fidelity & Multi-Format Round-Trip (Priority: P3)

**Goal**: 100% byte-exact round-trip verification across all intermediate levels.

**Independent Test**: `swift test -c release --filter SingleCoreDeflateOracleTests` passes 0 errors.

### Implementation for User Story 3

- [ ] T012 [P] [US3] Verify continuous multi-block dynamic Huffman bitstream emission without intermediate padding in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T013 [US3] Execute automated oracle test suite in `Tests/TTZipTests/SingleCoreDeflateOracleTests.swift`

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Zero-warning compiler audit and quickstart validation.

- [ ] T014 [P] Verify zero compiler warnings in `Sources/CTTZipBridge/`
- [ ] T015 Run end-to-end `quickstart.md` validation suite
