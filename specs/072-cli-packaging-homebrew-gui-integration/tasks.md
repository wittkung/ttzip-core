# Tasks: CLI Release Packaging, Homebrew Formula Distribution, and Desktop GUI Diagnostic Integration

**Feature Branch**: `072-cli-packaging-homebrew-gui-integration`  
**Created**: 2026-08-18  
**Status**: Ready for Implementation  
**Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/072-cli-packaging-homebrew-gui-integration/spec.md) | **Plan**: [`plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/072-cli-packaging-homebrew-gui-integration/plan.md)

---

## Phase 1: Setup & Foundational Infrastructure

**Purpose**: Core data models, state extensions, and packaging directory foundations.

- [x] T001 Implement `CLIPackageConfig` and `CLIPackageManifest` in `Sources/TTZipCore/CLI/CLIPackageManifest.swift`.
- [x] T002 Extend `OverlayState` with `showArchiveInspectorModal: Bool` and `inspectingArchivePath: String?` in `Sources/TTZipApp/ViewModels/AppSubStates.swift`.

---

## Phase 2: User Story 1 - Universal 2 Packaging & Homebrew Tap Pipeline (Priority: P1) 🎯 MVP

**Goal**: Deliver an automated, deterministic packaging script and Homebrew Formula for `ttzip-cli`.

**Independent Test**: Running `./scripts/package_cli_release.sh --dry-run` and `swift test --filter CLIPackagingTests`.

- [x] T003 [P] [US1] Implement `scripts/package_cli_release.sh` supporting Universal 2 build, `dsymutil`, `strip -x`, self-generating man/completions, and `COPYFILE_DISABLE=1` tarball creation.
- [x] T004 [US1] Implement production-ready `Formula/ttzip-cli.rb` conforming to Homebrew binary tap standards.
- [x] T005 [P] [US1] Create unit test suite `Tests/TTZipTests/CLIPackagingTests.swift` validating packaging script outputs, tarball hierarchy, and Homebrew Formula syntax.

---

## Phase 3: User Story 2 - Desktop GUI Diagnostics & Inspector Integration (Priority: P2)

**Goal**: Integrate `TTZipCore.Standards` subsystems into `TTZipApp` with `ArchiveInspectorViewModel`, `ArchiveInspectorSheet`, and Explorer context actions.

**Independent Test**: `swift test --filter ArchiveInspectorViewTests` and manual GUI inspector invocation.

- [x] T006 [P] [US2] Implement `ArchiveInspectorViewModel.swift` in `Sources/TTZipApp/ViewModels/ArchiveInspectorViewModel.swift` with async background scanning and thread-safe caching.
- [x] T007 [US2] Implement `ArchiveInspectorSheet.swift` in `Sources/TTZipApp/Views/ArchiveInspectorSheet.swift` with 4 tabs: 标准规范, 幻数锚点, 扩展字段, 合规体检.
- [x] T008 [P] [US2] Add diagnostic status badge and inspector trigger button in `Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift`.
- [x] T009 [US2] Add "查看归档标准与体检..." context menu action in `Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView.swift`.
- [x] T010 [P] [US2] Create unit test suite `Tests/TTZipTests/ArchiveInspectorViewTests.swift` verifying ViewModel state transitions and diagnostic cache integrity.

---

## Phase 4: Polish & Full Verification

**Purpose**: Run all regression gates, performance floors, and consistency analysis.

- [x] T011 Run full test suite (`swift test`) and performance floors (`swift test --filter XCTestPerformanceMeasureTests`).
- [x] T012 Run local 6-stage automated CI gate (`./scripts/run_local_ci_gate.sh`).
- [x] T013 Execute `speckit-converge` and `speckit-analyze` to assert 100% specification and implementation convergence.

---

## Dependencies & Execution Order

```
[Phase 1: Setup & Data Models (T001..T002)]
         │
         ├───▶ [Phase 2: US1 Packaging & Homebrew (T003..T005)] 🎯 MVP
         │
         └───▶ [Phase 3: US2 Desktop GUI Diagnostics (T006..T010)]
                   │
                   ▼
         [Phase 4: Polish & Full Verification (T011..T013)]
```
