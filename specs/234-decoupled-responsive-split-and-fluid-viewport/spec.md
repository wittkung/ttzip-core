# Feature Specification: 234 - Decoupled Responsive Split Architecture & Fluid Viewport Geometry

> **Target Version**: TTWorkspace v6.3 (Decoupled Split & Responsive Geometry Edition)  
> **Status**: Accepted Architecture Specification  
> **Classification**: Core Presentation Architecture & Viewport Governance  

---

## 1. System Overview & Problem Statement

### 1.1 Problem Statement
1. **Multi-SSOT Layout Persistence Hazard**:
   - `TTZip_UserRightSidebarWidth` (absolute pixel points, e.g., 419.1pt) and `TTZip_UserRightInspectorRatio` (proportional ratio, e.g., 0.4212) coexisted in `@AppStorage` and `UserDefaults` without synchronization locks or clear master-slave authority.
   - When introducing the canonical 2:1 width ratio ($W_{\text{center}} = \frac{2}{3} W_{\text{avail}}$, $W_{\text{right}} = \frac{1}{3} W_{\text{avail}}$), legacy persisted dirty state overrode the new declaration, causing running instances to lock onto an uncalibrated 1.37:1 ratio (447px vs 326px).
   - Zero layout schema versioning or migration guards existed in `TTZipApp.init()`.
2. **Miller Columns Viewport Geometry Underflow (82px Dead Space)**:
   - In `FinderMillerColumnsView`, column width calculation was treated on a single-column basis. For $N \ge 2$, it rigidly defaulted to `defaultColumnWidth = 260pt`.
   - When viewing directories with few columns or compact widths (e.g. 2 columns taking 365px inside a 447px container), the remaining ~82px was left unallocated, exposing an unfinished raw green background gap between the active column and the inspector divider.
3. **Three-Column Layering Inversion & God-Shell Anti-Pattern**:
   - `MainLayoutMetrics.compute` directly inspected business workspace states (`isCompressWorkspaceActive`, `isStorageInspectorActive`), violating container-content orthogonality.
   - Dividers were physically owned by child panes (`SidebarColumnView` and `InspectorColumnView`) rather than the parent multi-pane split coordinator, duplicating gesture physics and boundary logic.
4. **Unbounded Elasticity & Ergonomic Violations on Wide Displays**:
   - On 4K / ultra-wide displays (window width 3840px), maintaining an unbounded $1/3$ ratio stretched the right inspector to >1200px, causing key-value attributes to separate by over 1100px.
   - Static layout calculation mistakenly invoked live touch rubber-band physics, violating minimum width invariants.

### 1.2 Objective & System Boundary
This specification establishes:
- **Layer 1: Window Shell & Layout SSOT**: Unify layout state under `WindowLayoutCoordinator` with hermetic versioned migration (`LayoutStateMigration` v1). Deprecate absolute pixel keys.
- **Layer 2: Structural Split Container**: Decouple split dividers into container-level mediators. Enforce canonical 2:1 proportional scaling ($2/3$ center vs $1/3$ right) with a 540pt ergonomic cap.
- **Layer 3: Viewport-Fill Miller Geometry**: Universal Underflow/Overflow leaf column expansion algorithm guaranteeing $\sum_{i=0}^{N-1} \text{width}_i \ge W_{\text{avail}}$ with 0.0pt dead margin.

---

## 2. User Stories & Acceptance Criteria

### User Story 1: Canonical 2:1 Split Ratio on Launch and Window Resizing
- **As a** TTZip user on macOS,
- **I want** the workspace center column and right inspector to always open and scale at a 2:1 width ratio,
- **So that** screen space is proportionally balanced across all window sizes without manual resizing.

#### Acceptance Criteria
1. **Given** a fresh launch or an upgrade from an older TTZip release with legacy `UserDefaults`,
   **When** `TTZipApp` boots,
   **Then** `LayoutStateMigration` MUST idempotently purge `TTZip_UserRightSidebarWidth` and reset `TTZip_UserRightInspectorRatio` to $1.0 / 3.0$ with `TTZip_LayoutStateVersion = 1`.
2. **Given** an active window resized from 938px to 1440px or 1920px,
   **When** `GeometryReader` fires,
   **Then** the ratio between center content width and right inspector width MUST maintain $2.0 : 1.0 \pm 0.01$ (subject only to min clamp 220pt and max ergonomic cap 540pt).
3. **Given** a user explicitly dragging the inspector divider,
   **When** the drag completes,
   **Then** the new custom ratio MUST be stored under Version 1 and preserved across subsequent restarts without being reverted.
4. **Given** a user double-clicking the inspector divider handle,
   **When** the gesture is detected,
   **Then** the inspector width MUST smoothly animate back to the canonical $1.0 / 3.0$ ratio with `NSHapticFeedbackManager` alignment haptics.

### User Story 2: Zero Dead Space in Miller Columns Directory Navigation
- **As a** user browsing nested folders in column view (`FinderMillerColumnsView`),
- **I want** the visible columns to seamlessly fill the available workspace width up to the inspector divider,
- **So that** no jagged green or gray blank space is wasted on the right.

#### Acceptance Criteria
1. **Given** $N = 2$ active columns inside an available workspace width of 447pt,
   **When** rendered in `FinderMillerColumnsView`,
   **Then** the active leaf column MUST absorb the remaining deficit ($447 - 182 = 265\text{pt}$), leaving 0.0pt dead margin against the inspector divider.
2. **Given** $N$ columns where total preferred width exceeds available width ($S \ge W_{\text{avail}}$),
   **When** rendered,
   **Then** all columns MUST retain their preferred widths ($\ge 240\text{pt}$) and smoothly enable horizontal scrolling with auto-scroll to the trailing leaf.
3. **Given** the leaf column flush against the inspector divider in Underflow mode,
   **When** rendered,
   **Then** its internal trailing resize handle MUST be suppressed to eliminate double hairlines and gesture ambiguity against the inspector divider.

### User Story 3: Storage Space Inspector Mutual Exclusion
- **As a** user opening the Storage Space Inspector tab,
- **I want** the main window's outer inspector to collapse automatically,
- **So that** only the storage treemap and its native reclaim panel are visible without visual clutter or 4-pane splitting.

---

## 3. Non-Functional Requirements
- **NFR-001 (Zero LOC Gate Violation)**: Every modified or created file MUST remain $\le 800$ LOC.
- **NFR-002 (Quality & Complexity Gate)**: Function LOC $\le 80$, McCabe Cyclomatic Complexity $\le 15$.
- **NFR-003 (Zero Regression Gate)**: All 291/291 Bazel test targets and all Swift/Rust package test suites MUST pass 100% green.
- **NFR-004 (Google Piper Invariant)**: Universal file headers strictly enforced; zero unauthorized licenses.
