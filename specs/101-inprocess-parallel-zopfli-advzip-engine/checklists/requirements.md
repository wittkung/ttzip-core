# Requirements Quality Matrix: In-Process 18-Core Parallel Zopfli/Advzip Engine

**Feature**: `specs/101-inprocess-parallel-zopfli-advzip-engine`

## 1. Content Quality
- [x] Clear performance target: 15x acceleration on Tier 7 (from 200s down to <20s).
- [x] Zero-CLI invariant: Eliminate all external Homebrew dependencies for MAS sandbox compliance.
- [x] Full-matrix pigz coverage: Expand pigz benchmarks to all 11 levels (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11).

## 2. Requirement Completeness
- [x] FR-001 (In-process parallel engine), FR-002 (32KB dictionary warmup), FR-003 (Early convergence), FR-004 (All-level pigz) defined.
- [x] Memory management and buffer safety rules specified.

## 3. Feature Readiness
- [x] Quantifiable success criteria (SC-001 ~ SC-004) defined.
- [x] Hard performance floor ($\ge 4.5\text{ MB/s}$ on Tier 7) enforced.
