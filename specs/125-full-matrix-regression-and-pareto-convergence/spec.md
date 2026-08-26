# Feature Specification: Full-Matrix Regression & Pareto Convergence

**Feature Branch**: `125-full-matrix-regression-and-pareto-convergence`
**Created**: 2026-08-19
**Status**: Draft
**Input**: User description: "推进 PR 6 (全矩阵回归测试、Pareto 前沿图表生成与收敛)"

---

## Clarifications

### Session 2026-08-19
- **Q1: Which datasets should be benchmarked for final single-core Pareto frontier convergence?**
  - **Decision**: Standard 100MB benchmark corpus (`enwik8` / `silesia`) and 250MB (513 files) mixed multi-modal workspace.
- **Q2: What outputs must be produced?**
  - **Decision**: High-resolution 2x Retina PNG Pareto charts, structured JSON benchmark reports in `docs/benchmarks/`, and full Spec Kit convergence across all 6 PRs.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Full-Matrix Pareto Frontier Plot Generation (Priority: P1)

As a developer and stakeholder, I want to execute the end-to-end single-core 1v1 Pareto frontier shootout between TTZip and libdeflate and generate an updated Retina plot, so that the Pareto frontier progress across all 12 compression tiers is documented.

**Why this priority**: Concludes the 6-PR single-core optimization campaign with empirical data.

**Independent Test**: Executed via `ZipSingleCoreParetoFrontierPkTests`.

**Acceptance Scenarios**:
1. **Given** the 100MB standard corpus, **When** benchmarked across all 12 tiers, **Then** an updated Pareto curve PNG is generated in `docs/benchmarks/`.
2. **Given** all 16 supported archive formats, **When** full regression is executed, **Then** all tests pass with 0 regressions.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST run full 12-tier single-core Pareto benchmark suite and plot updated charts.
- **FR-002**: System MUST run full test suite across all 16 formats (525+ tests) with 100% pass rate.
- **FR-003**: System MUST update `docs/benchmarks/` with timestamped JSON and Markdown reports.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: **All Tests Green**: 100% test suite pass rate (`swift test`).
- **SC-002**: **Pareto Dominance**: TTZip single-core Fast profiles dominate or match competitors across the entire Pareto frontier.
- **SC-003**: **Documentation Convergence**: Complete Markdown and JSON artifacts generated.
