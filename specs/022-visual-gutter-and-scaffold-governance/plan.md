# Implementation Plan: TTZip 视觉呼吸槽、全 Tab 脚手架收敛与垂直对齐治理

- **Feature**: `specs/022-visual-gutter-and-scaffold-governance`
- **Specification**: `specs/022-visual-gutter-and-scaffold-governance/spec.md`
- **Classification**: `[Full SDD]`
- **Status**: Ready for Tasks

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **UI Framework**: Native SwiftUI 6 on macOS (Apple Silicon optimized).
- **Core Design Tokens**: `TTZipTheme` (Zen Gold, Bamboo Green, Paper White, 52pt Header, 38pt Safe Area).
- **Layout Paradigm**: 3-Column Editorial Inset Layout with 8pt Zen Gutter.

### 1.2 Constitution Check
- **Principle I (Pure Rust Core & Swift 6 Interop)**: UI changes strictly confined to presentation layer; no Rust FFI regression.
- **Principle II (Defensive Systems Architecture)**: Layout negotiation defensively handles all extreme window dimensions.
- **Principle III (Single File LOC $\le 800$)**: All modified files kept strictly under 550 LOC.
- **Principle IV (Local CI/CD Gate Enforcement)**: `swift build` and `swift test` 100% passed without `--no-verify`.

---

## 2. Proposed Changes

### Component 1: 容器顶格对齐与消灭裁切 (US1)
- **[MODIFY] [MainView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/MainView.swift)**:
  - `HStack(spacing: 0)` -> `HStack(alignment: .top, spacing: 0)`
  - `detailArea.frame(..., alignment: .topLeading)` 并移除裸 `.clipped()`
- **[MODIFY] [KeepAliveTabContainer.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Components/KeepAliveTabContainer.swift)**:
  - `ZStack` -> `ZStack(alignment: .topLeading)`

### Component 2: 8pt 呼吸槽与 Inset 浮动卡片 (US2)
- **[MODIFY] [ResizableDividerHandle.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/Components/ResizableDividerHandle.swift)**:
  - 重构为 `gutterWidth: 8.0pt` 隐式呼吸槽，常态透明，Hover/Drag 浮现金线拉手。
- **[MODIFY] [MacEditorialSidebar.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/Sidebar/MacEditorialSidebar.swift)**:
  - 移除右侧常驻 0.5pt 硬边框。
- **[MODIFY] [HomeExplorerContainerView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift)**:
  - 移除 `leading: 0`，改为标准 `padding(.leading, 8)`。

### Component 3: 全 Tab 100% 脚手架统合与 Y=90pt 金线贯通 (US3)
- **[MODIFY] [PasswordVaultView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/PasswordVaultView.swift)**:
  - 顶层接入 `TTZipWorkspaceScaffold`，锁定/解锁仅切换 content，消灭 90pt 锁态跳变与断层。
- **[MODIFY] [PasswordVaultLockedView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/PasswordVaultLockedView.swift)** & **[PasswordVaultUnlockedView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/PasswordVaultUnlockedView.swift)**:
  - 移除手写 Header 与金线。
- **[MODIFY] [BenchmarkView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/BenchmarkView.swift)**:
  - 接入 `TTZipWorkspaceScaffold`。
- **[MODIFY] [MainView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/MainView.swift)**:
  - 补齐 `MediaPreview` 与 `larkSync` 缺省态脚手架。

### Component 4: 快捷目录栏与细节打磨 (US4)
- **[MODIFY] [DiskDirectoryBrowserView.swift](file:///Users/kevintung/Documents/dev/products/ttzip/apple/Sources/TTZipApp/Views/Explorer/DiskDirectoryBrowserView.swift)**:
  - 快捷目录 ScrollView 首端增加 `padding(.leading, 8)` 消除首字裁切。
