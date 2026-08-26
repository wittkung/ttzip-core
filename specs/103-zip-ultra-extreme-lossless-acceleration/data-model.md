# Data Model: ZIP Tier 6/7 零损失加速与分块调度

## 1. 实体与参数定义 (Entities & Parameters)

### `ZipCompressionProfile` (更新后的参数约束)
```swift
public struct ZipCompressionProfile: Sendable, Equatable, Identifiable {
    public let id: String                          // 档位唯一标识符
    public let name: String                        // UI/CLI 显示名称
    public let level: ArchiveCompressionLevel      // 通用抽象等级
    public let deflateLevel: Int32                 // libdeflate 底层压缩等级 (0..12)
    public let zopfliIterations: Int32             // Zopfli 迭代轮次 (0..15)
    public let blockSplitting: Bool                // 最优块切分开关
    public let maxBlockSplits: Int32               // 最大切分块数 (0..15)
    public let earlyExitThreshold: Double          // 不动点自适应早退收敛阈值 (0.0001)
    public let targetThroughputFloorMBs: Double    // Release 门禁底线 (MB/s)
}
```

### 块尺寸自适应模型 (Block Sizing Model)
| 场景 / 档位 | 默认块大小 (`actualBlockSize`) | 历史字典大小 (`historySize`) | 并发单位上下文内存 |
| :--- | :--- | :--- | :--- |
| **Tier 0 ~ Tier 4** | `max(4MB, (fileSize + 15) / 16)` | 32 KB | $\approx 256\text{ KB}$ |
| **Tier 5 (Graph Fast)** | `min(2MB, fileSize)` | 32 KB | $\approx 1.5\text{ MB}$ |
| **Tier 6 (Ultra Zopfli)**| `min(2MB, fileSize)` | 32 KB | $\approx 2.5\text{ MB}$ |
| **Tier 7 (Extreme Peak)**| `min(2MB, fileSize)` | 32 KB | $\approx 2.8\text{ MB}$ |
