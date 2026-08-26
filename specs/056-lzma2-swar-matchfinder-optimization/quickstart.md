# Quickstart & Verification Guide for Feature 056

## Scenario 1: FastLZMA2 核心单元测试验证
* **Command**:
  ```bash
  swift test --filter FastLZMA2Tests
  ```
* **Expected Output**:
  ```
  Test Suite 'FastLZMA2Tests' passed ... 100% tests passed
  ```
* **Failure Diagnostic**:
  * 若块压缩失败，检查 `ttzip_match_len_neon` 返回的长度是否超过 `max_len` 或存在负数越界。
  * 检查 `memcpy` 64-bit load 的指针是否在 `data_size` 边界内。

---

## Scenario 2: 7-Zip 归档桥接与全流程解压验证
* **Command**:
  ```bash
  swift test --filter SevenZipBridgeTests
  ```
* **Expected Output**:
  ```
  Test Suite 'SevenZipBridgeTests' passed ... 100% tests passed
  ```
* **Failure Diagnostic**:
  * 若 7z 写入校验失败，说明 LZMA2 块的 bitstream 出现了 chunk 截断或 Range Coder 概率树状态未正常推进。

---

## Scenario 3: 性能门禁与吞吐底线验证
* **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
* **Expected Output**:
  ```
  7Z Level 1 throughput >= 3,200 MB/s
  7Z Level 5 throughput >= 480 MB/s
  ```
* **Failure Diagnostic**:
  * 若 Level 1 吞吐跌破 3,200 MB/s，检查 `ttzip_is_block_all_zero_neon` 快速旁路是否被意外绕过。
