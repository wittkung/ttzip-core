# Quickstart: Testing Decisive Dominance & Zero Regression

```bash
# 1. 执行全格式 92 项竞品 1v1 对决
TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests

# 2. 自动化性能回归审计
python3 scripts/audit_performance_regression.py

# 3. 性能门禁断言
swift test --filter XCTestPerformanceMeasureTests

# 4. 全套单元测试回归
./scripts/run_all_tests.sh
```
