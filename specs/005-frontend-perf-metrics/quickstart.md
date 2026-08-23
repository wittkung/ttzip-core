# Quickstart & Verification Guide: 前端性能指标体系

**Feature**: `005-frontend-perf-metrics`
**Date**: 2026-08-15
**Status**: Ready

## 1. Automated Unit & Hard Floor Gate Tests

```bash
# 1. 运行前端性能指标与硬门禁单元测试
swift test --filter FrontendPerformanceGateTests

# 2. 运行全量单元测试与核心吞吐门禁
swift test --filter XCTestPerformanceMeasureTests
swift test
```

## 2. GUI Benchmark Verification

1. 启动应用并切换至 "Benchmark" 标签页。
2. 在测试模式下拉菜单中选择 "前端与 UI 渲染性能矩阵"。
3. 点击 "开始全套压测"，观察实时速度仪表与 4 大指标明细列表。
