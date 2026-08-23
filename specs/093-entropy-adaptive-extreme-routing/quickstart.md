# Quickstart: Entropy-Adaptive Intelligent Extreme Routing

## Scenario 1: 运行自适应熵分流测试

### Command
```bash
swift test --filter EntropyAdaptiveExtremeRoutingTests
```

### Expected Output
- 低熵数据自动路由至 Method 8，高熵数据（随机数/加密流）自动路由至 Method 0。
- 生成的 ZIP 归档通过 `/usr/bin/unzip -t` 检验。
