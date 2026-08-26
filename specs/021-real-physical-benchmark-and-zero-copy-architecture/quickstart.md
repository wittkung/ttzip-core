# Quickstart & Verification Guide: 021-real-physical-benchmark-and-zero-copy-architecture

**Feature**: 021-real-physical-benchmark-and-zero-copy-architecture
**Date**: 2026-08-15
**Status**: Completed

---

## 1. 验证 C 语言解析器规范安全性与逆向 EOCD 扫描

### Command
```bash
swift test --filter ArchiveSpecIntegrityTests
```

### Expected Output
```text
Test Suite 'ArchiveSpecIntegrityTests' passed
Executed 3 tests, with 0 failures
```

### Failure Diagnostic
- **排查路径**：查看 `Sources/CTTZipBridge/CTTZipParser.c` 中的 `ttzip_find_eocd` 实现。
- **常见原因**：若再次出现跳步加载或未对齐指针解引用导致 EOCD 未命中，检查回溯搜索窗口 `search_back` 是否包含完整的 65,557 字节。

---

## 2. 验证 11 项纯物理性能硬门禁 (Release 模式)

### Command
```bash
swift test -c release --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Suite 'XCTestPerformanceMeasureTests' passed
Executed 11 tests, with 0 failures
```

### Failure Diagnostic
- **排查路径**：查看 `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift` 中各场景实测数据。
- **常见原因**：
  - 若 `testZipStore_HugeFile_XCTestMeasureMetrics` 低于 5000 MB/s，检查 `ZipStoreStreamWriter.swift` 是否正常走 4MB 并发 `pwrite` 分块通道。
  - 若 `testSevenZipCompression_Level1_ThroughputFloor` 异常，检查 `ttzip_lzma2_fast_encoder.c` 中 level 1 参数配置。

---

## 3. 验证全量 591+ 单元测试回归全绿

### Command
```bash
swift test
```

### Expected Output
```text
Executed 591 tests, with 0 failures
```

### Failure Diagnostic
- **排查路径**：若有单测失败，定位具体测试用例文件及对应模块，检查 Swift 6 并发安全或数据模型一致性。

---

## 4. 验证全格式 46 维自动化性能零倒退审计

### Command
```bash
python3 scripts/audit_performance_regression.py
```

### Expected Output
```text
🔴 严重性能倒退阻断明细 (< -10.0%): 0 项
[AUDIT PASSED] 零严重性能倒退！
```

### Failure Diagnostic
- **排查路径**：查看 `docs/benchmarks/latest_regression_audit.md` 中的红色阻断项列表。
- **常见原因**：若存在 >10% 倒退，检查对应格式的编解码 Fast-Path 是否被中转降级或存在多核锁竞争。

