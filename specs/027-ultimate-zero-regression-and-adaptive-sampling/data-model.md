# Data Model: 027-ultimate-zero-regression-and-adaptive-sampling

## 1. AdaptiveBenchmarkRunnerState

自适应微基准采样与生命周期状态模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `payloadBytes` | Int64 | 是 | 测试数据集字节大小 |
| `effectivePassCount` | Int | 是 | 计算后的有效采样轮次（短时负载 $\le 15\text{MB}$ 时为 $\ge 3$） |
| `bestCompressSeconds` | Double | 是 | 最小压缩耗时（下限 $10^{-6}\text{s}$） |
| `bestExtractSeconds` | Double | 是 | 最小解压耗时（下限 $10^{-6}\text{s}$） |

## 2. SevenZipEntropySamplingContext

7Z 高熵快速旁路上下文模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `bufferSize` | Integer | 是 | 待压缩缓冲区大小 |
| `entropyValue` | Number | 是 | 9 点分布式抽样计算的香农熵值（0.0 ~ 8.0） |
| `isHighEntropyBypass` | Boolean | 是 | 是否触发 Store / Copy 模式直通 |
