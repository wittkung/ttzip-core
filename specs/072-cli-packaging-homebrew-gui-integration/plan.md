# Implementation Plan: 072-cli-packaging-homebrew-gui-integration

## 1. Executive Summary

This feature delivers:
1. **Automated Universal 2 Release Packaging & Homebrew Tap Pipeline**: A deterministic packaging script `scripts/package_cli_release.sh` building `arm64` and `x86_64` binaries, extracting `.dSYM`, stripping local symbols via `strip -x`, self-generating BSD man pages and 4-shell completion scripts, and generating `Formula/ttzip-cli.rb`.
2. **Desktop GUI Diagnostics & Standards Integration**: Integrating `ArchiveFormatStandardSpec`, `ArchiveMagicSignatureScanner`, `ZipExtraFieldParser`, and `StandardsComplianceChecker` into `TTZipApp` with `ArchiveInspectorViewModel`, `ArchiveInspectorSheet`, and Explorer context actions.

---

## 2. Technical Context & Constitution Check

| Invariant / Constraint | Requirement | Mitigation / Architecture Solution |
| :--- | :--- | :--- |
| **Zero Subprocess Overhead** | GUI must not spawn CLI subprocesses | Reuses `TTZipCore.Standards` in-process Swift + C static library bindings directly |
| **UI Non-Blocking (<5ms)** | Large archives must not freeze MainActor | Background `Task.detached` + thread-safe cache `ArchiveDiagnosticsCache` |
| **Channel Isolation** | MAS build (`-DMAS_BUILD`) cleanliness | GUI views conditionally compile Sparkle; packaging scripts remain decoupled |
| **Clean Packaging** | Release tarball must have 0 `._*` pollution | `COPYFILE_DISABLE=1` and `tar --no-mac-metadata --no-xattrs` |
| **Hard Performance Floors** | Zero throughput regression | All 13 gates in `XCTestPerformanceMeasureTests` must pass |

---

## 3. Phase 0: Research Items

- R001 [SUBAGENT:research] 《Universal 2 Release Packaging & Homebrew Formula Architecture》: Evaluated unified SPM `--arch arm64 --arch x86_64` vs `lipo` fallback, `strip -x` symbol stripping, and Homebrew formula DSL conventions.
- R002 [SUBAGENT:research] 《Desktop GUI Diagnostics & Standards Integration》: Evaluated `@MainActor` MVVM state management, background async diagnostic scanning, and multi-tab SwiftUI modal presentation.

---

## 4. Phase 1: Design & System Contracts

- **Data Models**: Defined in [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/072-cli-packaging-homebrew-gui-integration/data-model.md) (`CLIPackageConfig`, `CLIPackageManifest`, `ArchiveInspectorState`, `ArchiveDiagnosticsCacheKey`).
- **Contracts**:
  - [`contracts/release_packaging_manifest.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/072-cli-packaging-homebrew-gui-integration/contracts/release_packaging_manifest.json): JSON Schema for packaging manifest.
  - [`contracts/gui_inspector_payload.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/072-cli-packaging-homebrew-gui-integration/contracts/gui_inspector_payload.json): JSON Schema for GUI diagnostic payload.
- **Verification Guide**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/072-cli-packaging-homebrew-gui-integration/quickstart.md).

---

## 5. Component Breakdown & Planned Changes

### [Packaging & Distribution Pipeline]
- `scripts/package_cli_release.sh`: Universal 2 build, dSYM extraction, strip, man/completion self-generation, tarball, and SHA-256 calculation.
- `Formula/ttzip-cli.rb`: Homebrew tap formula definition.
- `Tests/TTZipTests/CLIPackagingTests.swift`: Unit tests for packaging script and formula verification.

### [Desktop GUI Diagnostics Integration]
- `Sources/TTZipApp/ViewModels/ArchiveInspectorViewModel.swift`: Async inspector state controller with caching.
- `Sources/TTZipApp/ViewModels/AppSubStates.swift`: Added `showArchiveInspectorModal` and `inspectingArchivePath` to `OverlayState`.
- `Sources/TTZipApp/Views/ArchiveInspectorSheet.swift`: Multi-tab SwiftUI sheet (`标准规范`, `幻数锚点`, `扩展字段`, `合规体检`).
- `Sources/TTZipApp/Views/Components/RightInspectorSidePanel.swift`: Quick diagnostic status badge and inspector trigger.
- `Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView.swift`: Context menu item "查看归档标准与体检...".
- `Tests/TTZipTests/ArchiveInspectorViewTests.swift`: Unit tests for ViewModel and diagnostic caching.
