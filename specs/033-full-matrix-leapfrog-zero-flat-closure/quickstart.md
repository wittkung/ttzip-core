# Quickstart & Verification: Feature 033

**Feature**: 全矩阵清零持平、波动与倒退并全面大幅跃升 (Feature 033)

---

## 1. 验证场景 1：全量单元回归与安全门禁

- **Command**:
  ```bash
  ./scripts/run_all_tests.sh
  ```
- **Expected Output**:
  ```
  ==========================================
  ✅ ALL TEST SUITES PASSED CLEANLY!
  ==========================================
  ```
- **Failure Diagnostic**:
  - 若有单测失败，检查 `TTLogger` 环形缓冲输出或是否有 C 指针空引用。

---

## 2. 验证场景 2：全格式 16 种格式 1v1 PK 基准测试

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```
  Test Suite 'AllFormatsPkSuiteTests' passed
  Executed 1 test, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - 若某格式解压失败，检查 `dispatchFastExtraction` 路由是否覆盖该文件扩展名。

---

## 3. 验证场景 3：全自动性能倒退清零与大幅超越审计

- **Command**:
  ```bash
  python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_071939.json
  ```
- **Expected Output**:
  ```
  🔴 严重倒退 (< -10%): 0 (0.0%)
  🟡 波动项 (-3% ~ -10%): 0 (0.0%)
  ```
- **Failure Diagnostic**:
  - 若有严重倒退项，检查 `ttzip_create_tar_native_c` 是否触发了 `fork()` 管道降级。
