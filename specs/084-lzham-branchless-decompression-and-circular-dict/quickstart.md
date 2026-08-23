# Quickstart & Validation Guide: 084-lzham-branchless-decompression-and-circular-dict

**Feature Directory**: `specs/084-lzham-branchless-decompression-and-circular-dict`  
**Created**: 2026-08-18  
**Status**: Completed  
**Spec Reference**: [`spec.md`](spec.md) | **Plan Reference**: [`plan.md`](plan.md)

---

## 1. Validation Scenarios

### Scenario 1: 11-Bit 哈夫曼单周期查表与 64-Bit 比特流预取单元验证

* **Command**:
  ```bash
  swift test --filter BranchlessDecompTests/testHuffman11BitFastLookup
  ```
* **Expected Output**:
  ```
  Test Case '-[TTZipTests.BranchlessDecompTests testHuffman11BitFastLookup]' passed.
  Executed 1 test, with 0 failures (0 unexpected) in 0.012 (0.012) seconds
  ```
* **Failure Diagnostic**:
  - 若测试失败并报符号解码不匹配：检查 `ttzip_huffman_lut_t` 中高 16 位码长 `len` 与低 16 位 `symbol` 的位排布；检查 `bit_buf` 移位方向是否严格为大端高位对齐。

---

### Scenario 2: $2^N$ 掩码环形字典更新 Fast-Path 与边界回绕 Slow-Path 验证

* **Command**:
  ```bash
  swift test --filter BranchlessDecompTests/testCircularRingDictFastAndSlowPath
  ```
* **Expected Output**:
  ```
  Test Case '-[TTZipTests.BranchlessDecompTests testCircularRingDictFastAndSlowPath]' passed.
  Executed 1 test, with 0 failures (0 unexpected) in 0.015 (0.015) seconds
  ```
* **Failure Diagnostic**:
  - 若在边界跨越处数据发生偏移：检查 `(MAX(src_ofs, dst_ofs) + match_len) > dict_size_mask` 判据是否在 `dst_ofs` 靠近末尾时正确将执行流分流至 Slow-Path；核对 `dict_size_mask = dict_size - 1` 的位与截断逻辑。

---

### Scenario 3: 自重叠与 RLE 单字节重复填充边界安全验证

* **Command**:
  ```bash
  swift test --filter BranchlessDecompTests/testOverlapAndRleMatchCopy
  ```
* **Expected Output**:
  ```
  Test Case '-[TTZipTests.BranchlessDecompTests testOverlapAndRleMatchCopy]' passed.
  Executed 1 test, with 0 failures (0 unexpected) in 0.008 (0.008) seconds
  ```
* **Failure Diagnostic**:
  - 若 `match_dist == 1` 或 `match_dist < match_len` 时输出破坏数据：检查是否在存在数据重叠（Overlap Hazard）的情况下错误调用了非重叠安全宏 `memcpy` 或未经栅障保护的 NEON 批量写入；断言 `match_dist == 1` 时进入特化的 `memset` 或逐步字节循环。

---

### Scenario 4: 全工程性能门禁与零倒退回归校验

* **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
* **Expected Output**:
  ```
  All performance floor gates passed without regressions.
  Executed 15 tests, with 0 failures (0 unexpected).
  ```
* **Failure Diagnostic**:
  - 若任何格式吞吐跌破门禁底线：通过 `git diff` 核查是否有热路径动态内存分配或未内联函数调用违背了热路径零成本抽象铁律。
