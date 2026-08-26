# Quickstart & Verification Guide: LZ4 Engine and Subsystems

**Feature**: `063-lz4-engine-analysis`
**Created**: 2026-08-17
**Status**: Ready

---

## Scenario 1: LZ4 Roundtrip Correctness and Acceleration Verification

### Command
```bash
swift test --filter Phase123FeatureCoverageTests/testNativeCBridgeZstdAndLZ4Roundtrip
```

### Expected Output
```text
Test Suite 'Phase123FeatureCoverageTests' passed.
Executed 1 test, with 0 failures (0 unexpected) in 0.015 seconds
```

### Failure Diagnostic
- **排查路径**：若测试失败提示解压后数据与原始数据不一致，检查 `Sources/CTTZipBridge/CTTZipStreamCoder.c` 中的 `ttzip_lz4_compress` 与 `ttzip_lz4_decompress` 是否正确传递了缓冲区指针与实际容量，或者在内存越界保护分支中是否有短读/零写情况。

---

## Scenario 2: Hard Throughput Floor Benchmark Verification

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests/testLZ4_Compression_ThroughputFloor
```

### Expected Output
```text
Test Case '-[TTZipTests.XCTestPerformanceMeasureTests testLZ4_Compression_ThroughputFloor]' passed.
Throughput: >= 6000.0 MB/s (Debug) / >= 10000.0 MB/s (Release)
Executed 1 test, with 0 failures in 0.040 seconds
```

### Failure Diagnostic
- **排查路径**：若吞吐跌破 6000 MB/s 底线，排查是否在热循环内引入了 `Data(count:)` 产生内核零填充中断，或检查编译是否禁用了向量化优化。

---

## Scenario 3: Full Format Matrix Regression Suite

### Command
```bash
swift test --filter AllFormatsAndAdvancedParametersMatrixTests/testFormat_LZ4
```

### Expected Output
```text
Test Case '-[TTZipTests.AllFormatsAndAdvancedParametersMatrixTests testFormat_LZ4]' passed.
Executed 1 test, with 0 failures in 0.052 seconds
```

### Failure Diagnostic
- **排查路径**：若创建或解压 `.tar.lz4` 归档失败，检查 `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c` 中 `LZ4F_compressFrame` 与 `Vendor/include/lz4frame.h` 库链接是否正常，或 `ttzip_tar_native.c` 中路径是否正确。
