# Implementation Tasks: XZ PR 2 Review Remediation & Retrospective

**Feature**: `specs/065-xz-pr2-review-remediation-and-audit-reflection`

---

## Phase 1: Upstream Code Remediation (User Story 1 - P1)

- [x] T001 [US1] Fix shift direction doc comments above `shift_left` and `shift_right` in `Vendor/worktrees/xz/pr2-arm64-crc64/src/liblzma/check/crc64_arm64.h`
- [x] T002 [US1] Fix `keep_high_bytes` and tail loading doc comments in `Vendor/worktrees/xz/pr2-arm64-crc64/src/liblzma/check/crc64_arm64.h`
- [x] T003 [US1] Fix macOS `is_arch_extension_supported` boolean error handling in `Vendor/worktrees/xz/pr2-arm64-crc64/src/liblzma/check/crc64_arm64.h`
- [x] T004 [US1] Verify clean compilation and zero warnings under ASan/UBSan in `Vendor/worktrees/xz/pr2-arm64-crc64/`

---

## Phase 2: Standalone Reproducibility Suite & Verification (User Story 2 - P1)

- [x] T005 [P] [US2] Create standalone zero-dependency reproduction tool in `scratch/reproduce_bench_crc64.c`
- [x] T006 [P] [US2] Execute physical reproduction benchmark and record live metrics against `contracts/benchmark-result.json`

---

## Phase 3: Root Cause Analysis & Audit Retrospective (User Story 3 - P2)

- [x] T007 [US3] Conduct comprehensive RCA on why comment drift and boolean fallback escaped audits in `specs/065-xz-pr2-review-remediation-and-audit-reflection/retrospective.md`
- [x] T008 [US3] Codify preventative audit rules in project engineering guidelines

---

## Phase 4: Verification, Git Update & Community Response (User Story 4 - P2)

- [x] T009 [US4] Run all 20/20 CTest test suites in `Vendor/worktrees/xz/pr2-arm64-crc64/`
- [x] T010 [US4] Commit fixes to `feat/arm64-crc64-clmul` and update upstream PR #241
- [x] T011 [US4] Draft comprehensive and humble community reply to `@Larhzu` and `@ssvb`
