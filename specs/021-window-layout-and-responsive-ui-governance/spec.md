# Feature Specification: TTZip Window Layout & Responsive UI Governance

- **Feature Directory**: `specs/021-window-layout-and-responsive-ui-governance`
- **Type**: [Full SDD]
- **Target Subsystem**: `apple/Sources/TTZipApp/` (SwiftUI Presentation & AppKit Windowing)
- **Status**: Planned

---

## 1. 业务背景与问题定义 (Context & Objectives)

TTZip 作为面向 Apple Silicon 的高性能原生归档与压缩工具，在多栏桌面交互、窗口缩放自适应、macOS 交通灯安全区避让及复杂表单流式排版方面存在多处架构与实现级缺陷：
1. **新建归档模式错误激活右侧栏**：在 `activeTab == .compressWorkspace` 时错误渲染多余的磁盘目录树，吞噬近 40% 的中央工作区横向空间；
2. **窗口最小物理基线脱节**：`minWidth: 460, minHeight: 380` 导致多栏布局无断点压缩，造成严重的界面破坏性重叠与文字省略号截断；
3. **安全区与 Y=90pt 金线割裂**：各 Tab 顶栏高度与 Top Padding 分散硬编码，导致 `PresetWorkspaceView` (Y=68pt)、`PluginsView` (Y=52pt) 与左侧边栏 (Y=90pt) 垂直断层并侵入交通灯安全区；
4. **悬浮 Omnibar 搜索框硬编码 480pt**：在小窗口下破框遮挡左侧红黄绿交通灯与右侧工具栏按钮；
5. **表单网格与列表缺陷**：压缩等级瓦片被打上省略号、分卷选项 HStack 横向溢出、嵌套 `List` 导致 macOS 滚轮事件劫持与死锁；
6. **米勒列与检查器排版缺陷**：米勒列键盘导航不平滑滚动视口、Folder 画板胶囊按钮 320~390pt 破框截断、Inspector 键值对窄屏截断；
7. **辅助板块缺陷**：预设编辑器遗漏格式/等级选择器孤岛断链、密码锁定态 526pt 缺少 ScrollView 矮窗口裁切、算法矩阵缺少 ScrollView。

---

## 2. 功能需求清单 (Functional Requirements)

### FR-01: 工作区拓扑与右侧栏隔离
- `isRightPanelAvailable` 仅在 `activeTab == .home && selectedDiskItem != nil` 时为 `true`。
- `compressWorkspace`、`presets`、`benchmark`、`vault`、`plugins`、`settings` 等所有全功能工作区严格独占 DetailArea 全宽。
- `RightInspectorSidePanel` 彻底移除 `activeTab == .compressWorkspace` 分支及嵌套的 `DiskDirectoryBrowserView`。

### FR-02: 响应式三级断点自适应引擎 (Responsive Breakpoints)
- 窗口最小物理基线设定为 `minWidth: 520, minHeight: 400`。
- 定义 `WindowLayoutTier`:
  - `compact` (`W < 820pt`): 左侧栏折叠至 64pt 图标轨，右侧栏隐藏，中央独占 `W - 64pt`。
  - `medium` (`820pt <= W < 1100pt`): 左侧栏标准 200pt，右侧 Inspector 选中时展开 240~280pt。
  - `expanded` (`W >= 1100pt`): 三栏自由停靠与拉伸，支持 3~5 级米勒列并排。

### FR-03: 统一工作区脚手架与 Y=90pt 金线绝对对齐
- 引入 `TTZipWorkspaceScaffold`，强制封装 38pt 交通灯安全区避让、52pt 标准 Header Bar、1.5pt Kintsugi Gold 金线与 16pt 浮动玻璃岛容器。
- `PluginsView`、`BenchmarkView`、`PasswordVaultView`、`PresetWorkspaceView`、`SettingsView` 全量接入。

### FR-04: 弹性 Omnibar 与交通灯防破框隔离
- `MainView` 顶部两侧保留 `Spacer(minLength: 140)` 交通灯与 Toolbar 刚性安全缓冲区。
- `LiquidGlassOmnibar` 废除固定 480pt 宽度，使用 `minWidth: 180, idealWidth: 380, maxWidth: min(480, totalWidth - 280)`。
- `BreadcrumbPathBarView` 在宽度紧张时自动应用 `~ / ... / Parent / Current` 动态折叠算法。

### FR-05: 流式折行表单与扁平弹性文件列表
- 压缩等级瓦片保证 `minWidth: 110pt` 配合 `minimumScaleFactor(0.85)`，彻底消除省略号。
- 实现 `TTFlowLayout` 替代分卷 `HStack`，支持按内容尺寸自适应流式折行。
- 移除 `CompressFileListView` 内部的 `List` 嵌套，改用纯 `VStack + ForEach` 扁平渲染，消除滚轮劫持。

### FR-06: 米勒列平滑居中与 Inspector 弹性排版
- 米勒列增加对 `hoveredColumnIndex` / `activeColumnIndex` 的 `onChange` 联动平滑居中滚动。
- `FolderMediaArtboardView` 胶囊按钮重构为三阶断点自适应（>380pt 完整标签，260~380pt 简写标签，<260pt 纯图标）。
- 属性键值对使用 `ViewThatFits` 实现宽屏单行、窄屏上下双行自动降级排版。

### FR-07: 辅助功能板块与设置闭环
- `PresetEditorCardView` 完整接入 `PresetFormatOptionTile` 与 `PresetLevelOptionTile`。
- `PasswordVaultLockedView` 外层包裹 `ScrollView`，修复矮窗口按钮裁切。
- `AlgorithmMatrixSheetView` 补充 `ScrollView`，迁移至 `TTZipTheme` 语义色与 `AppLocalizationState`。

---

## 3. 非功能性需求 (Non-Functional Requirements)

- **NFR-01 (Swift 6 纯粹性与编译零告警)**: 所有改动 100% 通过 `swift build` 与 `swift test` 0 错误、0 警告编译。
- **NFR-02 (单文件行数治理)**: 所有新增/修改文件单文件行数 $\le 800$ LOC。
- **NFR-03 (设计系统契约)**: 100% 遵守 `ttzip-ui-design-system` 规范，严禁直接使用系统原生色。
