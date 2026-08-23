# Quickstart: 026-zero-regression-final-mile

## 1. 验证 TAR 10MB 单文件与 7Z 100MB 高熵解压

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  [▶ TAR 10MB L6 Log Extract] Throughput: >= 8654.9 MB/s -> PASS [PERF_OPTIMAL]
  [▶ 7Z 100MB L1 (AES) Extract] Throughput: >= 8171.5 MB/s -> PASS [PERF_OPTIMAL]
  ```
- **Failure Diagnostic**:
  - 检查 `ttzip_tar_native.c` 单文件旁路是否命中，检查 `CTTZipBridge_7zNativeDecoder.c` 256KB 切片是否对齐。

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
