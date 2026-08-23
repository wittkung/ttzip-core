# Feature Specification: LZ4 Upstream Partial Decompression Fast Short-Circuit Guard

## 1. Background & Motivation
`LZ4_decompress_safe_partial()` is widely used in downstream systems like Linux kernel (EROFS filesystem), ClickHouse, and RocksDB to verify record headers and magic signatures without full block decompression.

### Core Testing Gap in Upstream
In the official upstream test suite `tests/decompress-partial.c`, the existing test only checks cases where `targetOutputSize == srcLen` (full uncompressed length). It completely lacks parametric coverage for arbitrary prefix lengths from 1 byte up to `srcLen - 1`.

## 2. Requirements & Constraints
- **C90 Strict Conformance**: Must strictly adhere to `CODING_STYLE` (C90 syntax, `/* ... */` comments only, top-of-block variable declarations, and `-Wc++-compat` friendly).
- **Comprehensive Oracle Coverage**: Iteratively test all prefix lengths 1 <= target <= srcLen, asserting `result >= target` and `memcmp(source, dst, target) == 0`.
- **Zero Library Mutation**: Pure test enhancement without mutating `lib/` core files, zero risk of regression.

## 3. Success Criteria
- `make -C tests decompress-partial`: 100% PASS with clean output `test decompress-partial OK`.
- `make -C tests test-lz4-basic`: 100% PASS.
- Pre-Flight 18 items 100% PASS.
