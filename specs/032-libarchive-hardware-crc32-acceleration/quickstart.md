# Quickstart Validation Guide: 032-libarchive-hardware-crc32-acceleration

本指南提供对 `archive_crc32.h` 硬件加速改造的验证步骤。

---

## 验证场景 1 (Scenario 1): Upstream CMake 构建与单元测试

- **Command**:
  ```bash
  cd Vendor/libarchive-upstream && mkdir -p build && cd build && cmake .. && make -j8 && ./bin/libarchive_test -r ../libarchive/test test_archive_string_conversion
  ```
- **Expected Output**:
  ```text
  [100%] Built target libarchive_test
  All tests passed. Total 0 failures.
  ```
- **Failure Diagnostic**:
  若编译失败，检查编译器是否支持 `<arm_acle.h>` 或预处理器宏条件保护。

---

## 验证场景 2 (Scenario 2): TTZip 全量单元测试与性能门禁

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'XCTestPerformanceMeasureTests' passed.
  Executed 12 tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  若测试失败，检查 C 桥接层与头文件是否发生符号冲突。

---

## 验证场景 3 (Scenario 3): 全格式 46 场景基准测试

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  All 46 benchmark pk tests passed with zero regression.
  ```
- **Failure Diagnostic**:
  若发现性能倒退，运行 `python3 scripts/audit_performance_regression.py` 进行逐项分析。
