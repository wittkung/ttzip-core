# Feature Specification: macOS Finder Native Integration, Desktop GUI Bilingual Localization, and Fast LZMA2 Micro-Tuning

**Feature Branch**: `074-finder-integration-gui-i18n-fast-lzma2-tuning`  
**Created**: 2026-08-18  
**Status**: Specified  
**Input**: "二 三 四推进，四的推进要尤其小心，必须确保是正向性能提升，而不导致任何性能退步"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - macOS Finder Native Integration & Spacebar QuickLook (Priority: P1) 🎯 MVP

As a macOS desktop power user, I want to interact with TTZip directly from Finder through context menu actions and spacebar QuickLook previews so that I can compress, extract, and inspect archives without launching full external applications.

**Why this priority**: Native OS integration bridges the gap between CLI/standalone GUI and everyday macOS workflows, delivering frictionless user experience.

**Independent Test**: Can be independently verified by executing Finder context action triggers, validating archive file type bindings, and testing QuickLook thumbnail/preview generator generation for all 16 supported formats.

**Acceptance Scenarios**:
1. **Given** one or more selected files/folders in Finder, **When** the user invokes the context action or drag service, **Then** TTZip generates an archive with the user's preferred format preset.
2. **Given** any of the 16 supported archive formats selected in Finder, **When** the user presses Spacebar, **Then** TTZip's QuickLook provider renders an instant, non-blocking preview displaying the internal directory tree structure and metadata.

---

### User Story 2 - Desktop App Complete Bilingual Localization & Runtime Switcher (Priority: P1)

As a global macOS user, I want TTZip desktop application to seamlessly support English and 简体中文 with instant runtime switching in Preferences, so that all menus, inspectors, settings, and dialogs are 100% natural and culturally aligned.

**Why this priority**: Polishes the user experience across all GUI touchpoints and aligns desktop SwiftUI with the CLI's existing robust localization framework.

**Independent Test**: Can be independently verified by switching language settings in `SettingsView`, asserting that all views (Miller columns, Toolbar, Inspector sheets, Password Vault, Menus) update instantaneously without app relaunch or UI distortion.

**Acceptance Scenarios**:
1. **Given** the user opens TTZip Preferences (`SettingsView`), **When** selecting "English" or "简体中文", **Then** the entire application UI re-renders dynamically with the chosen language.
2. **Given** a non-English/non-Chinese locale, **When** the app boots in "Follow System", **Then** standard fallback cascades gracefully without missing string keys.

---

### User Story 3 - Fast LZMA2 Micro-Architecture Hardware Tuning & Zero-Regression Invariant (Priority: P1) ⚡️

As a high-performance compression user, I want 7Z and LZMA2 compression pipelines to maximize Apple Silicon hardware utilization (memory bandwidth, SIMD vectorization, and multi-stream partitioning) while strictly ensuring zero performance regression across all existing compression levels and archive formats.

**Why this priority**: Delivers verifiable positive throughput improvements on high-compression workloads while protecting the hard performance floor matrix.

**Independent Test**: Must be independently measured via the 4-step performance optimization protocol: Pre-baseline measurement ➔ targeted implementation ➔ Post-optimization differential audit asserting $\ge 0\%$ positive gain and $0$ performance regression across `XCTestPerformanceMeasureTests` and `AllFormatsPkSuiteTests`.

**Acceptance Scenarios**:
1. **Given** LZMA2 Level 1 and Level 5 compression workloads on Apple Silicon, **When** executing benchmarks, **Then** physical throughput achieves positive gain ($\Delta > 0\%$) over current baseline without increasing peak memory footprints.
2. **Given** all 13 hard performance floors in `XCTestPerformanceMeasureTests`, **When** the full regression suite runs, **Then** all 13 floors pass 100% with zero regression ($\Delta \ge -3.0\%$ margin check).

---

### Edge Cases

- **Corrupted / Damaged Archives in QuickLook**: When previewing an unreadable or truncated archive in Finder, the QuickLook provider must display a clean, non-crashing diagnostic badge without freezing the Finder process.
- **Dynamic Localization in Active Sheets**: When switching languages while an Inspector sheet or Password prompt is active, all dynamic text elements must re-layout smoothly without clipping or truncated labels.
- **Very Short / Uncompressible Payloads in LZMA2**: When compressing randomized or sub-kilobyte data under Fast LZMA2, the engine must gracefully short-circuit to uncompressed store or fast literal blocks without CPU cycle waste.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide native Finder integration handlers for archive creation and extraction.
- **FR-002**: System MUST provide a dedicated QuickLook preview generator capable of generating lightweight hierarchy previews for all supported archive formats.
- **FR-003**: System MUST provide a global `SettingsView` in `TTZipApp` with a language picker supporting "Follow System", "简体中文", and "English".
- **FR-004**: System MUST synchronize GUI localization keys with `TTZipLocalizationManager` across all views, sheets, toolbars, and app menus.
- **FR-005**: System MUST strictly adhere to the 4-Step Performance Optimization Protocol for all LZMA2 and C bridge tuning:
  1. Scope Demarcation.
  2. Pre-Optimization Baseline Measurement.
  3. Targeted Implementation & Hardening.
  4. Post-Optimization Differential Audit with Zero-Regression Verification.
- **FR-006**: System MUST maintain 100% backward compatibility and bit-for-bit extraction idempotency across all archive formats.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: QuickLook preview generation latency for 10,000-item archives is under **50ms**.
- **SC-002**: Runtime language switching in GUI takes **< 10ms** with zero screen flicker and zero app restart requirements.
- **SC-003**: Fast LZMA2 compression throughput demonstrates positive gain ($\Delta > 0\%$) without regressing any of the 13 hard performance floors in `XCTestPerformanceMeasureTests`.
- **SC-004**: 100% passing rate across the local 6-stage automated CI gate (`./scripts/run_local_ci_gate.sh`).

---

## Assumptions

- Target operating system is macOS 14.0+ (Sonoma) running on Apple Silicon (M-series) or Intel x86_64.
- QuickLook previews run out-of-process and do not block system QuickLook daemon threads.
- All hardware-specific vector instructions are guarded with runtime CPU capability checks.
