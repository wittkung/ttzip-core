# Quickstart: 大师级系统工程与 AI 调度验证指南 (Craftsmanship Engineering Quickstart)

**Feature Branch**: `042-craftsmanship-engineering-and-orchestration`  
**Feature Directory**: `specs/042-craftsmanship-engineering-and-orchestration`  
**Created**: 2026-08-17  
**Status**: Active  

---

## 1. 场景一：零告警与跨平台 C/Swift 编译构建验证

### 1.1 Command
```bash
swift build -c release
```

### 1.2 Expected Output
- 编译通过，输出 `Build complete!`，0 warning, 0 error。
- `ttzip-cli` 与 `TTZipApp` 成功生成于 `.build/release/` 目录。

### 1.3 Failure Diagnostic
- 若出现 `warning: capture of non-Sendable type in Sendable closure`：排查 Swift 6 并发闭包中的原始指针捕获，将指针转为 `UInt` 位模式传递。
- 若出现 `warning: implicit conversion loses integer precision`：排查 64 位整数向 `size_t` 转换处是否缺失 `SSIZE_MAX` Clamp。

---

## 2. 场景二：性能硬门禁全矩阵达标验证 (XCTestPerformanceMeasureTests)

### 2.1 Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### 2.2 Expected Output
- 全部 13 项性能门禁测试用例 100% Passed：
  - `testZipStore_HugeFile_XCTestMeasureMetrics` $\ge 6500.0\text{ MB/s}$ (Debug) / $\ge 7500.0\text{ MB/s}$ (Release)
  - `testTarZstdDirect_50MB_ThroughputFloor` $\ge 13500.0\text{ MB/s}$ (Debug) / $\ge 19000.0\text{ MB/s}$ (Release)
  - `testZipDecompression_ThroughputFloor` $\ge 6500.0\text{ MB/s}$ (Debug) / $\ge 8800.0\text{ MB/s}$ (Release)
  - `testSevenZipDecompression_ThroughputFloor` $\ge 6500.0\text{ MB/s}$ (Debug) / $\ge 6800.0\text{ MB/s}$ (Release)

### 2.3 Failure Diagnostic
- 若 `testZipStore_HugeFile_XCTestMeasureMetrics` 吞吐低于 6500 MB/s：排查是否误加了 `msync(MS_SYNC)` 同步刷盘，确保使用并发 16MB `pwrite` 与 APFS Extent 克隆。

---

## 3. 场景三：客观测试预言机与全量回归验证

### 3.1 Command
```bash
swift test
```

### 3.2 Expected Output
- 输出 `Test Suite 'All tests' passed`，620 项测试全部通过（0 失败、0 错误）。
- Golden Corpus 5 组 `.uu` 缺陷样本完成解码与解压验证。
- 系统 `/usr/bin/tar` 双向差分测试完成互操作性断言。

### 3.3 Failure Diagnostic
- 若 `ArchiveGoldenCorpusTests` 失败：检查 `UUDecoder.swift` 是否正确过滤头部 metadata 行。
- 若 `SystemDifferentialTests` 失败：排查 TAR 512 字节边界填充与 POSIX `ustar` 头部魔数对齐。
