# Architecture Contract: 234 - Decoupled Responsive Split Architecture & Fluid Viewport Geometry

> **Phase**: Architectural Contract Specifications  

---

## 1. Migration Contract (`LayoutStateMigration`)

```swift
@MainActor
public enum LayoutStateMigration {
    public static let layoutStateVersionKey = "TTZip_LayoutStateVersion"
    public static let currentVersion = 1
    
    public static func migrateIfNeeded(defaults: UserDefaults = .standard) {
        let storedVersion = defaults.integer(forKey: layoutStateVersionKey)
        guard storedVersion < currentVersion else { return }
        
        // 1. Reset right inspector ratio to canonical 1.0 / 3.0 (2:1 ratio)
        defaults.set(Double(MainLayoutMetrics.defaultRightInspectorRatio), forKey: "TTZip_UserRightInspectorRatio")
        // 2. Eliminate deprecated absolute pixel width key to eliminate multi-SSOT hazards
        defaults.removeObject(forKey: "TTZip_UserRightSidebarWidth")
        // 3. Mark migration complete
        defaults.set(currentVersion, forKey: layoutStateVersionKey)
    }
}
```

---

## 2. Universal Column Geometry Contract (`FinderMillerColumnsView`)

```swift
public static func computeColumnWidth(
    for index: Int,
    totalColumns: Int,
    availableWidth: CGFloat,
    columnWidths: [Int: CGFloat] = [:],
    defaultColumnWidth: CGFloat = 260.0,
    minComfortableWidth: CGFloat = 240.0
) -> CGFloat {
    guard totalColumns > 0, index >= 0, index < totalColumns else {
        return defaultColumnWidth
    }
    
    let preferredWidth: (Int) -> CGFloat = { idx in
        if let custom = columnWidths[idx] {
            return max(custom, minComfortableWidth)
        }
        return defaultColumnWidth
    }
    
    let totalPreferredWidth = (0..<totalColumns).reduce(CGFloat(0)) { $0 + preferredWidth($1) }
    
    // Underflow mode: expand the active leaf column to flush against the inspector divider
    if totalPreferredWidth < availableWidth {
        if index == totalColumns - 1 {
            let precedingSum = (0..<(totalColumns - 1)).reduce(CGFloat(0)) { $0 + preferredWidth($1) }
            return max(preferredWidth(index), availableWidth - precedingSum)
        } else {
            return preferredWidth(index)
        }
    }
    
    // Overflow mode: preserve preferred widths and allow horizontal scrolling
    return preferredWidth(index)
}
```
