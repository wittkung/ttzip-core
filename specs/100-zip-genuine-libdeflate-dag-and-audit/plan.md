# Implementation Plan: Genuine Libdeflate DAG Routing & Codebase Disconnect Audit

**Feature**: `specs/100-zip-genuine-libdeflate-dag-and-audit`

## Technical Context
- **Architecture**: In-process C static library bindings (`CTTZipBridge`) with Swift 6.0 engine layer (`TTZipCore`).
- **Core Focus**: Eliminate all fake routes, parameter clamping, silent fallbacks, and thread-safety defects discovered in the static audit.

## Proposed Changes

### Phase 1: Core C Bridge Fixes
1. **[MODIFY] [`Sources/CTTZipBridge/CTTZipStreamCoder.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipStreamCoder.c)**:
   - Remove `(level == 6 ? 4 : level)` tampering.
   - Refactor `ttzip_get_tls_compressor` to clean 1:1 level mapping $1..12$.
   - Refactor `ttzip_raw_deflate_block_compress` to use pure `libdeflate_deflate_compress`.
2. **[MODIFY] [`Sources/CTTZipBridge/CTTZipExtract.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipExtract.c)**:
   - Fix `total_entries` to `uint64_t` (resolve ZIP64 65535+ file truncation).
3. **[MODIFY] [`Sources/CTTZipBridge/CTTZipBridge_GzParallel.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_GzParallel.c)**:
   - Fix `bz_level` and `z_level` parameter clamping.
4. **[MODIFY] [`Sources/CTTZipBridge/CTTZipBridge_Snappy.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_Snappy.c)**:
   - Add `dispatch_once` thread-safe initialization for CRC32C table.
5. **[MODIFY] [`Sources/TTZipCore/ArchiveWriter+Dispatch.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/ArchiveWriter+Dispatch.swift)**:
   - Fix fallback branch to pass actual `Int32(level.rawValue)` instead of `advancedOptions.zstdLevel`.

### Phase 2: Verification & Pareto Re-run
1. Re-run `ZipMultiCoreParetoFrontierPkTests` to measure genuine Libdeflate Level 1~12 / DAG performance.
2. Run full test suite (`swift test`) to assert zero regression across all 16 formats.
