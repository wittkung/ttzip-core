# Data Model: ZIP 强类型透明 Profile 与性能门禁

## 1. 实体定义 (Entities)

### `ZipCompressionProfile`
强类型不可变数据结构，表达 ZIP 格式下一个具体压缩档位的完整物理配置与性能基准。

```swift
public struct ZipCompressionProfile: Sendable, Equatable, Identifiable {
    public let id: String                          // 唯一标识符，如 "zip_tier_1_fast"
    public let name: String                        // UI/CLI 显示名称，如 "Fast (1)"
    public let level: ArchiveCompressionLevel      // 上层通用枚举映射 (.store, .level1 ... .level7)
    public let deflateLevel: Int32                 // libdeflate 底层 C 原生等级 (0..12)
    public let zopfliIterations: Int32             // Zopfli 多轮迭代轮次 (0..15)
    public let blockSplitting: Bool                // 是否启用局部香农熵动态最优块切分
    public let maxBlockSplits: Int32               // 最大切分块数 (0..15)
    public let earlyExitThreshold: Double          // 自适应早退收敛阈值 (0.0001 即 0.01%)
    public let targetThroughputFloorMBs: Double    // Release 模式下 18 核心物理性能门禁底线 (MB/s)
}
```

#### 字段约束规范与映射表
| 字段 | 类型 | 必填 | 取值范围 / 约束 | 语义说明 |
| :--- | :--- | :--- | :--- | :--- |
| `id` | `String` | 是 | 非空字符串 | 档位唯一标识 |
| `name` | `String` | 是 | 非空字符串 | 展示名称 |
| `level` | `ArchiveCompressionLevel` | 是 | `.store ... .level7` | 通用上层枚举 |
| `deflateLevel` | `Int32` | 是 | `0 ... 12` | libdeflate 底层压缩等级 |
| `zopfliIterations` | `Int32` | 是 | `0 ... 15` | 图论/Zopfli 迭代轮次 |
| `blockSplitting` | `Bool` | 是 | `true / false` | 最优块切分开关 |
| `maxBlockSplits` | `Int32` | 是 | `0 ... 15` | 最大切分块数量 |
| `earlyExitThreshold` | `Double` | 是 | `0.0 ... 1.0` | 早期收敛自适应剪枝阈值 |
| `targetThroughputFloorMBs` | `Double` | 是 | `> 0.0` | Release 模式吞吐硬门禁 (MB/s) |

---

### `TTZipZopfliOptions` (C 桥接层对应结构体)
```c
typedef struct {
    int compression_level;       // 对应 deflateLevel
    int num_iterations;          // 对应 zopfliIterations
    int block_splitting;         // 对应 blockSplitting (0/1)
    int max_block_splits;        // 对应 maxBlockSplits
    double early_exit_threshold; // 对应 earlyExitThreshold
} TTZipZopfliOptions;
```
