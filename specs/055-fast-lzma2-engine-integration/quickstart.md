# Quickstart: Fast-LZMA2 Multi-Threaded Engine Integration

**Feature Directory**: `specs/055-fast-lzma2-engine-integration`

**Created**: 2026-08-17

---

## Scenario 1: Fast-LZMA2 基础单元测试与差分解压验证

验证 Fast-LZMA2 模块的基本块压缩、多线程调度及与系统解压器的双向差分正确性。

- **Command**:
  ```bash
  swift test --filter FastLZMA2Tests
  ```
- **Expected Output**:
  ```text
  Test Suite 'FastLZMA2Tests' passed at ...
  Executed X tests, with 0 failures (0 unexpected) in ... seconds
  ```
- **Failure Diagnostic**:
  - 若提示 C 符号未定义（如 `_ttzip_fl2_compress_block`）：检查 [Sources/CTTZipBridge/include/ttzip_fl2_lzma2.h](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_fl2_lzma2.h) 是否已正确在 [module.modulemap](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/module.modulemap) 中导出。
  - 若差分解压失败：检查输出 LZMA2 块首字节的 DictSize 编码（`dict_size` 映射至 0~40 阶数）与 Range Coder 的 Flush 对齐。

---

## Scenario 2: 7Z Level 5 高压缩等级性能门禁验证

验证在 Level 5 高压缩等级下，Fast-LZMA2 能够将吞吐提升至 $\ge 800\text{MB/s}$ (Debug) / $\ge 1200\text{MB/s}$ (Release)，达成 SC-001。

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests/test7zLevel5HighCompressionThroughput
  ```
- **Expected Output**:
  ```text
  Test Case '-[TTZipTests.XCTestPerformanceMeasureTests test7zLevel5HighCompressionThroughput]' passed (Throughput: >= 800.0 MB/s).
  ```
- **Failure Diagnostic**:
  - 若吞吐低于 800 MB/s：排查 [Sources/CTTZipBridge/ttzip_fl2_bridge.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_fl2_bridge.c) 中线程池是否成功绑定全部 P-Core（`get_p_core_count()`），或检查是否意外退化为传统 liblzma 串行路径。
  - 若发生 E-Core 线程拖尾：检查 Radix 匹配查找器工作窃取队列分块尺寸是否过大（应保持 2MB~8MB 微分块）。

---

## Scenario 3: 7Z Level 1 极速模式零倒退与 NEON Fast-Path 门禁验证

验证 Level 1 压缩依然命中自研 NEON Fast-Path，吞吐维持在 $\ge 3,200\text{MB/s}$，达成 SC-002。

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests/test7zLevel1CompressionSpeed
  ```
- **Expected Output**:
  ```text
  Test Case '-[TTZipTests.XCTestPerformanceMeasureTests test7zLevel1CompressionSpeed]' passed (Throughput: >= 3200.0 MB/s).
  ```
- **Failure Diagnostic**:
  - 若 Level 1 性能跌破 3,200 MB/s：检查路由分发器（`SevenZipLZMA2HybridStrategy`）是否在 `level == 1` 时错误调用了 Fast-LZMA2 默认引擎而非自研 NEON 直通路径。

---

## Scenario 4: 全格式 46 项全矩阵回归与零倒退审计

验证 Fast-LZMA2 引入后，全库所有 16 种归档格式无任何功能或性能倒退。

- **Command**:
  ```bash
  TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests
  python3 scripts/audit_performance_regression.py
  ```
- **Expected Output**:
  ```text
  [PASS] 0 regressions detected across all 46 benchmark scenarios.
  ```
- **Failure Diagnostic**:
  - 若报告倒退项：查阅生成的 JSON 审计报告，比对与基准 `604d44d` 的绝对吞吐差值，排查修改是否污染了热路径全局锁或引入了中间堆内存分配。
