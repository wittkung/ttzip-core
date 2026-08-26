# Quickstart & Verification Guide for Feature 057

## Scenario 1: SWAR 优化专属性能基准测试
* **Command**:
  ```bash
  swift test --filter SwarOptimizationBenchmarkTests
  ```
* **Expected Output**:
  ```
  Test Suite 'SwarOptimizationBenchmarkTests' passed
  ASCII Scan Throughput: > 3,000 MB/s
  Header Sniffing Ops: > 50,000,000 ops/s
  ```
* **Failure Diagnostic**:
  * 检查 `memcpy` 64-bit 边界是否正确加了 `i + 8 <= len` 防护。

---

## Scenario 2: 字符集与格式检测回归测试
* **Command**:
  ```bash
  swift test --filter CharsetDetectorTests
  swift test --filter FormatSupportTests
  ```
* **Expected Output**:
  ```
  All tests passed with 0 failures
  ```
* **Failure Diagnostic**:
  * 检查 GB18030 / UTF-8 特殊多字节字符的解析逻辑是否产生漏判。
