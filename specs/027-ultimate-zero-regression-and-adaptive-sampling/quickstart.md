# Quickstart: 027-ultimate-zero-regression-and-adaptive-sampling

## 1. 验证全矩阵自适应微基准与全格式测试

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'AllFormatsPkSuiteTests' passed
  ```

---

## 2. 验证全格式历史最高峰值硬门禁审计

- **Command**:
  ```bash
  python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json
  ```
- **Expected Output**:
  ```text
  🎉 [AUDIT PASSED] 全格式性能达标，无严重倒退！
  ```
