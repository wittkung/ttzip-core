# Research & Decision Matrix: Multiplatform SDK, Dual-Licensing & Repository Topology

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Status**: `COMPLETED` (Revised with Upstream Open-Source Governance)  

---

## 1. Dual-Tier Licensing Architecture

### Decision
Implement a **Tiered Licensing Model**:
1. **Core Infrastructure & SDKs (`ttzip-core`)**: Permissive **`BSD-3-Clause OR Apache-2.0` Dual-License**.
2. **End-User Client Applications (`ttzip-apple`, `ttzip-windows`, etc.)**: Copyleft **`GPLv3` License** with strict Trademark & Trade Dress protection.

### Rationale & Precedents
1. **The Infrastructure vs. Consumer Application Dichotomy**:
   - **Building on SDKs is constructive ecosystem development**: When third-party games, backup tools, or AI pipelines embed `ttzip-core`, they create new value. Permissive dual-licensing allows them to embed the SDK without fear of forced source disclosure, driving TTZip toward becoming the global industrial compression standard.
   - **Repackaging complete GUI applications is opportunistic rent-seeking**: When third parties take a completed desktop/mobile GUI app, change the icon, and resell it on app stores, they extract value without contributing. `GPLv3` mandates 100% source disclosure for any derivative client app, while Trademark law prevents unauthorized commercial branding.
2. **Industry Validation**:
   - **OBS Studio**: Core `libobs` is permissive/modular; the OBS desktop application is `GPLv2/GPLv3`.
   - **VLC Media Player**: `libvlc` is LGPL/permissive for embedding; the VLC Player application is `GPLv3`.
   - **Cyberduck**: Native Mac/Windows client is `GPLv3` (generating millions on the Mac App Store while remaining 100% open-source on GitHub).

---

## 2. Multiplatform UI Naming & Architecture (`ttzip-apple`)

### Decision
Name the Apple platform client repository **`ttzip-apple`** rather than `ttzip-macos`.

### Rationale
1. **SwiftUI Code Reuse ($\ge 85\%$)**: Core data models (`ArchiveEntry`), business states (`AppViewState`), themes (`TTZipTheme`), and view hierarchies are 100% shared between macOS, iOS, iPadOS, and visionOS.
2. **Unified Xcode Project**: A single `ttzip-apple` repository cleanly manages multiplatform targets:
   - `TTZip (macOS)`: AppKit `NSOutlineView`, QuickLook, FinderSync extensions.
   - `TTZip (iOS/iPadOS)`: Document Provider and Share extensions.
   - `TTZip (visionOS)`: Spatial computing windows.
3. **Symmetrical Cross-Platform Client Roadmap**:
   - `ttzip-apple` (SwiftUI / AppKit)
   - `ttzip-windows` (WinUI 3 / C#)
   - `ttzip-android` (Jetpack Compose / Kotlin)
   - `ttzip-linux` (GTK4 / Libadwaita)

---

## 3. Rust Workspace 3-Crate Architecture

### Decision
Refactor `rust/` workspace from 2 crates (`ttzip-glue`, `ttzip-tui`) into **3 specialized crates**:

```
rust/
├── ttzip-engine/    # [NEW] Pure algorithm crate, no FFI, #![forbid(unsafe_code)], rlib
├── ttzip-glue/      # C-ABI FFI thin export layer (catch_unwind), staticlib/cdylib
└── ttzip-cli/       # [RENAME from ttzip-tui] 19 subcommands + Ratatui TUI, binary
```

### Rationale
1. **crates.io Readiness**: `ttzip-engine` can be consumed directly by Rust developers with zero FFI baggage.
2. **Direct PyO3 Binding**: Native language bindings (Python `pyo3`) bind directly to `ttzip-engine` in Safe Rust without C-ABI marshaling.
3. **FFI Specialization**: `ttzip-glue` specializes solely in C-ABI symbol export, zero-copy buffer views, and `catch_unwind` panic containment.

---

## 4. Public C-ABI Subset (`ttzip.h`) vs Internal Bridge (`ttzip_rust_glue.h`)

### Decision
Maintain **two C header tiers**:
1. `ttzip_rust_glue.h` (550 LOC): Internal comprehensive bridge used by `TTZipCore` (Swift) and system extensions.
2. `ttzip.h` ($\le 100\text{ LOC}$): Minimal, public C11 SDK interface with semver stability guarantees for external C/C++ integrators.

---

## 5. Upstream Open-Source Governance & Subtree Update Pipeline

### Decision
Codify automated upstream license harvesting and lifecycle maintenance:
1. **Triple-Attribution Manifest**:
   - `ACKNOWLEDGEMENTS.md` (Human-readable upstream author credits & full license texts).
   - `docs/THIRD_PARTY_LICENSES.md` (Web & release documentation).
   - `Acknowledgements.plist` (Bundled into macOS App "About" box).
2. **Automated Harvesting & CI Audit**:
   - `scripts/generate_acknowledgements.py` automatically updates manifests upon upstream bumps.
   - `scripts/audit_licenses.py` prevents copyleft contamination and invalid SPDX headers on every Git push.
3. **Subtree Maintenance Strategy**:
   - Upstream engines in `Vendor/*-upstream` are tracked via Git Subtrees (`git subtree pull --squash`), serving as differential test oracles and upstream contribution branches.

---

## 6. Zero-Cloud-Cost Local CI/CD Pipeline

### Decision
Consolidate all verification into local Git hooks (`.git/hooks/pre-push` and `scripts/run_local_ci_gate.sh`), utilizing local multi-core parallelism:
1. **Stage 1**: Single-file LOC defense gate (`lint_loc_gate.sh`, $\le 800\text{ LOC}$).
2. **Stage 2**: Rust test suite (unit, property-based, and clippy).
3. **Stage 3**: C-ABI symbol export & header consistency verification.
4. **Stage 4**: Swift test suite (`swift test --parallel`) and benchmark regression gate.
