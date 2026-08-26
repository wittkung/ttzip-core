# Phase 0: Research & Decision Records

## Feature: 021-window-layout-and-responsive-ui-governance

---

### Research Item 1: Window Architecture & Responsive Breakpoints
- **Unknown/Problem**: 如何在 SwiftUI + AppKit 混合架构中实现无破损的三栏桌面级自适应折叠？
- **Decision**: 建立 `WindowLayoutTier`（Compact `<820pt`, Medium `820~1100pt`, Expanded `≥1100pt`），窗口最小物理尺寸提升至 `520x400`。
- **Rationale**: 
  - 在 Compact 模式下，左侧边栏自动进入 64pt 图标栏，右侧 Inspector 自动隐藏，中央 DetailArea 获得最大空间；
  - 避免了 `HStack` 在空间不足时压缩子视图产生破坏性截断。
- **Alternatives Considered**: 纯 AppKit `NSSplitViewController` 改造（改造成本高，破坏现有 SwiftUI 声明式状态同步）。
- **Source**: macOS HIG Window Management, `apple/Sources/TTZipApp/TTZipApp.swift`, `MainView.swift`.

---

### Research Item 2: Safe Area & Golden Line Alignment Standard
- **Unknown/Problem**: 如何根治各 Tab 分散编写 `padding(.top, 38)` 造成的 Y=90pt 金线错位？
- **Decision**: 封装标准脚手架 `TTZipWorkspaceScaffold`。
- **Rationale**: 
  - 顶栏高度 52pt、金线 Y=90pt 和 38pt 交通灯安全区在脚手架内部闭环，业务视图无权也无需自行声明 padding；
  - 一劳永逸消除 `PresetWorkspaceView` (Y=68pt)、`PluginsView` (Y=52pt) 的垂直断层。
- **Alternatives Considered**: 在各子视图中手动逐一修正（容易在未来开发中再次发生样板代码衰减）。
- **Source**: `ttzip-ui-design-system` Manual v2.5.0, `TTZipTheme.swift`.

---

### Research Item 3: Liquid Glass Omnibar Safe Area & Elastic Shrink
- **Unknown/Problem**: 为什么悬浮 Omnibar 会在窄屏下遮挡 macOS 红黄绿交通灯？
- **Decision**: 
  1. 在 `MainView` 顶部两侧强制保留 `Spacer(minLength: 140)` 刚性缓冲区；
  2. Omnibar 废除固定 480pt，升级为弹性宽度 `minWidth: 180, idealWidth: 380, maxWidth: min(480, totalWidth - 280)`；
  3. `BreadcrumbPathBarView` 引入动态折叠算法 (`~ / ... / Parent / Current`)。
- **Rationale**: 彻底阻断任何组件侵入左上角交通灯物理区域 ($X \in [12, 70]\text{pt}$)，保证窗口拖拽与控制可用。
- **Alternatives Considered**: 在窄屏时完全隐藏 Omnibar（损失路径与搜索核心能力）。
- **Source**: `LiquidGlassOmnibar.swift`, `BreadcrumbPathBarView.swift`.

---

### Research Item 4: Elimination of Nested List Scroll Hijacking & FlowLayout
- **Unknown/Problem**: 压缩模态框内部 `List(...).frame(height: 140)` 为什么会卡死外层 ScrollView？
- **Decision**: 
  1. 移除 `List`，改用纯 `VStack + ForEach` 扁平渲染，根据文件数量（1~4 个紧凑自适应，>4 个限高 180pt 渐变遮罩）；
  2. 实现原生 `TTFlowLayout` 替代分卷 `HStack` 与长文本 Toggle 网格。
- **Rationale**: 消除嵌套 `NSScrollView` 滚轮捕获冲突，实现 macOS 物理级丝滑滚动。
- **Alternatives Considered**: 使用 AppKit `NSViewRepresentable` 拦截滚轮事件向上转发（脆弱且存在版本兼容性问题）。
- **Source**: SwiftUI Layout Protocol, `CompressFileListView.swift`.
