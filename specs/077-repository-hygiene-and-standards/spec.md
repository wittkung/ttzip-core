# Feature Specification: Repository Hygiene, Governance Standards, and Contribution Architecture

**Feature Branch**: `077-repository-hygiene-and-standards`

**Created**: 2026-08-18

**Status**: Draft

**Input**: User description: "全面提高项目仓库专业性（不过 github 的 cicd 别开，没额度 我们尤其要提高的是分支策略 issue 和 pr 要求等等 以及仓库的干净，检查哪些不应该放到仓库的"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Standardized Issue Reporting & PR Contribution Workflow (Priority: P1)

As an open-source contributor or core maintainer, I want structured, standardized GitHub Issue and Pull Request templates along with comprehensive contributing guidelines, so that all bug reports, performance regression filings, feature proposals, and code contributions provide the exact technical context, reproducibility steps, and quality gate assertions required by TTZip.

**Why this priority**: Community health, governance rigor, and code quality depend directly on structured communication channels and enforceable PR checklists (e.g. performance floor verification, thread sanitizer checks, C pointer safety invariants).

**Independent Test**:
Can be verified by inspecting and validating all GitHub issue forms (`.github/ISSUE_TEMPLATE/`), the pull request template (`.github/pull_request_template.md`), and `CONTRIBUTING.md` against GitHub community profile standards and schema specifications.

**Acceptance Scenarios**:
1. **Given** a contributor opening an issue on GitHub, **When** they choose a template, **Then** they are presented with purpose-built structured forms for Bug Reports, Feature Requests, or Performance Regression filings with mandatory fields (macOS version, CPU architecture, format, reproduce steps).
2. **Given** a contributor submitting a Pull Request, **When** the PR draft is created, **Then** `.github/pull_request_template.md` is populated with mandatory verification checklists covering: Swift 6 concurrency, Memory/Thread sanitizers, C bridge pointer safety, Zero-Regression performance gates, and channel compatibility.
3. **Given** a contributor reading `CONTRIBUTING.md`, **When** navigating contribution instructions, **Then** they find clear branching rules, Conventional Commit standards, local build/test commands, and upstream synchronization invariants.

---

### User Story 2 - Comprehensive Repository Cleanliness & `.gitignore` / `.gitattributes` Hardening (Priority: P1)

As a developer or maintainer, I want the repository to be free of transient build outputs, local test artifacts, vendor upstream clones, IDE debris, and unneeded binaries, while maintaining hardened `.gitignore` and `.gitattributes` files, so that git clones remain lightweight, diffs remain clean, and zero untracked garbage enters version control.

**Why this priority**: Preventing repository pollution, accidental commits of huge build folders or binary caches, and cross-platform line-ending discrepancies is foundational to repository hygiene.

**Independent Test**:
Can be verified by auditing all tracked and untracked files across the workspace, ensuring all temporary directories (`.build*`, `build/`, `dist/`, `build_dist/`, `scratch/`, `reports/`, `site/`, `Vendor/*-upstream`) are strictly ignored, and `.gitattributes` correctly classifies binary, vendored, and text assets.

**Acceptance Scenarios**:
1. **Given** a clean clone or dirty build workspace, **When** `git status` is executed, **Then** temporary build directories (`.build_custom`, `.build_di_test`, `.build_tmp`, `dist`, `build`, `build_dist`), local benchmark output, vendor upstream directories (`Vendor/zlib-ng-upstream/`, `Vendor/libarchive-upstream/`, etc.), and OS metadata (`.DS_Store`) are completely ignored.
2. **Given** text and binary assets across the repository, **When** committed or checked out across different environments, **Then** `.gitattributes` enforces LF line normalization for code/markdown, marks static C binaries and images as binary, and marks `Vendor/` and `.specify/` as vendored/generated for GitHub Linguist statistics.

---

### User Story 3 - Formalized Git Branching Strategy & Release Lifecycle (Priority: P2)

As a maintainer or contributor, I want clear, documented git branching taxonomy and commit message conventions, so that development, hotfixing, release preparation, and upstream patch isolation follow predictable, disciplined workflows.

**Why this priority**: Unstructured branch naming and vague commit logs make history tracking, cherry-picking, and bisecting difficult in a high-performance systems codebase.

**Independent Test**:
Can be verified by checking the documented branching model in `CONTRIBUTING.md` and `docs/governance/BRANCHING_STRATEGY.md`, validating prefix conventions (`feat/`, `fix/`, `perf/`, `chore/`, `release/`, `upstream/`), and commit message rules (`type(scope): subject`).

**Acceptance Scenarios**:
1. **Given** a developer starting a new optimization or fix, **When** creating a branch, **Then** they follow the standard taxonomy (`feat/<name>`, `perf/<format>-<optimization>`, `fix/<issue-id>`).
2. **Given** a developer drafting commit messages, **When** formatting the log, **Then** it conforms to Conventional Commits with scope and issue references (e.g. `perf(lzma2): optimize swar matchfinder throughput`).

---

### User Story 4 - CI/CD Quota Protection & Offline Local Pre-Flight Verification Gate (Priority: P2)

As a repository maintainer with limited GitHub Actions runner minutes, I want automated push/PR CI triggers to remain strictly disabled (`workflow_dispatch` only), complemented by a robust, single-command local pre-flight verification script (`scripts/pre_flight_check.sh`), so that full quality gates (formatting, linter, parallel unit tests, performance floors) can be executed locally without consuming cloud CI quotas.

**Why this priority**: Saves GitHub runner minutes while preserving 100% test and quality gate rigor through automated local verification before PR submission.

**Independent Test**:
Can be verified by checking `.github/workflows/ci-cd.yml` trigger configurations (confirming `push:` and `pull_request:` remain disabled or commented out, with only `workflow_dispatch` active) and executing `scripts/pre_flight_check.sh` locally.

**Acceptance Scenarios**:
1. **Given** a commit pushed to `main` or a new PR opened, **When** GitHub Actions evaluates the workflow, **Then** no automatic runner jobs are triggered, preserving Actions runner quota.
2. **Given** a developer preparing a PR, **When** running `./scripts/pre_flight_check.sh`, **Then** the script sequentially validates repository cleanliness, SwiftLint, full unit test suite, and performance gate thresholds, outputting a clear PASS/FAIL summary.

---

## Edge Cases

- **Large untracked directories in workspace**: Ensuring `.gitignore` pattern matching handles nested directories (e.g. `Vendor/zlib-ng-upstream/`, `Vendor/worktrees/`, `.build_custom/`) without impacting tracked vendor files (`Vendor/*.a`, `Vendor/*.h`).
- **Linguist language statistics distortion**: Large third-party C vendor sources or generated spec files might artificially skew the repository language percentage away from Swift; `.gitattributes` must properly annotate `linguist-vendored` and `linguist-generated`.
- **Pre-flight script execution in sandbox / without external network**: Pre-flight script must operate completely in-process using local tools (`swift`, `swiftlint`, `git`) without network calls.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Repository MUST provide GitHub Issue form templates (`.github/ISSUE_TEMPLATE/`) for Bug Reports, Feature Proposals, Performance Regressions, and an Issue Config (`config.yml`).
- **FR-002**: Repository MUST provide a comprehensive Pull Request template (`.github/pull_request_template.md`) enforcing strict verification items (unit tests, performance floor check, zero memory leaks, thread safety, C bridge invariants).
- **FR-003**: Repository MUST update `CONTRIBUTING.md` with complete documentation covering Git branching strategy, Conventional Commits, local build commands, performance floors, and upstream collaboration protocol.
- **FR-004**: Repository MUST maintain an exhaustive `.gitignore` covering all SwiftPM, Xcode, CMake, temporary benchmark artifacts, build distributions, vendor worktrees, and OS metadata files.
- **FR-005**: Repository MUST provide a `.gitattributes` file specifying standard line endings (`* text=auto eol=lf`), binary file classifications, and Linguist overrides for vendored C libraries and generated assets.
- **FR-006**: Repository MUST preserve GitHub Actions quota protection in `.github/workflows/ci-cd.yml` by maintaining `workflow_dispatch` as the only active trigger, preventing unwanted runner execution on push/PR events.
- **FR-007**: Repository MUST provide a local pre-flight quality verification script (`scripts/pre_flight_check.sh`) that automates cleanliness checks, test suites, and performance gate evaluations locally.
- **FR-008**: Repository MUST clean up any existing unneeded, transient, or polluting files in the workspace (such as `.DS_Store`, transient build caches, and outdated untracked temp files).

### Key Entities

- **Governance Documents**: `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `docs/governance/BRANCHING_STRATEGY.md`.
- **GitHub Configurations**: `.github/ISSUE_TEMPLATE/*.yml`, `.github/pull_request_template.md`, `.github/workflows/ci-cd.yml`.
- **Git Attributes & Ignore**: `.gitignore`, `.gitattributes`.
- **Automation Scripts**: `scripts/pre_flight_check.sh`.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of temporary build directories, local benchmark dumps, and vendor worktrees are cleanly ignored by `.gitignore` with zero dirty artifacts appearing in a standard `git status`.
- **SC-002**: GitHub community health profile achieves full coverage with Issue Templates, PR Template, Contributing Guide, Security Policy, and Code of Conduct present and structurally valid.
- **SC-003**: 0 automatic GitHub Actions quota minutes consumed on regular push/PR events, while retaining full manual dispatch capability.
- **SC-004**: A single local command (`./scripts/pre_flight_check.sh`) can comprehensively verify repository hygiene, linting, tests, and performance gates in under 30 seconds.

---

## Assumptions

- Developers and maintainers work on macOS 14+ with standard Swift 6 toolchain installed.
- Cloud CI runs remain manual (`workflow_dispatch`) to avoid exceeding GitHub account quota limits.
- Upstream C libraries in `Vendor/` are maintained as static `.a` binaries and headers, with full upstream checkouts isolated in ignored directories.

---

## Clarifications

### Session 2026-08-18: Repository Hygiene and Governance Scope Alignment

- **Q1: GitHub CI/CD Trigger Strategy**:
  - *Resolution*: Keep GitHub Actions automatic push and pull_request triggers completely disabled (commented out) to prevent consuming GitHub runner minutes quota. Rely solely on manual `workflow_dispatch` and offline local pre-flight automation (`scripts/pre_flight_check.sh`).
- **Q2: File Exclusion Scope**:
  - *Resolution*: Strictly ignore and eliminate all transient build directories (`.build_custom`, `.build_di_test`, `.build_tmp`, `dist`, `build`, `build_dist`, `build_app`), local vendor upstream clones (`Vendor/*-upstream/`, `Vendor/worktrees/`, `Vendor/turbobench/`), ephemeral benchmark artifacts (`scratch_bench`, `payload`, `ditto_out.zip`, `*.tbb`), and OS metadata (`.DS_Store`).
- **Q3: GitHub Community Standards & Governance Profile**:
  - *Resolution*: Implement structured YAML Issue Forms for Bug Reports, Feature Requests, and Performance Regressions; create a thorough PR template with a multi-point verification checklist; create a dedicated branching strategy guide in `docs/governance/BRANCHING_STRATEGY.md`; update `CONTRIBUTING.md`, `SECURITY.md`, and `CODE_OF_CONDUCT.md`.

