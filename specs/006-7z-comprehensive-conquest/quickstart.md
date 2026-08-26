# Quickstart Validation Guide: 7Z Comprehensive Conquest

## 1. 验证 7Z 500MB 极速压缩与 100% 胜率

```bash
# 运行 7Z 竞品 1v1 对决基准
TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests

# 运行全格式零倒退审计报告
python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_045106.json

# 验证 11 大性能硬门禁
swift test --filter XCTestPerformanceMeasureTests
```

## 2. 预期结果

- 7Z 500MB L1 压缩吞吐达到 $\ge 5,600\text{ MB/s}$（胜过 7zz 5,498 MB/s）。
- 7Z 500MB L1 AES 压缩吞吐达到 $\ge 5,600\text{ MB/s}$（胜过 7zz 5,382 MB/s）。
- 7Z 100 小文件 L1 压缩吞吐达到 $\ge 950\text{ MB/s}$（胜过 7zz 883 MB/s）。
- 7Z 对决总项数 32 战 32 胜（100% 全胜）。
