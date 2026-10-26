# Cross-Owner Production Integration Contract & Handoff (Spec 232)

> **Ownership Boundary**:
> - **Owner Q5** owns `products/ttzip/docs/twin/semantic-search.md`, `products/ttzip/core/specs/232-embedding-semantic-search/**`, `products/ttzip/core/rust/crates/ttzip-semantic/**`, `products/ttzip/core/rust/ttzip-engine/src/uniffi_api/semantic_search.rs`, `products/ttzip/apple/Sources/TTZipApp/Services/SemanticSearch/**`, and `products/ttzip/apple/Tests/TTZipAppTests/SemanticSearchTests.swift`.
> - **Owner W1** owns Global Search panel invocation (`Views/GlobalSearch/GlobalSearchPanelController.swift`), authorized directory bookmark lifecycle (`Services/RootFolderAccessManager.swift`), and window/archive routing (`Services/Windows/BrowserWindowRouter.swift`, `Services/Windows/BrowserWindowRegistry.swift`, `ViewModels/AppViewState.swift`, `ViewModels/AppViewState+ArchiveOperations.swift`, `TTZipApp.swift`).
> - **Owner H** owns Bazel packaging, UniFFI binding generation (`Sources/TTZipCore/Generated/ttzip_engine.swift`), and signed `.app` bundle dylib embedding (`products/ttzip/apple/BUILD.bazel`, `third_party/llama_cpp/BUILD.bazel`).
> - **Non-Overlapping Rule**: Q5 and W1 do not modify the same host files concurrently. All code snippets below reference **only real, compiled types and methods** verified in the repository.

---

## 1. Handoff Status with Owner H (Build, UniFFI & Test Runtime Dylib)

### 1.1 Already Landed in Workspace (Verified by Real Bazel Runs)
1. **Upstream `@llama_cpp` Header & Dylib Mounted**:
   - `products/ttzip/core/rust/crates/ttzip-semantic/BUILD.bazel` compiles `ttzip_llama_bridge` with `-DTTZIP_HAS_UPSTREAM_LLAMA_HEADER=1` and `deps = ["//third_party/llama_cpp:llama_cpp"]`.
2. **Build-Stage Test RPATH Configured (Runtime `.xctest` Symlink Removed)**:
   - `SemanticSearchTests.swift` no longer creates any runtime symlink inside `OmniSearchTests.xctest/Contents/Frameworks/libllamacpp.dylib`.
   - Instead, `cc_library(name = "ttzip_llama_bridge")` in `products/ttzip/core/rust/crates/ttzip-semantic/BUILD.bazel` propagates build-time macOS `linkopts` RPATHs (`@loader_path/../../OmniSearchTests.xctest.runfiles/_main/third_party/llama_cpp/lib` and `@loader_path/../../../../../third_party/llama_cpp/lib`) so `@rpath/libllamacpp.dylib` resolves directly from Bazel's declared `data` runfiles without mutating shared build outputs at test runtime.
   - Owner H may optionally move or duplicate this `rpath` linkopt onto `macos_unit_test(name = "OmniSearchTests")` in `products/ttzip/apple/BUILD.bazel`.
3. **UniFFI Registration & Compile Flag Active**:
   - `Sources/TTZipCore/Generated/ttzip_engine.swift` includes `UniFFISemanticSearchEngine`, and `TTZipAppServices` / `OmniSearchTests` compile with `-DTTZIP_UNIFFI_SEMANTIC_SEARCH_LINKED`.

### 1.2 Remaining Owner H Item: Signed `.app` Bundle Packaging
- Production `LlamaRuntimeEngine` (`LlamaDylibLoadPolicy::BundledOnly`) loads exclusively from bundled paths (`@executable_path/../Frameworks/libllamacpp.dylib`, `@loader_path/../Frameworks/libllamacpp.dylib`, `@rpath/libllamacpp.dylib`).
- Owner H must ensure `TTZip.app/Contents/Frameworks/libllamacpp.dylib` is bundled and signed with matching team ID during `apple/scripts/bundle_app.sh` / `macos_application` packaging, and verify model path resolution (`TTZIP_GGUF_MODEL_PATH` or `~/Library/Application Support/com.metastudyline.ttzip/Models/embeddinggemma-300M-Q8_0.gguf`) inside the signed `.app`.

---

## 2. Lifecycle & Field Contract Between Q5 (`Services/SemanticSearch`) and W1 (`GlobalSearch` / `Windows` / `AppViewState`)

### 2.1 Service Initialization (`SemanticSearchEngine.shared`)
Q5 provides a process-wide actor singleton in [`SemanticSearchEngine.swift`](file://../apple/Sources/TTZipApp/Services/SemanticSearch/SemanticSearchEngine.swift):
- `SemanticSearchEngine.shared`: initialized with `SemanticSearchEngine.defaultStorageDirectory()` (`~/Library/Application Support/com.metastudyline.ttzip/SemanticIndex`), backed by `UniFFIRustSemanticCoreBridge` -> `UniFFISemanticSearchEngine`.
- Checking model readiness from `@MainActor` (e.g., `TTZipApp` or `GlobalSearchPanelController`):
  ```swift
  let state: SemanticModelAvailability = await SemanticSearchEngine.shared.modelState()
  // Cases:
  // - .ready(SemanticModelDescriptor)
  // - .notInstalled / .corrupted(reason:) / .unsupportedRuntime(reason:) -> engine automatically degrades to lexical-only
  // - .engineUnlinked(reason:)
  ```

### 2.2 Authorized Scope Onboarding & Revocation Lifecycle

| Lifecycle Event | W1 Host Owner & Method | Q5 `SemanticSearchEngine.shared` Call | Scope ID Format |
| :--- | :--- | :--- | :--- |
| **App launch restores bookmarks** | `RootFolderAccessManager.shared.restoreBookmarks()` (`Services/RootFolderAccessManager.swift:124`) | For each `dirURL` in `RootFolderAccessManager.shared.accessingURLs`:<br>`await SemanticSearchEngine.shared.registerAuthorizedDirectory(url: dirURL, startIndexingImmediately: true)` | `"dir:\(canonicalPath)"` via `SemanticSearchEngine.scopeId(forDirectoryURL: dirURL)` |
| **User grants folder access** | `RootFolderAccessManager.shared.requestAccess()` / `persistBookmark(for: url)` (`Services/RootFolderAccessManager.swift:62`) | `await SemanticSearchEngine.shared.registerAuthorizedDirectory(url: url, startIndexingImmediately: true)` | `"dir:\(canonicalPath)"` |
| **User revokes folder access** | `RootFolderAccessManager.shared.revokeAccess(for: url)` (`Services/RootFolderAccessManager.swift:222`) | `await SemanticSearchEngine.shared.revokeAuthorizedDirectory(url: url)` (cancels active indexing, evicts persistent `.ttsi` cache, removes scope) | `"dir:\(canonicalPath)"` |
| **User revokes all folders** | `RootFolderAccessManager.shared.revokeAllAccess()` (`Services/RootFolderAccessManager.swift:241`) | Capture `let urls = RootFolderAccessManager.shared.accessingURLs` before revoking, then for each `url`: `await SemanticSearchEngine.shared.revokeAuthorizedDirectory(url: url)` | `"dir:\(canonicalPath)"` |
| **User opens archive in window** | `AppViewState.loadArchive(path:password:bookmarkSession:)` (`ViewModels/AppViewState+ArchiveOperations.swift:580`) | Register `.openArchive` scope via `SemanticSearchEngine.scopeId(forArchivePath: path)` and feed text entries via `upsertArchiveEntries(scopeId:entries:)` | `"archive:\(canonicalPath)"` via `SemanticSearchEngine.scopeId(forArchivePath: path)` |
| **User closes archive in window** | `AppViewState.closeCurrentArchive()` (`ViewModels/AppViewState+ArchiveOperations.swift`) | `await SemanticSearchEngine.shared.revokeScope(scopeId: SemanticSearchEngine.scopeId(forArchivePath: path))` | `"archive:\(canonicalPath)"` |

#### Compilable Onboarding Helper Example for W1 (`RootFolderAccessManager` / `AppViewState`)
```swift
import Foundation
import TTZipAppServices

extension RootFolderAccessManager {
    /// Synchronizes all currently active security-scoped directories with SemanticSearchEngine.
    public func syncAuthorizedDirectoriesToSemanticSearch() {
        let urls = Array(self.accessingURLs)
        Task.detached(priority: .utility) {
            for url in urls {
                _ = await SemanticSearchEngine.shared.registerAuthorizedDirectory(
                    url: url,
                    isAuthorized: true,
                    startIndexingImmediately: true
                )
            }
        }
    }
}
```

---

### 2.3 Query Execution & Cooperative Cancellation Contract (`GlobalSearchPanelController`)

In [`GlobalSearchPanelController.swift`](file://../apple/Sources/TTZipApp/Views/GlobalSearch/GlobalSearchPanelController.swift), W1 already maintains:
- `private var searchTask: Task<Void, Never>?`
- `private var currentGeneration: UInt64 = 0`

When `queryDidChange()` or `hide()` runs:
1. Capture `let previousGen = self.currentGeneration` before incrementing `self.currentGeneration &+= 1`.
2. Cancel `self.searchTask?.cancel()` and cooperatively notify Rust to abort any in-flight vector scan for `previousGen`:
   ```swift
   if previousGen > 0 {
       Task {
           await SemanticSearchEngine.shared.cancelSearch(requestId: previousGen)
       }
   }
   ```
3. Inside the debounced `searchTask`, query `SemanticSearchEngine.shared.search(_:)` using the active window UUID from `BrowserWindowRegistry.shared.activeWindowID`:
   ```swift
   let activeWindowId = BrowserWindowRegistry.shared.activeWindowID?.uuidString ?? "global_search_panel"
   let semanticResponse = await SemanticSearchEngine.shared.search(
       SemanticSearchRequest(
           requestId: gen,
           windowId: activeWindowId,
           query: trimmed,
           limit: 30,
           scopeIds: [] // empty searches all registered & authorized scopes
       )
   )
   guard !Task.isCancelled, gen == self.currentGeneration, !semanticResponse.isCancelled else { return }
   let semanticItems: [GlobalSearchResultItem] = semanticResponse.hits.map { $0.toGlobalSearchResultItem() }
   ```

---

### 2.4 Field Mapping & Open Request Contract (`SemanticSearchHit` / `SemanticOpenRequest` ↔ `GlobalSearchResultItem` / `BrowserWindowRouter`)

Q5 provides two verified conversion paths on [`SemanticSearchHit`](file://../apple/Sources/TTZipApp/Services/SemanticSearch/SemanticSearchContracts.swift):
1. `hit.toGlobalSearchResultItem() -> GlobalSearchResultItem` (direct mapping to W1's panel model in `Models/GlobalSearchResultItem.swift`).
2. `await SemanticSearchEngine.shared.buildOpenRequest(for: hit) -> SemanticOpenRequest` (Rust-backed open descriptor).

#### Complete Field-by-Field Mapping Table

| `SemanticSearchHit` Field | `.authorizedDirectory` Hit Value | `.openArchive` Hit Value | `SemanticOpenRequest` Field | `GlobalSearchResultItem` Field (`hit.toGlobalSearchResultItem()`) | W1 Consumer (`GlobalSearchPanelController` / `BrowserWindowRouter` / `AppViewState`) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `scopeId` | `"dir:/Volumes/Work/Docs"` | `"archive:/Volumes/Work/a.zip"` | `scopeId` |*(part of `id`)* | Scope lookup / telemetry |
| `sourceKind` | `.authorizedDirectory` | `.openArchive` | `sourceKind` | `hitType = .archiveEntry` | Determines icon rendering (`doc.text.fill`) in `GlobalSearchPanelView` |
| `containerOrRootPath` | `"/Volumes/Work/Docs"` | `"/Volumes/Work/a.zip"` | — | — | Authorized root folder or outer archive file path |
| `displayPath` | `"sub/report.md"` | `"src/lib.rs"` | — | `title = (displayPath as NSString).lastPathComponent` | Rendered as row title in `GlobalSearchOverlayView` |
| `resolvedOuterFilePath` | `"/Volumes/Work/Docs/sub/report.md"` | `"/Volumes/Work/a.zip"` | `outerPath` | `diskPath = resolvedOuterFilePath` (for directory)<br>`archivePath = nil` (for directory) / `containerOrRootPath` (for archive) | `item.targetKind` evaluates to `.diskItem(path: diskPath)` for directory hits, triggering `routeGlobalSearchDiskItem`. For archive hits, `item.targetKind` evaluates to `.archiveEntry(archivePath:entrySubpath:)`. |
| `archiveEntryPath` | `nil` | `"src/lib.rs"` (full relative subpath inside archive) | `archiveEntryPath` | `entrySubpath = nil` (for directory) / `archiveEntryPath` (for archive) | Passed as 2nd arg to `onSelectEntry?(item.archivePath, sub)` |
| `lineStart` / `lineEnd` | `UInt32?` (1-based line numbers) | `UInt32?` (1-based line numbers) | `lineNumber` (`lineStart`) | `matchedLineStart = lineStart.map(Int.init)`<br>Formatted into `subtitle` (`"... · L10-L14 · <snippet>"`) | **Metadata / UI subtitle display only — NO body line-jump consumer exists yet (see §2.5)** |
| `matchKind` | `.contentSemantic` / `.pathSemantic` / `.lexicalOnly` | `.contentSemantic` / `.pathSemantic` / `.lexicalOnly` | — | `matchBadge` (`"AI"` or `"Lexical"`) | Badge in `GlobalSearchOverlayView` |

#### Compilable Window & Archive Entry Routing for W1 (`TTZipApp.swift` & `BrowserWindowRouter.swift`)

> **Important Observation in W1's Current Host Code**:
> In [`TTZipApp.swift:92-94`](file://../apple/Sources/TTZipApp/TTZipApp.swift#L92-L94), `controller.onSelectEntry` is currently wired as:
> ```swift
> controller.onSelectEntry = { archivePath, _ in
>     BrowserWindowRouter.shared.routeGlobalSearchResult(itemURL: URL(fileURLWithPath: archivePath))
> }
> ```
> Notice that the second closure parameter (`entrySubpath`) is currently ignored (`_`) and only reveals the outer archive file in the directory browser.

Using **only real existing types and methods** on `BrowserWindowRegistry.shared`, `BrowserWindowControlling`, and `AppViewState` (`ViewModels/AppViewState.swift:21`, `ViewModels/AppViewState+ArchiveOperations.swift:115`), W1 can wire `onSelectEntry` (or `SemanticOpenRequest`) as follows:

```swift
// In BrowserWindowRouter.swift (Owner W1):
@MainActor
public func routeGlobalSearchArchiveEntry(archivePath: String, entrySubpath: String) {
    let standardizedArchivePath = URL(fileURLWithPath: archivePath).standardizedFileURL.path

    // 1. Prefer an existing window that already has this archive open, or the active window, or any registered window.
    let targetController: (any BrowserWindowControlling)? =
        registry.allControllers.first(where: {
            guard let openPath = $0.currentArchivePath else { return false }
            return URL(fileURLWithPath: openPath).standardizedFileURL.path == standardizedArchivePath
        })
        ?? registry.activeController
        ?? registry.allControllers.first

    guard let controller = targetController else { return }

    // 2. If the archive is already loaded in `AppViewState`, select the exact internal `entrySubpath` directly.
    if let viewState = controller as? AppViewState,
       let openPath = viewState.currentArchivePath,
       URL(fileURLWithPath: openPath).standardizedFileURL.path == standardizedArchivePath {
        viewState.selectArchiveEntry(path: entrySubpath, inArchive: standardizedArchivePath)
        focusWindow(for: controller)
        return
    }

    // 3. Otherwise load the archive in `AppViewState` and then select `entrySubpath` once loaded.
    if let viewState = controller as? AppViewState {
        focusWindow(for: controller)
        Task { @MainActor in
            let loaded = await viewState.loadArchive(path: standardizedArchivePath)
            if loaded {
                viewState.selectArchiveEntry(path: entrySubpath, inArchive: standardizedArchivePath)
            }
        }
    } else {
        controller.openArchiveInWindow(
            at: URL(fileURLWithPath: standardizedArchivePath),
            password: nil,
            bookmarkSession: nil
        )
        focusWindow(for: controller)
    }
}
```

And in `TTZipApp.setupGlobalSearchOverlay()` (`TTZipApp.swift:88-95`):
```swift
controller.onOpenArchive = { archivePath in
    BrowserWindowRouter.shared.routeGlobalSearchResult(
        itemURL: URL(fileURLWithPath: archivePath)
    )
}
controller.onSelectEntry = { archivePath, entrySubpath in
    BrowserWindowRouter.shared.routeGlobalSearchArchiveEntry(
        archivePath: archivePath,
        entrySubpath: entrySubpath
    )
}
```

---

### 2.5 Explicit Capability Boundary: Body Line Number (`lineStart` / `lineEnd` / `lineNumber`)

1. **What Q5 Produces**:
   - Rust `SemanticSearchCoordinator` and `ttzip_semantic` chunk text files by line boundaries and populate `lineStart: UInt32?` and `lineEnd: UInt32?` on every `SemanticSearchHit`, plus `lineNumber: UInt32?` on `SemanticOpenRequest`.
   - `SemanticSearchHit.toGlobalSearchResultItem()` formats `L<lineStart>-L<lineEnd>` into `GlobalSearchResultItem.subtitle` alongside the matched text snippet so the user can see which lines matched in the search overlay.
2. **What Host Consumers Currently Support (No Body Line-Jump Consumer Yet)**:
   - `GlobalSearchResultItem` (`Models/GlobalSearchResultItem.swift`) has no `lineNumber` stored property.
   - `GlobalSearchPanelController.onOpenArchive` (`(String) -> Void`) and `onSelectEntry` (`(String, String) -> Void`) do not pass a line number parameter.
   - `BrowserWindowRouter.routeGlobalSearchResult(itemURL:)` and `AppViewState.selectArchiveEntry(path:inArchive:)` (`ViewModels/AppViewState+ArchiveOperations.swift:115`) select the file or archive entry in the browser tree/table (`selectionState.selectedEntryID = entryPath`), **without scrolling a text preview pane to a specific line number**.
   - Therefore, Q5 explicitly records that **body line positioning is not yet consumed by the host UI/window router**; `lineStart` / `lineEnd` / `lineNumber` are currently preserved in `SemanticSearchHit` / `SemanticOpenRequest` and displayed in `GlobalSearchResultItem.subtitle` only.
