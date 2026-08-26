# Data Model: 023-last-mile-zero-regression-and-adaptive-peak-gates

## 1. RegressionClosureSummary

记录本次倒退收敛与门禁覆盖的核心实体。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `auditTimestamp` | String (ISO-8601) | 是 | 审计时间戳 |
| `baselineReport` | String | 是 | 历史最优基准报告路径 |
| `latestReport` | String | 是 | 最新测试报告路径 |
| `totalDimensionsEvaluated` | Integer | 是 | 评估的总细分维度数（262） |
| `improvedCount` | Integer | 是 | 性能提升项数（$\Delta > +3.0\%$） |
| `unchangedCount` | Integer | 是 | 性能持平项数（$\pm 3.0\%$ 以内） |
| `warningCount` | Integer | 是 | 轻微波动告警项数（$-3.0\% \sim -10.0\%$） |
| `criticalRegressionCount` | Integer | 是 | 严重倒退阻断项数（$\Delta < -10.0\%$） |
| `criticalRegressions` | Array<CriticalRegressionItem> | 是 | 严重倒退明细列表 |

## 2. CriticalRegressionItem

严重性能倒退细分项。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `format` | String | 是 | 归档格式（如 "wim", "dmg", "7z"） |
| `dimension` | String | 是 | 场景名称（如 "500MB 大文件数据块 (500MB)"） |
| `level` | Integer | 是 | 压缩等级（如 1, 6） |
| `isEncrypted` | Boolean | 是 | 是否加密 |
| `operation` | String | 是 | 操作类型（"compress" 或 "extract"） |
| `baselineThroughputMBs` | Number | 是 | 历史最优吞吐（MB/s） |
| `currentThroughputMBs` | Number | 是 | 当前实测吞吐（MB/s） |
| `regressionPercent` | Number | 是 | 变动百分比（负数） |
| `status` | String | 是 | 状态（"RESOLVED" 或 "BLOCKED"） |

## 3. InlineMkdirCacheState

7Z C 引擎写盘热循环中的栈上内联目录缓存结构。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `lastParentDir` | String | 是 | L1 单槽热缓存路径（1024 字节字符数组） |
| `slotCount` | Integer | 是 | L2 哈希槽位数量（固定 64 槽） |
| `hitCount` | Integer | 是 | 缓存命中系统调用减免次数 |
