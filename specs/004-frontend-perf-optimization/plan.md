# Implementation Plan: 前端性能深度优化 (Frontend Performance Optimization)

**Branch**: `004-frontend-perf-optimization` | **Date**: 2026-08-15 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/004-frontend-perf-optimization/spec.md)

**Input**: Feature specification from `specs/004-frontend-perf-optimization/spec.md`

## Summary

本计划针对 TTZip 前端界面在大归档加载、实时搜索过滤、分栏磁盘浏览以及高频任务进度广播场景下的性能瓶颈进行系统性优化。通过引入 `ArchiveTreeStore` 状态容器进行目录树构建的后台异步化与 Memoization、建立基于 Swift 结构化并发的任务防抖搜索管线、应用有界 LRU 缓存管理分栏磁盘元数据、并对高频进度事件实施硬件刷新率对齐的节流控制，全面保证 UI 界面在任何极限工况下均维持 60/120 FPS 极致流畅。

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), AppKit + SwiftUI
**Primary Dependencies**: In-process `CTTZipBridge`, Combine, macOS 14.0+ SDK
**Storage**: In-memory LRU cache, NSUserDefaults (for preferences only)
**Testing**: Swift Package Manager `swift test`, XCTest unit test suites
**Target Platform**: macOS 14.0+ (Apple Silicon NEON prioritized, Intel compatible)
**Project Type**: Native macOS Desktop GUI + Core Engine Library
**Performance Goals**: 50k 文件首屏加载 <= 150ms，滚动 60/120 FPS，搜索按键响应 <= 8ms，进度 UI CPU 占用降低 >= 50%
**Constraints**: 零热路径开销，严格遵守 Zip 引擎冻结规则，全量 525+ 单元测试与性能门禁 100% 通过

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Zero-Cost Abstraction on Hot Paths**: 优化仅限于 `TTZipApp` 和视图状态层，不侵入 `Sources/TTZipCore/Zip/`、`Sources/CTTZipBridge/` 等编解码热路径。
- [x] **Frozen Subsystems Protection**: 不修改 `zip-engine-freeze.md` 中的任何冻结文件。
- [x] **No Shared Locks in Parallel Loops**: 所有节流与缓存控制均在调度层，不侵入并发压缩循环。
- [x] **Strict Logging Discipline**: 严禁裸 `print` / `NSLog`，统一使用 `TTLogger`。
- [x] **Hard Performance Floor Preservation**: 确保 `XCTestPerformanceMeasureTests` 门禁吞吐指标零倒退。

## Project Structure

### Documentation (this feature)

```text
specs/004-frontend-perf-optimization/
├── spec.md              # 规格文档
├── checklists/
│   └── requirements.md  # 规格质量校验清单
├── plan.md              # 实施计划
├── research.md          # 瓶颈分析与技术决策
├── data-model.md        # 状态实体与数据模型
├── quickstart.md        # 快速验证指南
├── contracts/
│   └── ui-contracts.md  # UI 契约
└── tasks.md             # 任务清单 (Phase 2 输出)
```

### Source Code

```text
Sources/TTZipApp/
├── ViewModels/
│   ├── AppViewState.swift                  # 状态协调器 (集成进度节流)
│   ├── ArchiveTreeStore.swift              # [NEW] 目录树记忆体与异步搜索 Store
│   └── AppSubStates.swift                  # 领域子状态
├── Views/
│   ├── ArchiveExplorerView.swift           # 归档主视图 (接入 ArchiveTreeStore)
│   └── Explorer/
│       ├── FinderMillerColumnsView.swift    # 分栏视图 (接入 LRU 缓存与异步扫描)
│       └── SingleMillerColumnView.swift     # 单列分栏组件
└── Services/
    ├── ThrottledProgressPublisher.swift    # [NEW] 高频进度节流调度器
    └── ExplorerLRUCache.swift              # [NEW] 有界线程安全 LRU 缓存

Tests/TTZipTests/
└── FrontendPerfOptimizationTests.swift      # [NEW] 前端性能与状态优化单元测试套件
```

## Implementation Phases

- **Phase 1 (基础工具与状态容器)**:
  - 实现 `ExplorerLRUCache` 与 `ThrottledProgressPublisher`。
  - 实现 `ArchiveTreeStore`，支持后台异步构建目录树与带防抖的异步搜索匹配。
- **Phase 2 (视图集成与重构)**:
  - 重构 `ArchiveExplorerView`，移除 computed property 中对 `ArchiveTreeBuilder.buildTree` 的重复同步调用，接入 `ArchiveTreeStore`。
  - 重构 `FinderMillerColumnsView`，接入 `ExplorerLRUCache` 与后台异步排序。
  - 在 `AppViewState` 注入 `ThrottledProgressPublisher`，对高频进度事件进行 60Hz 门控。
- **Phase 3 (验证与基准回归)**:
  - 编写 `FrontendPerfOptimizationTests` 全面覆盖缓存命中、树构建性能、搜索防抖与进度节流。
  - 执行 `swift test` 与 `swift test --filter XCTestPerformanceMeasureTests` 验证零性能倒退。
