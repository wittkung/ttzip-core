# Layout & Gutter Contracts

## 1. 布局容器顶格对齐契约 (Alignment Invariance Contract)

| 容器组件 | 强制声明的对齐规则 | 严禁行为 |
| :--- | :--- | :--- |
| `MainView.HStack` | `alignment: .top` | 严禁省略 alignment (默认 .center) |
| `MainView.detailArea` | `frame(..., alignment: .topLeading)` | 严禁裸用 `.clipped()` 掩盖尺寸 |
| `KeepAliveTabContainer.ZStack` | `alignment: .topLeading` | 严禁省略 alignment (默认 .center) |
| `TTZipWorkspaceScaffold` | `alignment: .topLeading` | 必须封装统一 38pt 顶距与 52pt 顶栏 |

## 2. 呼吸槽与浮动卡片几何契约 (Zen Gutter Contract)

- `GutterWidth`: 严格为 `8.0pt`；
- `SidebarTrailingBorder`: 移除常驻物理实线；
- `WorkspaceCardLeading`: 严格为 `8.0pt`（`TTZipTheme.Spacing.xs`）；
- `GoldenLinePosition`: $Y = 38.0 + 52.0 = 90.0\text{pt}$ 在所有 Tab 恒定成立。
