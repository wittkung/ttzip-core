# Data Model: 022-full-matrix-zero-regression-and-throughput-closure

## 1. Entities

### 1.1 `RegressionClosureMatrixRecord`
全格式 28 项性能倒退清零与全矩阵比对审计记录实体。

| 字段名 | 类型 | 必填 | 描述 |
|:---|:---|:---:|:---|
| `format_name` | `string` | 是 | 归档格式名称（如 "zip", "7z", "tar.zst", "dmg"） |
| `scenario_name` | `string` | 是 | 测试场景名称（如 "500MB 大文件数据块", "高熵物理Payload"） |
| `compression_level` | `integer` | 是 | 压缩等级（0, 1, 5, 6, 9） |
| `encryption_type` | `string` | 是 | 加密模式（"NONE", "AES256"） |
| `operation` | `string` | 是 | 操作类型（"compress", "decompress"） |
| `baseline_throughput_mbs` | `number` | 是 | 历史最优基准吞吐 (MB/s) |
| `current_throughput_mbs` | `number` | 是 | 当前实测吞吐 (MB/s) |
| `delta_percent` | `number` | 是 | 性能变动百分比（例如 +5.2 或 -1.8） |
| `is_regression` | `boolean` | 是 | 是否发生严重性能倒退（delta < -10.0%） |

### 1.2 `ThroughputFloorVerificationRecord`
11 项 Release 性能硬门禁全量达标验证记录实体。

| 字段名 | 类型 | 必填 | 描述 |
|:---|:---|:---:|:---|
| `metric_name` | `string` | 是 | 门禁测试项标识 |
| `payload_bytes` | `integer` | 是 | 测试载荷字节数 |
| `floor_mbs` | `number` | 是 | 硬门禁吞吐底线 (MB/s) 或耗时上限 (s) |
| `measured_mbs` | `number` | 是 | 实际测算吞吐 (MB/s) 或实际耗时 (s) |
| `is_passed` | `boolean` | 是 | 门禁是否通过 |
| `margin_percent` | `number` | 是 | 达标裕量百分比 |
