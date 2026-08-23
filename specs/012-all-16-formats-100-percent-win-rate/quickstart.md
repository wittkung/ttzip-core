# Quickstart: 100% Win Rate Verification

```bash
# 1. 执行全 16 种格式基准压测
TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests

# 2. 运行零性能倒退审计
python3 scripts/audit_performance_regression.py

# 3. 运行 11 大性能硬门禁
swift test --filter XCTestPerformanceMeasureTests
```
