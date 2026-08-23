# Tasks: 工业级 Git Worktree A/B 基准对标与统计显著性自动化流水线 (Feature 166)

**Feature ID**: `166-worktree-ab-benchmark-and-statistical-delta-workflow`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Telemetry Infrastructure

- [ ] T001 Enhance `tests/c/bench_main.c` to output structured JSON when `--json <file>` is specified

---

## Phase 2: User Story 1 (P1) - Statistical Delta Calculation Engine

- [ ] T002 [P] [US1] Implement Welch's t-test and Lentz continued fraction in `scripts/statistical_delta.py`
- [ ] T003 [P] [US1] Implement ANSI terminal table formatting and Markdown/JSON report export in `scripts/statistical_delta.py`

---

## Phase 3: User Story 2 (P2) - Master Worktree A/B Orchestration Script

- [ ] T004 [P] [US2] Implement `scripts/benchmark_ab.sh` with robust trap handler, detached worktree creation, and isolated `-O3` compilation
- [ ] T005 [P] [US2] Implement interleaved sampling loop ($B \to C \to B \to C$) and parameter parsing in `scripts/benchmark_ab.sh`

---

## Phase 4: Verification & Live A/B Comparison

- [ ] T006 [US1] Run `./scripts/benchmark_ab.sh HEAD~1 HEAD --runs 3`
- [ ] T007 [US1] Verify clean worktree teardown and inspect generated Markdown report
- [ ] T008 [US1] Run `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
