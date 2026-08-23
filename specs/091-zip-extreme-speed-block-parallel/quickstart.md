# Quickstart: ZIP Extreme Speed Multi-Core Block-Parallel Mode

## Scenario 1: 单文件 100MB 极速多核压缩验证

### Command
```bash
swift test --filter SoftwareParetoFrontierPkTests
```

### Expected Output
- `TTZip (ZIP Extreme Fast)` 或 `TTZip Extreme` 吞吐达到 **>= 10,000 MB/s**。
- 输出文件经 `/usr/bin/unzip -t` 测试全部通过，CRC32 无损匹配。

### Failure Diagnostic
- 若吞吐未能达到预期，检查 GCD 调度是否受限于单线程 I/O 或 Block Size 设置过小。
