# Tasks: Upstream Contribution Methodology, Lessons Learned, and Engineering Governance

**Feature Directory**: `specs/133-upstream-contribution-lessons-and-governance`  
**Target Subject**: 上游开源贡献方法论、微架构底层原理、工程治理规范与知识树沉淀  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Constitutional Invariants

- [x] T001 [P] Update TTZip Engineering Constitution with Section 6 ("Upstream Open-Source Contribution & Hardware Grounding Protocol") in `.specify/memory/constitution.md`
- [x] T002 [P] Update Subagent Guidelines for Upstream PR Contributions in `.agents/rules/upstream-contribution.md`

---

## Phase 2: User Story 1 - Automated Upstream Pre-Flight Quality Gate (Priority: P1)

- [x] T003 [P] [US1] Implement automated compiler flag parity check and dual-build verification in `scripts/upstream_audit_gate.py`
- [x] T004 [P] [US1] Implement Google Benchmark JSON parser, CV statistics calculation ( \le 1.5\%$), and 50-point non-regression assertion in `scripts/upstream_audit_gate.py`
- [x] T005 [US1] Align report generation script with canonical 50-row workload sequence in `scripts/upstream_report_gen.py`

---

## Phase 3: User Story 2 - Upstream Contribution Guide & Standards (Priority: P2)

- [x] T006 [P] [US2] Author comprehensive Upstream Contribution Standard Operating Guide in `docs/study/upstream_contribution_guide.md`
- [x] T007 [P] [US2] Create third-party license and copyright compliance guide in `docs/THIRD_PARTY_LICENSES.md`

---

## Phase 4: User Story 3 - Educational Case Study & Knowledge Tree Integration (Priority: P3)

- [x] T008 [P] [US3] Author in-depth AArch64 SIMD & Longest Match pedagogical case study in `docs/study/case_study_arm64_simd_journey.md`
- [x] T009 [P] [US3] Add annotated assembly walkthrough and microarchitectural port diagram in `docs/study/case_study_arm64_simd_journey.md`

---

## Phase 5: Verification & Gate Assertion

- [x] T010 Execute end-to-end Pre-Flight Audit Gate validation using `scripts/upstream_audit_gate.py` on zlib-ng worktree
- [x] T011 Verify 100% test pass on TTZip core test suites (`swift test`) and check zero-regression invariants
