# Quickstart: 003-fix-explorer-sorting

## Usage

```swift
import TTZipApp

let items: [DiskItemInfo] = ...
let sortedByName = DiskItemSorter.sort(items, by: .nameAsc)
let sortedByDateNewest = DiskItemSorter.sort(items, by: .dateDesc)
let sortedBySizeLargest = DiskItemSorter.sort(items, by: .sizeDesc)
```

## Running Unit Tests

```bash
swift test --filter DiskSortOptionTests
```
