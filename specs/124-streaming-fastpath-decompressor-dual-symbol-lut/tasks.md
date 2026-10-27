# Implementation Tasks: Streaming Fast-Path Decompressor & Dual-Symbol LUT

**Feature**: `124-streaming-fastpath-decompressor-dual-symbol-lut`
**Created**: 2026-08-19

---

## Task Matrix

### Phase 1: Decompressor Engine Implementation

- [x] T001 [P] [US1] Implement 10-bit dual-symbol LUT decoding loop in [`Sources/CTTZipBridge/native_inflate/ttzip_inflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_inflate/ttzip_inflate_engine.c)
- [x] T002 [US2] Implement NEON 128-bit match copy and SWAR short offset broadcast in [`Sources/CTTZipBridge/native_inflate/ttzip_inflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_inflate/ttzip_inflate_engine.c)

### Phase 2: Comprehensive Test Suite & Benchmark

- [x] T003 [P] [US1] Implement oracle equivalence and throughput floor tests in [`Tests/TTZipTests/StreamingDecompressorDualSymbolLutTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/StreamingDecompressorDualSymbolLutTests.swift)
- [x] T004 [US1] Run full regression tests (`swift test --filter XCTestPerformanceMeasureTests`) to verify 0 regressions.

---

## Dependencies

- T001 -> T002 -> T003 -> T004
