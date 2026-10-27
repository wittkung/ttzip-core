# Implementation Plan: Repository Hygiene, Governance Standards, and Contribution Architecture

**Branch**: `077-repository-hygiene-and-standards` | **Date**: 2026-08-18 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/077-repository-hygiene-and-standards/spec.md`

---

## Summary

This plan executes a comprehensive institutional upgrade to TTZip's repository governance, contribution workflow, repository cleanliness, and CI/CD quota efficiency. It introduces structured GitHub YAML Issue Forms (Bug Reports, Performance Regressions, Feature Requests), an enforceable multi-gate Pull Request Template, a formalized Git Branching Strategy with Conventional Commits, an exhaustive `.gitignore` and `.gitattributes` configuration with Linguist overrides, and an offline single-command local pre-flight quality verification script (`scripts/pre_flight_check.sh`), while ensuring GitHub Actions automated push/PR runner triggers remain disabled to consume zero cloud CI quota.

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`), C11 / POSIX, Bash 5+ / Zsh, YAML / JSON Schema Draft-07  
**Primary Dependencies**: SwiftPM, Clang/LLVM, SwiftLint (`.swiftlint.yml`), `Vendor/*.a` in-process C libraries  
**Storage**: File system, Git metadata (`.git/`, `.gitignore`, `.gitattributes`, `.github/`)  
**Testing**: `swift test --parallel`, `swift test --filter XCTestPerformanceMeasureTests`, `scripts/pre_flight_check.sh`  
**Target Platform**: macOS 14.0+ (Sonoma, Sequoia; Apple Silicon M1-M5 prioritized, Intel x86_64 compatible)  
**Project Type**: Systems Engineering Desktop App & CLI Toolchain (In-Process C + Swift 6 Architecture)  
**Performance Goals**:
- Core Performance Floor: 0% regression against historical peak matrix (`604d44d` / `GEMINI.md` §IV.3.1)
- Pre-Flight Verification Script execution time: < 30 seconds for full local suite (cleanliness + lint + tests + perf gate)
- Actions Runner Minutes consumed automatically on push/PR: 0 minutes
**Constraints**:
- GitHub Actions automated `push` and `pull_request` triggers must remain disabled (`workflow_dispatch` only)
- Zero intermediate memory allocations and zero locking on hot paths
- MAS Sandbox compatibility (`-DMAS_BUILD`) and Direct Sparkle isolation (`#if !MAS_BUILD`) must be verified
**Scale/Scope**: 16 archive formats, 584+ unit tests, complete repository root and `.github/` governance suite

---

## Constitution Check

*GATE: Evaluated against `.specify/memory/constitution.md`*

| Constitution Principle | Status | Evaluation & Compliance Details |
| :--- | :--- | :--- |
| **1. Architecture & Tech Boundaries** | ✅ PASS | Preserves 100% in-process C bindings; maintains dual-channel release gates (MAS `-DMAS_BUILD` vs Direct Sparkle). |
| **2. Hot-Path Performance Invariants** | ✅ PASS | PR template and pre-flight script mandate execution of `XCTestPerformanceMeasureTests` asserting throughput floors (ZIP >= 1500 MB/s, 7Z >= 3200 MB/s, TAR.ZST >= 15000 MB/s). |
| **3. Subsystem Freeze & Safety** | ✅ PASS | PR checklist and invariant linter enforce strict freeze protections on core ZIP engines unless explicitly unlocked. |
| **4. The Four Systemic Invariants** | ✅ PASS | Stream-First, Invariant-First (POSIX AT-APIs), Bounds-First (Magic/Sanitizers), and Oracle-First (Differential tests) are codified into PR review checklists. |
| **5. Verification & Quality Gates** | ✅ PASS | All changes verified via local pre-flight script with zero `--no-verify` bypass allowed; CI triggers configured for manual dispatch only. |

---

## Phase 0: Research Items

- R001 [SUBAGENT:research] 《GitHub Issue Forms & PR Template Standards》: Investigate YAML Issue Forms schema, fields for Bug Reports, Performance Regressions, Feature Proposals, `config.yml`, and multi-gate PR verification checklist.
- R002 [SUBAGENT:research] 《Repository Hygiene & Gitattributes Configuration》: Audit untracked build artifacts, benchmark outputs, and upstream worktrees; design exhaustive `.gitignore` and `.gitattributes` with Linguist overrides.
- R003 [SUBAGENT:research] 《Git Branching Strategy & Conventional Commits Governance》: Define branch taxonomy (`main`, `feat/*`, `perf/*`, `fix/*`, `upstream/*`, `release/*`) and Conventional Commits scope rules.
- R004 [SUBAGENT:research] 《CI/CD Quota Protection & Local Pre-Flight Architecture》: Configure `.github/workflows/ci-cd.yml` for manual dispatch and design standalone local verification script `scripts/pre_flight_check.sh`.

---

## Phase 1: Design Artifacts & Contracts

- `specs/077-repository-hygiene-and-standards/data-model.md`: Governance entities, Issue/PR schema models, Pre-flight status models, and Git rule models.
- `specs/077-repository-hygiene-and-standards/contracts/issue-form-schema.json`: Strict JSON Schema for GitHub YAML Issue Forms.
- `specs/077-repository-hygiene-and-standards/contracts/pr-template-schema.json`: Strict JSON Schema for Pull Request Template structure and verification gates.
- `specs/077-repository-hygiene-and-standards/contracts/pre-flight-report-schema.json`: Strict JSON Schema for Pre-Flight verification script execution report.
- `specs/077-repository-hygiene-and-standards/contracts/git-hygiene-schema.json`: Strict JSON Schema for Gitignore & Gitattributes rule assertions.
- `specs/077-repository-hygiene-and-standards/quickstart.md`: Runnable validation scenarios for Issue Forms, PR Template, Gitignore/Attributes, and Pre-Flight script.

---

## Planned Component Modifications

### Component 1: GitHub Governance & Issue / PR Templates (`.github/`)
- **[NEW]** [`.github/ISSUE_TEMPLATE/bug_report.yml`](file:///Users/kevintung/Documents/dev/TTZip/.github/ISSUE_TEMPLATE/bug_report.yml): Structured Bug Report form with hardware architecture, format dropdown, macOS version, and reproduction steps.
- **[NEW]** [`.github/ISSUE_TEMPLATE/performance_regression.yml`](file:///Users/kevintung/Documents/dev/TTZip/.github/ISSUE_TEMPLATE/performance_regression.yml): Dedicated Performance Regression form requiring baseline vs observed MB/s, delta $\Delta\%$, hardware profile, and CLI repro command.
- **[NEW]** [`.github/ISSUE_TEMPLATE/feature_request.yml`](file:///Users/kevintung/Documents/dev/TTZip/.github/ISSUE_TEMPLATE/feature_request.yml): Feature Request and Architecture Proposal form with hot-path allocation impact and systems design checklist.
- **[NEW]** [`.github/ISSUE_TEMPLATE/config.yml`](file:///Users/kevintung/Documents/dev/TTZip/.github/ISSUE_TEMPLATE/config.yml): Global issue form configuration disabling blank issues and linking to security advisories and discussions.
- **[NEW]** [`.github/pull_request_template.md`](file:///Users/kevintung/Documents/dev/TTZip/.github/pull_request_template.md): Comprehensive PR template with 5-stage verification checklist (Performance Floors, Swift 6 Concurrency, ASan/TSan, C Bridge Safety, Dual-Channel Compatibility) and differential benchmark table.
- **[MODIFY]** [`.github/workflows/ci-cd.yml`](file:///Users/kevintung/Documents/dev/TTZip/.github/workflows/ci-cd.yml): Confirm and harden `workflow_dispatch` manual-only trigger to guarantee 0 automatic cloud runner minute consumption.

### Component 2: Repository Cleanliness, Git Ignore & Attributes (`.gitignore`, `.gitattributes`)
- **[MODIFY]** [`.gitignore`](file:///Users/kevintung/Documents/dev/TTZip/.gitignore): Comprehensive coverage for SwiftPM (`.build_*/`), Xcode (`DerivedData`, `xcuserdata`), build distributions (`build/`, `dist/`, `build_dist/`, `build_app/`), scratch/benchmark output (`scratch/`, `reports/`, `payload/`, `scratch_bench/`, `*.bin`, `*.tbb`), and vendor upstream clones (`Vendor/*-upstream/`, `Vendor/turbobench/`, `Vendor/worktrees/`).
- **[NEW]** [`.gitattributes`](file:///Users/kevintung/Documents/dev/TTZip/.gitattributes): Global LF line normalization, binary file declarations, and GitHub Linguist classification (`linguist-vendored`, `linguist-generated`, `linguist-documentation`).

### Component 3: Community Governance & Documentation (`CONTRIBUTING.md`, `docs/governance/`)
- **[MODIFY]** [`CONTRIBUTING.md`](file:///Users/kevintung/Documents/dev/TTZip/CONTRIBUTING.md): Restructure into comprehensive guide covering Branching Strategy, Conventional Commits, local test/build commands, Performance Invariants, and PR verification rules.
- **[NEW]** [`docs/governance/BRANCHING_STRATEGY.md`](file:///Users/kevintung/Documents/dev/TTZip/docs/governance/BRANCHING_STRATEGY.md): Detailed Git branching taxonomy, commit prefix conventions, and upstream patch isolation workflows.
- **[MODIFY]** [`SECURITY.md`](file:///Users/kevintung/Documents/dev/TTZip/SECURITY.md): Update reporting instructions, supported versions matrix, and vulnerability handling timeline.
- **[MODIFY]** [`CODE_OF_CONDUCT.md`](file:///Users/kevintung/Documents/dev/TTZip/CODE_OF_CONDUCT.md): Align with standard Contributor Covenant v2.1.

### Component 4: Local Automation & Pre-Flight Quality Gate (`scripts/`)
- **[NEW]** [`scripts/pre_flight_check.sh`](file:///Users/kevintung/Documents/dev/TTZip/scripts/pre_flight_check.sh): High-performance standalone local pre-flight script executing Repo Hygiene, Invariant Linting, SwiftLint, Parallel Unit Tests, and Core Performance Floor Gate with formatted summary reporting.

---

## Complexity Tracking

> **Constitution Compliance**: Zero violations. No architectural complexity additions. All tools operate in-process using existing toolchains.
