# Quickstart Validation: Block-Splitting & Cost Evaluation

**Feature**: `123-block-splitting-and-cost-evaluation`
**Created**: 2026-08-19

---

## Validation Scenarios

### Scenario 1: Cost Evaluator Accuracy & Speed
Validates that `ttzip_eval_huffman_bit_costs` chooses the optimal representation across varied entropy profiles.

- **Command**:
  ```bash
  swift test --filter BlockSplittingAndCostEvaluationTests/testCostEvaluatorAccuracyAndSpeed
  ```
- **Expected Output**:
  ```text
  Test Suite 'BlockSplittingAndCostEvaluationTests' passed
  Executed 1 test, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - Verify that static cost formula accounts for length slots 257..285 and distance slots 0..29.

---

### Scenario 2: Multi-Block Streaming Continuity Test
Validates that continuous 256KB~1MB streams split across multiple 64KB blocks retain 32KB cross-block sliding history.

- **Command**:
  ```bash
  swift test --filter BlockSplittingAndCostEvaluationTests/testMultiBlockStreamingContinuity
  ```
- **Expected Output**:
  ```text
  Test Suite 'BlockSplittingAndCostEvaluationTests' passed
  ```
