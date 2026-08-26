# Tasks: libdeflate-Aligned Single-Core DEFLATE Engine with Apple Silicon Optimization

**Feature Branch**: `116-libdeflate-aligned-core-and-apple-silicon-supremacy`
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Align core headers and sequence data structures.

- [ ] T001 [P] Define `struct deflate_sequence` and 256 KB `struct hc_matchfinder` in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.h`
- [ ] T002 [P] Configure compression profile mappings for Levels 1 through 9 in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Integrate canonical baseline pipeline matching libdeflate ratio.

- [ ] T003 [P] Implement canonical 256 KB 16-bit relative index matchfinder with signed saturation sliding in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T004 [P] Implement dynamic entropy-guided block splitting (`SOFT_MAX_BLOCK_LENGTH = 300,000`) in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T005 [P] Implement linear-time in-place Moffat-Katajainen Huffman builder and precode RLE generator in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.c`

---

## Phase 3: User Story 1 - Canonical Baseline Alignment & Perfect Space Parity (Priority: P1) 🎯 MVP

**Goal**: Bitstream compression ratio matching libdeflate Level 3 (3.34 MB) and Level 6 (3.21 MB) on 100MB `enwik8`.

**Independent Test**: `swift test -c release --filter SingleCoreDeflatePkTests` validates size parity.

### Implementation for User Story 1

- [ ] T006 [US1] Route Level 3 to Greedy/Fast-Lazy (`max_search_depth = 12`, `nice_match_len = 14..32`) in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T007 [US1] Route Level 6 to Deep-Lazy (`max_search_depth = 35`, `nice_match_len = 65`, 2-step lookahead) in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T008 [US1] Verify baseline compression sizes on 100MB `enwik8` in `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift`

---

## Phase 4: User Story 2 - Apple Silicon Vectorized & Multi-Port Acceleration (Priority: P2)

**Goal**: TTZip Tier 3 >= 1.20 GB/s (> libdeflate L3 1.07 GB/s) and Tier 4 >= 800 MB/s (> libdeflate L6 722 MB/s).

**Independent Test**: `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests` verifies upper-right envelope.

### Implementation for User Story 2

- [ ] T009 [P] [US2] Implement Patch 1: Hybrid 128-bit NEON + 64-bit GPR SWAR Tier-0 `lz_extend_neon` in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c`
- [ ] T010 [P] [US2] Implement Patch 2: Multi-candidate load unrolling in `hc_matchfinder_longest_match` in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_lazy.c`
- [ ] T011 [P] [US2] Implement Patch 3: 64-bit GPR fused sequence bitstream packing in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T012 [US2] Run full Pareto benchmark and assert complete upper-right envelope dominance

---

## Phase 5: User Story 3 - Deterministic Bit-Stream Fidelity & Multi-Format Round-Trip (Priority: P3)

**Goal**: 100% byte-exact round-trip verification across all levels.

**Independent Test**: `swift test -c release --filter SingleCoreDeflateOracleTests` passes 0 errors.

### Implementation for User Story 3

- [ ] T013 [US3] Execute automated oracle test suite in `Tests/TTZipTests/SingleCoreDeflateOracleTests.swift`
- [ ] T014 [P] Compiler warning and memory safety audit in `Sources/CTTZipBridge/`
