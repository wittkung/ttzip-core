# Phase 1: Data Model & Layout Entity Specifications

## Feature: 021-window-layout-and-responsive-ui-governance

---

### Entity 1: WindowLayoutTier (窗口响应式三级断点)

```swift
public enum WindowLayoutTier: Equatable, Sendable {
    case compact    // Total Width < 820pt (紧凑模式：左侧 64pt 图标轨，右侧栏隐藏，聚焦主工作区)
    case medium     // 820pt <= Total Width < 1100pt (标准桌面模式：左侧 200pt 导航，右侧按需泊入)
    case expanded   // Total Width >= 1100pt (宽屏专业模式：三栏全展开，多级米勒列流畅并排)
    
    public static func evaluate(width: CGFloat) -> WindowLayoutTier {
        if width < 820 { return .compact }
        if width < 1100 { return .medium }
        return .expanded
    }
}
```

---

### Entity 2: TTZipWorkspaceScaffold (统一工作区脚手架)

- **Header Bar Height**: `52pt` (`TTZipTheme.Layout.headerBarHeight`)
- **Top Safe Area Offset**: `38pt` (`TTZipTheme.Layout.topBarOffset`)
- **Golden Line**: `1.5pt` (`TTZipTheme.Layout.kintsugiGoldLineHeight`) at $Y = 90.0\text{pt}$
- **Card Background**: `Color.primary.opacity(0.025)`
- **Card Border**: `Color.primary.opacity(0.07)` 1px line, Continuous 16pt corner radius.

---

### Entity 3: BreadcrumbSegment & Dynamic Collapse State

```swift
public struct BreadcrumbSegment: Identifiable, Equatable {
    public let id: String
    public let title: String
    public let fullURL: URL
    public let isRoot: Bool
    public let isLast: Bool
}
```

- **Dynamic Collapse Algorithm**:
  - If `segments.count > 3` and `maxAvailableWidth < 340pt`:
    - Result: `[segments[0], EllipsisSegment("..."), segments[count-2], segments[count-1]]`
  - Otherwise:
    - Result: Full `segments` array.
