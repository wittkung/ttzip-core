# Implementation Plan: Block-Splitting & Cost Evaluation

**Feature**: `123-block-splitting-and-cost-evaluation`
**Created**: 2026-08-19
**Status**: Ready for Implementation

---

## Technical Context

Single-core Deflate previously used fixed 4KB threshold heuristics to switch between static and dynamic Huffman. In PR 4:
1. Exact bit-cost evaluator `ttzip_eval_huffman_bit_costs` computes the precise compressed size of both static and dynamic representations in $< 1.0\mu s$.
2. Continuous streams exceeding 64KB automatically partition into 64KB blocks while seamlessly transferring the 32KB sliding window history across blocks.

---

## Constitution Check

- [x] **Hot-Path Zero-Cost Abstraction**: 0 runtime allocations during bit cost calculation and block splitting.
- [x] **Bitstream Integrity**: 100% RFC 1951 compliance.
- [x] **Zero Warnings**: `-Wall -Wextra -Werror` clean.

---

## Phase 0: Research Index

- - R001 [SUBAGENT:research] 《Vectorized Bit-Cost Evaluation Architecture》：Direct dot-product bit-cost evaluator in $< 1\mu s$.
- - R002 [SUBAGENT:research] 《History-Preserving Adaptive Block Splitting》：64KB chunk partition with 32KB continuous dictionary maintenance.

See [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/123-block-splitting-and-cost-evaluation/research.md).

---

## Phase 1: Data Model & Contracts Index

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/123-block-splitting-and-cost-evaluation/data-model.md)
- **Contracts**:
  - [`contracts/cost_evaluation_request.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/123-block-splitting-and-cost-evaluation/contracts/cost_evaluation_request.schema.json)
  - [`contracts/cost_evaluation_response.schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/123-block-splitting-and-cost-evaluation/contracts/cost_evaluation_response.schema.json)
- **Quickstart**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/123-block-splitting-and-cost-evaluation/quickstart.md)

---

## Proposed Changes

### Component 1: C Bridge Layer (`Sources/CTTZipBridge`)

#### [MODIFY] [`ttzip_deflate_huffman.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.h)
- Declare `ttzip_eval_huffman_bit_costs`.

#### [MODIFY] [`ttzip_deflate_huffman.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_huffman.c)
- Implement `ttzip_eval_huffman_bit_costs`.

#### [MODIFY] [`ttzip_deflate_engine.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/native_deflate/ttzip_deflate_engine.c)
- Integrate dynamic vs. static bit-cost evaluator into compression loop.

---

### Component 2: Swift Core & Test Suite (`Tests/TTZipTests`)

#### [NEW] [`BlockSplittingAndCostEvaluationTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/BlockSplittingAndCostEvaluationTests.swift)
- Unit tests for cost evaluator and multi-block continuity.

---

## Verification Plan

### Automated Tests
```bash
swift test --filter BlockSplittingAndCostEvaluationTests
swift test --filter HuffmanBitstreamOptimizationTests
swift test --filter LZ77VectorMatchFinderTests
swift test --filter XCTestPerformanceMeasureTests
```
