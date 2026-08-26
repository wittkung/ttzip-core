# Data Model & Metrics Entities: 前端性能指标体系

**Feature**: `005-frontend-perf-metrics`
**Date**: 2026-08-15
**Status**: Ready

## 1. Metrics Structures

```swift
/// 单次目录树构建性能指标
public struct TreeBuildMetric: Codable, Sendable, Equatable {
    public let entryCount: Int
    public let rootNodeCount: Int
    public let durationMs: Double
    public let throughputItemsPerSec: Double
    
    public init(entryCount: Int, rootNodeCount: Int, durationMs: Double) {
        self.entryCount = entryCount
        self.rootNodeCount = rootNodeCount
        self.durationMs = durationMs
        self.throughputItemsPerSec = durationMs > 0 ? (Double(entryCount) / (durationMs / 1000.0)) : 0
    }
}

/// 搜索与过滤性能指标
public struct SearchFilterMetric: Codable, Sendable, Equatable {
    public let datasetSize: Int
    public let query: String
    public let matchedCount: Int
    public let durationMs: Double
    public let filterThroughputItemsPerSec: Double
    
    public init(datasetSize: Int, query: String, matchedCount: Int, durationMs: Double) {
        self.datasetSize = datasetSize
        self.query = query
        self.matchedCount = matchedCount
        self.durationMs = durationMs
        self.filterThroughputItemsPerSec = durationMs > 0 ? (Double(datasetSize) / (durationMs / 1000.0)) : 0
    }
}

/// LRU 内存缓存性能指标
public struct LRUCacheMetric: Codable, Sendable, Equatable {
    public let operationsCount: Int
    public let capacity: Int
    public let durationMs: Double
    public let opsPerSec: Double
    public let hitRatio: Double
    
    public init(operationsCount: Int, capacity: Int, durationMs: Double, hitRatio: Double) {
        self.operationsCount = operationsCount
        self.capacity = capacity
        self.durationMs = durationMs
        self.opsPerSec = durationMs > 0 ? (Double(operationsCount) / (durationMs / 1000.0)) : 0
        self.hitRatio = hitRatio
    }
}

/// 进度节流性能指标
public struct ProgressThrottleMetric: Codable, Sendable, Equatable {
    public let totalEvents: Int
    public let emittedEvents: Int
    public let throttledEvents: Int
    public let suppressionRatio: Double
    public let durationMs: Double
    
    public init(totalEvents: Int, emittedEvents: Int, durationMs: Double) {
        self.totalEvents = totalEvents
        self.emittedEvents = emittedEvents
        self.throttledEvents = max(0, totalEvents - emittedEvents)
        self.suppressionRatio = totalEvents > 0 ? (Double(self.throttledEvents) / Double(totalEvents)) * 100.0 : 0.0
        self.durationMs = durationMs
    }
}

/// 前端性能全套基准报告
public struct FrontendPerformanceReport: Codable, Sendable, Equatable {
    public let timestamp: Date
    public let hardwareSummary: String
    public let treeBuildMetrics: [TreeBuildMetric]
    public let searchFilterMetrics: [SearchFilterMetric]
    public let lruCacheMetrics: [LRUCacheMetric]
    public let throttleMetrics: [ProgressThrottleMetric]
    public let isAllPassed: Bool
}
```
