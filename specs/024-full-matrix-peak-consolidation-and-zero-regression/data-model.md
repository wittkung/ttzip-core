# Data Model: 024-full-matrix-peak-consolidation-and-zero-regression

## 1. ConsolidatedPeakPerformanceMatrix

用于固化和跟踪全格式 262 项历史最高纪录的数据实体。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `format` | String | 是 | 归档格式 (如 "zip", "7z", "tar.zst", "dmg", "wim") |
| `dimensionName` | String | 是 | 场景名称 (如 "500MB 大文件数据块 (500MB)") |
| `level` | Integer | 是 | 压缩等级 (如 1, 6) |
| `isEncrypted` | Boolean | 是 | 是否包含 AES 加密 |
| `maxCompressMBs` | Number | 是 | 历史上达到的最高压缩吞吐 (MB/s) |
| `maxExtractMBs` | Number | 是 | 历史上达到的最高解压吞吐 (MB/s) |
| `bestCompressReport` | String | 是 | 取得最高压缩吞吐的报告源文件 |
| `bestExtractReport` | String | 是 | 取得最高解压吞吐的报告源文件 |

## 2. DmgAdaptiveDispatchContext

DMG 密码感知分发上下文。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `archivePath` | String | 是 | DMG 镜像文件物理路径 |
| `destinationDir` | String | 是 | 解压目标目录 |
| `hasPassword` | Boolean | 是 | 是否传入非空密码 |
| `dispatchTarget` | String | 是 | 分发引擎 ("C_DIRECT" 或 "SEVENZIP_AES") |
