# Data Model: 021-real-physical-benchmark-and-zero-copy-architecture

**Feature**: 021-real-physical-benchmark-and-zero-copy-architecture
**Date**: 2026-08-15
**Status**: Completed

---

## 1. StoreArchiveExecutionOptions (Store 模式归档执行选项)

描述调用 Store 模式归档时的物理写盘与零拷贝配置模型。

| 字段名 | 类型 | 必填 | 默认值 | 描述 |
| :--- | :--- | :--- | :--- | :--- |
| `outputPath` | `string` | 是 | - | 目标归档文件绝对路径 |
| `inputPaths` | `string[]` | 是 | - | 待归档的源文件或目录路径列表 |
| `skipMacJunk` | `boolean` | 是 | `true` | 是否跳过 macOS 系统垃圾文件（.DS_Store 等） |
| `enableZeroCopy` | `boolean` | 是 | `false` | 是否尝试 APFS 零拷贝（Extent Clone） |
| `chunkSizeBytes` | `integer` | 是 | `4194304` | 物理并发写盘时的分块大小（字节，默认 4MB） |

---

## 2. BenchmarkPhysicalMetricRecord (基准测试纯物理度量模型)

描述基准测试与性能门禁运行过程中捕获的真实物理性能度量模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `scenarioName` | `string` | 是 | 测试场景名称（如 ZIP Store Direct I/O） |
| `payloadBytes` | `integer` | 是 | 原始测试载荷大小（字节） |
| `elapsedSeconds` | `number` | 是 | 纯编解码与落盘执行耗时（秒） |
| `throughputMBs` | `number` | 是 | 物理实测吞吐速率（MB/s） |
| `isZeroCopyUsed` | `boolean` | 是 | 该次测试是否使用了 APFS 零拷贝（门禁中必须恒为 false） |
| `isPassed` | `boolean` | 是 | 是否达到物理门禁阈值 |
| `gateFloorMBs` | `number` | 是 | 该场景设定的硬门禁吞吐底线（MB/s） |

---

## 3. ParsedEocdRecord (ZIP EOCD 解析防御性实体模型)

描述 C 语言解析器从 ZIP 文件末端提取的 EOCD 关键记录。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `totalEntries` | `integer` | 是 | 归档中条目总数 |
| `cdSize` | `integer` | 是 | Central Directory 大小（字节） |
| `cdOffset` | `integer` | 是 | Central Directory 起始偏移（字节） |
| `isZip64` | `boolean` | 是 | 是否存在有效的 Zip64 扩展定位器 |
| `zip64EocdOffset` | `integer` | 是 | Zip64 EOCD 记录偏移（若无则为 0） |

---

## 4. AllFormatsRegressionAuditRecord (全格式自动化性能零倒退审计模型)

描述跨全部 16 种格式与全维度场景的自动化性能回归比对模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `format` | `string` | 是 | 归档格式标识符（如 `zip`, `7z`, `tar.zst`, `wim` 等） |
| `scenario` | `string` | 是 | 测试载荷场景描述（如 500MB 大文件数据块、拟真日志文本等） |
| `level` | `string` | 是 | 压缩级别枚举（`L1`, `L6`, `Store`） |
| `isEncrypted` | `boolean` | 是 | 是否启用加密（AES-256） |
| `operation` | `string` | 是 | 操作类型（`compress`, `extract`） |
| `baselineMBs` | `number` | 是 | 历史基准吞吐速率（MB/s） |
| `currentMBs` | `number` | 是 | 当前实测吞吐速率（MB/s） |
| `deltaPercent` | `number` | 是 | 相对基准的增益或倒退百分比 |
| `status` | `string` | 是 | 状态分级（`gain`, `flat`, `warning`, `critical_regression`） |

