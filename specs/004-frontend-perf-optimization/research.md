# Technical Research: 前端性能深度优化 (Frontend Performance Optimization)

**Feature**: `004-frontend-perf-optimization`
**Date**: 2026-08-15
**Status**: Completed

## 1. Background & Bottleneck Analysis

通过对 TTZip 前端视图层与状态分发层的全面静态分析，确认了以下四大核心性能瓶颈：

### 瓶颈 1：SwiftUI 计算属性重复构建整棵目录树
- **现象**: `ArchiveExplorerView` 中定义了计算属性 `var rootTreeNodes: [ArchiveTreeNode] { ArchiveTreeBuilder.buildTree(from: entries) }`。
- **根因**: 在 SwiftUI 响应式机制下，任何视图状态变化（如光标选中项 `selectedEntryID` 改变、预览抽屉 `showPreviewPanel` 展开/折叠、微小动画触发）都会导致 `body` 重新求值，从而反复同步调用 `ArchiveTreeBuilder.buildTree`。当归档包包含数万条目时，主线程产生极大的重复对象树分配与 CPU 尖峰。
- **解决决策**: 将 `rootTreeNodes` 转为受控状态或引入 `ArchiveTreeStore` 记忆体，仅在 `entries` 或归档路径变更时执行异步后台构建，主线程仅做指针引用绑定。

### 瓶颈 2：实时搜索过滤同步阻塞主线程
- **现象**: `filteredEntries` 计算属性在主线程对 `entries` 执行全量字符串匹配 `entries.filter { ... }`。
- **根因**: 用户每次敲击键盘（按键、退格、中文 IME 选词），主线程同步遍历数万个条目，导致键盘输入掉帧与光标卡顿。
- **解决决策**: 建立 `SearchFilterPipeline`，采用 100ms 防抖 (Debounce) 与 Swift 结构化并发 Task 协作取消，在后台线程并发过滤并流式返回结果。

### 瓶颈 3：高频压缩/解压进度通知打爆主线程 RunLoop
- **现象**: `AppViewState.onProgressUpdated` 和 `onBatchProgressUpdated` 作为 `ArchiveProgressObserverProtocol` 观察者，直接在非主线程接收到每秒上千次的高频字节/条目进度更新后立即向 `@MainActor` 发起任务分发。
- **根因**: 当引擎以 1500+ MB/s 处理数千小文件时，每秒触发上千次主线程 `@Published` 变更，导致 SwiftUI 界面以非物理可感知的帧率疯狂重绘，阻塞用户操作与主事件循环。
- **解决决策**: 引入 `ThrottledProgressPublisher` 享元/装饰器，以硬件刷新率（<= 60Hz / 16.6ms 间隔门控）进行进度采样与节流，保障主线程空闲率。

### 瓶颈 4：分栏文件浏览器缓存缺乏有界管理与异步调度
- **现象**: `FinderMillerColumnsView` 中的 `cachedColumnItems` 为无界字典，且排序在某些路径下与视图渲染耦合。
- **解决决策**: 规范化 LRU 缓存策略，限制内存驻留上限为 64 个活跃目录，并由 `MillerColumnDirectoryScanner` 统一提供异步加载与排序。

---

## 2. Architectural Decisions & Tradeoffs

| 决策点 | 选定方案 | 替代方案 (已否决) | 选定理由 |
| :--- | :--- | :--- | :--- |
| **目录树缓存管理** | 独立 `ArchiveTreeStore` 状态容器 + 异步构建 | 仅在 View 内部用 `@State` 缓存 | 独立 Store 便于单元测试、跨组件共享及状态备忘录 (Memento) 捕获。 |
| **搜索防抖机制** | Combine / Concurrency 协作 Task (100ms) | 同步每帧计算 | 防抖能彻底消除连续键入时的无效计算与主线程丢帧。 |
| **进度事件节流** | 毫秒级时间戳门控 (Monotonic Timestamp Gate, 16.6ms) | GCD Timer 轮询 | 时间戳门控零定时器开销，仅在真实有数据到来时按需放行。 |
| **分栏缓存策略** | 有界线程安全 LRU 缓存 (最大 64 目录) | 无界 Dictionary / 每次重新读盘 | 兼顾极速 0ms 切回体验与长期运行内存安全。 |

---

## 3. Constitution & Performance Invariant Alignment

- **零成本抽象与热路径保护**: 所有前端优化代码严格限制在 `TTZipApp` 和上层展示管道中，绝不侵入 `Sources/TTZipCore/Zip/`、`Sources/CTTZipBridge/` 等编解码热路径。
- **内存安全与指针安全**: 零全局锁，使用 Swift 6.0 `@MainActor` 与 `Sendable` 线程安全模型。
