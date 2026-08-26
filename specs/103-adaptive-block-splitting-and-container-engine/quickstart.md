# Phase 1 Quickstart & Validation Guide

**Feature Branch / Spec Directory**: `specs/103-adaptive-block-splitting-and-container-engine`  
**Created**: 2026-08-19  
**Status**: Completed  

---

## 1. Test Scenarios & Execution Matrix

### Scenario 1: Adaptive Block Splitter & 3-Way Bit Cost Validation
- **Command**:
  ```bash
  swift test --filter AdaptiveBlockSplitTests
  ```
- **Expected Output**:
  - `testAdaptiveBlockSplit_ObservationSampling_L1Drift`: Passed (Entropy shift correctly triggers block boundary).
  - `testAdaptiveBlockSplit_ThreeWayArbitration`: Passed (Store chosen on random noise, Static chosen on small text, Dynamic chosen on structured data).
  - `testAdaptiveBlockSplit_TailAbsorption`: Passed (Sub-5KB tails absorbed without fragmentation).

### Scenario 2: Zero-Overhead GZIP & ZLIB Container Serialization & Consensus
- **Command**:
  ```bash
  swift test --filter FastContainerStreamTests
  ```
- **Expected Output**:
  - `testGzipContainer_RoundTrip_AndAppleGzipConsensus`: Passed (Bit-for-bit match with macOS `gzip -t` and Apple `libcompression`).
  - `testZlibContainer_RoundTrip_AndAppleZlibConsensus`: Passed (Bit-for-bit match with Apple `libcompression`).
  - `testContainer_ThroughputFloor`: Passed ($\ge 1500$ MB/s Debug, $\ge 2000$ MB/s Release).

### Scenario 3: Closed-Loop Zero-Regression Floor Enforcement
- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  - 13/13 standard performance measure tests pass with 0 regressions.
