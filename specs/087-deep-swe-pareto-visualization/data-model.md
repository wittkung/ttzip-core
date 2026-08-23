# Data Model: Academic-Grade Pareto Frontier Visualization

## 1. 软件家族枚举 (Software Family)

```swift
public enum SoftwareFamily: String, Codable, CaseIterable, Identifiable, Sendable {
    case ttzip         = "TTZip"
    case sevenZip      = "7-Zip"
    case appleNative   = "Apple Native"
    case keka          = "Keka"
    case theUnarchiver = "The Unarchiver"
    case other         = "Other"

    public var brandColorHex: String {
        switch self {
        case .ttzip:         return "#2563EB"
        case .sevenZip:      return "#D97706"
        case .appleNative:   return "#DC2626"
        case .keka:          return "#0D9488"
        case .theUnarchiver: return "#9333EA"
        case .other:         return "#64748B"
        }
    }

    public var isHero: Bool { self == .ttzip }
    public var lineWidth: Double { isHero ? 2.8 : 2.2 }
    public var haloRibbonWidth: Double { isHero ? 24.0 : 0.0 }
}
```

## 2. 软件家族轨迹线模型 (Software Family Trajectory)

```swift
public struct SoftwareFamilyTrajectory: Sendable, Identifiable {
    public var id: String { family.rawValue }
    public let family: SoftwareFamily
    public let points: [ParetoPoint]
    public let heroPillPoint: ParetoPoint?
}
```

## 3. 散点模型扩展 (Pareto Point)

| 字段名 | 类型 | 必填 | 描述 |
| :--- | :--- | :---: | :--- |
| `id` | `String` | 是 | 散点唯一标识符 (如 `"ttzip_tar_zst"`) |
| `algorithm` | `String` | 是 | 算法或软件配置名称 |
| `level` | `Int` | 是 | 压缩等级 (如 `1`, `6`, `9`) |
| `throughputMBs` | `Double` | 是 | 吞吐速度 (MB/s) |
| `spaceSavingsPct` | `Double` | 是 | 空间节省率 (%) |
| `compressedBytes` | `Int64` | 是 | 压缩后物理字节数 |
| `uncompressedBytes` | `Int64` | 是 | 原始未压缩物理字节数 |
| `paretoRank` | `Int` | 是 | 帕累托非支配层级 (1 为前沿) |
| `isParetoOptimal` | `Bool` | 是 | 是否为全局帕累托最优前沿点 |
| `isOnConvexEnvelope`| `Bool` | 是 | 是否位于上凸包包络线上 |

## 4. 2D 贝塞尔控制点模型 (Bézier Segment)

```swift
public struct CubicBezierSegment: Sendable {
    public let startPoint: CGPoint
    public let controlPoint1: CGPoint
    public let controlPoint2: CGPoint
    public let endPoint: CGPoint
}
```
