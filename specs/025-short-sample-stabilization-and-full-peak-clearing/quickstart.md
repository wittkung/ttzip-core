# Quickstart: 025-short-sample-stabilization-and-full-peak-clearing

## 1. 验证短时微基准多轮自适应采样与峰值稳定性

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  [▶ ZIP 10MB L1 (AES) Log Compress/Extract] Throughput: >= Peak Floor -> PASS [PERF_OPTIMAL]
  ```
- **Failure Diagnostic**:
  - 检查 `CompetitorBenchmarkRunner.swift` 中短时负载是否正确执行了预热与 3 轮采样取最佳耗时。

---

## 2. 验证全量 262 项历史最高峰值门禁审计

- **Command**:
  ```bash
  python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json
  ```
- **Expected Output**:
  ```text
  🎉 [AUDIT PASSED] 全格式性能达标，无严重倒退！
  ```
- **Failure Diagnostic**:
  - 查看 `docs/benchmarks/latest_regression_audit.md` 中的逐项差异明细。
