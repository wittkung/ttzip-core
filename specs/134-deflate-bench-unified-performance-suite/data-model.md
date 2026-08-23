# Data Model: Deflate-Bench Unified Performance & Test Suite Modernization

**Feature Directory**: `specs/134-deflate-bench-unified-performance-suite`  
**Target Subject**: 纯内存语料生成器、基准测试矩阵与结果报告实体定义  

---

## 1. Core Entities Definition

### BenchmarkCorpusConfig
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `corpus_type` | `string` (enum: ["text", "short_match", "dna", "random", "literals", "mixed", "realistic_rgb", "striped_rgb"]) | Yes | 语料物理类型 |
| `buffer_size_bytes` | `integer` | Yes | 目标生成字节大小（如 131072, 1048576） |
| `seed` | `integer` | Yes | 确定性 PRNG 初始种子 |

---

### CodecBenchmarkResult
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `engine_name` | `string` (enum: ["libdeflate", "zlib-ng", "zstd", "lz4", "snappy"]) | Yes | 压缩引擎名称 |
| `corpus_type` | `string` | Yes | 语料类型 |
| `payload_size_bytes` | `integer` | Yes | 原始输入字节大小 |
| `compression_level` | `integer` | Yes | 压缩级别 |
| `compressed_size_bytes`| `integer` | Yes | 压缩后输出字节大小 |
| `compression_ratio` | `number` | Yes | 压缩比（原始/压缩后） |
| `compress_duration_ns` | `number` | Yes | 压缩中位数耗时（纳秒） |
| `compress_throughput_mb_s` | `number` | Yes | 压缩吞吐速率 (MB/s) |
| `decompress_duration_ns` | `number` | Yes | 解压中位数耗时（纳秒） |
| `decompress_throughput_mb_s` | `number` | Yes | 解压吞吐速率 (MB/s) |
| `cv_percentage` | `number` | Yes | 变异系数百分比 |
| `integrity_verified` | `boolean` | Yes | 往返解压 memcmp 字节校验是否 100% 通过 |

---

### BenchmarkSuiteSummary
| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `suite_timestamp` | `string` (date-time) | Yes | 测试执行 UTC 时间戳 |
| `total_points_evaluated` | `integer` | Yes | 评估测试点总数（50） |
| `total_duration_ms` | `number` | Yes | 全矩阵总耗时（毫秒，门禁 < 1000ms） |
| `median_cv_percentage` | `number` | Yes | 全矩阵中位数变异系数 |
| `all_integrity_passed` | `boolean` | Yes | 所有点解压校验是否全绿 |
| `matrix_results` | `Array<CodecBenchmarkResult>` | Yes | 50 点详细测量列表 |
