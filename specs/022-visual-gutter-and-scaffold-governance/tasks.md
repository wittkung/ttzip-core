# Tasks: TTZip 视觉呼吸槽、全 Tab 脚手架收敛与垂直对齐治理

- **Feature**: `specs/022-visual-gutter-and-scaffold-governance`
- **Specification**: `specs/022-visual-gutter-and-scaffold-governance/spec.md`
- **Implementation Plan**: `specs/022-visual-gutter-and-scaffold-governance/plan.md`

---

## Phase 1: 容器顶格对齐与消灭裁切 (US1)

- [x] T001 [US1] Explicitly lock topLeading alignment and remove bare .clipped() in `apple/Sources/TTZipApp/Views/MainView.swift`
- [x] T002 [P] [US1] Explicitly lock topLeading alignment in `apple/Sources/TTZipApp/Components/KeepAliveTabContainer.swift`

---

## Phase 2: 8pt 呼吸槽 (Zen Gutter) 与 Inset 浮动卡片 (US2)

- [x] T003 [P] [US2] Refactor ResizableDividerHandle with 8pt gutter width and implicit drag pill in `apple/Sources/TTZipApp/Views/Components/ResizableDividerHandle.swift`
- [x] T004 [P] [US2] Remove trailing hard border line in `apple/Sources/TTZipApp/Views/Sidebar/MacEditorialSidebar.swift`
- [x] T005 [P] [US2] Standardize leading padding to 8pt for Inset card in `apple/Sources/TTZipApp/Views/Explorer/HomeExplorerContainerView.swift`

---

## Phase 3: 全 Tab 100% 脚手架统合与 Y=90pt 金线贯通 (US3)

- [x] T006 [US3] Lift TTZipWorkspaceScaffold to root in `apple/Sources/TTZipApp/Views/PasswordVaultView.swift`
- [x] T007 [P] [US3] Strip duplicate header and gold line from `apple/Sources/TTZipApp/Views/PasswordVaultLockedView.swift` and `apple/Sources/TTZipApp/Views/PasswordVaultUnlockedView.swift`
- [x] T008 [P] [US3] Adopt TTZipWorkspaceScaffold in `apple/Sources/TTZipApp/Views/BenchmarkView.swift`
- [x] T009 [P] [US3] Standardize MediaPreview and larkSync fallback scaffolds in `apple/Sources/TTZipApp/Views/MainView.swift`

---

## Phase 4: 快捷目录栏与细节打磨 (US4)

- [x] T010 [P] [US4] Add leading padding to horizontal shortcut ScrollView in `apple/Sources/TTZipApp/Views/Explorer/DiskDirectoryBrowserView.swift`

---

## Phase 5: 质量门禁与测试验证

- [x] T011 Execute full Swift build and test validation via `swift test`
- [x] T012 Verify quickstart validation scenarios in `specs/022-visual-gutter-and-scaffold-governance/quickstart.md`
