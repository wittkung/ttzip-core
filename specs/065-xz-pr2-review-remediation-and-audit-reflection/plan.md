# Implementation Plan: XZ PR 2 Review Remediation, Reproducibility Suite & Retrospective

**Feature Directory**: `specs/065-xz-pr2-review-remediation-and-audit-reflection`

**Created**: 2026-08-17

**Status**: Ready

---

## Technical Context

- **Upstream Repository**: `tukaani-project/xz` (PR #241).
- **Worktree**: `Vendor/worktrees/xz/pr2-arm64-crc64/` on branch `feat/arm64-crc64-clmul`.
- **Key Modules**:
  - `src/liblzma/check/crc64_arm64.h`: Core 4-way PMULL vector folding, unaligned tail masking, Barrett reduction, CPU feature detection.
  - `scratch/reproduce_bench_crc64.c`: Standalone, zero-dependency C reproduction benchmark.

---

## Phase 0: Research & Grounding Findings

- R001: Investigate x86 vs ARM64 shift direction conventions in vector registers.
  - *Decision*: In ARM NEON, `shift_left` (`vqtbl1q_u8(v, vmasks + 32 - amount)`) shifts elements toward higher byte indices (clearing lowest bytes). Document this explicitly in comment.
  - *Source*: ARM Architecture Reference Manual & `src/liblzma/check/crc_x86_clmul.h`.
- R002: Investigate Darwin `sysctlbyname` error handling in `crc32_arm64.h`.
  - *Decision*: Adopt upstream pattern from `crc32_arm64.h:134-138`: `if (sysctlbyname(...) != 0) return false; return has_pmull;`.
  - *Source*: [`src/liblzma/check/crc32_arm64.h:126-139`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/xz-upstream/src/liblzma/check/crc32_arm64.h#L126-L139).
- R003: Investigate standalone test harnesses for community reproduction (@ssvb).
  - *Decision*: Build a self-contained ANSI C11 file with embedded reference table generator, PMULL kernel, golden ECMA-182 vectors, and memory clobber loops.
  - *Source*: TTZip Benchmark Suite & `src/liblzma/check/crc_clmul_consts_gen.c`.

---

## Phase 1: Data Model, Contracts & Quickstart

- **Data Model**: `data-model.md` defines test vector tuples `(input_bytes, expected_crc64)` and benchmark metrics `(throughput_mbs, speedup_ratio)`.
- **Contracts**: `contracts/benchmark-result.json` defines JSON schema for benchmark assertions.
- **Quickstart**: `quickstart.md` defines 1-line compilation and execution commands.

---

## Proposed Changes

### Component: Upstream XZ Worktree (`Vendor/worktrees/xz/pr2-arm64-crc64`)

#### [MODIFY] [`src/liblzma/check/crc64_arm64.h`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/xz/pr2-arm64-crc64/src/liblzma/check/crc64_arm64.h)
- Line 67: Update `keep_high_bytes` doc comment to state masking/clearing low bytes.
- Line 75: Update `shift_left` doc comment to state left shift (clearing lowest bytes).
- Line 83: Update `shift_right` doc comment to state right shift (clearing highest bytes).
- Line 246: Update tail handling comment to state masking low bytes.
- Line 295-304: Update macOS `is_arch_extension_supported` to match `crc32_arm64.h` error handling.

### Component: Reproducibility Suite & Retrospective

#### [NEW] `scratch/reproduce_bench_crc64.c`
- Single-command zero-dependency benchmark testing golden vectors, 0..65536 byte sweeps, and 50 iterations of 64MB buffers.

#### [NEW] `specs/065-xz-pr2-review-remediation-and-audit-reflection/retrospective.md`
- Deep RCA on why comment drift and boolean fallback escaped previous checks.

---

## Verification Plan

### Automated Tests
1. **CTest Suite**: `ctest --test-dir Vendor/worktrees/xz/pr2-arm64-crc64/build-asan --output-on-failure`
2. **Standalone Benchmark**: `clang -O3 scratch/reproduce_bench_crc64.c -o scratch/reproduce_bench_crc64 && scratch/reproduce_bench_crc64`
