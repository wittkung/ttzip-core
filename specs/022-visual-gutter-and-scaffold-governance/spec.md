# Feature Specification: TTZip 视觉呼吸槽、全 Tab 脚手架收敛与垂直对齐治理 (022)

- **Feature Directory**: `specs/022-visual-gutter-and-scaffold-governance`
- **Created**: 2026-08-26
- **Classification**: `[Full SDD]`
- **Status**: Draft

---

## 1. 业务背景与问题描述

在 021 窗口响应式治理后，真实截图与深入调研暴露了以下深层 UI 架构与视觉对齐缺陷：
1. **Header 裁切与半截文字**：`MainView` 中的 `HStack`、`detailArea` 和 `KeepAliveTabContainer` 存在三层隐式 `.center` 居中对齐，在首帧异步数据尚未撑满时，中央卡片被向上推入负 Y 坐标（$Y < 0$），随后被 `detailArea.clipped()` 物理裁切，导致 `EXPLORER / 文件浏览器` 标题与右侧胶囊只露出下半截文字。
2. **左侧栏与中央工作区分界死板无呼吸感**：`MacEditorialSidebar` 带有 0.5pt 硬边线，`ResizableDividerHandle` 带有 1.0pt 灰线，且 `HomeExplorerContainerView` 声明了 `padding(.leading, 0)`，导致 16pt 大圆角卡片生硬撞死在分割线上，产生漏斗形死角与三重线框重叠，缺乏 macOS Inset Floating Island 呼吸感。
3. **三栏 Y=90pt 金线在特定状态下断层**：`PasswordVaultLockedView`（锁定态）缺失 38pt 顶距、52pt 顶栏与金线，从 $Y=0$ 顶格渲染并遮挡交通灯；解锁后突然弹出金线产生 90pt 剧烈跳变；`MediaPreview` 与 `larkSync` 缺省态同样缺少统一脚手架。
4. **快捷目录首字裁切**：`DiskDirectoryBrowserView` 快捷目录 ScrollView 首端缺少 8pt 呼吸内边距，第一个标签 `文档` 的“文”字被截断。

---

## 2. 目标与验收标准 (Acceptance Criteria)

### User Story 1 (US1): 消除垂直居中漂移与 Header 裁切
- **AC 1.1**: 全链路容器（`MainView.HStack`、`detailArea`、`KeepAliveTabContainer.ZStack`）显式锁定 `alignment: .topLeading` 或 `alignment: .top`。
- **AC 1.2**: 移除 `detailArea` 上的裸 `.clipped()` 修饰符，通过尺寸协商而非粗暴裁切处理布局。
- **AC 1.3**: 无论是冷启动首帧、异步数据加载中还是窗口任意高度下，中央工作区 Header 标题与右侧胶囊 100% 完整可见，0 裁切。

### User Story 2 (US2): 8~12pt 呼吸槽 (Zen Gutter) 与 Inset 浮动卡片
- **AC 2.1**: `ResizableDividerHandle` 重构为 `gutterWidth: 8.0pt` 隐式呼吸槽，常态透明，仅在 Hover/Drag 时点亮金缮拉手与微光。
- **AC 2.2**: `MacEditorialSidebar` 移除常驻 0.5pt 硬边框，融入和纸通透材质。
- **AC 2.3**: `HomeExplorerContainerView` 移除 `leading: 0`，改为标准 `padding(.leading, 8)`，与全 App 其余 Tab 保持 100% 一致的 Inset Floating Island 优雅形态。

### User Story 3 (US3): 全 Tab 100% 脚手架收敛与 Y=90pt 绝对平齐
- **AC 3.1**: `HomeExplorerContainerView`、`BenchmarkView`、`PasswordVaultView`（含锁定与解锁态）、`MediaPreview`、`larkSync` 缺省态全量收敛至 `TTZipWorkspaceScaffold`。
- **AC 3.2**: 密码保险箱在锁定/解锁状态切换时，顶栏与金线保持物理绝对静止，消灭 90pt 视觉跳变。
- **AC 3.3**: 全产品线 7 大 Tab 在任意窗口尺寸下，三栏 Kintsugi Gold Line 严格对齐于 **$Y = 90.0\text{pt}$**。

### User Story 4 (US4): 快捷目录栏与细节打磨
- **AC 4.1**: `DiskDirectoryBrowserView` 快捷目录 ScrollView 首端增加 `padding(.leading, 8)`，首项文字 100% 完整显示。
