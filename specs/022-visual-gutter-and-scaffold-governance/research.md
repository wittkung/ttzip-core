# Research: TTZip 视觉呼吸槽、全 Tab 脚手架收敛与垂直对齐治理

## 1. 架构决策记录 (ADR)

### ADR 1: 全链路锁定 TopLeading 对齐并移除裸 `.clipped()`
- **Decision**: 在 `MainView.HStack(alignment: .top)`、`detailArea.frame(..., alignment: .topLeading)`、`KeepAliveTabContainer.ZStack(alignment: .topLeading)` 显式声明顶格对齐，并彻底移除 `MainView` 中的 `detailArea.clipped()`。
- **Rationale**: 根除 SwiftUI 隐式 `.center` 居中计算在首帧或数据未撑满时将卡片推入负 Y 坐标（$Y < 0$）随后被 GPU 物理削顶的恶性缺陷。
- **Alternatives Considered**: 在子组件内部添加固定 Spacer 或负 offset（脆弱且违背布局协商原则，被否决）。

### ADR 2: 引入 8.0pt Zen Gutter 隐式分界槽与 Inset 浮动卡片
- **Decision**: 将 `ResizableDividerHandle` 重构为宽度为 `8.0pt` 的隐式呼吸槽，常态完全透明，Hover/Drag 时动态显现金缮微光与触控拉手；`MacEditorialSidebar` 移除 0.5pt 常驻硬边线；`HomeExplorerContainerView` 采用标准 `padding(.leading, 8)`。
- **Rationale**: 消除 16pt 大圆角直接撞在直线上的漏斗形死角与三重线框重叠，使中央工作区恢复为现代 macOS Inset 浮动卡片，带来充足的呼吸感。
- **Alternatives Considered**: 保持 `leading: 0` 并把卡片左侧改为直角（破坏全局卡片语言一致性，被否决）。

### ADR 3: 强制实施 100% `TTZipWorkspaceScaffold` 统合
- **Decision**: 扩展 `TTZipWorkspaceScaffold`，使 `HomeExplorerContainerView`、`BenchmarkView`、`PasswordVaultView`（提升至根节点）、`MediaPreview`、`larkSync` 缺省态全部接入 Scaffold。
- **Rationale**: 唯一脚手架原则，统一管理 38pt 交通灯安全区、52pt 标准 Header 和 1.5pt 金线，彻底消灭手写重复代码与锁定/解锁状态切换时的 90pt 跳动。
- **Alternatives Considered**: 在各子页面分别修复各自的 Header（维护成本高且必然再次分叉，被否决）。

### ADR 4: 快捷目录 ScrollView 首端呼吸内边距
- **Decision**: 在 `DiskDirectoryBrowserView` 的快捷目录横向 ScrollView 内部首端增加 8pt 内边距。
- **Rationale**: 杜绝首项标签贴死视口边缘被渐变遮罩切除文字。
