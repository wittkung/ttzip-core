# Feature Specification: Swift Codebase Slimming & Redundant Code Purge

**Pipeline Level**: `[Lean SDD]`
**Feature Branch**: `203-swift-codebase-slimming`
**Created**: 2026-08-22
**Status**: Draft
**Input**: User description: "去全面瘦身。"

---

## Executive Summary & Scope

Now that the high-performance Rust core (`ttzip-glue`) and standalone CLI engine (`ttzip-tui`) are fully operational and verified, this feature executes a systematic physical code slimming across the Swift codebases:
1. **CLI Layer Slimming**: Purge redundant duplicate CLI subcommands and runners from `Sources/TTZipCLI` that replicate Rust's 18 subcommands, slimming `Sources/TTZipCLI` down to an ultra-thin SwiftPM wrapper delegating to the unified engine.
2. **Core Layer Slimming**: Eliminate obsolete intermediate OOP abstractions, unused legacy Swift algorithm duplicates, and dead helper classes in `Sources/TTZipCore`, consolidating them into lean, direct FFI facade models.
3. **Pristine Preservation**: Preserve 100% of native macOS UI (`TTZipApp`) and localization catalog assets without breaking view bindings or UX functionality.
4. **Verification Gate**: Guarantee that all 4 automated CI stages (`lint_loc_gate`, `swift test`, `ttzip-bench`, `run_rust_tests`) pass 100% green with zero regressions.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Lean & Rapid Swift Build Pipeline (Priority: P1)

As a macOS software developer and CI engineer, I want the Swift target build times and file count significantly reduced by removing duplicate Swift code, so that compilation is fast and maintenance is unified in a single authoritative Rust engine.

**Acceptance Scenarios**:
1. **Given** the pruned Swift codebase, **When** running `swift test` and `swift build`, **Then** the package builds cleanly with zero errors or missing symbol warnings.
2. **Given** the standalone Rust CLI, **When** invoking CLI operations, **Then** all 18 subcommands continue to operate with $< 5\text{ms}$ startup latency.

---

### User Story 2 - Zero UI & Functionality Regression (Priority: P2)

As an end-user of TTZip macOS app, I want all desktop features (archiving, browsing, inspecting, password recovery, QuickLook, Finder integration) to remain 100% functional and responsive.

**Acceptance Scenarios**:
1. **Given** `TTZipApp`, **When** performing compression, decompression, or tree navigation, **Then** the app delegates smoothly to the underlying engine with 60fps UI updates.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST prune obsolete duplicate Swift CLI handlers and runners from `Sources/TTZipCLI`, consolidating the target into a minimal entry point.
- **FR-002**: System MUST retain all required data models, facade protocols, and C-ABI bridge bindings in `Sources/TTZipCore` needed by `TTZipApp`.
- **FR-003**: System MUST preserve 100% of `Sources/TTZipApp` (144 UI and service files) without modifying user-facing UI behaviors.
- **FR-004**: System MUST ensure `Package.swift` builds cleanly across all remaining targets.
- **FR-005**: All remaining files MUST satisfy single-file LOC thresholds ($\le 800\text{ LOC}$) and codebase invariants.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Swift codebase file count reduced significantly while maintaining 100% feature parity.
- **SC-002**: `swift test` executes and passes all test suites with zero failures.
- **SC-003**: Rust workspace tests (`cargo test --workspace`) pass 100%.
- **SC-004**: Full 4-stage automated gate (`./scripts/run_local_ci_gate.sh`) passes 100% green.
