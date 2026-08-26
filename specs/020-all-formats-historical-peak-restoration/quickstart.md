# Quickstart & Verification Guide: 020 All-Formats Historical Peak Restoration

**Feature**: 020 All-Formats Historical Peak Restoration  
**Directory**: `specs/020-all-formats-historical-peak-restoration/`  
**Status**: Ready  

---

## 1. 验证场景一：500MB 大文件 7z 压缩性能验证

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests
  ```
- **Expected Output**:
  - `[7z] 500MB 大文件数据块 (500MB) L1 (无) 压缩`: 吞吐量 $\ge 18,000$ MB/s。
  - 零错误，零断言失败。
- **Failure Diagnostic**:
  - 检查 `ttzip_lzma2_compress_block_tuned` 中的 `is_zero_block` 分支是否直通 `encode_zero_chunk_2mb`；
  - 确认单大文件使用了 `mmap` 与 `writev` 单次落盘。

---

## 2. 验证场景二：全格式 11 项吞吐硬门禁回归测试

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  - `Executed 11 tests, with 0 failures (0 unexpected)`.
  - 所有 11 项场景（TAR.ZST Direct, ZIP Decompression, ZIP Store Direct, 7Z Decompression 等）全部输出 `PASS [PERF_ACCEPTABLE]` 或 `PASS [PERF_OPTIMAL]`。
- **Failure Diagnostic**:
  - 检查是否在热路径中误引入了 `ArchiveValidationPipeline` 耗时校验或动态对象树分配。

---

## 3. 验证场景三：全格式自动化零倒退审计

- **Command**:
  ```bash
  python3 scripts/audit_performance_regression.py docs/benchmarks/peak_performance_matrix.json $(ls -t docs/benchmarks/benchmark_report_*.json | head -n 1)
  ```
- **Expected Output**:
  - 退出码为 0，终端输出 `[AUDIT PASSED] 全格式基准无严重性能倒退 (< -10.0%)`。
- **Failure Diagnostic**:
  - 查阅 `docs/benchmarks/latest_regression_audit.md` 定位红灯场景，检查对应格式的 C 桥接层 Fast-Path 分发配置。
