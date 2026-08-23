# Feature Specification: 072-cli-packaging-homebrew-gui-integration

## Title
CLI Release Packaging, Homebrew Formula Distribution, and Desktop GUI Diagnostic Integration

## Metadata
- **Feature Directory**: `specs/072-cli-packaging-homebrew-gui-integration/`
- **Created**: 2026-08-18
- **Status**: Draft / In Progress
- **Target Branch**: `main`
- **Priority**: P1

---

## 1. Executive Summary

TTZip CLI has achieved complete POSIX compliance, UNIX pipe streaming, dynamic shell auto-completions (Zsh, Bash, Fish, Nushell), and BSD mdoc troff man page generation.

This feature completes the distribution loop and desktop UI synergy:
1. **Automated Universal Release Packaging & Homebrew Tap**: Deliver `scripts/package_cli_release.sh` to compile Universal 2 (`arm64` + `x86_64`) binaries, assemble standard UNIX directory hierarchies with embedded man pages and completions, generate tarballs, and dynamically produce production-grade `Formula/ttzip-cli.rb`.
2. **Desktop GUI Diagnostics & Standards Integration**: Seamlessly connect `ArchiveFormatStandardSpec`, `ArchiveMagicSignatureScanner`, `ZipExtraFieldParser`, and `StandardsComplianceChecker` into `TTZipApp` (Inspector Sheet, Archive Health Check Dialog, and Context Menus), bridging CLI infrastructure with desktop user experience.

---

## 2. User Scenarios & Personas

### Scenario 1: Command-Line Developer / Homebrew User (User Story 1 - Distribution)
As a developer on macOS, I want to install `ttzip-cli` via `brew install wittkung/tap/ttzip-cli` or download a standalone release tarball so that I get the CLI binary, manual page (`man ttzip-cli`), and shell auto-completion out of the box with zero manual configuration.

### Scenario 2: Release Automation Engineer (User Story 2 - Packaging)
As a maintainer, I want to run `./scripts/package_cli_release.sh --version 1.0.0` so that a stripped, optimized Universal 2 release bundle (`ttzip-cli-v1.0.0-darwin-universal.tar.gz`) is compiled, self-generates its man page and completions, calculates SHA-256, and outputs an updated Homebrew formula.

### Scenario 3: macOS Desktop User (User Story 3 - GUI Inspector & Diagnostics)
As a macOS power user managing compressed archives in `TTZipApp`, I want to open the **Archive Inspector** or run **Archive Health Check** from the context menu to view RFC standard citations, Magic signature layout, Zip64/AES parameters, and detect header corruptions or Zip Slip vulnerabilities without opening a terminal.

---

## 2.1 Clarifications & Design Decisions

- **C1 (Target Architectures)**: `scripts/package_cli_release.sh` defaults to Universal 2 (`arm64` + `x86_64`) using `lipo -create`, and accepts `--arch <arm64|x86_64|universal>` for target-specific builds.
- **C2 (Formula Organization)**: The Homebrew Formula is stored at `Formula/ttzip-cli.rb`, fully compatible with `brew tap wittkung/tap` or direct repository taps.
- **C3 (GUI Entry Points)**: Inspector and Health Check are integrated into `TTZipApp` via:
  1. Main Menu (`Tools` -> `Archive Inspector`, `Tools` -> `Health Check`)
  2. Explorer Toolbar icon badge
  3. Archive item context menu (`Inspect Archive Metadata...`, `Run Health Check...`)

---

## 3. Functional Requirements

- **FR-001 [Packaging]**: `scripts/package_cli_release.sh` must support building Universal 2 (`arm64` and `x86_64`) binaries via `swift build -c release` with `-O3` optimization and symbol stripping.
- **FR-002 [UNIX Hierarchy Assembly]**: The packaging script must automatically execute the compiled `ttzip-cli` binary to self-generate:
  - `share/man/man1/ttzip-cli.1` (via `ttzip-cli man`)
  - `share/zsh/site-functions/_ttzip-cli` (via `ttzip-cli completion zsh`)
  - `share/bash-completion/completions/ttzip-cli` (via `ttzip-cli completion bash`)
  - `share/fish/vendor_completions.d/ttzip-cli.fish` (via `ttzip-cli completion fish`)
- **FR-003 [Homebrew Formula]**: `Formula/ttzip-cli.rb` must conform to Homebrew official standards, installing binaries to `bin/`, manual pages to `man1/`, and completions to respective Homebrew directories.
- **FR-004 [GUI Inspector Model]**: `TTZipApp` must integrate an `ArchiveInspectorViewModel` displaying:
  - Official specification name and citation (RFC, ISO, POSIX, PKWARE)
  - Magic signature matched anchor and bytes
  - Extra field breakdown (UTC timestamps, WinZip AES, Zip64)
  - Multi-volume and container properties
- **FR-005 [GUI Health Check Dialog]**: `TTZipApp` must provide an interactive health check modal / popover executing `StandardsComplianceChecker` and presenting visual pass/warn badges and violation descriptions.

---

## 4. Non-Functional Requirements & Invariants

- **NFR-001 (Zero External CLI Dependency in GUI)**: The GUI Inspector and Health Check features must invoke `TTZipCore` in-process APIs directly, with zero `Process()` or CLI subprocess spawning.
- **NFR-002 (UI Performance Invariants)**: Inspector metadata extraction must complete in under 5.0 ms for archives up to 100,000 entries.
- **NFR-003 (Deterministic Packaging)**: Tarball creation must generate deterministic checksums when built from identical sources.
- **NFR-004 (Channel Isolation)**: MAS build flags (`-DMAS_BUILD`) must compile cleanly without referencing Sparkle or external CLI scripts.

---

## 5. Success Criteria & Verification Metrics

1. `./scripts/package_cli_release.sh --version 1.0.0 --dry-run` and live package generation succeed with 0 errors and produce a valid `.tar.gz` and `Formula/ttzip-cli.rb`.
2. Unit tests in `Tests/TTZipTests/CLIPackagingTests.swift` verify release archive structure, SHA-256 calculation, and Formula syntax.
3. Unit and UI tests in `Tests/TTZipTests/ArchiveInspectorViewTests.swift` verify GUI diagnostic view models and zero regression in `FrontendPerformanceGateTests`.
4. All 6 stages in `./scripts/run_local_ci_gate.sh` pass with 100% green status.
