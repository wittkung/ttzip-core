# Research & Technical Architecture: 003-fix-explorer-sorting

## 1. Root Cause Analysis of Sorting Bugs

### Bug 1: Missing Date Sorting Implementation
- **Location**: `Sources/TTZipApp/Views/Explorer/DiskDirectoryBrowserView.swift:51-52`
- **Root Cause**: `.dateDesc` and `.dateAsc` branches literally returned:
  ```swift
  case .dateDesc, .dateAsc:
      return a.name.localizedStandardCompare(b.name) == .orderedAscending
  ```
  This was an incomplete stub from initial development.
- **Fix**: Compare `a.modificationDate` vs `b.modificationDate`, properly handling `nil` (virtual items or unindexed files).

### Bug 2: Incomplete Tie-Breaker Handling
- **Root Cause**: When two items share identical primary sort keys (e.g. two 0-byte files under `.sizeAsc`, or two items created at the same second under `.dateDesc`), `.sorted` in Swift is not guaranteed to be stable unless a full strict weak ordering with deterministic secondary comparison is provided.
- **Fix**: Apply a 3-tier comparator hierarchy:
  1. `isDirectory` (Folder grouping)
  2. Primary Sort Key (`name`, `size`, `date`, `kind`)
  3. Secondary Tie-Breaker (`name.localizedStandardCompare`)
  4. Tertiary Tie-Breaker (`path.compare` for absolute determinism)

### Bug 3: Inconsistent Localized Natural Number Sorting
- **Requirement**: macOS Finder uses `localizedStandardCompare` for natural numeric sorting (`"file2.txt"` < `"file10.txt"`).
- **Fix**: Use `localizedStandardCompare` across all string comparisons (`name`, `kindText`).

---

## 2. Architecture & Design Patterns

We apply the **Strategy Pattern** and **Pure Function Comparator**:
1. `DiskItemSorter`: A dedicated utility engine providing:
   ```swift
   public enum DiskItemSorter {
       public static func sort(_ items: [DiskItemInfo], by option: DiskSortOption) -> [DiskItemInfo]
       public static func compare(_ a: DiskItemInfo, _ b: DiskItemInfo, option: DiskSortOption) -> Bool
   }
   ```
2. Single Point of Truth: `DiskDirectoryBrowserView.sortItems` delegates directly to `DiskItemSorter.sort(items, by: option)`.
3. 100% Testability: Pure functional comparator can be tested with synthetic `DiskItemInfo` objects in `DiskSortOptionTests.swift` without needing real disk I/O.
