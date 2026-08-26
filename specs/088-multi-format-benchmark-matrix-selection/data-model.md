# Data Model: Multi-Tier Format Selection & Benchmark Architecture

## 1. 4 阶格式分层定义 (Benchmark Format Tier)

```swift
public enum BenchmarkFormatTier: String, Codable, CaseIterable, Identifiable, Sendable {
    case tier1_universal = "Tier 1: Universal (ZIP)"
    case tier2_extreme   = "Tier 2: Extreme (7Z)"
    case tier3_modern    = "Tier 3: Modern (TAR.ZST)"
    case tier4_inMemory  = "Tier 4: In-Memory (LZ4)"

    public var id: String { rawValue }

    /// 代表格式短名
    public var primaryFormat: String {
        switch self {
        case .tier1_universal: return "ZIP"
        case .tier2_extreme:   return "7Z"
        case .tier3_modern:    return "TAR.ZST"
        case .tier4_inMemory:  return "LZ4"
        }
    }

    /// 核心算法描述
    public var underlyingAlgorithm: String {
        switch self {
        case .tier1_universal: return "Deflate (32KB Window, Huffman)"
        case .tier2_extreme:   return "LZMA2 (64MB-1GB Dict, Range Coder)"
        case .tier3_modern:    return "Zstandard (FSE, Repcodes)"
        case .tier4_inMemory:  return "LZ4 (Byte-aligned, No Entropy)"
        }
    }

    /// 综合评分权重 (总和 = 1.0)
    public var compositeWeight: Double {
        switch self {
        case .tier1_universal: return 0.30
        case .tier2_extreme:   return 0.25
        case .tier3_modern:    return 0.25
        case .tier4_inMemory:  return 0.20
        }
    }
}
```

## 2. 格式矩阵预设策略 (Format Matrix Preset)

```swift
public enum FormatMatrixPreset: String, Codable, CaseIterable, Identifiable, Sendable {
    case fourTier = "4tier"
    case classic  = "classic"
    case modern   = "modern"
    case all16    = "all16"

    public var id: String { rawValue }

    public var includedFormats: [String] {
        switch self {
        case .fourTier: return ["ZIP", "7Z", "TAR.ZST", "LZ4"]
        case .classic:  return ["ZIP", "7Z", "TAR.GZ", "TAR.BZ2"]
        case .modern:   return ["TAR.ZST", "LZ4", "BROTLI", "SNAPPY"]
        case .all16:    return ["ZIP", "7Z", "TAR", "TAR.ZST", "TAR.GZ", "TAR.BZ2", "TAR.XZ", "WIM", "DMG", "LZ4", "LZIP", "LRZIP", "AAR", "ISO", "BROTLI", "SNAPPY"]
        }
    }
}
```

## 3. 综合效能评分模型 (Composite Score Report)

```swift
public struct CompositeScoreReport: Codable, Sendable, Identifiable, Equatable {
    public var id: String { softwareName }
    public let softwareName: String
    public let compositeScore: Double           // 综合效能分 (Base-1000)
    public let geometricMeanThroughputMBs: Double // 几何平均吞吐 (MB/s)
    public let averageSpaceSavingsPct: Double     // 平均空间节省率 (%)
    public let tierSubScores: [BenchmarkFormatTier: Double]
    public let paretoEfficiencyIndex: Double     // 帕累托效率指数 PEI (0.0 ~ 1.0)
}
```
