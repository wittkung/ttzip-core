# Implementation Plan: Upstream Contribution Methodology, Lessons Learned, and Engineering Governance

**Feature Directory**: `specs/133-upstream-contribution-lessons-and-governance`  
**Target Subject**: 上游开源贡献方法论、微架构底层原理、工程治理规范与知识树沉淀  
**Status**: Ready for Execution  

---

## 1. Technical Context & Scope

During the integration and upstream PR contribution of `zlib-ng` PR #2416 and `libarchive` PRs, core lessons were learned around microarchitectural register domain crossings (FPR vs GPR), compiler optimization heuristics, Google Benchmark statistical variance, and maintainer communication discipline.

This plan operationalizes these lessons across three concrete layers:
1. **Automated Tooling Layer**: Deploying `scripts/upstream_audit_gate.py` to enforce 5-repetition cross-over sampling, CV $\le 1.5\%$, and zero single-point regression ($> 2\%$).
2. **Constitutional Governance Layer**: Amending `.specify/memory/constitution.md` with Section 6 ("Upstream Open-Source Contribution & Hardware Grounding Protocol").
3. **Pedagogical & Knowledge Tree Layer**: Documenting `docs/study/case_study_arm64_simd_journey.md` and `docs/study/upstream_contribution_guide.md` for the startup's educational knowledge graph.

---

## 2. Constitution Check

- **Zero Unverified AI Submissions**: All upstream PR proposals must pass the pre-flight gate.
- **Hardware Grounding Invariant**: Line-by-line disassembly check (`otool -tv`) required.
- **Multi-Workload Parity Invariant**: All 8 workloads must demonstrate non-regression.
- **Logging & Diagnostics Discipline**: Python CLI scripts use structured formatted outputs.

---

## 3. Phase 0 & Phase 1 Artifacts Index

- **Phase 0 Research**:
  - `research.md`: Contains R001 (Automated Pre-Flight Gate), R002 (AArch64 Register File Latencies), and R003 (Open Source Governance & Educational Integration).
- **Phase 1 Contracts & Models**:
  - `data-model.md`: Contains `UpstreamAuditReport`, `CompilerParityAudit`, `DualBuildAudit`, `BenchmarkPoint`, `CvSummary`, `AuditVerdict`.
  - `contracts/upstream_audit_report.json`: JSON Schema Draft-07 compliant, zero bare objects.
  - `quickstart.md`: Validation scenarios for US1, US2, and US3.

---

## 4. Component Changes Breakdown

### A. Scripts & Tooling
- `scripts/upstream_audit_gate.py` [NEW]: Automated CLI pre-flight gate enforcing compiler parity, dual-build, disassembly, CV, and regression thresholds.
- `scripts/upstream_report_gen.py` [MODIFY]: Synchronize table sorting and schema to match canonical sequence.

### B. Engineering Constitution & Rules
- `.specify/memory/constitution.md` [MODIFY]: Add Section 6 defining the 5 Inviolable Upstream Contribution Invariants.
- `.agents/rules/upstream-contribution.md` [MODIFY]: Update subagent directives with latest hardware and etiquette rules.

### C. Documentation & Educational Curriculum
- `docs/study/case_study_arm64_simd_journey.md` [NEW]: Complete pedagogical case study analyzing PR #2416 from naive intuition to microarchitecture and humility.
- `docs/study/upstream_contribution_guide.md` [NEW]: Standard operating guide for upstream contributions.
