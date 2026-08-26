# Tasks: AArch64 compare256 2x-Unrolled NEON+VORR+UMAXV Optimization

**Feature Branch / Directory**: `specs/110-aarch64-compare256-zero-overhead-optimization`  
**Status**: COMPLETED / CONVERGED  

---

## Phase 1: Setup & Upstream Architecture Verification

- [x] T001 Verify clean upstream worktree at `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256` on branch `feat/arm64-swar-compare256`.
- [x] T002 Build baseline binaries and execute microbenchmarks (`benchmark_zlib` / `benchmark_compare256`).

---

## Phase 2: Implementation & Microarchitecture Hardening

- [x] T003 Implement 2x-Unrolled NEON + VORR + Single UMAXV architecture in `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/arch/arm/compare256_neon.c`.
- [x] T004 Build static library `libz-ng.a` and verify zero compiler warnings under Apple Clang 17 (`-Wall -Wextra -O3`).
- [x] T005 Verify inlining and register allocation via `otool -tv` on `compare256_neon.c.o` (assert 0 stack spills).

---

## Phase 3: Validation & Full Matrix Benchmarking

- [x] T006 Run full test suite (`ctest`) and assert 100% pass rate (70/70 tests under strict build).
- [x] T007 Run 5-repetition median benchmarks across all 8 Google Benchmark data types (`develop_bench_all_types.json` vs `unrolled2x_bench.json`).
- [x] T008 Assert zero regression on `literals` (L3 $\le 10.357\text{ ms}$) and $\ge 20\%$ speedup on `text` L1.
- [x] T009 Prepare and submit finalized upstream response on GitHub PR #2416 upon user authorization.
