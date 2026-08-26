# Interface Contracts: 前端性能基准执行中枢

**Feature**: `005-frontend-perf-metrics`
**Date**: 2026-08-15
**Status**: Ready

## 1. Frontend Benchmark Runner Protocol

```swift
public protocol FrontendBenchmarkRunnerProtocol: Sendable {
    func runTreeBuildBenchmark(entryCounts: [Int]) async -> [TreeBuildMetric]
    func runSearchFilterBenchmark(datasetSize: Int, queries: [String]) async -> [SearchFilterMetric]
    func runLRUCacheBenchmark(operationsCount: Int) async -> [LRUCacheMetric]
    func runThrottleBenchmark(eventCount: Int) async -> [ProgressThrottleMetric]
    func runFullFrontendSuite() async -> FrontendPerformanceReport
}
```
