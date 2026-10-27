# Implementation Tasks: Block-Splitting & Cost Evaluation

**Feature**: `123-block-splitting-and-cost-evaluation`
**Created**: 2026-08-19

---

## Task Matrix

### Phase 1: Cost Evaluator Implementation

- [x] T001 [P] [US1] Implement `ttzip_eval_huffman_bit_costs` in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.c) and declare in [`ttzip_deflate_huffman.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.h)

### Phase 2: Engine Integration

- [x] T002 [US1] Integrate `ttzip_eval_huffman_bit_costs` into block decision in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c)
- [x] T003 [P] [US2] Ensure seamless 32KB history preservation across split blocks in [`Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c)

### Phase 3: Comprehensive Test Suite

- [x] T004 [P] [US1] Implement unit tests in [`Tests/TTZipTests/BlockSplittingAndCostEvaluationTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/BlockSplittingAndCostEvaluationTests.swift)
- [x] T005 [US1] Run full regression tests (`swift test --filter XCTestPerformanceMeasureTests`) to verify 0 regressions.

---

## Dependencies

- T001 -> T002 -> T003 -> T004 -> T005
