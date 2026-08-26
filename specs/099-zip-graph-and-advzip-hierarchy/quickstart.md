# Quickstart Validation Guide: ZIP 7-Tier Graph & Advzip Conquest

**Feature Directory**: `specs/099-zip-graph-and-advzip-hierarchy`  
**Created**: 2026-08-18  

---

## 1. 验证场景一：7 大黄金梯队端到端归档与解压校验

### Command
```bash
swift test --filter ZipMultiCoreParetoFrontierPkTests
```

### Expected Output
```text
Test Case '-[TTZipTests.ZipMultiCoreParetoFrontierPkTests testZipMultiCoreParetoFrontier]' passed
🏆 纯 ZIP 格式 18 核心满载极限对决图表已生成: pareto_pk_zip_multicore.png
```

### Success Assertion
- 生成的 `pareto_pk_zip_multicore.png` 横坐标为 **`压缩后大小 (MB)`**；
- 包含完整的 7 个黄金档位（Level 1 至 Level 7）；
- Level 5 吞吐在 150~400 MB/s 区间，大小 ~3.15 MB；
- Level 7 大小 $\le 2.99\text{ MB}$（超越或持平 `advzip -4`）。

---

## 2. 验证场景二：全量回归与硬性能门禁验证

### Command
```bash
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Executed all performance tests with 0 failures
```

### Failure Diagnostic
- 若 Level 1 吞吐 $< 1500\text{ MB/s}$，检查 `ZipExtremeBlockWriter.swift` 是否引入多余锁竞争；
- 若 Level 7 空间节省率 $< 97.01\%$，检查 32KB 跨块字典注入与迭代轮次设置。
