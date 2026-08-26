# Feature Specification: macOS 原生 Quick Look 快速预览与 Finder 拖拽单文件提取集成 (Feature 168)

**Feature ID**: `168-native-macos-quicklook-and-finder-integration`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (macOS Native Integration, User Experience, Latency Optimization)

---

## 1. Executive Summary

在 macOS 原生桌面生态中，用户对归档管理工具的最关键核心体验在于：
1. **空格键一键 Quick Look 预览 (Space-bar Quick Look Preview)**：无需解压整个归档，在归档浏览器中选中任意文件（如图片、PDF、文本、代码、视频）按下空格键，即刻调出 macOS 系统级原生预览浮窗；
2. **拖拽至访达 (Drag-to-Finder Single/Multi-Item Extraction)**：选中归档内的一个或多个文件，直接拖放到 macOS 访达（Finder）文件夹或桌面，即刻在后台流式提取该单条目并完成落盘。

传统第三方工具（如 The Unarchiver、Keka）往往需要先解压整个数 GB 的归档，或者在 `/tmp` 产生大量残留文件。
本特性的目标是：**结合 Feature 164 的 `ArchiveSelectiveExtractor` 与 Feature 167 的 7z Solid 流式跳过引擎，打造毫秒级低延迟的 macOS Quick Look 预览控制器 (`QuickLookPreviewCoordinator`) 与 Finder `NSItemProvider` 拖拽写入桥接，配合严格的临时内存/缓存生命周期自动回收机制，实现真正的系统级原生体验**。

---

## 2. User Scenarios

### User Scenario 1 (US1) - 归档内任意文件空格键毫秒级 Quick Look 预览
- **As a**: 经常查阅加密或大体积压缩包的设计师 / 开发者
- **I want to**: 在 TTZip 归档浏览器中选中一个 50MB 的 PDF 或 PNG，按下空格键
- **So that**: 预览浮窗在 3~10 毫秒内瞬间呈现，不阻塞主线程 UI，关闭预览后临时缓存自动释放。

### User Scenario 2 (US2) - 拖拽归档内文件直达访达 / 桌面 (Drag-to-Finder)
- **As a**: macOS 日常办公用户
- **I want to**: 将 7z/ZIP 压缩包内的单个文件直接拖拽至 Finder 某个文件夹
- **So that**: 文件即刻出现在目标目录中，无需先全量解压再寻找目标文件。

### User Scenario 3 (US3) - 临时缓存防御性生命周期管理与安全沙盒隔离
- **As a**: 关注磁盘空间与隐私安全的用户
- **I want to**: 频繁预览归档中的文件
- **So that**: 系统退出或窗口切换时，所有临时预览文件 100% 自动安全粉碎清理，零垃圾残留。

---

## 3. Functional Requirements

- **REQ-001 (SwiftUI Quick Look Coordinator)**: 实现 `QuickLookPreviewCoordinator`，封装 macOS `QLPreviewController` / `quickLookPreview`，监听键盘空格键事件并基于 `ArchiveSelectiveExtractor` 异步拉取单条目二进制流。
- **REQ-002 (Finder Drag-and-Drop Bridge)**: 为归档树行视图（`ArchiveRowView` / `CentralExplorerListView`）集成 `.onDrag` 与 `NSItemProvider`，支持 Promise-based 延迟流式提取写入到拖放目标 URL。
- **REQ-003 (Sandboxed Cache Manager & Ephemeral Guard)**: 实现 `EphemeralPreviewCacheManager`，在 `~/Library/Caches/com.wittkung.ttzip/PreviewCache/` 下创建临时隔离沙盒，支持 TTL 超时清理与 App 退出时 `removeAll()`。
- **REQ-004 (Solid 7z & Encrypted Archive Bridge)**: 自动感知 7z Solid 块与 AES-256 加密条目，若遇到加密则弹出单条目密码请求，解密后直通预览。
- **REQ-005 (Unit Test & A/B Zero-Regression Gating)**: 编写 `QuickLookPreviewTests` 与 `FinderDragDropExtractionTests`，并通过 `./scripts/benchmark_ab.sh` 5 轮交替采样验证无性能回归。

---

## 4. Success Criteria

1. **Quick Look 唤起延迟**: 选中常见文件（< 10MB）按下空格键至渲染浮窗呈现时间 $\le 15	ext{ ms}$；
2. **内存与磁盘零泄漏**: 连续预览 50 个不同文件后，驻留物理内存 $\le 50	ext{ MB}$，预览缓存目录在关闭时体积清零；
3. **测试覆盖率**: 单元测试 100% 通过；
4. **统计 A/B 门禁无回归**: `./scripts/benchmark_ab.sh HEAD WIP --runs 5` 保持 `PASSED_NO_REGRESSION`。
