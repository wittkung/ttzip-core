# Quickstart: 023-last-mile-zero-regression-and-adaptive-peak-gates

## 1. 验证 WIM 500MB 大文件解压吞吐 ($\ge 10,800\text{ MB/s}$)

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  [▶ WIM 500MB L1 Extract] Throughput: >= 10800 MB/s -> PASS [PERF_OPTIMAL]
  ```
- **Failure Diagnostic**:
  - 检查 `Sources/CTTZipBridge/ttzip_native_archive.c` 中 `.wim` 探测是否生效。
  - 检查解压前是否触发了 `fcntl(fd, F_RDAHEAD, 1)` 与 16KB 页对齐写。

---

## 2. 验证 7Z 100 小文件解压吞吐恢复 ($\ge 1,450\text{ MB/s}$)

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  [▶ 7Z Batch Small Files (10MB/100 files) L1 Extract] Throughput: >= 1450 MB/s -> PASS [PERF_OPTIMAL]
  ```
- **Failure Diagnostic**:
  - 检查 `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c` 中 `last_parent_dir` 与 `mkdir_cache` 是否正确拦截了同目录 `mkdir` 系统调用。

---

## 3. 运行全量零倒退审计门禁

- **Command**:
  ```bash
  python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json
  ```
- **Expected Output**:
  ```text
  ✅ 全格式 262 项测试维度性能审计通过！未发现严重性能倒退 (< -10.0%)。
  ```
- **Failure Diagnostic**:
  - 若退出码为 1，查看 `docs/benchmarks/latest_regression_audit.md` 中的红色阻断项并修复对应格式管道。
