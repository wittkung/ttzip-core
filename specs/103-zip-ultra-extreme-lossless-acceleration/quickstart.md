# Quickstart & Verification Guide: Feature 103 (ZIP Tier 6/7 Lossless Acceleration)

## 1. 验证场景 1: 100MB enwik8 压缩体积零损失验证
- **Command**:
  ```bash
  swift test --filter ZipExtremeBlockWriterTests
  ```
- **Expected Output**:
  - `enwik8` (100MB) 在 Tier 6 下输出体积 $\le 2,994,000$ 字节，Tier 7 下输出体积 $\le 2,958,000$ 字节；
  - 原生 `/usr/bin/unzip -t` 校验通过，0 CRC errors。
- **Failure Diagnostic**:
  - 若体积膨胀，检查 32KB 跨块滑动字典注入（`history_size`）是否生效。

## 2. 验证场景 2: 帕累托 18 核心性能门禁实测
- **Command**:
  ```bash
  TTZIP_FORCE_BENCH_RERUN=1 swift test --filter ZipMultiCoreParetoFrontierPkTests
  ```
- **Expected Output**:
  - Tier 6 吞吐 $\ge 8.0\text{ MB/s}$；
  - Tier 7 吞吐 $\ge 0.8\text{ MB/s}$；
  - 图表工件更新：`pareto_pk_zip_multicore.png`。
- **Failure Diagnostic**:
  - 若吞吐未达预期，检查分块是否落入 2MB Tile 区间及 L2 缓存命中情况。
