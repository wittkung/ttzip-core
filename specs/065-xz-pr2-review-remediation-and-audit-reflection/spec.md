# Feature Specification: XZ PR 2 Review Remediation, Reproducibility Suite & Audit Retrospective

**Feature Directory**: `specs/065-xz-pr2-review-remediation-and-audit-reflection`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "非常需要，然后我觉得需要好好说明一下，我们这个是基于自己项目 ttzip 的一些进展来整理提交的，部分内容是我的 agent 来帮助生成和工作的，但经过了详细的手动审计，虽然还有一些遗漏，然后她关心的那些问题我们要好好测试和复现，并说明具体的情况，而且我们需要好好反思为什么会出这些问题，而且没有被审计出来，我们的工作流和审计出来什么问题"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Upstream Code Review Remediation (Priority: P1)

As a contributor to the upstream `tukaani-project/xz` repository, I need to address all 3 code review points raised by lead maintainer Lasse Collin (`@Larhzu`):
1. Fix inverted shift comments above `shift_left` and `shift_right` to match their actual implementation and names.
2. Fix `keep_high_bytes` comment so it accurately states byte masking/clearing rather than shifting.
3. Fix the boolean return in `is_arch_extension_supported` on macOS Darwin when `sysctlbyname` fails or `has_pmull` is zero.

**Why this priority**: Directly resolves reviewer feedback from the project maintainer, ensuring code correctness and strict comment-code synchronization.

**Independent Test**:
- Inspect `src/liblzma/check/crc64_arm64.h` to verify comment alignment and boolean correctness.
- Recompile with all sanitizers (ASan/UBSan) and run test suites.

**Acceptance Scenarios**:
1. **Given** `shift_left` and `shift_right` in `crc64_arm64.h`, **When** inspecting their doc comments, **Then** `shift_left` specifies left shift clearing lowest bytes, and `shift_right` specifies right shift clearing highest bytes.
2. **Given** `keep_high_bytes` in `crc64_arm64.h`, **When** inspecting its doc comment, **Then** it accurately describes clearing the lowest `16 - count` bytes.
3. **Given** macOS runtime check in `is_arch_extension_supported()`, **When** `sysctlbyname` returns a non-zero error code or sets `has_pmull` to 0, **Then** the function returns `false` (or `has_pmull != 0`).

---

### User Story 2 - Standalone Reproducibility Suite & Test Vectors (Priority: P1)

As an open-source reviewer (e.g. `@ssvb`), I need a 100% self-contained, zero-dependency reproducibility benchmark script and rigorous correctness test vectors that can be compiled and executed on any Linux/macOS ARM64 system in seconds to verify throughput and mathematical exactness.

**Why this priority**: Directly answers community skepticism regarding AI-assisted benchmarks and provides irrefutable physical proof of throughput and correctness.

**Independent Test**:
- Compile and run `scratch/reproduce_bench_crc64.c` with `clang -O3` or `gcc -O3` and verify bit-exact check and physical speedup.

**Acceptance Scenarios**:
1. **Given** `reproduce_bench_crc64.c`, **When** compiled on Apple Silicon / AArch64 Linux, **Then** it executes standard ECMA-182 CRC64 test vectors (`"123456789"` -> `0x6C40DF5F0B497347`) and compares against generic slice-by-4.
2. **Given** 50 iterations over 64MB buffers with memory clobbers enforced, **When** benchmark runs, **Then** hardware PMULL achieves >= 30 GB/s throughput with 100% bit-exact parity.

---

### User Story 3 - Root Cause Analysis & Workflow Retrospective (Priority: P2)

As the project lead and engineering team, we need to document a thorough retrospective on why these specific issues slipped past our automated test gates and manual code reviews, identifying systemic weaknesses in our AI-human collaboration workflow.

**Why this priority**: Prevents future regressions and establishes permanent improvements in our audit checklist for upstream PRs.

**Independent Test**:
- Formulate the retrospective document containing root causes, detection gaps, and actionable process guardrails.

**Acceptance Scenarios**:
1. **Given** the 3 review findings, **When** analyzing root causes, **Then** document why comment drift occurred during x86->ARM translation and why the macOS Darwin test environment masked the boolean fallback bug.
2. **Given** the audit failure, **When** reviewing Spec Kit / Agent rules, **Then** add explicit AST/boolean branch validation and comment-code synchronization checks.

---

### User Story 4 - Transparent & Humble Community Response (Priority: P2)

As a contributor (`Witt Kung`), I need to post a clear, humble, and technically thorough response on GitHub PR #241 explaining that the optimization was derived from our research in the TTZip engine, openly disclosing AI tool assistance while taking full human responsibility, thanking Lasse Collin for the review, and providing ssvb with the reproducibility suite.

**Why this priority**: Establishes community trust, openness, and high standards of scientific transparency.

**Independent Test**:
- Draft and review the response text before submitting to GitHub.

**Acceptance Scenarios**:
1. **Given** community comments on PR #241, **When** reviewing the response draft, **Then** it clearly explains the project background, acknowledges AI code assistance and human verification, thanks reviewers, and presents reproduction instructions.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `crc64_arm64.h` MUST have perfectly synchronized doc comments for `shift_left`, `shift_right`, and `keep_high_bytes`.
- **FR-002**: `is_arch_extension_supported()` MUST return `false` on macOS if `sysctlbyname` fails or `has_pmull` is 0.
- **FR-003**: The project MUST provide a standalone, zero-dependency C reproduction benchmark file (`reproduce_bench_crc64.c`).
- **FR-004**: The project MUST include exhaustive tests verifying bit-exact CRC64 outputs across all buffer sizes ($0 \dots 65,536$) and offsets ($0 \dots 63$).
- **FR-005**: The team MUST complete a formal Root Cause Analysis (RCA) artifact covering the audit blind spots.
- **FR-006**: The team MUST draft and publish an open community response and updated commit to PR #241.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of Lasse Collin's 3 review points resolved in code and verified with zero compiler warnings.
- **SC-002**: All 20/20 CTest test suites in `Vendor/worktrees/xz/pr2-arm64-crc64` pass with AddressSanitizer and UndefinedBehaviorSanitizer.
- **SC-003**: Standalone benchmark compiles with a single command (`clang -O3 reproduce_bench_crc64.c`) and completes in under 3 seconds with unambiguous output.
- **SC-004**: RCA and workflow improvement guidelines committed to project memory.

---

## Assumptions

- Target build system: Apple Clang 15.0+ and GCC 11+ on AArch64 / macOS 14+ / Linux.
- Upstream PR branch `wittkung/xz:feat/arm64-crc64-clmul` will be amended and force-pushed cleanly.
