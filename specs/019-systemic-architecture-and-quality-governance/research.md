# Research & Architectural Audit: 019 Systemic Architecture & Quality Governance Hardening

- **Feature Directory**: `specs/019-systemic-architecture-and-quality-governance`
- **Date**: 2026-08-25
- **Status**: Completed
- **Authors**: Antigravity AI & TTZip Multi-Agent Research Cluster

---

## 1. Domain 1: Intent Routing & Multi-Entrypoint Architecture

### 1.1 Five-Entrypoint Codebase Audit

| Entrypoint | Source Files | Current Mechanism | Critical Vulnerabilities & Defects |
| :--- | :--- | :--- | :--- |
| **1. macOS FinderSync Extension** | `TTZipFinderSync/FinderSync.swift`<br>`TTZipCore/FinderSyncHelper.swift` | `ttzip://action?type=\(action)&paths=\(paths)` | 5/10 actions (`inspect_archive`, `autofill_password`, `compute_hash`, `compress_separate`, `compress_and_delete_source`) fall into `default: break` or open compress workspace incorrectly. Multi-path parsing breaks on POSIX paths with `\|`. |
| **2. URL Schemes & AppKit Delegation** | `TTZipApp/TTZipApp.swift`<br>`TTZipApp/AppDelegate.swift`<br>`TTZipApp/MainView+Toolbar.swift` | `NSApplicationDelegate.openFiles`<br>SwiftUI `.onOpenURL` | Dual `.onOpenURL` in `TTZipApp` and `MainView` causes uncoordinated state race conditions. Lack of UTI/extension checks routes non-archives to archive reader. |
| **3. SwiftUI Drag & Drop** | `HomeDropZoneView.swift`<br>`SingleMillerColumnView.swift`<br>`MillerColumnItemRowView.swift`<br>`CompressFileListView.swift` | `NSItemProvider.loadItem(forTypeIdentifier:)` | **Critical Bug**: `HomeDropZoneView` attempts `item as? Data`, which fails silently on macOS where AppKit yields `NSURL`/`URL`, completely breaking drag-and-drop. Dropping on archive file attempts directory move instead of in-place mutation. |
| **4. In-App Context Menus** | `MillerColumnItemRowView+ContextMenu.swift`<br>`SingleMillerColumnView.swift`<br>`ArchiveExplorerView.swift` | View callbacks + ad-hoc `NotificationCenter` posts | Inconsistent multi-path encoding (`\n` vs `\|` vs `[String]`). "Quick Look" menu item calls `activateFileViewerSelecting` (Finder reveal) instead of native QuickLook panel. Untyped notifications with `Any?` payloads. |
| **5. Top-Level AppKit Menu & Toolbar** | `TTZipMenuCommands.swift`<br>`AppKitMenuSynchronizer.swift`<br>`MainView+Toolbar.swift` | `NotificationCenter.post`<br>SwiftUI `.keyboardShortcut` | **Dead Notifications**: Cmd+N and Cmd+O post `"TTZip_TriggerNewArchive"` and `"TTZip_TriggerOpenArchive"` which have **ZERO listeners** in the entire codebase. Toolbar shortcuts hidden when `currentArchivePath == nil`. |

### 1.2 Unified `AppIntent` & `AppIntentDispatcher` Architecture

```
                               ┌────────────────────────┐
                               │   Incoming Triggers    │
                               │ (5 Entrypoint Sources) │
                               └───────────┬────────────┘
                                           │
                                           ▼
                               ┌────────────────────────┐
                               │    AppIntentParser     │
                               │  • URL Normalization   │
                               │  • NSItemProvider Safe │
                               │  • Path Sanitization   │
                               └───────────┬────────────┘
                                           │ (AppIntentEnvelope)
                                           ▼
                               ┌────────────────────────┐
                               │  AppIntentDispatcher   │
                               │     (@MainActor)       │
                               │ • Frontmost Activation │
                               │ • State Coordination   │
                               └───────────┬────────────┘
                                           │
                    ┌──────────────────────┼──────────────────────┐
                    ▼                      ▼                      ▼
          ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐
          │   AppViewState    │  │ KeepAlive Tab FSM │  │ In-Place Engine   │
          │ (NavigationState) │  │  (Session Reload) │  │   (Async Action)  │
          └───────────────────┘  └───────────────────┘  └───────────────────┘
```

---

## 2. Domain 2: SwiftUI Lifecycle & KeepAlive Tab Architecture

### 2.1 Tab Persistence & Observation Models

| Workspace Tab | View Container | ViewModel / State | Observation Model | Lifecycle Vulnerabilities |
| :--- | :--- | :--- | :--- | :--- |
| `.home` | `HomeExplorerContainerView` | `AppViewState` | `@Observable` + `@MainActor` | `DiskDirectoryBrowserView` caches initial `rootDirectory` in `@State`, ignoring external Omnibar/CLI navigation. `FinderMillerColumnsView` global `NSEvent` key monitor never unregisters in `.onDisappear`, hijacking arrow keys in all other tabs. |
| `.compressWorkspace` | `CompressModalView` | `CompressFormSession` | `@Observable` + `@MainActor` | Session stored in view-local `@State`. Re-entrant activation with identical paths or empty paths is ignored by `.onChange`. Lingering completion sheets remain visible upon re-entry. |
| `.presets` | `PresetWorkspaceView` | `PresetWorkspaceViewModel` | `ObservableObject` (Legacy Combine) | Preset list loaded only on `init()`. Mutations in Compress modal do not reflect in Preset workspace. |
| `.benchmark` | `BenchmarkView` | `BenchmarkViewModel` | `ObservableObject` (Legacy Combine) | Synthetic memory buffers retained in background indefinitely. |
| `.vault` | `PasswordVaultView` | `PasswordVaultViewModel` | `ObservableObject` (Legacy Combine) | Password focus and lock-state refresh bound only to single `.onAppear`. |
| `.settings` | `SettingsView` | Fragmented `@AppStorage` | None | Decentralized state prevented by lack of unified ViewModel. |

### 2.2 Formal `StatefulTabProtocol` & Lifecycle Modifier

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

public protocol StatefulTabViewModelProtocol: AnyObject {
    func onTabActivated(payload: TabActivationPayload)
    func onTabDeactivated()
    func onReceiveDynamicPayload(_ payload: TabActivationPayload)
}
```

---

## 3. Domain 3: Testing Infrastructure & State Transition Coverage

### 3.1 Test Suite Audit (195 Swift Tests + Rust Microkernels)

* **Current Coverage**:
  * `apple/Tests/TTZipAppTests` (127 Tests): High coverage on pure algorithms (`CommandPatternTests` 21, `DiskSortOptionTests` 15, `POSIXPathSanitizerTests` 8, `AsyncPathAutocompletionTests` 9, `ArchivePrototypeTests` 10).
  * `core/Tests/TTZipTests` (68 Tests): Format parsers, UniFFI C-ABI symbols, Ed25519 verifier, APFS CoW, memory sanitization (`memset_s`).
* **Identified Critical Gaps**:
  * **Zero State Transition Integration Tests**: No tests verifying navigation state machine transitions under active operations.
  * **Zero Cross-Tab Intent Injection Tests**: No tests verifying that an incoming intent correctly refreshes cached KeepAlive tabs without dropping payload.
  * **Zero URL Scheme Roundtrip Parsing Tests**: No tests verifying `ttzip://` query extraction, pipe-splitting, and percent-decoding edge cases (e.g. CJK, spaces).
  * **Zero Cross-Process Language Notification Tests**: No tests verifying Darwin CFNotification delivery between Main App, FinderSync, and QuickLook.

### 3.2 Designed State Transition Test Architecture

```
TTZipAppTests/
├── AppNavigationStateFlowTests.swift      (Tab transitions, KeepAlive retention, Overlay stacking)
├── FinderSyncIntentMappingTests.swift     (URL scheme parsing, 10 action identifiers, Darwin notifications)
└── Harnesses/
    ├── MockFileURLHarness.swift           (Temporary sandbox file/folder generation & RAII cleanup)
    ├── MockDarwinNotificationHarness.swift (CFNotificationCenter Darwin event capture)
    └── KeepAliveTabHarness.swift          (Simulated KeepAlive container state transitions)
```

---

## 4. Domain 4: Build System, Compiler Diagnostics & Release Engineering

### 4.1 Build Scripts & Compiler Flags Audit

| File | Identified Issue | Severity | Proposed Fix |
| :--- | :--- | :--- | :--- |
| `core/Install-TTZip.command` | Invokes `swift build --product TTZipApp` inside `core/` where `TTZipApp` target does not exist. | **High** | Delegate GUI builds to `apple/scripts/bundle_app.sh --release`. |
| `core/Package.swift` | Contains `swiftSettings: [.unsafeFlags(["-no-whole-module-optimization", "-enable-batch-mode"])]`, disabling WMO in release mode. | **High** | Remove `.unsafeFlags`; enable `.enableUpcomingFeature("StrictConcurrency")`. |
| `apple/Package.swift` | Missing strict concurrency and `-warnings-as-errors` flags. | **Medium** | Add `.enableUpcomingFeature("StrictConcurrency")` and `-warnings-as-errors`. |
| `apple/scripts/bundle_app.sh` | Omit `strip -x` in release; misses `--options runtime --timestamp` for Sparkle Developer ID signing. | **Medium** | Add symbol stripping and hardened runtime timestamp flags. |
| Monorepo Hygiene | Orphaned directories in `core/Sources/` (`TTZipApp`, `TTZipFinderSync`, `TTZipQuickLook`) and duplicate `core/*.html` marketing files. | **Medium** | Deploy `scripts/lint_repo_hygiene.sh` gate script and purge dead copies. |

### 4.2 Deterministic Repository Hygiene Linter (`scripts/lint_repo_hygiene.sh`)

An automated linter script checking:
1. No rogue HTML/web assets in `core/` root.
2. No orphaned App targets in `core/Sources/`.
3. No `.unsafeFlags` disabling WMO compiler optimizations.
4. No forbidden macOS metadata clutter (`.DS_Store`, `._*`).
5. No dirty staging artifacts in `dist/`.
