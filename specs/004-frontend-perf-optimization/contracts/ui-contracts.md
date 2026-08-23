# UI & Component Contracts: 前端性能优化

**Feature**: `004-frontend-perf-optimization`
**Date**: 2026-08-15
**Status**: Ready

## 1. Archive Explorer UI Contract

```swift
// ArchiveExplorerView 消费 ArchiveTreeStore 保证单向数据流与零主线程卡顿
public protocol ArchiveExplorerStoreProtocol: AnyObject {
    var rootNodes: [ArchiveTreeNode] { get }
    var isBuildingTree: Bool { get }
    var filteredEntries: [ArchiveEntry] { get }
    var isFiltering: Bool { get }
    
    func updateEntries(_ entries: [ArchiveEntry]) async
    func filter(query: String) async
    func clear()
}
```

## 2. Progress Observer Throttling Contract

```swift
// 解耦高频数据流与 UI 渲染循环
public protocol ProgressThrottlingProtocol: Sendable {
    func shouldEmit(now: UInt64) -> Bool
}
```
