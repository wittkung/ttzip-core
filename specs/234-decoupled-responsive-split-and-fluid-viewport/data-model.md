# Data Model Specification: 234 - Decoupled Responsive Split Architecture & Fluid Viewport Geometry

> **Phase**: Data Models & State Invariants  

---

## 1. Persistent State Invariants

### 1.1 `UserDefaults` Storage Schema
```
┌─────────────────────────────────┬──────────┬─────────────┬────────────────────────────────────────────────────────┐
│ Key                             │ Type     │ Default     │ Description                                            │
├─────────────────────────────────┼──────────┼─────────────┼────────────────────────────────────────────────────────┤
│ TTZip_LayoutStateVersion        │ Int      │ 1           │ Version of layout persistence schema.                  │
│ TTZip_UserRightInspectorRatio   │ Double   │ 1.0 / 3.0   │ Canonical normalized ratio of right inspector width.   │
│ TTZip_UserLeftSidebarWidth      │ Double   │ 200.0       │ Clamped left sidebar navigation width (200~300pt).     │
│ [DEPRECATED]                    │          │             │                                                        │
│ TTZip_UserRightSidebarWidth     │ Double   │ NULL        │ Purged by LayoutStateMigration v1 (Multi-SSOT removed).│
└─────────────────────────────────┴──────────┴─────────────┴────────────────────────────────────────────────────────┘
```

---

## 2. In-Memory Domain Models

### 2.1 `MainLayoutMetrics` Struct
```swift
@MainActor
public struct MainLayoutMetrics: Sendable {
    public static let minLeftSidebarWidth: CGFloat = 200.0
    public static let maxLeftSidebarWidth: CGFloat = 300.0
    public static let defaultLeftSidebarWidth: CGFloat = 200.0

    public static let minRightSidebarWidth: CGFloat = 220.0
    public static let defaultRightInspectorRatio: CGFloat = 1.0 / 3.0
    public static let maxRightInspectorErgonomicCap: CGFloat = 540.0
    public static let hairlineBorderWidth: CGFloat = 0.5

    public let shouldShowLeft: Bool
    public let shouldShowRight: Bool
    public let availableContentWidth: CGFloat
    public let effectiveRightWidth: CGFloat
    public let effectiveMaxRightWidth: CGFloat
    public let minRightSidebarWidth: CGFloat
    public let centerWidth: CGFloat
    public let shouldShowSearch: Bool
    public let omnibarMaxWidth: CGFloat
    public let goldenLineBottomY: CGFloat
    public let trafficLightMargin: CGFloat
}
```

### 2.2 `MillerColumnGeometry` Universal State Machine
```swift
public struct MillerColumnGeometry: Sendable {
    public let index: Int
    public let effectiveWidth: CGFloat
    public let showTrailingDivider: Bool
    public let isUnderflow: Bool
}
```
- **Underflow Condition**: $\sum_{i=0}^{N-1} \max(\text{customWidth}_i \text{ ?? } W_{\text{def}}, W_{\text{min}}) < W_{\text{avail}}$
- **Leaf Expansion Rule**: $\text{width}_{N-1} = W_{\text{avail}} - \sum_{j=0}^{N-2} w_j$
- **Trailing Divider Rule**: `showTrailingDivider = (index < N - 1) || !isUnderflow`
