# Tasks: AArch64 Pareto-Optimal Zero-Regression compare256 Engine

**Feature**: `specs/118-aarch64-compare256-pareto-optimal-engine`

## Phase 1: Core Engine Implementation

- [x] T001 [P] [US1] Implement 16B direct subregister probe for 0..15B early exit with zero horizontal reduction in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/arch/arm/compare256_neon.c`
- [x] T002 [P] [US2] Implement 32B unrolled vector reduction loop for 32..256B with unified branch consolidation in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/arch/arm/compare256_neon.c`
- [x] T003 [US3] Implement 16..31B transition stage with direct lane extraction in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/arch/arm/compare256_neon.c`

## Phase 2: Verification & Performance Hardening

- [x] T004 Execute exhaustive 8,224-combination bit-exact validation across all alignments and lengths in `scratch/verify_early_probe_suite.c`
- [x] T005 Execute full 71/71 zlib-ng standard CTest test suite under CMake build
- [x] T006 Execute 0..128B fine-grained microbenchmark matrix and verify zero regression across all test lengths
