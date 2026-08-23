# Data Model: 032-libarchive-hardware-crc32-acceleration

本数据模型定义 CRC32 硬件加速计算上下文、基准指标与测试配置实体的字段约束。

---

## 1. CRC32ComputationContext (CRC32 计算上下文实体)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `initialCrc` | `Integer` | 是 | 初始 CRC 累加值 (通常为 0) |
| `dataLength` | `Integer` | 是 | 输入数据缓冲区字节长度 (>= 0) |
| `executionPath` | `String` | 是 | 实际命中的执行路径 (`"ARM_ACLE_8WAY"`, `"ARM_ACLE_SCALAR"`, `"PORTABLE_TABLE"`) |
| `alignmentOffset` | `Integer` | 是 | 达到 8 字节对齐前处理的前置字节数 (0..7) |
| `unrolledChunksCount` | `Integer` | 是 | 64 字节展开主循环执行的块数 |
| `trailingBytesCount` | `Integer` | 是 | 剩余尾部单字节处理数 (0..7) |

---

## 2. CRC32BenchmarkMetric (性能基准指标实体)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `bufferSizeBytes` | `Integer` | 是 | 基准测试缓冲区大小 (例如 10485760 字节 = 10MB) |
| `durationSeconds` | `Double` | 是 | 计算耗时 (秒) |
| `throughputMBs` | `Double` | 是 | 实测计算吞吐 (MB/s) |
| `speedupVsBaseline` | `Double` | 是 | 相对原 256 表基准的加速比 (如 20.5x) |
| `isHardwareAccelerated` | `Boolean` | 是 | 是否启用了硬件指令加速 |
