# Tasks: TTZip Window Layout & Responsive UI Governance

- **Feature**: `specs/021-window-layout-and-responsive-ui-governance`
- **Specification**: `specs/021-window-layout-and-responsive-ui-governance/spec.md`
- **Implementation Plan**: `specs/021-window-layout-and-responsive-ui-governance/plan.md`

---

## Phase 1: Setup & Foundational Components

- [x] T001 [P] Create `TTFlowLayout` custom SwiftUI Layout component in `apple/Sources/TTZipApp/Views/Components/TTFlowLayout.swift`
- [x] T002 [P] Create `TTZipWorkspaceScaffold` standard container component in `apple/Sources/TTZipApp/Views/Components/TTZipWorkspaceScaffold.swift`
- [x] T003 Upgrade application minimum window dimensions to 520x400 in `apple/Sources/TTZipApp/TTZipApp.swift`

---

## Phase 2: Core Windowing & Workspace Topology Isolation

- [x] T004 [US1] Refactor `MainView` workspace topology and breakpoint system in `apple/Sources/TTZipApp/Views/MainView.swift`
- [x] T005 [P] [US1] Remove invalid `compressWorkspace` branch and nested browser in `apple/Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift`
- [x] T006 [P] [US1] Implement dynamic path collapse algorithm in `apple/Sources/TTZipApp/Views/Components/BreadcrumbPathBarView.swift`
- [x] T007 [P] [US1] Adapt elastic container width in `apple/Sources/TTZipApp/Views/Components/LiquidGlassOmnibar.swift`

---

## Phase 3: Compression Modal & Form Engine Overhaul

- [x] T008 [US2] Eliminate nested List and implement flat elastic file list in `apple/Sources/TTZipApp/Views/CompressFileListView.swift`
- [x] T009 [P] [US2] Implement flow layout for volume splitting and cleanup toggles in `apple/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift`
- [x] T010 [P] [US2] Fix compression level tile minimum width to 110pt in `apple/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView+Components.swift`

---

## Phase 4: Explorer, Inspector & Module Harmonization

- [x] T011 [P] [US3] Add hovered/active column index change listener for smooth centering in `apple/Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift`
- [x] T012 [P] [US3] Refactor capsule action bar with 3-tier breakpoints in `apple/Sources/TTZipApp/Views/Explorer/FolderMediaArtboardView.swift`
- [x] T013 [P] [US3] Implement dynamic single/double line adaptive layout in `apple/Sources/TTZipApp/Views/Explorer/FolderMediaArtboardView+Grid.swift`
- [x] T014 [P] [US4] Re-integrate format and compression level option tiles in `apple/Sources/TTZipApp/Views/Presets/PresetEditorCardView.swift`
- [x] T015 [P] [US4] Adopt TTZipWorkspaceScaffold in `apple/Sources/TTZipApp/Views/PresetWorkspaceView.swift`
- [x] T016 [P] [US4] Wrap ScrollView and adjust inner padding in `apple/Sources/TTZipApp/Views/Vault/PasswordVaultLockedView.swift`
- [x] T017 [P] [US4] Add Spacer to header actions in `apple/Sources/TTZipApp/Views/Vault/PasswordVaultUnlockedView.swift`
- [x] T018 [P] [US4] Adopt TTZipWorkspaceScaffold in `apple/Sources/TTZipApp/Views/Plugins/PluginsView.swift`
- [x] T019 [P] [US4] Adopt TTZipWorkspaceScaffold and add horizontal scrolling in `apple/Sources/TTZipApp/Views/SettingsView.swift`
- [x] T020 [P] [US4] Add ScrollView and theme tokens in `apple/Sources/TTZipApp/Views/CompressionSummarySheetView.swift`

---

## Phase 5: Verification & Quality Gate Sign-Off

- [x] T021 Execute full Swift build and test validation via `swift build` and `swift test`
- [x] T022 Validate end-to-end quickstart scenarios in `specs/021-window-layout-and-responsive-ui-governance/quickstart.md`
