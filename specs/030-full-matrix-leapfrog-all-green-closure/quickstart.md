# Quickstart: 030-full-matrix-leapfrog-all-green-closure

## 1. 运行全格式 46 项全矩阵基准压测

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'AllFormatsPkSuiteTests' passed
  ```

---

## 2. 运行自动化零倒退与大幅超越审计

- **Command**:
  ```bash
  python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json
  ```
- **Expected Output**:
  ```text
  🎉 [AUDIT PASSED] 全格式性能达标，无严重倒退！
  ```
