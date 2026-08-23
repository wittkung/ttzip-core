# Implementation Plan: SwiftUI 桌面端与 C11 纯微内核深度打通及设计系统落地 (Feature 164)

**Feature ID**: `164-swiftui-desktop-c-bridge-integration-and-design-system`  
**Created**: 2026-08-21  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **Target Architecture**: macOS 14+ (Sonoma / Sequoia), Apple Silicon ARM64, Swift 5.10 / 6.0 Concurrency (`AsyncStream`, `@MainActor`), AppKit/SwiftUI Hybrid (`NSOutlineView`, `QLPreviewPanel`, `NSFilePromiseProvider`), C11 Clang Native Bridge.
- **Core Principles**:
  - Zero-heap progress event tunneling via C11 `mach_absolute_time()` throttling (16.6ms 60fps).
  - TTZip Design System strict geometry (Y=90pt Golden Rule Line, 52pt header bars, WSJ Editorial serif typography, Kintsugi Gold `#D4AF37`).
  - Single-entry on-demand streaming extraction for Space-bar Quick Look and Finder Drag-and-Drop.
  - Radix tree Flat Index Projection keeping memory < 25MB for 100k+ file nodes.

### 1.2 Constitution Check
- [x] **Zero Cloud Quota / 100% Local**: All UI state, preview generation, and archive parsing run purely locally.
- [x] **Strict Native Library Dominance**: Direct C bridge binding for progress and cancellation without intermediary wrappers.
- [x] **Zero Bare Objects & Schema Strictness**: JSON telemetry contract (`contracts/desktop-ui-bridge-schema.json`) enforces strict draft-07 types.
- [x] **60fps UI & Zero Regression**: Throttled progress streaming ensures 0 main-thread hitches, full battery test suites pass.

---

## 2. Phase 0 & Phase 1 Artifacts Index

- [x] **Phase 0 Research**: [`research.md`](research.md)
  - `- R001 [SUBAGENT:research] 《60fps C-to-Swift 无锁进度流与取消桥接机制》`
  - `- R002 [SUBAGENT:research] 《TTZip 设计系统 Token 与三栏 Y=90pt 金线绝对对齐》`
  - `- R003 [SUBAGENT:research] 《单文件按需 Quick Look 预览与访达拖拽无中间态解压》`
  - `- R004 [SUBAGENT:research] 《10 万+ 文件 Radix 树虚拟化扁平索引投影模型》`
- [x] **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- [x] **Phase 1 Contract**: [`contracts/desktop-ui-bridge-schema.json`](contracts/desktop-ui-bridge-schema.json)
- [x] **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)

---

## 3. Component Breakdown & Planned Changes

### Component 1: C11 & Swift Concurrency Progress Bridge (`Sources/CTTZipBridge/`, `Sources/TTZipCore/`)
- [MODIFY] `Sources/CTTZipBridge/include/ttzip_archive.h`: Expand `ttzip_archive_progress_cb` to support atomic cancellation flag and monotonic timestamp rate limiting.
- [MODIFY] `Sources/TTZipCore/ConcurrencyBridge.swift`: Implement `AsyncStream<ArchiveProgress>` bridge with backpressure and immediate cancellation propagation.

### Component 2: Design System Layout & Header Alignment (`Sources/TTZipApp/`)
- [MODIFY] `Sources/TTZipApp/Views/MainView.swift`: Align Sidebar (200pt), Central Explorer (min 450pt), and Inspector (280pt) with top inset 38pt and 52pt header bars.
- [MODIFY] `Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift`: Ensure Kintsugi Gold rule line is rendered at exact Y = 90pt.
- [MODIFY] `Sources/TTZipApp/Views/Sidebar/MacEditorialSidebar.swift`: Standardize typography and 52pt header bar.
- [MODIFY] `Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift`: Align inspector top padding and golden rule bar.

### Component 3: Single-Item Quick Look & Drag-to-Finder (`Sources/TTZipApp/`, `Sources/TTZipCore/`)
- [MODIFY] `Sources/TTZipCore/ArchiveSelectiveExtractor.swift`: Route single-entry extraction through C11 streaming bypass to avoid full-archive temporary decompression.
- [MODIFY] `Sources/TTZipApp/Views/Explorer/NativeArchiveOutlineView.swift`: Hook Space-bar key event (`keyCode == 49`) to trigger `QLPreviewPanel.shared()` with the single-extracted ephemeral item.

### Component 4: Virtualized Tree & Performance Verification (`tests/`)
- [NEW] `tests/TTZipAppTests/ProgressStreamingBridgeTests.swift`: Unit tests for 60fps throttled streaming and cooperative cancellation.
- [NEW] `tests/TTZipAppTests/DesignSystemLayoutAlignmentTests.swift`: Tests asserting Y=90pt geometry and token conformance.
- [NEW] `tests/TTZipAppTests/SelectiveSingleItemExtractionTests.swift`: Tests asserting single-item extraction under 10ms without full archive decompression.

---

## 4. Verification Plan

1. **Swift Unit Tests**:
   - `swift test --filter ProgressStreamingBridgeTests`
   - `swift test --filter DesignSystemLayoutAlignmentTests`
   - `swift test --filter SelectiveSingleItemExtractionTests`
2. **C Regression Gates**:
   - `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
