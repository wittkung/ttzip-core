# Phase 0 Research: Repository Hygiene, Governance Standards, and Contribution Architecture

**Feature Branch**: `077-repository-hygiene-and-standards`
**Date**: 2026-08-18
**Status**: Completed

---

## Research Items Summary

| Item ID | Research Domain | Target Output | Status |
| :--- | :--- | :--- | :--- |
| **R001** | GitHub YAML Issue Forms & PR Verification Template | `.github/ISSUE_TEMPLATE/*.yml`, `.github/pull_request_template.md` | ✅ Complete |
| **R002** | Repository Hygiene, `.gitignore` & `.gitattributes` | `.gitignore`, `.gitattributes` | ✅ Complete |
| **R003** | Git Branching Strategy & Conventional Commits | `CONTRIBUTING.md`, `docs/governance/BRANCHING_STRATEGY.md` | ✅ Complete |
| **R004** | CI/CD Quota Protection & Local Pre-Flight Architecture | `.github/workflows/ci-cd.yml`, `scripts/pre_flight_check.sh` | ✅ Complete |

---

## Detailed Research Findings

### R001: GitHub YAML Issue Forms & PR Template Specification

- **Decision**:
  - Implement structured GitHub YAML Issue Forms (`.github/ISSUE_TEMPLATE/bug_report.yml`, `performance_regression.yml`, `feature_request.yml`) and `config.yml`.
  - Disable un-templated issues via `blank_issues_enabled: false` in `config.yml` while adding direct links to Security Advisories, GitHub Discussions, Architecture docs, and Benchmark reports.
  - Implement a comprehensive `.github/pull_request_template.md` mandating:
    1. Scope & Affected Subsystem tagging.
    2. Five non-negotiable verification gates: Core Performance Floor (Zero-Regression against peak floor `604d44d`), Swift 6 Concurrency & Actor isolation, Sanitizers matrix (ASan/TSan), C Bridge Pointer Safety & Dead-Store elimination, and Dual-Channel Compatibility (Direct vs MAS `-DMAS_BUILD`).
    3. Mandatory Differential Benchmark comparison table (Baseline MB/s vs PR MB/s with $\Delta\%$).
    4. Exact console verification commands and log attachments.
- **Rationale**:
  - Eliminates ambiguous and incomplete bug reports by enforcing structured dropdowns for Apple Silicon vs Intel, macOS version, format type, and distribution channel.
  - Fixes performance tracking by mandating quantitative baseline-vs-observed throughput numbers on the 46 historical peak benchmarks before review.
  - Eliminates back-and-forth triage questions.
- **Alternatives Considered**:
  - *Legacy Freeform Markdown Issue Templates (`.md`)*: Rejected because reporters regularly omit critical environment metadata (architecture, channel, commit hash), requiring multiple rounds of triage comments.
  - *GitHub Discussions Only for Bugs*: Rejected because systems bug tracking requires deterministic issue states, automated triage labels (`bug`, `triage`), and explicit linking to PR fixes.
- **Source**:
  - GitHub Docs: [Syntax for GitHub's form schema](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-githubs-form-schema)
  - Project Invariants: `GEMINI.md` (§IV.3, §IV.3.1, §IV.5, §V, §VII.3)
  - Subagent Research: Conversation `d8f96fe7-4311-43af-8a61-bbc931ad05b7`

---

### R002: Repository Cleanliness, `.gitignore` & `.gitattributes` Hardening

- **Decision**:
  - Upgrade `.gitignore` to comprehensively cover:
    1. macOS System & Finder metadata (`.DS_Store`, `._*`, `.Spotlight-V100`, `.Trashes`).
    2. SwiftPM & Xcode build outputs (`.build/`, `.build_*/`, `build/`, `dist/`, `build_dist/`, `build_app/`, `DerivedData/`, `xcuserdata/`, `*.xcworkspace/`, `.swiftpm/`).
    3. C/C++ intermediate objects (`*.o`, `*.obj`, `*.dSYM/`, `*.so`, `*.dylib`, CMake caches).
    4. Vendor upstream checkouts (`Vendor/*-upstream/`, `Vendor/turbobench/`, `Vendor/worktrees/`).
    5. Binary targets & static libraries (global `*.a` ignore with explicit whitelisting for `!Vendor/lib/*.a` and `!Vendor/TTZipVendor.xcframework/**/*.a`).
    6. Benchmark outputs and scratch debris (`payload/`, `scratch_bench/`, `scratch/`, `reports/`, `*.bin`, `*.tbb`, `test_*`).
    7. Spec Kit and local environment overrides (`.env*`, `.specify/feature.json`).
  - Create `.gitattributes` to:
    1. Enforce global Unix LF normalization (`* text=auto eol=lf`) and language-specific diff drivers (`*.swift diff=swift`, `*.c diff=c`).
    2. Protect all binary files (`*.a`, `*.dylib`, `*.xcframework`, `*.png`, `*.icns`, `*.zip`, `*.7z`, `*.tar`, `*.zst`, `*.dmg`, etc.) from CRLF/LF line mangling.
    3. Override GitHub Linguist statistics by marking `Vendor/**` as `linguist-vendored`, `specs/**`, `.specify/**`, `.agents/**`, and `*.patch` as `linguist-generated`, and `docs/**` as `linguist-documentation`.
- **Rationale**:
  - Prevents multi-gigabyte build artifacts, local benchmarks, and third-party vendor checkouts from polluting `git status` or being inadvertently committed.
  - Ensures repository language breakdown accurately reflects TTZip's Swift & C core without dilution by vendored upstream C sources or extensive markdown specs.
  - Guarantees deterministic line endings across heterogeneous developer machines.
- **Alternatives Considered**:
  - *Minimal default `.gitignore`*: Rejected because TTZip produces heavy multi-format benchmark artifacts (`scratch/`, `reports/`) and custom build folders (`.build_custom`, `.build_di_test`) that quickly clutter workspace status.
  - *Git Submodules for `Vendor/`*: Rejected because submodules introduce high clone friction and recursive checkout failures in CI/sandbox environments; precompiled static frameworks + whitelisted ignore rules maintain zero-dependency builds.
- **Source**:
  - Local repository files: `Package.swift`, `Vendor/`, `scripts/package_cli_release.sh`, `.gitignore`
  - GitHub Linguist Overrides Documentation: [github-linguist/linguist](https://github.com/github-linguist/linguist/blob/master/docs/overrides.md)
  - Subagent Research: Conversation `07e7bbf9-4a65-4e54-84a5-539a7ed88c63`

---

### R003: Git Branching Strategy & Conventional Commits Governance

- **Decision**:
  - Adopt a semantic Short-Lived Feature Branch model based off `main` with defined prefix taxonomy:
    - `main`: Protected production trunk (PR only, linear history).
    - `feat/<name>`: New archiving features, format additions.
    - `perf/<format>-<optimization>`: Algorithmic, SIMD, and parallel throughput breakthroughs.
    - `fix/<issue-id>-<slug>`: Bug fixes and security patches.
    - `upstream/<lib>-<patch>`: Isolated upstream contribution patches (preserves 3-stage `infra` ➔ `feat` ➔ `test` commits).
    - `release/v<version>`: Release stabilization and appcast / version bumps.
    - `docs/<name>`, `chore/<name>`: Documentation, build toolchain updates.
  - Enforce Conventional Commits v1.0.0 (`type(scope): subject`):
    - Types: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `chore`, `ci`.
    - Scopes: Format domains (`zip`, `7z`, `tar`, `zstd`, `lzma2`, `lz4`, `brotli`, `lzip`, `lrzip`, `wim`, `dmg`, `iso`, `snappy`, `aar`), Core domains (`crypto`, `bridge`, `stream`, `security`), UI/CLI (`app`, `cli`, `bench`), Infrastructure (`build`, `ci`, `vendor`).
  - Document the full strategy in `docs/governance/BRANCHING_STRATEGY.md` and link directly in `CONTRIBUTING.md`.
- **Rationale**:
  - Guarantees clean, bisectable history (`git bisect run swift test`) where every single commit compiles and passes test gates.
  - Segregates external upstream library patches (`Vendor/*`) from TTZip application glue, matching upstream maintainer contribution requirements.
  - Automates changelog generation and SemVer release notes.
- **Alternatives Considered**:
  - *Classic Git Flow (`develop` + `master`)*: Rejected due to unnecessary merge friction, delayed integration, and complex dual-merge backports that degrade linear `git bisect`.
  - *Freeform commit messages*: Rejected because automated release notes and subsystem-targeted changelogs become impossible to parse.
- **Source**:
  - Conventional Commits Specification v1.0.0 (https://www.conventionalcommits.org/en/v1.0.0/)
  - Project specifications: `CONTRIBUTING.md`, `GEMINI.md` (§VII.4, §VII.5)
  - Subagent Research: Conversation `6f1c648f-9255-4c89-a386-968e7c232813`

---

### R004: CI/CD Quota Protection & Local Pre-Flight Verification Architecture

- **Decision**:
  - Configure `.github/workflows/ci-cd.yml` with **solely `workflow_dispatch`** (manual trigger). Keep automatic `push` and `pull_request` triggers strictly disabled to consume **0 automatic GitHub Actions runner minutes**.
  - Engineer a robust, single-command offline local verification script at `scripts/pre_flight_check.sh`:
    - **Stage 1 (Hygiene Gate)**: Scans for unignored `.DS_Store`, dirty worktree files, and untracked build artifacts.
    - **Stage 2 (Static Analysis Gate)**: Runs `scripts/lint_codebase_invariants.py --strict` and `swiftlint --strict`.
    - **Stage 3 (Unit Test Gate)**: Runs full test suite in parallel across all CPU cores (`swift test --parallel`).
    - **Stage 4 (Performance Floor Gate)**: Runs `swift test --filter XCTestPerformanceMeasureTests` sequentially in isolated thread state.
    - **Stage 5 (Summary)**: Generates a formatted ASCII execution summary with per-stage timings and exit code 0/1.
- **Rationale**:
  - GitHub Actions macOS-14 Apple Silicon runners consume quota at a 10x multiplier. Disabling automated triggers prevents rapid depletion of monthly minutes while preserving cloud release packaging for tags.
  - Local Apple Silicon execution completes the 584+ unit tests and performance floors in under 20 seconds, providing faster feedback than remote cloud containers.
  - Standardizes the pre-submission check so contributors can guarantee PR acceptance before pushing code.
- **Alternatives Considered**:
  - *Lightweight push CI*: Rejected because even minimal jobs trigger container allocation and the 10x runner minute multiplier on macOS.
  - *Git pre-push hook only*: Rejected because hooks can be bypassed with `--no-verify`, cannot easily be run standalone in custom scripts, and lack structured summary reporting.
- **Source**:
  - `.github/workflows/ci-cd.yml` (lines 1–128)
  - `scripts/run_local_ci_gate.sh`, `scripts/lint_codebase_invariants.py`, `.swiftlint.yml`
  - Subagent Research: Conversation `2a9e6df4-5518-4041-9b6e-400d49f1844c`
