# Technical Research: 前端性能指标监控与基准测试体系

**Feature**: `005-frontend-perf-metrics`
**Date**: 2026-08-15
**Status**: Completed

## 1. Metric System Dimensions & Design

为了全面量化并长效守护 TTZip 前端性能，设计以下 4 大核心维度的指标采集体系：

### 维度 1：目录树构建指标 (Tree Construction Metrics)
- **指标项**:
  - `durationMs`: 构建整棵树所消耗的毫秒数
  - `entryCount`: 输入的扁平条目数 (1k, 10k, 50k, 100k)
  - `rootNodeCount`: 顶层直接子节点数
  - `throughputItemsPerSec`: 构建吞吐量 (条目/秒)
- **基准门禁**: 50,000 条目构建 $\le 80\text{ ms}$ (吞吐 $\ge 625,000\text{ items/s}$)。

### 维度 2：实时搜索过滤指标 (Search & Filter Metrics)
- **指标项**:
  - `durationMs`: 过滤匹配耗时
  - `datasetSize`: 搜索池条目总数
  - `matchedCount`: 匹配命中条目数
  - `filterThroughput`: 过滤吞吐量 (条目/秒)
- **基准门禁**: 20,000 条目过滤 $\le 10\text{ ms}$ (吞吐 $\ge 2,000,000\text{ items/s}$)。

### 维度 3：LRU 内存缓存与存取效率 (LRU Cache Metrics)
- **指标项**:
  - `readLatencyNs`: 单次命中读取延迟 (纳秒级)
  - `writeLatencyNs`: 写入与淘汰延迟
  - `evictionCount`: 触发容量淘汰的次数
  - `hitRatio`: 缓存命中率
- **基准门禁**: 10,000 次读写总耗时 $\le 5\text{ ms}$ (平均单次 $< 500\text{ ns}$)。

### 维度 4：高频事件与进度节流指标 (Progress Throttling Metrics)
- **指标项**:
  - `totalEvents`: 引擎总发出事件数
  - `emittedEvents`: 经节流放行派发至主线程的事件数
  - `throttledEvents`: 拦截丢弃的高频事件数
  - `suppressionRatio`: 节流拦截率 (百分比)
  - `averageIntervalMs`: 实际派发平均间隔 (目标 $16.6\text{ ms}$)
- **基准门禁**: 10,000 次突发事件下，拦截率 $\ge 95\%$，主线程无队列积压。

---

## 2. Technical Architecture & Runner

- **Runner**: `FrontendBenchmarkRunner` 单例 / 引擎，支持以异步方式生成内存模拟数据集并批量执行压测。
- **UI Integration**: 扩展 `BenchmarkViewModel` 与 `BenchmarkView`，新增 `BenchmarkMode.frontend` 前端性能模式。
- **Hard Gate**: 编写独立单元测试 `FrontendPerformanceGateTests`，集成进 CI 与 `swift test`。
