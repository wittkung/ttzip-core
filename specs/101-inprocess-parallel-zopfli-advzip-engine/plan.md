# Implementation Plan: In-Process 18-Core Parallel Zopfli/Advzip Engine

**Feature**: `specs/101-inprocess-parallel-zopfli-advzip-engine`

## Technical Context
- **Module**: `Sources/CTTZipBridge/` (C engine layer) and `Sources/TTZipCore/Zip/` (Parallel block orchestration).
- **Core Optimization**: In-process 18-core multi-pass block-split Zopfli/Advzip engine with 32KB window warmup, early exit, and complete removal of external CLI processes.

## Proposed Changes

### Phase 1: In-Process C Engine & Orchestration
1. **[NEW] [`Sources/CTTZipBridge/ttzip_zopfli_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_zopfli_engine.c)**:
   - Implement in-process multi-pass Zopfli & iterative block-split compressor.
2. **[NEW] [`Sources/CTTZipBridge/include/ttzip_zopfli_engine.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_zopfli_engine.h)**:
   - Expose C interface for block-level multi-pass compression.
3. **[MODIFY] [`Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift)**:
   - Route Level 6 (5 passes) and Level 7 (15 passes) to 18-core in-process concurrent engine.
   - Remove `/opt/homebrew/bin/pigz` and `/opt/homebrew/bin/advzip` fallback.

### Phase 2: Benchmark Expansion & Validation
1. **[MODIFY] [`Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift)**:
   - Expand `pigzLevels` to all 11 native levels: `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11]`.
2. **[MODIFY] [`docs/benchmarks/competitor_cache_zip.json`](file:///Users/kevintung/Documents/dev/TTZip/docs/benchmarks/competitor_cache_zip.json)**:
   - Cache all 11 pigz levels for instant execution.
