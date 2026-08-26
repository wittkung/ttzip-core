# Data Model: 094 Entropy-Aware Tiered Chunking Engine

## 1. `EntropyTierCategory`
- `tier1LowEntropy`: $H < 3.5\text{ bits/byte}$ ➔ 推荐分块 $2048\text{ KB}$ (2MB)
- `tier2MediumEntropy`: $3.5 \le H < 6.0\text{ bits/byte}$ ➔ 推荐分块 $512\text{ KB}$
- `tier3MediumHighEntropy`: $6.0 \le H < 7.35\text{ bits/byte}$ ➔ 推荐分块 $128\text{ KB}$
- `tier4HighEntropy`: $H \ge 7.35\text{ bits/byte}$ ➔ 推荐分块 $0$ (Direct Store Method 0)

## 2. `AdaptiveChunkingProfile`
- `entropy`: `Double`
- `recommendedBlockSize`: `Int`
- `compressionMethod`: `UInt16` (0 = Store, 8 = Deflate)
- `targetCacheLevel`: `String` ("L2-Global", "L2-Cluster", "L1-Private", "Bus-DMA")
