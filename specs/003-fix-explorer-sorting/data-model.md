# Data Model: 003-fix-explorer-sorting

## 1. DiskSortOption (Existing, Revalidated)

```swift
public enum DiskSortOption: String, CaseIterable, Identifiable, Codable, Sendable {
    case nameAsc = "名称 (A-Z)"
    case nameDesc = "名称 (Z-A)"
    case sizeDesc = "体积 (从大到小)"
    case sizeAsc = "体积 (从小到大)"
    case dateDesc = "修改时间 (最新)"
    case dateAsc = "修改时间 (最早)"
    case kind = "文件类型"
    
    public var id: String { rawValue }
    public var iconName: String { ... }
}
```

## 2. DiskItemSorter (New Pure Functional Engine)

```swift
public enum DiskItemSorter {
    /// Sorts a collection of DiskItemInfo items according to the selected option.
    public static func sort(_ items: [DiskItemInfo], by option: DiskSortOption) -> [DiskItemInfo]
    
    /// Strict weak ordering comparator between two DiskItemInfo items.
    public static func isOrderedBefore(_ a: DiskItemInfo, _ b: DiskItemInfo, option: DiskSortOption) -> Bool
}
```

## 3. Comparison Truth Table

| Option | Primary Condition ($a$ vs $b$) | Equal/Nil Behavior | Secondary Tie-Breaker | Tertiary Tie-Breaker |
| :--- | :--- | :--- | :--- | :--- |
| `nameAsc` | `name.localizedStandardCompare` == `.orderedAscending` | N/A | `path.compare` | N/A |
| `nameDesc` | `name.localizedStandardCompare` == `.orderedDescending` | N/A | `path.compare` | N/A |
| `sizeDesc` | `rawSizeBytes > rawSizeBytes` | Equal sizes fall through | `nameAsc` | `path.compare` |
| `sizeAsc` | `rawSizeBytes < rawSizeBytes` | Equal sizes fall through | `nameAsc` | `path.compare` |
| `dateDesc` | Both non-nil: $a.date > b.date$<br>One non-nil: non-nil is before nil | Equal dates / both nil fall through | `nameAsc` | `path.compare` |
| `dateAsc` | Both non-nil: $a.date < b.date$<br>One non-nil: non-nil is before nil | Equal dates / both nil fall through | `nameAsc` | `path.compare` |
| `kind` | `kindText.localizedStandardCompare` == `.orderedAscending` | Equal kinds fall through | `nameAsc` | `path.compare` |

*Note*: In all cases, `isDirectory` takes highest precedence (folders first).
