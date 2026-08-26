# Quickstart: Feature 013 Grand Slam Verification

```bash
# 1. 执行全 16 种格式全矩阵压测
TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests

# 2. 执行自动化零性能倒退审计
python3 scripts/audit_performance_regression.py

# 3. 验证 11 大性能硬门禁
swift test --filter XCTestPerformanceMeasureTests
```
