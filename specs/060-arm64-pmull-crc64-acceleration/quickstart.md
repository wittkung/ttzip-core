# Quickstart Validation Guide: 060-arm64-pmull-crc64-acceleration

**Feature Name**: ARM64 PMULL 硬件级 CRC64 (ECMA-182) 加速引擎接入  
**Status**: Ready  
**Created**: 2026-08-17  
**Parent Plan**: [plan.md](./plan.md)

---

## 1. 验证场景一：黄金测试向量与数学精度核验

### Command
```bash
swift test --filter CRC64HardwareTests.testGoldenVectorAndDifferential
```

### Expected Output
```text
Test Suite 'CRC64HardwareTests' passed.
Executed 1 test, with 0 failures (0 unexpected) in 0.005 (0.005) seconds
```

### Failure Diagnostic
- 若出现 `0x6C40DF5F0B497347` 匹配失败：检查 `fold512`、`fold128`、`mu_p` 常量是否反转，检查尾部 $1 \sim 15$ 字节掩码 `vmasks_64` 查表偏移量是否正确对齐。

---

## 2. 验证场景二：0~256 字节穷举差分与非对齐内存测试

### Command
```bash
swift test --filter CRC64HardwareTests.testExhaustiveDifferentialAndUnaligned
```

### Expected Output
```text
Test Suite 'CRC64HardwareTests' passed.
Executed 1 test, with 0 failures (0 unexpected) in 0.012 (0.012) seconds
```

### Failure Diagnostic
- 若在小于 16 字节或小于 8 字节处发生断言错误：检查 `ttzip_crc64.c` 中 `size < 8` 与 `size < 16` 的分支合并逻辑与 `memcpy` 字节序。

---

## 3. 验证场景三：10MB 硬件吞吐门禁压测

### Command
```bash
swift test --filter CRC64HardwareTests.testThroughputPerformanceFloor
```

### Expected Output
```text
[CRC64 Hardware PMULL] Measured Throughput: > 30000 MB/s (Target: >= 30,000 MB/s)
Test Suite 'CRC64HardwareTests' passed.
```

### Failure Diagnostic
- 若吞吐低于 30,000 MB/s：检查是否开启了 Debug 额外检查、是否遗漏了 4 路 64 字节向量折叠主循环，或在内层循环引入了栈溢出/内存拷贝。

---

## 4. 验证场景四：全量性能门禁与系统回归

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
All performance measure tests executed and passed without throughput degradation.
```

### Failure Diagnostic
- 若其他格式性能受影响：检查 `ttzip_crc64` 是否误改动了冻结文件或公共热路径宏定义。
