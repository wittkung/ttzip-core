# Feature Specification: 003-fix-explorer-sorting

## 1. Executive Summary & Problem Description

TTZip's file browser and Miller Column explorer feature (`DiskDirectoryBrowserView`, `FinderMillerColumnsView`, `SingleMillerColumnView`) currently have critical bugs in item sorting:
1. **Broken Date Sorting**: Sorting options `.dateDesc` and `.dateAsc` ignore `modificationDate` / `creationDate` and erroneously fallback to alphabetical name ascending sort (`a.name.localizedStandardCompare(b.name) == .orderedAscending`).
2. **Missing Secondary/Tie-Breaker Sort Keys**: When items have identical size (`.sizeAsc` / `.sizeDesc`), identical dates, or identical kind (`.kind`), sort results are non-deterministic or inconsistent.
3. **Incomplete Edge Case Handling**: Handling of `nil` modification dates (e.g. virtual items, unreadable files), mixed folder/file sorting, numeric string sorting (e.g., `file2.txt` vs `file10.txt`), and case-insensitive/localized sorting needs strict guarantees.
4. **Lack of Automated Test Coverage**: No dedicated unit tests exist for `DiskSortOption` sorting algorithms.

This feature refactors and fixes the sorting engine with comprehensive TDD unit test suites, guaranteeing 100% test coverage and strict determinism across all 7 sort options.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Sorting by Date (Modification Time)
* **Given** a directory containing files modified at different times (`today`, `yesterday`, `last year`, and items with `nil` date),
* **When** the user selects "修改时间 (最新)" (`.dateDesc`),
* **Then** newest items appear first, folders remain grouped on top, and items with `nil` date are placed at the end with secondary alphabetical sorting.
* **When** the user selects "修改时间 (最早)" (`.dateAsc`),
* **Then** oldest items appear first, folders remain grouped on top, and items with `nil` date are placed at the end with secondary alphabetical sorting.

### User Scenario 2: Sorting by File Size
* **Given** a directory containing files of sizes 10KB, 500MB, 0B, and multiple folders (0 bytes),
* **When** the user selects "体积 (从大到小)" (`.sizeDesc`),
* **Then** folders stay grouped on top, followed by 500MB, 10KB, 0B; identical sized items are deterministically sorted alphabetically by name.
* **When** the user selects "体积 (从小到大)" (`.sizeAsc`),
* **Then** folders stay grouped on top, followed by 0B, 10KB, 500MB; identical sized items are sorted alphabetically.

### User Scenario 3: Sorting by File Kind (Type)
* **Given** a directory with mixed extensions (`.zip`, `.7z`, `.pdf`, `.png`, `.txt`),
* **When** the user selects "文件类型" (`.kind`),
* **Then** items are grouped by their human-readable type description alphabetically, with secondary sorting by name.

### User Scenario 4: Natural Numeric & Localized Name Sorting
* **Given** files named `item1.txt`, `item2.txt`, `item10.txt`, `item20.txt`, `item100.txt`,
* **When** sorting by name (`.nameAsc` / `.nameDesc`),
* **Then** natural Finder numeric ordering is preserved (`item1` < `item2` < `item10` < `item20` < `item100`).

---

## 3. Functional Requirements

- **FR-01 (Folder Grouping Guarantee)**: In all 7 sort options (`.nameAsc`, `.nameDesc`, `.sizeAsc`, `.sizeDesc`, `.dateAsc`, `.dateDesc`, `.kind`), folders MUST strictly remain grouped on top (or follow specified grouping rules) before regular files.
- **FR-02 (Date Sorting Correctness)**:
  - `.dateDesc`: Sorts by `modificationDate` descending (`$0 > $1`). Items with non-nil dates precede `nil` dates. Ties break via natural name ascending.
  - `.dateAsc`: Sorts by `modificationDate` ascending (`$0 < $1`). Items with non-nil dates precede `nil` dates. Ties break via natural name ascending.
- **FR-03 (Size Sorting Correctness)**:
  - `.sizeDesc`: Sorts by `rawSizeBytes` descending (`$0 > $1`). Ties break via natural name ascending.
  - `.sizeAsc`: Sorts by `rawSizeBytes` ascending (`$0 < $1`). Ties break via natural name ascending.
- **FR-04 (Name Sorting Correctness)**:
  - `.nameAsc`: Sorts by `name.localizedStandardCompare` ascending.
  - `.nameDesc`: Sorts by `name.localizedStandardCompare` descending.
- **FR-05 (Kind Sorting Correctness)**:
  - `.kind`: Sorts by `kindText.localizedStandardCompare` ascending, with secondary sort by `name.localizedStandardCompare` ascending.
- **FR-06 (Determinism & Stability)**: Every sort comparison MUST be total and deterministic. Identical primary values MUST be deterministically broken by secondary and tertiary keys (`name`, `path`).
- **FR-07 (TDD Test Suite)**: A standalone XCTest suite `DiskSortOptionTests.swift` MUST test all 7 sort options against normal cases, tie cases, empty collections, single items, large sets, and edge cases.

---

## 4. Success Criteria

1. All 7 `DiskSortOption` enum cases pass exhaustive automated unit tests.
2. `swift test --filter DiskSortOptionTests` passes 100% with 0 failures.
3. No performance degradation in Miller Column directory rendering or cache resolution.
4. All existing tests in the TTZip test suite continue to pass.
