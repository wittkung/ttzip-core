# Quickstart & Verification Guide: Feature 102 (ZIP Transparent Profiles & Performance Gates)

## 1. 验证场景 1: ZIP 全量性能门禁测试 (XCTestPerformanceMeasureTests)
- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  - `ZIP Level 1 Compression` 吞吐 $\ge 1500\text{ MB/s}$ (Debug) / $\ge 2000\text{ MB/s}$ (Release)
  - `ZIP Level 6 Compression` 吞吐 $\ge 1100\text{ MB/s}$ (Debug) / $\ge 1350\text{ MB/s}$ (Release)
  - `ZIP Decompression` 吞吐 $\ge 7500\text{ MB/s}$ (Debug) / $\ge 10000\text{ MB/s}$ (Release)
  - 退出码 `exit code 0`，0 failures, 0 unexpected。
- **Failure Diagnostic**:
  - 若吞吐跌破门禁，检查 `ZipExtremeBlockWriter.swift` 是否引入了额外内存拷贝或锁阻塞。

## 2. 验证场景 2: 18 核心帕累托黄金档位对决与严格单调性测试 (ZipMultiCoreParetoFrontierPkTests)
- **Command**:
  ```bash
  swift test --filter ZipMultiCoreParetoFrontierPkTests
  ```
- **Expected Output**:
  - 8 大黄金档位（Tier 0..7）物理实测通过；
  - 压缩后体积单调递减：Tier 0 (100MB) > Tier 1 (~3.5MB) > Tier 2 (~3.35MB) > Tier 3 (~3.35MB) > Tier 4 (~3.19MB) > Tier 5 (~3.10MB) > Tier 6 (~2.99MB) > Tier 7 (~2.95MB)；
  - 图表工件物理落盘：`pareto_pk_zip_multicore.png`。
- **Failure Diagnostic**:
  - 若出现重叠点，核对 `ZipCompressionProfile.forLevel(level)` 是否发生重复映射。
