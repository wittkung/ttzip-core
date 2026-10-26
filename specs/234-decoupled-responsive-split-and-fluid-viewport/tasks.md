# Task List: 234 - Decoupled Responsive Split Architecture & Fluid Viewport Geometry

> **Status**: Ready for Implementation  

---

- [ ] **T001 [US1] Create LayoutStateMigration Service**
  - Path: `products/ttzip/apple/Sources/TTZipApp/Services/LayoutStateMigration.swift`
  - Implement `migrateIfNeeded(defaults:)` with Version 1 schema update.
- [ ] **T002 [US1] Bootstrap LayoutStateMigration in TTZipApp**
  - Path: `products/ttzip/apple/Sources/TTZipApp/TTZipApp.swift`
  - Invoke `LayoutStateMigration.migrateIfNeeded()` at line 1 of `TTZipApp.init()`.
- [ ] **T003 [US1] Refactor MainLayoutMetrics for Single Ratio SSOT & Ergonomic Cap**
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/Main/MainLayoutMetrics.swift`
  - Remove `rightWidth` argument; decouple static calculation from rubber-band physics; add 540pt ergonomic cap; add `isStorageInspectorActive` mutual exclusion.
- [ ] **T004 [US1] Clean Up InspectorColumnView & MainView Persistence**
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/Main/InspectorColumnView.swift`
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/MainView.swift`
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/Main/MainView+LayoutEvents.swift`
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/Main/MainView+Lifecycle.swift`
  - Remove all references to `TTZip_UserRightSidebarWidth` and dead `rightWidth` parameters.
- [ ] **T005 [US2] Implement Universal Viewport-Fill Algorithm in FinderMillerColumnsView**
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift`
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/Explorer/SingleMillerColumnView.swift`
  - Implement Underflow / Overflow leaf expansion and trailing divider suppression.
- [ ] **T006 [US3] Align SearchOverlayView Coordinates & Compress Form Bounds**
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/MainView.swift`
  - Path: `products/ttzip/apple/Sources/TTZipApp/Views/CompressFileListView.swift`
  - Compensate search overlay for sidebar offset; set 880pt max width on compression form.
- [ ] **T007 [US1, US2] Comprehensive Unit Test Suite**
  - Path: `products/ttzip/apple/Tests/TTZipAppTests/ResponsiveLayoutBoundsTests.swift`
  - Test migration from dirty UserDefaults; 2:1 ratio scaling across 900-3840px; Miller columns Underflow / Overflow math.
- [ ] **T008 Monorepo Quality Gates & App Bundle Verification**
  - Run `swift test --package-path products/ttzip/apple`.
  - Run `bazel test -c opt //...`.
  - Run `python3 tools/ci/lint_loc_gate.py`, `lint_code_quality.py`, `license_audit.py`.
  - Build and install to `/Applications/TTZip.app`.
