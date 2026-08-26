# Contract: Keep-Alive Tab Lifecycle & Re-Entrancy FSM

- **Specification**: `specs/019-systemic-architecture-and-quality-governance`
- **Domain**: SwiftUI Presentation, KeepAlive Tab Containers, Reactive View Models
- **Language Mode**: Swift 6 Strict Concurrency (`@Observable`, `@MainActor`)

---

## 1. Lifecycle Protocol Specification

```swift
public enum TabActivationPayload: Sendable, Equatable {
    case none
    case home(directoryURL: URL, selectedPath: String?)
    case compress(inputPaths: [String], targetDirectory: String?, presetID: UUID?)
    case presets(presetID: UUID?, autoEdit: Bool)
    case benchmark(customPath: String?, mode: BenchmarkMode?)
    case vault(requestUnlockFocus: Bool)
    case settings(tab: SettingsTab)
}

@MainActor
public protocol StatefulTabViewModelProtocol: AnyObject {
    /// Fired when tab enters active foreground (both initial appearance and subsequent re-activation).
    func onTabActivated(payload: TabActivationPayload)
    
    /// Fired when user leaves the tab for another workspace tab.
    func onTabDeactivated()
    
    /// Fired when already-active tab receives new dynamic parameters.
    func onReceiveDynamicPayload(_ payload: TabActivationPayload)
}
```

---

## 2. Container Contract & ViewModifier

```swift
public struct KeepAliveTabContainer<Content: View>: View {
    public init(
        activeTab: WorkspaceTab,
        currentPayload: TabActivationPayload = .none,
        @ViewBuilder content: @escaping (WorkspaceTab, Bool) -> Content
    )
}

extension View {
    public func onTabLifecycle(
        isActive: Bool,
        payload: TabActivationPayload,
        onActivate: @escaping (TabActivationPayload) -> Void,
        onDeactivate: @escaping () -> Void = {}
    ) -> some View
}
```

---

## 3. Invariants

1. **Re-activation Guarantee**: When `activeTab` switches to an already-visited tab, `onTabActivated` MUST fire with the current `payload`.
2. **Keyboard Event Isolation**: Global key monitors (`NSEvent.addLocalMonitorForEvents`) MUST be paused when `isActive == false` and resumed only when `isActive == true`.
3. **No Stale Inputs**: Re-entering the Compression Workspace with new paths MUST replace previous items and reset completion summary states.
