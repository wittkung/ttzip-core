# Data Model: Entropy-Adaptive Intelligent Extreme Routing

## 1. `EntropyProbeResult`
- `entropyBitsPerByte`: `Double` (0.0 ~ 8.0 bits/byte)
- `estimatedRatio`: `Double` (预估压缩比)
- `recommendedMethod`: `UInt16` (0 = Method 0 Store, 8 = Method 8 Deflate)
- `sampleSizeBytes`: `Int` (默认 4096 字节)

## 2. `ExtremeCompressionMode`
- `automatic`: 基于香农熵与采样探测自适应分流 (默认推荐)
- `forceDeflate`: 强制多核分块 Deflate (Method 8)
- `forceStore`: 强制直通存储 (Method 0)
