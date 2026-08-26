# Contract Specification: TTZip Window & Layout Interface Contracts

## Subsystem: SwiftUI Presentation & Window Scaffolding

---

### Contract 1: `TTZipWorkspaceScaffold`
- **Type**: SwiftUI View Scaffold
- **Interface**:
  ```swift
  public struct TTZipWorkspaceScaffold<HeaderTrailing: View, Content: View>: View {
      public init(
          sectionName: String,
          title: String,
          isCardEnclosed: Bool = true,
          contentPadding: EdgeInsets = EdgeInsets(top: 0, leading: 0, bottom: 0, trailing: 0),
          @ViewBuilder headerTrailing: () -> HeaderTrailing,
          @ViewBuilder content: () -> Content
      )
  }
  ```
- **Invariants**:
  1. Top offset MUST be fixed at `38pt` (`padding(.top, 38)`).
  2. Header Bar MUST be fixed at `height: 52`.
  3. Kintsugi Gold Line MUST be positioned precisely at $Y = 90.0\text{pt}$ with `height: 1.5`.

---

### Contract 2: `TTFlowLayout`
- **Type**: SwiftUI Custom Layout Protocol Implementation
- **Interface**:
  ```swift
  public struct TTFlowLayout: Layout {
      public init(horizontalSpacing: CGFloat = 6, verticalSpacing: CGFloat = 6)
  }
  ```
- **Behavior**:
  1. Calculates subview sizes and dynamically wraps to the next line when exceeding container width.
  2. Guarantees 0 horizontal boundary overflow.

---

### Contract 3: Workspace Tab Right Panel Policy
- **Type**: State Machine Policy
- **Invariant**:
  ```swift
  let isRightPanelAvailable: Bool = (viewModel.activeTab == .home && viewModel.selectedDiskItem != nil)
  ```
  All other workspace tabs (`compressWorkspace`, `presets`, `benchmark`, `vault`, `plugins`, `settings`) MUST have `isRightPanelAvailable == false`.
