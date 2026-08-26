# Data Model: 020 All-Formats Historical Peak Restoration

**Feature**: 020 All-Formats Historical Peak Restoration  
**Directory**: `specs/020-all-formats-historical-peak-restoration/`  
**Status**: Ready  

---

## 1. Entities & Fields

### `ArchiveCompressionJob`
| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `jobId` | `string (UUID)` | 是 | 归档任务唯一标识符 |
| `format` | `string (enum)` | 是 | `zip`, `7z`, `tar`, `tar.zst`, `tar.gz`, `tar.bz2`, `tar.xz`, `lzip`, `lz4`, `brotli`, `lrzip`, `aar`, `snappy`, `wim`, `dmg`, `iso` |
| `level` | `integer (0..9)` | 是 | 压缩级别 (0: Store, 1: Fastest, 6: Normal, 9: Ultra) |
| `inputPaths` | `array<string>` | 是 | 输入文件或目录绝对路径数组 |
| `outputPath` | `string` | 是 | 目标归档文件绝对路径 |
| `isEncrypted` | `boolean` | 是 | 是否启用密码加密 |
| `password` | `string (nullable)` | 否 | 加密密码 |
| `skipMacJunk` | `boolean` | 是 | 是否过滤 macOS 垃圾文件 (.DS_Store / __MACOSX) |

### `ArchiveExtractionJob`
| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `jobId` | `string (UUID)` | 是 | 解压任务唯一标识符 |
| `archivePath` | `string` | 是 | 输入归档文件绝对路径 |
| `destinationDir` | `string` | 是 | 解压目标文件夹绝对路径 |
| `isEncrypted` | `boolean` | 是 | 归档是否加密 |
| `password` | `string (nullable)` | 否 | 解密密码 |
| `skipMacJunk` | `boolean` | 是 | 是否跳过 macOS 垃圾文件提取 |

### `BenchmarkRegressionEntry`
| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `format` | `string` | 是 | 归档格式名称 |
| `scenario` | `string` | 是 | 测试场景名称 |
| `level` | `integer` | 是 | 压缩级别 |
| `isEncrypted` | `boolean` | 是 | 是否加密 |
| `operation` | `string (enum)` | 是 | `compress` 或 `extract` |
| `peakThroughputMBs` | `number` | 是 | 历史峰值吞吐量 (MB/s) |
| `currentThroughputMBs`| `number` | 是 | 当前实测吞吐量 (MB/s) |
| `changeRatioPercent` | `number` | 是 | 变动百分比 $\Delta\%$ |
| `isRegression` | `boolean` | 是 | 是否判定为严重性能倒退 ($< -10.0\%$) |
