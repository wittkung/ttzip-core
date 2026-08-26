# Implementation Plan: LZ4 Upstream Partial Decompression Guard

## Technical Approach
1. **Target Repository**: `Vendor/worktrees/lz4/partial-guard` (Branch `feat/partial-decompress-guard`).
2. **Phase 1: Test Oracle Expansion**:
   - Enhance `tests/decompress-partial.c` to test all target output sizes from 1 byte up to full block size across different compression levels and block sizes.
3. **Phase 2: Core Optimization**:
   - In `lib/lz4.c`, add `if (partialDecoding && (op >= oend)) break;` in the two-stage safe shortcut after match copy.
4. **Phase 3: Validation**:
   - Run GNU Make tests (`make -C programs lz4`, `make -C tests test-lz4`, `make -C tests frametest`).
   - Run CMake build (`cmake -B build -S build/cmake && cmake --build build`).
   - Audit code against `CODING_STYLE` and `.clang-format`.
