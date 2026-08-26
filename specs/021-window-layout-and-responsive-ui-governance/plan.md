# Implementation Plan: TTZip Window Layout & Responsive UI Governance

- **Feature Directory**: `specs/021-window-layout-and-responsive-ui-governance`
- **Branch**: `main`
- **Specification**: `specs/021-window-layout-and-responsive-ui-governance/spec.md`
- **Research**: `specs/021-window-layout-and-responsive-ui-governance/research.md`

---

## 1. Technical Context & Constitution Compliance

- **Target Ecosystem**: macOS 14.0+ (SwiftUI 6 + AppKit native bridge)
- **Design System**: TTZip Design System v2.5.0 (WSJ Typography + Kintsugi Gold Y=90pt + Bamboo Green)
- **Constitution Gates**:
  - `UniFFI Mandatory Standard`: UI 变更不破坏底层 UniFFI 数据绑定与异步契约。
  - `Single-File LOC Threshold`: 所有新增/修改文件单文件行数 $\le 800$ LOC。
  - `Zero In-Tree Path Invariant`: 保证零相对源码树路径依赖。

---

## 2. Proposed Changes & Implementation Phases

### Phase 1: Core Layout Components & Window Infrastructure
- [NEW] `apple/Sources/TTZipApp/Views/Components/TTZipWorkspaceScaffold.swift`:
  封装 38pt 交通灯安全区、52pt 标准 Header、1.5pt 金线与 16pt 浮动卡片。
- [NEW] `apple/Sources/TTZipApp/Views/Components/TTFlowLayout.swift`:
  基于 SwiftUI `Layout` 协议实现的高性能自适应流式折行容器。
- [MODIFY] `apple/Sources/TTZipApp/TTZipApp.swift`:
  提升窗口最小物理尺寸至 `minWidth: 520, minHeight: 400`。
- [MODIFY] `apple/Sources/TTZipApp/Views/MainView.swift`:
  - 修复 `isRightPanelAvailable`（仅在 `.home` 且有选中项时可用）；
  - 引入 `WindowLayoutTier`（Compact, Medium, Expanded）断点状态机；
  - 弹性化改造 `LiquidGlassOmnibar` 并在两侧保留 140pt 交通灯缓冲；
  - 移除 ZStack 绝对定位的浮动 Toggle 按钮。

### Phase 2: Compression Modal & Form Engine Overhaul
- [MODIFY] `apple/Sources/TTZipApp/Views/CompressFileListView.swift`:
  移除嵌套 `List`，改用纯 `VStack + ForEach` 扁平渲染，消除滚轮劫持。
- [MODIFY] `apple/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift`:
  分卷压缩与清理 Toggle 接入 `TTFlowLayout`。
- [MODIFY] `apple/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView+Components.swift`:
  级别瓦片保证最小 110pt 宽度，消除省略号截断。
- [MODIFY] `apple/Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift`:
  移除 `compressWorkspace` 分支及嵌套的 `DiskDirectoryBrowserView`。

### Phase 3: Explorer & Inspector Responsive Enhancement
- [MODIFY] `apple/Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift`:
  增加 `hoveredColumnIndex` / `activeColumnIndex` 的 `onChange` 平滑滚动居中。
- [MODIFY] `apple/Sources/TTZipApp/Views/Explorer/FolderMediaArtboardView.swift`:
  工具栏重构为三阶断点胶囊，修复 320~390pt 破框溢出。
- [MODIFY] `apple/Sources/TTZipApp/Views/Explorer/FolderMediaArtboardView+Grid.swift`:
  属性键值对使用 `ViewThatFits` 实现宽屏单行、窄屏双行自适应。
- [MODIFY] `apple/Sources/TTZipApp/Views/Components/BreadcrumbPathBarView.swift`:
  增加动态路径折叠算法。

### Phase 4: Module Harmonization & Bug Remediation
- [MODIFY] `apple/Sources/TTZipApp/Views/Presets/PresetEditorCardView.swift`:
  重新集成 `PresetFormatOptionTile` 与 `PresetLevelOptionTile`。
- [MODIFY] `apple/Sources/TTZipApp/Views/PresetWorkspaceView.swift`:
  接入 `TTZipWorkspaceScaffold`，修复 Y=68pt 错位。
- [MODIFY] `apple/Sources/TTZipApp/Views/Vault/PasswordVaultLockedView.swift`:
  包裹 `ScrollView`，修复 500pt 矮窗口按钮裁切。
- [MODIFY] `apple/Sources/TTZipApp/Views/Vault/PasswordVaultUnlockedView.swift`:
  顶栏增加 `Spacer()` 避让，接入脚手架。
- [MODIFY] `apple/Sources/TTZipApp/Views/Plugins/PluginsView.swift`:
  接入 `TTZipWorkspaceScaffold`，修复 Y=52pt 顶栏与安全区错位。
- [MODIFY] `apple/Sources/TTZipApp/Views/SettingsView.swift`:
  接入 `TTZipWorkspaceScaffold`，子选项卡支持横向滚动。
- [MODIFY] `apple/Sources/TTZipApp/Views/CompressionSummarySheetView.swift`:
  算法矩阵包裹 `ScrollView`，迁移至 `TTZipTheme` 标准语义色与 `AppLocalizationState`。

---

## 3. Verification & Governance
- 运行 `swift build` 与 `swift test` 确保 0 错误、0 警告。
- 运行 contracts lint 校验设计工件。
