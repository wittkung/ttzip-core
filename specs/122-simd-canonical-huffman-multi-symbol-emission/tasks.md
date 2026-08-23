# Implementation Tasks: SIMD Canonical Huffman Coding & Multi-Symbol Emission

**Feature**: `122-simd-canonical-huffman-multi-symbol-emission`
**Created**: 2026-08-19

---

## Task Matrix

### Phase 1: Bitstream Engine Extensions

- [x] T001 [P] [US1] Implement `ttzip_bs_write_bits64` (up to 56 bits in single 64-bit store) in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_bitstream.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_bitstream.h)

### Phase 2: Multi-Symbol & Small-File Huffman Optimization

- [x] T002 [US1] Implement packed 4-field match token serialization in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c)
- [x] T003 [US1] Implement dual-literal pairing serialization loop in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c)
- [x] T004 [P] [US2] Implement sub-4KB static Huffman bypass in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c)

### Phase 3: Comprehensive Test Suite & Benchmark

- [x] T005 [P] [US1] Implement multi-symbol bitstream oracle tests in [`Tests/TTZipTests/HuffmanBitstreamOptimizationTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/HuffmanBitstreamOptimizationTests.swift)
- [x] T006 [P] [US2] Re-run 250MB compound mixed workspace single-core 1v1 duel against libdeflate to assert $\ge 800\text{ MB/s}$ throughput.

### Phase 4: Verification & Convergence

- [x] T007 [US1] Run full test suite (`swift test --filter XCTestPerformanceMeasureTests`) to assert zero regression across all formats.

---

## Dependencies

- T001 -> T002 -> T003 -> T004 -> T005 -> T006 -> T007
