# Data Model: 025-short-sample-stabilization-and-full-peak-clearing

## 1. BenchmarkSamplingConfig

自适应基准采样配置模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `payloadBytes` | Int64 | 是 | 测试数据集字节大小 |
| `warmupPasses` | Int | 是 | 预热轮次（短时负载为 1，大负载为 0） |
| `measuredPasses` | Int | 是 | 正式计时采样轮次（短时负载为 3，大负载由参数指定） |
| `minDurationFloorSeconds` | Double | 是 | 最小耗时下限保护值（`1e-6`，即 1 微秒） |

## 2. BenchmarkMultiPassResult

多轮测试统计聚合模型。

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `minDuration` | Double | 是 | 最优耗时（秒），用于计算最高峰值吞吐 (Peak MB/s) |
| `medianDuration` | Double | 是 | 中位数耗时（秒），用于稳态参考 |
| `allDurations` | Array<Double> | 是 | 包含全部采样的原始耗时数组 |
