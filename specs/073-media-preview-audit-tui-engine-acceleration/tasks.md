# Tasks: Desktop Media Preview Audit, CLI Interactive TUI Mode, and SIMD Decompression Acceleration

**Feature Branch**: `073-media-preview-audit-tui-engine-acceleration`  
**Created**: 2026-08-18  
**Status**: Ready for Implementation  
**Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/073-media-preview-audit-tui-engine-acceleration/spec.md) | **Plan**: [`plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/073-media-preview-audit-tui-engine-acceleration/plan.md)

---

## Phase 1: Setup & Foundational Infrastructure

**Purpose**: Core data models and TUI terminal utilities.

- [x] T001 Implement `TUISessionState`, `TUIVisibleRow`, and `TUIPeekContent` in `Sources/TTZipCore/CLI/TUI/TUISessionModels.swift`.
- [x] T002 Implement `TerminalRawModeManager` with POSIX `<termios.h>` raw mode switching and RAII signal restoration in `Sources/TTZipCore/CLI/TUI/TerminalRawModeManager.swift`.

---

## Phase 2: User Story 1 - Desktop Media Preview Deep Audit & Polish (Priority: P1) 🎯 MVP

**Goal**: Eliminate memory leaks, guarantee 100% full-resolution image rendering, and harden AVPlayer/CoreAudio lifecycles in `TTZipApp`.

**Independent Test**: `swift test --filter MediaPreviewAuditTests`.

- [x] T003 [P] [US1] Implement `ImageIOThumbnailCache.swift` in `Sources/TTZipApp/Services/ImageIOThumbnailCache.swift` using `CGImageSourceCreateThumbnailAtIndex` with max pixel clamp.
- [x] T004 [US1] Harden `VideoAudioPlayerPreviewView.swift` and `UnifiedAudioPlayerView.swift` with 5-step explicit AVPlayer / CoreAudio HAL teardown on `.onDisappear`.
- [x] T005 [P] [US1] Add `NSFilePromiseProvider` virtual item drag-and-drop support in `Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView.swift`.
- [x] T006 [US1] Create unit test suite `Tests/TTZipTests/MediaPreviewAuditTests.swift` validating image downsampling, AVPlayer teardown, and drag promise models.

---

## Phase 3: User Story 2 - CLI Interactive Terminal TUI Mode (`explore`) (Priority: P1)

**Goal**: Deliver an interactive ANSI/VT100 terminal archive explorer with Vim keybindings, spacebar multi-select, and in-terminal peek.

**Independent Test**: `swift test --filter InteractiveTUITests` and `ttzip-cli explore <archive>`.

- [x] T007 [P] [US2] Implement `TUIKeyParser.swift` in `Sources/TTZipCore/CLI/TUI/TUIKeyParser.swift` for multi-byte ANSI sequences and Vim keys.
- [x] T008 [US2] Implement `InteractiveTUIExplorer.swift` in `Sources/TTZipCore/CLI/TUI/InteractiveTUIExplorer.swift` with double-buffered single-flush rendering, tree virtualization, and peek modal (`p`).
- [x] T009 [P] [US2] Extend `CLICommandSpec.swift` with `explore` subcommand specification.
- [x] T010 [US2] Wire `explore` command dispatch in `Sources/TTZipCLI/CLICommandRouter.swift`.
- [x] T011 [P] [US2] Create unit test suite `Tests/TTZipTests/InteractiveTUITests.swift` validating TUI state transitions, key event handling, and viewport virtualization.

---

## Phase 4: User Story 3 - Core SIMD Acceleration & Pipeline Hardening (Priority: P2)

**Goal**: Optimize LZ4/ZSTD stream decompression and verify PMULL CRC interleave pipelines.

**Independent Test**: `swift test --filter XCTestPerformanceMeasureTests`.

- [x] T012 [P] [US3] Verify and harden 64KB page-aligned buffer allocations and PMULL hardware checksumming in `Sources/CTTZipBridge/`.
- [x] T013 [US3] Verify LZ4/ZSTD streaming decompression throughput under `XCTestPerformanceMeasureTests`.

---

## Phase 5: Polish & Full Verification

**Purpose**: Run all regression gates, performance floors, and CI pipelines.

- [x] T014 Run full unit test suite (`swift test`).
- [x] T015 Run local 6-stage automated CI gate (`./scripts/run_local_ci_gate.sh`).
- [x] T016 Execute `speckit-converge` and `speckit-analyze` to assert 100% specification and implementation convergence.

---

## Dependencies & Execution Order

```
[Phase 1: Setup & Foundations (T001, T002)]
         │
         ├───▶ [Phase 2: US1 Media Preview Hardening (T003..T006)] 🎯 MVP
         │
         ├───▶ [Phase 3: US2 CLI Interactive TUI Mode (T007..T011)]
         │
         └───▶ [Phase 4: US3 Core SIMD Hardening (T012..T013)]
                         │
                         ▼
               [Phase 5: Verification (T014..T016)]
```
