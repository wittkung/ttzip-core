# Tasks: 003-fix-explorer-sorting

## Phase 1: Setup & TDD Test Infrastructure

**Purpose**: Create test files and test fixture helpers

- [x] T001 Create `Tests/TTZipTests/DiskSortOptionTests.swift` skeleton with test fixture helpers (`createTestItem(...)`)
- [x] T002 [P] Define synthetic test datasets in `DiskSortOptionTests.swift` (files with varying dates, sizes, kinds, numeric names, folders vs files)

---

## Phase 2: Foundational Engine & Date Sorting (Priority: P1) 🎯 MVP

**Goal**: Implement `DiskItemSorter` and fix broken `.dateDesc` and `.dateAsc` sorting with robust `nil` date handling

- [x] T003 [P] Write failing unit tests for `.dateDesc` and `.dateAsc` (including non-nil vs nil dates and tie-breakers) in `DiskSortOptionTests.swift`
- [x] T004 Implement `DiskItemSorter.swift` with folder grouping and `.dateDesc` / `.dateAsc` comparator in `Sources/TTZipApp/Services/DiskItemSorter.swift`
- [x] T005 Verify `DiskSortOptionTests` date test suite passes (TDD Green)

---

## Phase 3: Size & Kind Sorting with Deterministic Tie-Breakers (Priority: P2)

**Goal**: Ensure `.sizeDesc`, `.sizeAsc`, and `.kind` have deterministic secondary and tertiary tie-breakers

- [x] T006 [P] Write failing unit tests for `.sizeDesc`, `.sizeAsc`, and `.kind` (including zero-byte files, identical sizes, identical kinds) in `DiskSortOptionTests.swift`
- [x] T007 Implement `.sizeDesc`, `.sizeAsc`, and `.kind` comparison logic with secondary `name` and tertiary `path` tie-breakers in `DiskItemSorter.swift`
- [x] T008 Verify `DiskSortOptionTests` size and kind test suite passes (TDD Green)

---

## Phase 4: Natural Numeric & Localized Name Sorting (Priority: P3)

**Goal**: Verify natural numeric sorting (`"file2.txt"` < `"file10.txt"`) and localized name comparison

- [x] T009 [P] Write failing unit tests for `.nameAsc` and `.nameDesc` with numeric strings and mixed folder/file sets in `DiskSortOptionTests.swift`
- [x] T010 Complete `.nameAsc` and `.nameDesc` comparison in `DiskItemSorter.swift` using `localizedStandardCompare`
- [x] T011 Verify `DiskSortOptionTests` name sorting test suite passes (TDD Green)

---

## Phase 5: View Integration & Single Point of Truth (Priority: P4)

**Goal**: Refactor `DiskDirectoryBrowserView` and `FinderMillerColumnsView` to delegate sorting to `DiskItemSorter`

- [x] T012 Refactor `DiskDirectoryBrowserView.sortItems` to delegate to `DiskItemSorter.sort(items, by: option)` in `Sources/TTZipApp/Views/Explorer/DiskDirectoryBrowserView.swift`
- [x] T013 Verify and test Miller column multi-column sorting integration with `DiskItemSorter`
- [x] T014 Write UI/integration tests in `DiskSortOptionTests.swift` testing `DiskDirectoryBrowserView.sortItems`

---

## Phase 6: Polish & Verification Gate

**Purpose**: Execute full regression testing and consistency analysis

- [x] T015 Run `swift test --filter DiskSortOptionTests` and ensure 100% pass rate
- [x] T016 Run full test suite `swift test` ensuring zero regressions across all modules
