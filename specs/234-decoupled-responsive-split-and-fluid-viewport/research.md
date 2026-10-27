# Architectural Research: 234 - Decoupled Responsive Split Architecture & Fluid Viewport Geometry

> **Phase**: Research & Forensic SOTA Benchmarking  
> **Source Verification**: Apple HIG, Cocoa `NSBrowser`, WebKit Splitter, ForkLift 4, Transmit 5  

---

## 1. Multi-SSOT Layout Persistence & State Restoration

### 1.1 Root Cause Forensic Analysis
- **Defect Observation**: User screenshot `media_1791791122727_4768c9df.png` on 938px window displayed:
  * Left sidebar: 162px
  * Center workspace: 447px
  * Right inspector: 326px
  * Measured ratio: $447 : 326 \approx 1.37 : 1$. Expected: $2.0 : 1.0$ ($515\text{px} : 258\text{px}$).
- **State Leakage Confirmation**:
  Executing `defaults read com.metastudyline.ttzip` on the host revealed:
  ```
  "TTZip_UserRightInspectorRatio" = "0.4212233040201005";
  "TTZip_UserRightSidebarWidth" = "419.1171875";
  ```
  Because SwiftUI `@AppStorage` reads persisted values upon property wrapper initialization, changing the declaration default to `defaultRightInspectorRatio = 1.0 / 3.0` has zero effect on any machine that ran previous versions of TTZip.
- **SOTA Industry Solution**:
  * macOS system apps (Safari, Mail, Xcode) employ schema versioning on saved layout configurations.
  * When structural layout invariants change, an explicit migration hook in `AppDelegate.applicationWillFinishLaunching` or `App.init()` compares `TTZip_LayoutStateVersion` and normalizes stale ratios to the new canonical baseline.

---

## 2. Miller Columns Fluid Viewport Geometry & Dead Margin Elimination

### 2.1 Cocoa `NSBrowser` vs Modern SOTA File Managers
- **Cocoa `NSBrowser` Limitations**:
  * Designed in NeXTSTEP with fixed column widths (`defaultColumnWidth`, `minColumnWidth`).
  * In an unconstrained frame with fewer columns than the viewport width, `NSBrowser` leaves empty canvas on the right.
- **ForkLift 4 & Transmit 5 Algorithm**:
  * In multi-column mode embedded in a bounded workspace pane, leaving raw background gaps creates ragged visual seams against the adjacent pane divider.
  * SOTA algorithm partitions viewport geometry into two distinct physical regimes:
    1. **Underflow Regime ($S < W_{\text{avail}}$)**: Total preferred width of $N$ columns is less than available viewport width. Ancestor columns ($0 \le i < N - 1$) stay at their preferred width. The active leaf column ($i = N - 1$) absorbs the entire surplus $\Delta = W_{\text{avail}} - \sum_{j=0}^{N-2} w_j$. Dead space is identically 0.0pt.
    2. **Overflow Regime ($S \ge W_{\text{avail}}$)**: Total preferred column width exceeds available width. Each column preserves its preferred width ($w_i \ge W_{\text{min}}$). The scroll container activates horizontal paging and autoscrolls to reveal the active leaf column.

---

## 3. Top-Level Architectural Decoupling: The 3-Tier Split Model

### 3.1 Mode-Bleed Elimination
- In clean architecture, the split container does not know about specific workspaces (such as `isCompressWorkspaceActive`).
- The right inspector slot receives its content polymorphically via a Slot Registry or View Coordinator.
- In `StorageSpaceInspector` mode, the workspace declares that the outer inspector is suppressed, collapsing the slot cleanly without nested split collisions.

### 3.2 Dynamic Gesture Physics vs Static Layout Geometry
- `FluidGesturePhysics.rubberBandClamp` is designed for live touch/drag tracking when an active gesture exceeds physical bounds.
- Applying `rubberBandClamp` to static frame calculations causes clamped widths to deviate from exact mathematical bounds during window resizing.
- Static layout must use deterministic clamping: $\min(\max(x, \text{min}), \text{max})$. Rubber-banding must be reserved exclusively for active drag gestures (`activeDragRightWidth != nil`).
