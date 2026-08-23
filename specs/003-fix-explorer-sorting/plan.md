# Implementation Plan: 003-fix-explorer-sorting

**Branch**: `003-fix-explorer-sorting` | **Date**: 2026-08-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-fix-explorer-sorting/spec.md`

## Summary

Refactor and fix item sorting in TTZip's file browser and Miller Column explorer by extracting a dedicated `DiskItemSorter` pure comparator engine, fixing the broken `.dateDesc` / `.dateAsc` sorting stubs, providing deterministic multi-tier tie-breakers (folders-first, primary, secondary name, tertiary path), and writing a comprehensive TDD unit test suite `DiskSortOptionTests.swift` covering 100% of cases.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`)
**Primary Dependencies**: Foundation, SwiftUI, TTZipCore
**Testing**: XCTest (`swift test --filter DiskSortOptionTests`)
**Target Platform**: macOS 14.0+ (Sonoma, Sequoia)
**Project Type**: macOS Desktop App / GUI Engine
**Performance Goals**: < 1ms to sort 2,000 items in memory; zero heap allocation during comparisons.
**Constraints**:
- Must preserve folder-on-top partitioning in all 7 sort options.
- Must preserve natural numeric sorting (`"file2.txt"` < `"file10.txt"`).
- Must handle `nil` dates deterministically.

---

## Constitution Check

| Principle | Assessment | Status |
| :--- | :--- | :--- |
| **P1: Zero Cost on Hot Paths** | Comparator operates on existing struct properties without secondary object trees | PASS |
| **P2: Strict Weak Ordering** | Comparator provides total deterministic ordering with tie-breakers | PASS |
| **P3: Logging Discipline** | No `print` / `NSLog` used | PASS |
| **P4: Test Gating** | 100% test coverage before completion | PASS |

---

## Project Structure

### Documentation
```text
specs/003-fix-explorer-sorting/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code
```text
Sources/TTZipApp/
├── Models/
│   ├── DiskSortOption.swift
│   └── DiskItemInfo.swift
├── Services/
│   └── DiskItemSorter.swift              # [NEW] Pure functional sorting engine
└── Views/Explorer/
    ├── DiskDirectoryBrowserView.swift    # [MODIFY] Delegate to DiskItemSorter
    └── FinderMillerColumnsView.swift     # [VERIFY] Verify column cache key usage

Tests/TTZipTests/
└── DiskSortOptionTests.swift             # [NEW] Comprehensive TDD test suite
```

---

## Verification Plan

### Automated Tests
```bash
# 1. Run new sorting TDD test suite
swift test --filter DiskSortOptionTests

# 2. Run full regression test suite
swift test
```
