# Quickstart Validation Guide: 031-libarchive-upstream-contributions-evaluation

本指南提供对上游贡献评估结果及各模块原型的端到端验证步骤。

---

## 验证场景 1 (Scenario 1): 验证 libarchive upstream 源码编译与测试套件

- **Command**:
  ```bash
  cd Vendor/libarchive-upstream && mkdir -p build && cd build && cmake .. && make -j8 && ./bin/libarchive_test -r ../libarchive/test test_read_format_7zip_*
  ```
- **Expected Output**:
  ```text
  [100%] Built target libarchive_test
  All 7z tests passed. Total 0 failures.
  ```
- **Failure Diagnostic**:
  若 CMake 找不到 liblzma 或 zstd，检查系统依赖安装：`brew install lzlib xz zstd libb2`。

---

## 验证场景 2 (Scenario 2): 验证 TTZip NEON CRC32 硬件基准与门禁

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
  若吞吐未达标，检查是否运行于 Apple Silicon 物理机以及是否有背景重负载干扰。

---

## 验证场景 3 (Scenario 3): 验证 7z AES-256 加密/解密完整性与全格式回归

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  ```text
  All 46 benchmark pk tests passed with zero regression.
  ```
- **Failure Diagnostic**:
  若发现 $\Delta < -3.0\%$，运行 `python3 scripts/audit_performance_regression.py` 定位具体格式与瓶颈模块。
