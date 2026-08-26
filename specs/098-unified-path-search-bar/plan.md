# Implementation Plan: Unified Path and Search Address Bar (一体化路径与搜索地址栏)

**Branch**: `098-unified-path-search-bar` | **Date**: 2026-08-18 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/098-unified-path-search-bar/spec.md`

## Summary

Evolve TTZip's top navigation bar from a standalone Spotlight file search bar into a **Unified Path and Search Omnibar** (`LiquidGlassOmnibar`).
- **Dual-State Hybrid Interaction**: Clickable breadcrumb pill when idle/unfocused; switches seamlessly to AppKit `NSTextField` with full text selection and auto-highlight when focused or triggered via `⌘L` / `⇧⌘G`.
- **Intelligent Dual-Mode Routing**: Automatically detects path prefixes (`/`, `~`, `.`, `file://`) to activate Path Navigation mode with non-blocking directory autocompletion; other text defaults to Spotlight Search mode.
- **Asynchronous Autocomplete & Micro-Caching**: Background query engine powered by `ExplorerLRUCache` to guarantee $\le 15\text{ ms}$ response and 60/120 FPS UI fluidity without main-thread blocking.
- **Zero-Friction Sandbox & Destination Dispatching**: Two-tier permission coordination with `RootFolderAccessManager` (silent non-modal check for suggestions; active prompt only on explicit commit) and automatic routing to directories, archives (`openArchiveAsFolder`), or regular files.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + AppKit / SwiftUI (macOS 14.0+ Sonoma)
**Primary Dependencies**: In-process Swift/AppKit frameworks, `TTZipCore`, `CTTZipBridge`, `AppKitMenuSynchronizer`
**Storage**: In-memory `ExplorerLRUCache<String, [DiskItemInfo]>` (capacity: 128) + `UserDefaults` security-scoped bookmarks
**Testing**: `swift test` (XCTest unit suites + `FrontendPerformanceGateTests`)
**Target Platform**: macOS 14.0+ (Apple Silicon arm64 & Intel x86_64)
**Project Type**: Desktop GUI Application (`TTZipApp`) + Shared Core (`TTZipCore`)
**Performance Goals**: Autocomplete latency $\le 15\text{ ms}$; cache hit latency $\le 0.1\text{ ms}$; 60/120 FPS render pass; zero main-thread disk I/O
**Constraints**: App Store sandbox compliance (`-DMAS_BUILD`); IME / TSM Chinese input immunity (`hasMarkedText` gating); zero bare `print` logging
**Scale/Scope**: 1 unified omnibar component, 2 services (sanitizer + autocompletion engine), 4 updated view integration points, 3 unit test suites

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant | Status | Verification & Evidence |
| :--- | :--- | :--- |
| **1. Stream-First & Zero-Copy** | PASS | Path autocompletion uses shallow metadata enumerations (`includingPropertiesForKeys: [.isDirectoryKey, .nameKey]`) without full file loads. |
| **2. Hot-Path Zero-Allocation** | PASS | All UI path parsing and caching operations use lightweight structs (`POSIXPathSanitizer`, `PathSuggestionItem`) and fixed-capacity LRU caches. |
| **3. Invariant-First Security** | PASS | Paths sanitized via `standardizedFileURL`, shell unescaping, and two-tier sandbox access via `RootFolderAccessManager.shared`. |
| **4. Zero Bare Logging** | PASS | All diagnostic logs utilize `TTLogger.debug` / `TTLogger.error` instead of `print()`. |
| **5. IME & TSM Input Method Immunity** | PASS | AppKit `control(_:textView:doCommandBySelector:)` intercepts keys only when `!textView.hasMarkedText`, preventing Chinese Pinyin candidate hijack. |

---

## Phase 0: Research Items & Decisions

- R001 [SUBAGENT:research] 《macOS 原生地址栏交互范式与 SwiftUI/AppKit 最佳实践》: Investigated Dual-State Hybrid Architecture (Breadcrumb pill idle + AppKit `NSTextField` edit), `⌘L`/`⇧⌘G` shortcuts, and IME `hasMarkedText` immunity. See [research.md](./research.md#research-item-r001-macos-native-address-bar--omnibar-interaction-patterns).
- R002 [SUBAGENT:research] 《高效异步路径解析、沙盒权限与模糊补全策略》: Investigated `POSIXPathSanitizer` normalization, `ExplorerLRUCache` background query pipeline, 4-way `DestinationDispatcher`, and `RootFolderAccessManager` integration. See [research.md](./research.md#research-item-r002-high-efficiency-asynchronous-path-resolution-directory-autocompletion-and-sandbox-handling).

---

## Phase 1: Design Artifacts & Contracts

- **Data Model**: [data-model.md](./data-model.md) — Definitive specifications for `AddressBarInputMode`, `PathResolutionType`, `PathResolutionResult`, `PathSuggestionItem`, `BreadcrumbSegment`, and `AddressBarState`.
- **Contracts**:
  - `contracts/address-bar-api-schema.json` [SUBAGENT:research]: JSON Schema for `NavigatePathRequest`, `NavigatePathSuccessResponse`, `NavigatePathErrorResponse`, and `AddressBarStateChangeEvent`.
  - `contracts/path-autocompletion-schema.json` [SUBAGENT:research]: JSON Schema for `AutocompleteRequest`, `AutocompleteSuccessResponse`, and `AutocompleteErrorResponse`.
- **Quickstart Guide**: [quickstart.md](./quickstart.md) — Automated test commands (`POSIXPathSanitizerTests`, `AsyncPathAutocompletionTests`, `DestinationDispatcherTests`) and manual verification steps.

---

## Project Structure & Component Changes

```text
TTZip/
├── Sources/
│   ├── TTZipApp/
│   │   ├── Services/
│   │   │   ├── POSIXPathSanitizer.swift           # [NEW] POSIX path normalization & unescaping
│   │   │   ├── AsyncPathAutocompletionEngine.swift# [NEW] Asynchronous path suggestion engine
│   │   │   ├── DestinationDispatcher.swift        # [NEW] Path destination classifier & router
│   │   │   └── AppKitMenuSynchronizer.swift       # [MODIFY] Register Cmd+L / Shift+Cmd+G
│   │   ├── ViewModels/
│   │   │   └── AppViewState.swift                 # [MODIFY] Expose omnibar navigation bindings
│   │   ├── Views/
│   │   │   ├── Components/
│   │   │   │   ├── OmnibarTextField.swift         # [NEW] AppKit NSTextField with IME immunity
│   │   │   │   ├── BreadcrumbPathBarView.swift    # [NEW] Idle clickable breadcrumb capsule view
│   │   │   │   ├── LiquidGlassOmnibar.swift       # [NEW] Unified address & search bar component
│   │   │   │   └── LiquidGlassSearchBar.swift     # [MODIFY] Deprecate / redirect to Omnibar
│   │   │   ├── Explorer/
│   │   │   │   └── HomeExplorerContainerView.swift# [MODIFY] Integrate dynamic breadcrumb navigation
│   │   │   └── MainView.swift                     # [MODIFY] Embed Omnibar in top navigation bar
└── Tests/
    └── TTZipTests/
        ├── POSIXPathSanitizerTests.swift          # [NEW] Path sanitization & expansion tests
        ├── AsyncPathAutocompletionTests.swift     # [NEW] Asynchronous autocompletion & cache tests
        └── DestinationDispatcherTests.swift       # [NEW] Destination routing & archive tests
```

---

## Complexity Tracking

*No constitution violations or unwarranted complexity introduced.*
