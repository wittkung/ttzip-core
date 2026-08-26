# Tasks: Near-Optimal DP Acceleration and Full-Spectrum Pareto Supremacy

**Feature Branch**: `117-near-optimal-dp-acceleration-and-pareto-supremacy`
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Align engine configuration and near-optimal DP structures.

- [ ] T001 [P] Verify `NearOptimalDPOptions` structures in `Vendor/libdeflate-upstream/lib/deflate_compress.c`
- [ ] T002 [P] Configure tier profile mapping in `Sources/TTZipCore/Zip/ZipCompressionProfile.swift`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Near-Optimal DP DAG shortest-path acceleration in C layer.

- [ ] T003 [P] Implement length-slot endpoint pruning in `deflate_find_min_cost_path` in `Vendor/libdeflate-upstream/lib/deflate_compress.c`
- [ ] T004 [P] Rescale pass convergence ladder (`max_optim_passes = 4`, `min_improvement_to_continue = 16`) in `Vendor/libdeflate-upstream/lib/deflate_compress.c`
- [ ] T005 [P] Rebuild static libraries `libdeflate.a` and `libTTZipVendor.a` via `scripts/build_libdeflate.sh`

---

## Phase 3: User Story 1 - Near-Optimal DP (Level 12) Forward-Pass Acceleration (Priority: P1) 🎯 MVP

**Goal**: Tier 4 (Level 12) throughput $\ge 35\text{ MB/s}$ with $\le 3.03\text{ MB}$ compressed size.

**Independent Test**: `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests` asserts Tier 4 speedup.

### Implementation for User Story 1

- [ ] T006 [US1] Wire Tier 4 High (deflateLevel: 12) execution in `Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`
- [ ] T007 [US1] Measure Tier 4 throughput on 100MB `enwik8` in `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift`

---

## Phase 4: User Story 2 & 3 - Full 8-Tier Pareto Envelope Dominance (Priority: P2)

**Goal**: Full 8-tier Pareto envelope strictly dominating all competitors.

**Independent Test**: `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter ZipSingleCoreParetoFrontierPkTests` generates final Pareto chart.

### Implementation for User Story 2 & 3

- [ ] T008 [US2] Verify Tier 1 (Fast), Tier 2 (Normal), Tier 3 (Maximum), Tier 4 (High) live execution in `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift`
- [ ] T009 [US3] Run full Pareto benchmark and export PNG chart to artifact directory
- [ ] T010 [P] Validate 100% test suite and local CI gate compliance
