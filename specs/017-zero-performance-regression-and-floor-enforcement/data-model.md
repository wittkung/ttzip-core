# Data Model & Schema Definitions (Feature 017)

**Feature**: Zero Performance Regression Governance & Hard Floor Invariant Enforcement  
**Directory**: `specs/017-zero-performance-regression-and-floor-enforcement/`

---

## 1. Entities

### 1.1 `BenchmarkRunRecord`
表示单次全格式基准对决运行的性能记录项。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `format` | `String` | 是 | 归档格式标识（如 `"zip"`, `"7z"`, `"tar.zst"`, `"brotli"` 等） |
| `dimensionName` | `String` | 是 | 测试数据集维度（如 `"500MB 大文件数据块"`, `"海量小文件"`, `"高熵物理Payload"` 等） |
| `level` | `Integer` | 是 | 压缩等级（如 `1`, `6`, `9`） |
| `isEncrypted` | `Boolean` | 是 | 是否启用加密（`true` 表示 AES 加密，`false` 表示明文） |
| `ttzipCompressMBs` | `Double` | 是 | TTZip 压缩吞吐量（MB/s） |
| `ttzipExtractMBs` | `Double` | 是 | TTZip 解压吞吐量（MB/s） |
| `compressThroughputMBs` | `Double` | 是 | 官方竞品 CLI 压缩吞吐量（MB/s） |
| `extractThroughputMBs` | `Double` | 是 | 官方竞品 CLI 解压吞吐量（MB/s） |
| `toolName` | `String` | 是 | 对比竞品工具名称（如 `"7-Zip 7zz CLI"`, `"Zstandard zstd"`, `"brotli CLI"` 等） |

---

### 1.2 `RegressionComparisonEntry`
表示新旧两次基准测试运行之间单项维度的比对结果。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `format` | `String` | 是 | 归档格式 |
| `dimensionName` | `String` | 是 | 数据集维度 |
| `level` | `Integer` | 是 | 压缩等级 |
| `isEncrypted` | `Boolean` | 是 | 是否加密 |
| `operation` | `String` | 是 | 操作类型（`"compress"` 或 `"extract"`） |
| `beforeMBs` | `Double` | 是 | 优化前历史吞吐（MB/s） |
| `afterMBs` | `Double` | 是 | 优化后当前吞吐（MB/s） |
| `deltaPercent` | `Double` | 是 | 变动百分比 $\Delta\% = \frac{\text{after} - \text{before}}{\text{before}} \times 100\%$ |
| `severity` | `String` | 是 | 倒退严重级别（枚举：`"PASS"`, `"IMPROVEMENT"`, `"WARNING"`, `"CRITICAL_REGRESSION"`） |

---

### 1.3 `RegressionAuditSummary`
表示全量 284 项指标的审计汇总结果。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `timestamp` | `String` | 是 | 审计执行 ISO 8601 时间戳 |
| `baselineFile` | `String` | 是 | 基准报告文件路径 |
| `latestFile` | `String` | 是 | 最新测试报告文件路径 |
| `totalMatchups` | `Integer` | 是 | 总对决指标数（固定为 284） |
| `improvedCount` | `Integer` | 是 | 提升项数（$\Delta > +3.0\%$） |
| `neutralCount` | `Integer` | 是 | 持平项数（$-3.0\% \le \Delta \le +3.0\%$） |
| `warningCount` | `Integer` | 是 | 轻微倒退告警项数（$-10.0\% \le \Delta < -3.0\%$） |
| `criticalCount` | `Integer` | 是 | 严重倒退阻断项数（$\Delta < -10.0\%$） |
| `exitCode` | `Integer` | 是 | 审计判定退出码（`0` 通过，`1` 阻断） |
