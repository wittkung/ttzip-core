# Feature Specification: SwiftUI 桌面端与 C11 纯微内核深度打通及设计系统落地 (Feature 164)

**Feature ID**: `164-swiftui-desktop-c-bridge-integration-and-design-system`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Product Experience, GUI & Design System, C-Bridge Streaming)

---

## 1. Executive Summary

TTZip 底层已经拥有工业级纯 C11 微内核、24 套通过的安全测试套件及高达 14 GB/s 的解压跑分。本特性的目标是将这一极致底座与 macOS SwiftUI 桌面端全链路打通，实现无缝的产品力跃迁：
1. **纯 C11 ➔ Swift 60fps 无锁进度流桥接**：实现轻量 C 回调 `ttzip_archive_progress_cb` 与 Swift `AsyncStream` 桥接，零内存抖动向 SwiftUI 派发解包/打包瞬时 MB/s、文件进度与 ETA；
2. **TTZip Zen $\times$ WSJ Editorial $\times$ Kintsugi Gold 设计系统全面对齐**：遵循 `ttzip-ui-design-system` 规范，实现三栏米勒列 Y=90pt 金线绝对对齐、52pt 顶栏、极细边框（`hairlineBorder`）、琉璃质感弹窗与深浅主题自适应；
3. **毫秒级空格键 Quick Look 预览与双向拖拽解压**：支持选中文档时按下 Space 键就地单文件快速预览，以及从归档浏览器直接拖拽文件至 Finder 触发单文件极速流式解压；
4. **大文件树虚拟化极速滚动**：基于 C11 Radix 树索引与 Swift `LazyVStack`，在面对 10 万+ 文件的巨型归档时保持 60fps 丝滑滚动与 0 卡顿搜索。

---

## 2. User Scenarios

### User Scenario 1 (US1) - 60fps 无锁解压与打包进度体验 (Realtime Streaming Telemetry)
- **As a**: macOS 桌面端日常用户
- **I want to**: 在点击解压或打包数 GB 大文件时，界面展现丝滑、平稳更新的进度条与瞬时吞吐仪表盘
- **So that**: 主线程绝对不卡死（0 掉帧、0 假死），可随时点击“取消”在毫秒级中止 C 线程池工作。

### User Scenario 2 (US2) - 典雅禅意与金缮金线视觉系统 (Zen & Kintsugi Gold Design System)
- **As a**: 追求视觉质感与专业排版的高级用户
- **I want to**: 使用符合 WSJ Editorial 衬线排版、Kintsugi Gold（#D4AF37）高光与毛玻璃材质的主窗口与弹窗
- **So that**: 无论在浅色（和纸白 Washi Paper）还是深色模式（深石墨 Deep Graphite / 墨黑 Ink Black）下均享有 Apple 原生级精致体验。

### User Scenario 3 (US3) - 单文件即时预览与拖拽至访达 (Quick Look & Drag-to-Finder)
- **As a**: 需要快速浏览归档内容的专业工作者
- **I want to**: 在归档浏览器中选中图片/代码/PDF 按下空格键即可即时预览，或直接将选中项目拖出窗口丢到桌面
- **So that**: 无需解压整个数 GB 的庞大压缩包，即可在毫秒级按需提取目标文件。

### User Scenario 4 (US4) - 10 万+ 文件超大归档树丝滑检索 (Lag-Free Virtualized Explorer)
- **As a**: 处理庞大工程包或数据集的系统工程师
- **I want to**: 打开包含 100,000+ 文件的深度嵌套归档并进行全文实时搜索
- **So that**: 界面在 1 毫秒内完成树状分层渲染，滚动流畅，搜索无任何输入延迟。

---

## 3. Functional Requirements

- **REQ-001 (Progress Streaming C Bridge)**: 在 `Sources/CTTZipBridge/include/CTTZipBridge_Archive.h` 中提供 `ttzip_archive_progress_cb` 与 `ttzip_archive_cancel_fn` 统一进度与取消句柄。
- **REQ-002 (Swift Async Bridge Adapter)**: 在 `Sources/TTZipCore/` 中实现 `ArchiveOperationBridge.swift`，通过 Swift 结构化并发 `AsyncStream<ArchiveProgressEvent>` 承接 C 回调。
- **REQ-003 (Design System Tokens & Theme)**: 落地并严格遵循 `TTZipTheme`，包含 `kintsugiGold`, `bambooGreen`, `cinnabarRed`, `deepGraphite`, `inkBlack`, `washiPaper`, `hairlineBorder` 及 Y=90pt 金线。
- **REQ-004 (Miller Columns / 3-Column Layout)**: 规范 Sidebar (200pt)、Central Workspace (min 450pt)、Inspector (280pt) 三栏 Y=90pt 水平绝对对齐与 52pt 顶栏标准。
- **REQ-005 (Compress Modal & Dashboard)**: 实现 640x520 液态玻璃压缩弹窗，提供 10 阶压缩梯度选择与实时硬件 Speedometer 仪表盘。
- **REQ-006 (Quick Look & Quick Preview Engine)**: 实现基于临时内存管道的 Space-bar 快速预览，支持文本、代码、图片、PDF。
- **REQ-007 (Drag & Drop Export)**: 实现 `NSItemProvider` / `.onDrag` 单文件与批量选中提取导出至 Finder。
- **REQ-008 (Large Tree Virtualization)**: 基于 `ttzip_radix_tree_t` 建立扁平化虚拟列表数据源，确保 100,000+ 条目滚动内存开销 < 30MB。

---

## 4. Success Criteria

1. **GUI 渲染帧率**: 解压与打包期间主界面保持稳定 **60 fps**，UI 线程 0 阻塞；
2. **三栏金线对齐精度**: 侧边栏、中央区、检查器顶栏金线高度绝对统一为 **Y = 90pt**（误差 0pt）；
3. **空格预览延迟**: 选中文件按下 Space 键到 Quick Look 弹窗出现时间 **< 50 毫秒**；
4. **10 万文件树内存开销**: 加载并展示 100,000 个文件节点时，Swift 视图层内存占用 **< 35 MB**；
5. **门禁与测试一致性**: 新增 UI 状态机与桥接层单测 100% 通过，`./scripts/run_optimization_gate.sh` 维持全绿。
