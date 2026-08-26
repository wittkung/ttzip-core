# Data Model: 视觉呼吸槽与工作区脚手架契约模型

## 1. 布局模型与间距契约 (Layout & Spacing Model)

```
+---------------------------------------------------------------------------------------------------+
| Window (Total Width = W, Total Height = H >= 400)                                                  |
| Top Safe Area Offset = 38.0 pt (Traffic Lights Zone Y in [0, 38])                                 |
+---------------------------------------------------------------------------------------------------+
|                                                                                                   |
| [Sidebar]          [Zen Gutter 8pt]  [Central Workspace Inset Card]        [Right Inspector Card] |
| Width: 200pt       Width: 8pt        Top: 38pt, Leading: 8pt, Bottom: 16pt  Top: 38pt, Right: 12pt  |
| Top: 38pt          (Implicit Drag)   Header: 52pt                           Header: 52pt          |
| Header: 52pt                         -----------------------------------   ---------------------  |
| -----------------                    Golden Line strictly at Y = 90.0 pt   Golden Line Y=90.0pt   |
| Golden Line                          Content Slot (Scrollable)             Content Slot           |
| Y = 90.0 pt                                                                                       |
+---------------------------------------------------------------------------------------------------+
```

## 2. 核心间距常量 (Design Tokens)

- `TTZipTheme.Layout.topBarOffset` = `38.0` (交通灯安全区)
- `TTZipTheme.Layout.headerBarHeight` = `52.0` (WSJ 标准顶栏)
- `TTZipTheme.Layout.kintsugiGoldLineHeight` = `1.5` (金缮分割线)
- `ResizableDividerHandle.gutterWidth` = `8.0` (分界呼吸槽)
- `CentralCard.leadingPadding` = `8.0` (中央卡片呼吸留白)
- `GoldenLine.absoluteY` = `38.0 + 52.0 = 90.0` (全产品线黄金水平线)
