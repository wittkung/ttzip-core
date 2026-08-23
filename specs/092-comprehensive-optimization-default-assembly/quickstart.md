# Quickstart Validation Guide: Comprehensive Optimization Default Assembly

**Feature**: `specs/092-comprehensive-optimization-default-assembly`  
**Date**: 2026-08-18  

---

## Scenario 1: Transparent Adaptive Store Downgrade for High-Entropy Files

### Purpose
Verify that compressing high-entropy files (e.g. pre-compressed archives or encrypted binaries) automatically executes in Direct Store mode with $< 5\,\mu\text{s}$ probe overhead, 0 volume expansion, and $> 5,000\,\text{MB/s}$ throughput.

### Command
```bash
swift test --filter TransparentAdaptivePipelineTests/testHighEntropyStoreAutoDowngrade
```

### Expected Output
```text
Test Case '-[TTZipTests.TransparentAdaptivePipelineTests testHighEntropyStoreAutoDowngrade]' passed (0.005 seconds).
```

---

## Scenario 2: Scientific Float32 Detection and Transparent Bit-Grooming

### Purpose
Verify that floating-point data streams are automatically detected via Stride Autocorrelation ($R(4) \ge 0.70$) and exponent variance ($\sigma_E \le 16$), scaling compression ratio by $> 2.5\times$.

### Command
```bash
swift test --filter TransparentAdaptivePipelineTests/testScientificFloatAutoDetectionAndBitGrooming
```

### Expected Output
```text
Test Case '-[TTZipTests.TransparentAdaptivePipelineTests testScientificFloatAutoDetectionAndBitGrooming]' passed (0.008 seconds).
```

---

## Scenario 3: Multi-Modal Competitor Benchmark Matrix Execution

### Purpose
Execute `CompetitorBenchmarkRunner` with multi-modal datasets (Float32, High-Entropy, Sparse, JSON) and verify 100% round-trip integrity and performance superiority across formats.

### Command
```bash
swift test --filter CompetitorMultiModalBenchmarkTests
```

### Expected Output
```text
Test Case '-[TTZipTests.CompetitorMultiModalBenchmarkTests testMultiModalDatasetBenchmarkIntegration]' passed (0.250 seconds).
```
