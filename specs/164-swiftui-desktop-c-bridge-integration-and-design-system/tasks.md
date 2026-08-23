# Tasks: SwiftUI 桌面端与 C11 纯微内核深度打通及设计系统落地 (Feature 164)

**Feature ID**: `164-swiftui-desktop-c-bridge-integration-and-design-system`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Foundational Progress Stream Types

- [x] T001 Update `Sources/CTTZipBridge/include/ttzip_archive.h` and `Sources/TTZipCore/ArchiveProgress.swift` with atomic cancellation and instantaneous MB/s telemetry

---

## Phase 2: User Story 1 (P1) - 60fps Lock-Free C-to-Swift Progress & Cancellation Stream Bridge

- [x] T002 [P] [US1] Implement `AsyncStream<ArchiveProgress>` bridge in `Sources/TTZipCore/ConcurrencyBridge.swift`
- [x] T003 [P] [US1] Write unit tests in `tests/TTZipAppTests/ProgressStreamingBridgeTests.swift`

---

## Phase 3: User Story 2 (P2) - TTZip Zen $\\times$ WSJ Editorial $\\times$ Kintsugi Gold Design System Alignment

- [x] T004 [P] [US2] Update `Sources/TTZipApp/Views/MainView.swift` to align Sidebar (200pt), Explorer (min 450pt), and Inspector (280pt) at Y=90pt
- [x] T005 [P] [US2] Update `Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift` and `Sources/TTZipApp/Views/Sidebar/MacEditorialSidebar.swift` with 52pt header bars and WSJ serif typography
- [x] T006 [P] [US2] Update `Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift` with Kintsugi Gold line alignment
- [x] T007 [P] [US2] Write layout alignment tests in `tests/TTZipAppTests/DesignSystemLayoutAlignmentTests.swift`

---

## Phase 4: User Story 3 (P3) - Single-Item On-Demand Quick Look Preview & Finder Drag-and-Drop

- [x] T008 [P] [US3] Update `Sources/TTZipCore/ArchiveSelectiveExtractor.swift` to use C11 streaming bypass for single-entry extraction
- [x] T009 [P] [US3] Add Space-bar Quick Look handler in `Sources/TTZipApp/Views/Explorer/NativeArchiveOutlineView.swift`
- [x] T010 [P] [US3] Write single-entry extraction tests in `tests/TTZipAppTests/SelectiveSingleItemExtractionTests.swift`

---

## Phase 5: Verification & Zero-Regression Gating

- [x] T011 [US1] Run `swift test --filter ProgressStreamingBridgeTests`
- [x] T012 [US2] Run `swift test --filter DesignSystemLayoutAlignmentTests`
- [x] T013 [US3] Run `swift test --filter SelectiveSingleItemExtractionTests`
- [x] T014 [US1] Run `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
