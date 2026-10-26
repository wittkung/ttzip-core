# Implementation Plan: 234 - Decoupled Responsive Split Architecture & Fluid Viewport Geometry

> **Phase**: Implementation Planning & Stage Breakdown  

---

## 1. Execution Stages

### Stage 1: Architectural Decoupling & Layout State Migration
- **Artifact**: `products/ttzip/apple/Sources/TTZipApp/Services/LayoutStateMigration.swift`
- Integrate into `TTZipApp.swift` `init()` bootstrap.
- Remove `userRightSidebarWidth` and all dead `rightWidth` parameters across:
  * `MainLayoutMetrics.swift`
  * `MainView.swift`
  * `MainView+LayoutEvents.swift`
  * `MainView+Lifecycle.swift`
  * `InspectorColumnView.swift`
- Decouple static calculation from `rubberBandClampRightSidebar` in `MainLayoutMetrics.swift`.
- Add ergonomic cap (540pt) on wide screens.
- Add mutual exclusion for `StorageInspectorView` (`shouldShowRight = false`).

### Stage 2: Miller Columns Fluid Geometry & 0 Dead Space
- **Artifact**: `products/ttzip/apple/Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift`
- Replace single-column branch in `computeColumnWidth` with pure universal Underflow/Overflow engine.
- Update `SingleMillerColumnView.swift` to accept `showTrailingDivider: Bool`.
- Suppress leaf column resize handle in Underflow mode.

### Stage 3: UI/UX Responsive Scaling Fixes
- Fix `SearchOverlayView` coordinate centering to anchor relative to middle column search capsule.
- Add responsive width constraint to `CompressWorkspaceView` forms.

### Stage 4: Test Suite & Gate Verification
- Update `ResponsiveLayoutBoundsTests.swift` with:
  * Migration test from dirty UserDefaults.
  * Preservation of user drag in v1.
  * 2:1 proportional invariants across 900px, 938px, 1440px, 1920px.
  * Miller column Underflow / Overflow invariants.
- Run `swift test --package-path products/ttzip/apple`.
- Run `bazel test -c opt //...`.
- Run all monorepo linters (`lint_loc_gate.py`, `lint_code_quality.py`, `license_audit.py`).

### Stage 5: Rebuild & Reinstall
- Execute `bash products/ttzip/apple/scripts/bundle_app.sh --release --install`.
- Relaunch `/Applications/TTZip.app` and verify live pixels.
