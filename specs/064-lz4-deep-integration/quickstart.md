# Quickstart: LZ4 Deep Integration Verification

**Feature**: `064-lz4-deep-integration`
**Created**: 2026-08-17

---

## 1. LZ4 Hard Floor Benchmark

```bash
swift test --filter XCTestPerformanceMeasureTests/testLZ4_Compression_ThroughputFloor
```

## 2. All Formats Matrix Regression

```bash
swift test --filter AllFormatsAndAdvancedParametersMatrixTests/testFormat_LZ4
```

## 3. InMemory Benchmark Matrix

```bash
swift test --filter InMemoryBenchmarkSuiteTests
```
