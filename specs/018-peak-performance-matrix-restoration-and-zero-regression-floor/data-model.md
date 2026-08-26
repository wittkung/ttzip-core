# Data Model & Schema Definitions (Feature 018)

**Feature**: Restoration Against Historical Peak Matrix & Hard 10% Floor Invariant  
**Directory**: `specs/018-peak-performance-matrix-restoration-and-zero-regression-floor/`

---

## 1. Entities

### 1.1 `PeakPerformanceEntry`
表示单项格式与场景在历史矩阵中记录的最高吞吐量。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `formatRaw` | `String` | 是 | 归档格式 |
| `dimensionName` | `String` | 是 | 数据集维度 |
| `levelRaw` | `Integer` | 是 | 压缩等级 |
| `isEncrypted` | `Boolean` | 是 | 是否加密 |
| `peakCompressMBs` | `Double` | 是 | 历史最高压缩吞吐 (MB/s) |
| `peakExtractMBs` | `Double` | 是 | 历史最高解压吞吐 (MB/s) |
| `lastUpdated` | `Double` | 是 | 更新时间戳 |

---

### 1.2 `PeakAuditComparison`
表示当前基准与历史最高峰值的偏差核验结果。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `format` | `String` | 是 | 归档格式 |
| `dimensionName` | `String` | 是 | 数据集维度 |
| `level` | `Integer` | 是 | 压缩等级 |
| `isEncrypted` | `Boolean` | 是 | 是否加密 |
| `operation` | `String` | 是 | 操作类型（`"compress"` 或 `"extract"`） |
| `peakMBs` | `Double` | 是 | 历史最高吞吐 (MB/s) |
| `currentMBs` | `Double` | 是 | 当前实测吞吐 (MB/s) |
| `dropPercent` | `Double` | 是 | 偏差百分比 |
| `isCritical` | `Boolean` | 是 | 是否超过 10.0% 严重退化红线 |
