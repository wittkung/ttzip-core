# Feature Specification: Upstream Contribution & Benchmark Verification Protocol Reconstruction

**Feature Directory**: `specs/131-upstream-contribution-and-benchmark-protocol`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "好好从这次 pr 里吸取经验吧，我们真的犯了太多错 /speckit-specify"

---

## 1. Executive Summary & Root Cause Retrospective

During the upstream contribution journey on PR #2416 (`zlib-ng/zlib-ng#2416`), four critical procedural and technical vulnerabilities were uncovered:

1. **Compiler Flag Asymmetry**: Initial baseline measurements lacked `-DWITH_NATIVE_INSTRUCTIONS=ON` while the candidate had it enabled, producing skewed initial macro figures.
2. **Thermal / DVFS Run-Order Bias**: Apple Silicon single-core saturation causes 1%~2% clock modulation over multi-minute runs; whichever binary executed first gained a physical temperature and frequency advantage.
3. **Template Literal Hardcoding**: Generating markdown tables dynamically while leaving static numeric placeholders in text paragraphs created data synchronization discrepancies.
4. **Premature Remote Mutation**: Invoking remote GitHub write APIs without explicit user inspection and authorization violated the pair-programming sovereignty boundary.

This feature establishes an **ironclad, automated protocol and tooling harness** to make these errors structurally impossible in all future upstream contributions and local engine benchmarking.

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - Dual Cross-Over In-Memory Benchmark Engine (Priority: P1)

As an engine developer and open-source contributor, I want an automated benchmark execution harness that enforces identical compiler flags and runs mirrored A/B and B/A execution orders, so that hardware thermal/DVFS drift is mathematically eliminated and all reported speedups represent genuine algorithmic gains.

**Why this priority**: Without strict flag parity and cross-over ordering, benchmark numbers are susceptible to hardware noise and configuration errors.

**Independent Test**: Execute the harness with two identical binaries; verify that the cross-over delta converges to exactly 0.00% ± 0.2% and that compiler flags are verified identical before execution starts.

**Acceptance Scenarios**:
1. **Given** a baseline build directory and a candidate build directory, **When** the benchmark harness is invoked, **Then** it inspects `CMakeCache.txt` and compiler invocations to assert 100% flag symmetry before executing any benchmarks.
2. **Given** verified build directories, **When** benchmarking begins, **Then** it executes Order A (Candidate first, Baseline second) for 5 repetitions and Order B (Baseline first, Candidate second) for 5 repetitions in 100% RAM-to-RAM mode.
3. **Given** both raw JSON outputs, **When** computing metrics, **Then** it outputs the Cross-Over Mean, Order A gain, Order B gain, and statistical noise classification.

---

### User Story 2 - Zero-Hallucination Dynamic Report Generator (Priority: P2)

As a contributor drafting pull request descriptions and maintainer responses, I want every single number, table cell, extreme value, and summary text snippet to be dynamically bound to JSON data points via AST/template variables, so that zero human or template hardcoding can ever leak into published documents.

**Why this priority**: Manually typing or editing numbers in markdown leads to subtle discrepancies that destroy credibility with upstream maintainers.

**Independent Test**: Feed synthetic JSON benchmark datasets with intentional edge values; verify that 100% of occurrences of metrics across the entire output document update dynamically with zero stale literals.

**Acceptance Scenarios**:
1. **Given** raw benchmark JSON files, **When** the report generator runs, **Then** it produces full markdown reports where all tables, summary ranges, and option descriptions are purely parameterized.
2. **Given** any modification to the raw JSON, **When** regenerating the report, **Then** all numbers across all sections update synchronously.

---

### User Story 3 - Remote Mutation Authorization & Pre-Flight Gate (Priority: P3)

As a senior technical user, I want the agentic workflow to strictly block any write/comment/edit calls to remote services (GitHub, GitLab, Package Registries) until the local artifacts are fully reviewed and explicit user authorization is granted.

**Why this priority**: Prevents embarrassing premature posts, unreviewed comments, or accidental overrides of upstream issue trackers.

**Independent Test**: Trigger a contribution flow; verify that all local preparation finishes into reviewable markdown files in `scratch/`, and the agent explicitly stops and asks for user authorization before touching remote APIs.

**Acceptance Scenarios**:
1. **Given** generated PR markdown descriptions and comment drafts, **When** preparation is complete, **Then** the system outputs local file links and pauses execution.
2. **Given** explicit user approval, **When** the publish command is run, **Then** it executes the remote mutation and verifies HTTP 200/201 response status.

---

### User Story 4 - Upstream Commit Architecture & SHA Parity Validator (Priority: P4)

As an open-source maintainer, I want git commit histories to be strictly organized into atomic commits (`refactor` and `feat`) with 7-character short SHAs matching the published report, 100% test suite passing, and zero compiler warnings.

**Why this priority**: Maintainers need clean, bisectable commits and verified options to merge conservative refactors independently of algorithmic changes.

**Independent Test**: Run the commit validator against a candidate branch; verify that all commit SHAs, conventional commit messages, CTest (100%), GTest (100%), and compiler warning status (`-Werror`) are verified.

**Acceptance Scenarios**:
1. **Given** a candidate branch, **When** audited, **Then** it checks that `refactor` and `feat` commits are atomic, self-contained, and free of trailing whitespace or macro leaks (`#undef` check).
2. **Given** markdown documentation, **When** audited against git HEAD, **Then** it validates that all referenced commit SHAs match `git rev-parse --short=7 HEAD~n`.

---


### User Story 5 - Pre-Submission Code Craftsmanship & DRY Assembly Encapsulation (Priority: P1)

As an upstream contributor, I want all architecture-specific inline assembly and repetitive SIMD load/compare blocks to be fully encapsulated into clean, scoped file-level helper macros and verified for zero code duplication, zero compiler warnings, and heritage comment preservation BEFORE any commit is pushed to the remote repository.

**Why this priority**: Submitting code with duplicated inline assembly blocks or missing author comments causes friction with maintainers and requires painful rebase churn.

**Independent Test**: Run a pre-commit static analysis check that audits for duplicate `__asm__` strings, missing `#undef` guards, unhandled parameters in `#else` fallbacks, and confirms Commit 1 is a 100% bit-exact pure refactoring.

**Acceptance Scenarios**:
1. **Given** inline assembly modifications in architecture-specific files, **When** audited before commit, **Then** all raw assembly must be encapsulated in scoped helper macros (`LOAD_16B_PAIR`) at the top of the file.
2. **Given** upstream comments regarding hardware quirks (e.g. post-indexed addressing), **When** refactoring, **Then** all original comments must be preserved in place.
3. **Given** cross-platform fallbacks (32-bit ARM, MSVC, GCC), **When** compiling, **Then** all fallback macros must use `Z_UNUSED` to guarantee 0 compiler warnings.
4. **Given** any algorithmic enhancement, **When** organizing git commits, **Then** the mechanical refactoring MUST be isolated into Commit 1 (bit-exact pure refactor) before the new algorithm in Commit 2.

## 3. Edge Cases & Defensive Constraints

1. **Unequal Iteration Counts**: If a benchmark iteration fails or aborts halfway, the cross-over engine must discard the run and report an error rather than computing skewed averages.
2. **CPU Throttling Extremes**: If the system detects severe background thermal throttling (>10% variance across repetitions), it must flag the dataset as unstable.
3. **Dirty Git Worktree**: The benchmark harness must refuse to run if uncommitted changes exist in the source tree to ensure bit-exact reproducibility.
4. **Cross-Platform ABI Safety**: All SIMD match comparison code must validate fallback paths on 32-bit ARM, MSVC, and big-endian systems.

---

## 4. Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST verify 100% bit-exact compiler flags (`CMAKE_BUILD_TYPE`, `CMAKE_C_FLAGS`, native architecture flags) between baseline and candidate before executing comparisons.
- **FR-002**: The system MUST execute benchmarks in dual cross-over mirrored order (A/B and B/A) with at least 5 repetitions per point in 100% in-memory RAM mode.
- **FR-003**: The system MUST dynamically generate all report tables, key takeaways, summary ranges, and option descriptions from raw JSON without hardcoded string literals.
- **FR-004**: The system MUST enforce a hard permission gate preventing any remote GitHub/GitLab write operations until explicit user approval is granted.
- **FR-005**: The system MUST validate that all commit SHAs referenced in documentation match the exact 7-character short SHAs on the git remote branch.
- **FR-006**: The system MUST enforce 100% DRY encapsulation of inline assembly into helper macros with  scoping before submission.
- **FR-007**: The system MUST verify 100% passing tests (CTest, GTest, Delta bit-exact validation) and zero warnings under `-Wall -Wextra -Werror` prior to publication.

---

## 5. Success Criteria *(mandatory)*

1. **Measurement Accuracy**: Benchmark cross-over variance between identical builds must be <= 0.20% across all data types.
2. **Zero-Hallucination Rate**: 100% of published numbers in PR descriptions and comments must match the underlying JSON files bit-for-bit.
3. **Zero Unauthorized Actions**: 0 remote API write calls may be executed without preceding user authorization.
4. **Full Test & Warning Compliance**: 100% test pass rate (71/71 CTest, 862/862 GTest) and 0 compiler warnings across supported toolchains.
