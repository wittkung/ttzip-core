# Implementation Plan: 前端性能指标监控与基准测试体系 (Frontend Performance Metrics System)

**Branch**: `005-frontend-perf-metrics` | **Date**: 2026-08-15 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/005-frontend-perf-metrics/spec.md)

**Input**: Feature specification from `specs/005-frontend-perf-metrics/spec.md`

## Summary

构建 TTZip 前端性能指标量化监控与自动化基准测试体系。设计涵盖目录树构建耗时、搜索过滤吞吐、LRU 缓存存取延迟以及高频进度节流拦截率等 4 大核心维度的指标实体（`FrontendPerformanceReport`），建立统一的基准执行引擎 `FrontendBenchmarkRunner`，并设立严格的自动化硬性能门禁测试 `FrontendPerformanceGateTests`，同时将前端性能矩阵集成至 GUI 控制台 `BenchmarkView`。

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), AppKit + SwiftUI
**Primary Dependencies**: `TTZipCore`, `TTZipApp`, Apple Silicon Hardware Detection
**Storage**: In-memory benchmark datasets, JSON report export
**Testing**: `swift test --filter FrontendPerformanceGateTests`, XCTest
**Target Platform**: macOS 14.0+
**Performance Goals**: 50k 条目树构建 $\le 80\text{ ms}$，20k 搜索吞吐 $\ge 2,000,000\text{ items/s}$，LRU 10k 操作 $\le 5\text{ ms}$，节流拦截率 $\ge 95\%$
**Constraints**: 零热路径污染，严格遵守 Zip 引擎冻结规则，全量 584+ 单元测试保持 100% 通过

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Zero-Cost Abstraction on Hot Paths**: 性能指标采集仅针对前端模型与视图调度层，绝不侵入 C 编解码热路径。
- [x] **Frozen Subsystems Protection**: 不修改任何 Zip 引擎冻结文件。
- [x] **Strict Logging Discipline**: 统一使用 `TTLogger`。
- [x] **Hard Performance Floor Preservation**: 确保所有现有与新增性能门禁 100% 达标。

## Project Structure

### Documentation

```text
specs/005-frontend-perf-metrics/
├── spec.md              # 规格说明
├── checklists/
│   └── requirements.md  # 质量检查清单
├── plan.md              # 实施计划
├── research.md          # 指标体系设计
├── data-model.md        # 性能指标实体定义
├── quickstart.md        # 快速验证指南
├── contracts/
│   └── metrics-contracts.md # 契约定义
└── tasks.md             # 任务分解
```

### Source Code

```text
Sources/TTZipCore/
└── Benchmark/
    ├── FrontendPerformanceMetrics.swift    # [NEW] 前端性能指标实体与报告模型
    └── FrontendBenchmarkRunner.swift        # [NEW] 前端性能基准执行引擎

Sources/TTZipApp/
└── Views/Benchmark/
    ├── BenchmarkViewModel.swift             # 扩展前端测试模式与结果绑定
    └── BenchmarkConfigSectionView.swift     # 支持前端性能矩阵切换

Tests/TTZipTests/
└── FrontendPerformanceGateTests.swift       # [NEW] 前端性能硬门禁单元测试套件
```

## Implementation Phases

- **Phase 1 (指标实体与执行引擎)**:
  - 实现 `FrontendPerformanceMetrics.swift`，定义 4 大维度指标与报告数据结构。
  - 实现 `FrontendBenchmarkRunner.swift`，提供纯内存标准数据集生成与并发压测。
- **Phase 2 (自动化门禁测试)**:
  - 编写 `FrontendPerformanceGateTests.swift`，设定不可跌破的硬性能门禁。
- **Phase 3 (GUI Benchmark 控制台扩展)**:
  - 在 `BenchmarkViewModel.swift` 与 `BenchmarkConfigSectionView.swift` 中增加前端性能测试模式，支持一键压测与指标呈现。
- **Phase 4 (全量回归验证)**:
  - 运行 `swift test --filter FrontendPerformanceGateTests` 与全量 `swift test`。
